#![no_main]
#![no_std]

// Tested on weact stm32h7b0 board + w25q64 spi flash

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};
use embassy_stm32::{gpio::{Level, Output, Speed}, spi::{self, Spi}};
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;

use st7735_lcd::Orientation;
use embedded_graphics::{
    pixelcolor:: Rgb565,
    prelude::*,
};
use mousefood::prelude::*;
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::{Frame, Terminal, style::*};

extern crate alloc;
// use embedded_alloc::TlsfHeap as Heap;
use embedded_alloc::LlffHeap as Heap;
#[global_allocator]
static HEAP: Heap = Heap::empty();
use alloc::boxed::Box;


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // RCC config
    let mut config = Config::default();
    info!("START");
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        // Needed for USB
        config.rcc.hsi48 = Some(Hsi48Config { sync_from_usb: true });
        // External oscillator 25MHZ
        config.rcc.hse = Some(Hse {
            freq: Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV5,
            mul: PllMul::MUL112,
            divp: Some(PllDiv::DIV2),
            divq: Some(PllDiv::DIV2),
            divr: Some(PllDiv::DIV2),
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV2;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.apb3_pre = APBPrescaler::DIV2;
        config.rcc.apb4_pre = APBPrescaler::DIV2;
        config.rcc.voltage_scale = VoltageScale::Scale0;
    }

    // Initialize peripherals
    let p = embassy_stm32::init(config);

    // Initialize HEAP
    
    use core::mem::MaybeUninit;
    use core::ptr::addr_of_mut;
    const HEAP_SIZE: usize = 128_000;
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    unsafe { HEAP.init(addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) };
    
    info!("Preparing display");

    let dc = Output::new(p.PE13, Level::Low, Speed::High);
    let cs = Output::new(p.PE11, Level::Low, Speed::High);
    let _lcd_led = Output::new(p.PE10, Level::Low, Speed::Low);

    let config: spi::Config = Default::default();

    let spi = Spi::new_txonly(p.SPI4, p.PE12, p.PE14, p.DMA1_CH0, config); // p.DMA1_CH0
    let spi_device = embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(
        spi, 
        cs
    ).unwrap();

    let mut disp = st7735_lcd::ST7735::new(
        spi_device, 
        dc, 
        DummyPin {}, 
        false, 
        true, 
        160, 
        80
    );

    disp.init(&mut embassy_time::Delay).unwrap();
    disp.set_orientation(&Orientation::LandscapeSwapped).unwrap();
    disp.set_offset(1, 26);
    disp.clear(Rgb565::BLACK).unwrap();


    let backend_config = EmbeddedBackendConfig {
        // Define how to display newly rendered widgets
        flush_callback: Box::new(move |disp: &mut st7735_lcd::ST7735<embedded_hal_bus::spi::ExclusiveDevice<Spi<'_, embassy_stm32::mode::Async>, Output<'_>, embedded_hal_bus::spi::NoDelay>, Output<'_>, DummyPin>| {
            // disp.clear(Rgb565::BLACK).unwrap();
        }),
        font_regular: embedded_graphics_unicodefonts::mono_7x13_atlas(),
        ..Default::default()
    };
    let backend: EmbeddedBackend<_, _> =
        EmbeddedBackend::new(&mut disp, backend_config);

    // Start ratatui with our simulator backend
    let mut terminal = Terminal::new(backend).unwrap();

    // Run an infinite loop, where widgets will be rendered

    loop {
        Timer::after_millis(100).await;
        terminal.draw(draw).unwrap();
    }

}

fn draw(frame: &mut Frame) {
    let text = "Ratatui on embedded devices!";
    let paragraph = Paragraph::new(text.dark_gray()).wrap(Wrap { trim: true });
    let bordered_block = Block::bordered()
        .border_style(Style::new().yellow())
        .title("Mousefood");
    frame.render_widget(paragraph.block(bordered_block), frame.area());
}

pub struct DummyPin {}
impl embedded_hal_1::digital::ErrorType for DummyPin {
    type Error = core::convert::Infallible;
}

impl embedded_hal_1::digital::OutputPin for DummyPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}