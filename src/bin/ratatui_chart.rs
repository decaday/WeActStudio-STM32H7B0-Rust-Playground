#![no_main]
#![no_std]

// Tested on weact stm32h7b0 board + w25q64 spi flash

use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex}, mutex::Mutex};
use embassy_time::{Delay, Timer};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};
use embassy_stm32::{gpio::{Level, Output, Speed}, spi::{self, Spi}};
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;

use edrv_st7735::{Display160x80Type1, ST7735};
use embedded_graphics::{
    framebuffer::{buffer_size, Framebuffer}, pixelcolor::{raw::{LittleEndian, RawU16},  Rgb565}, prelude::*
};
use mousefood::prelude::*;

// ---- Ratatui imports for the Chart example ----
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};
use ratatui::{Frame, Terminal, style::*};
use ratatui::symbols::Marker;
use ratatui::text::{Span};


extern crate alloc;
use embedded_alloc::LlffHeap as Heap;
#[global_allocator]
static HEAP: Heap = Heap::empty();
use alloc::boxed::Box;
use alloc::vec;

type FramebufferType = Framebuffer<Rgb565, RawU16, LittleEndian, 160, 80, { buffer_size::<Rgb565>(160, 80) }>;
static SHARED_FB: StaticCell<Mutex<CriticalSectionRawMutex, FramebufferType>> = StaticCell::new();


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // RCC config
    let mut config = Config::default();
    info!("START");
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        config.rcc.hsi48 = Some(Hsi48Config { sync_from_usb: true });
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
    const HEAP_SIZE: usize = 0x10_000;
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    unsafe { HEAP.init(addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) };
    

    let dc = Output::new(p.PE13, Level::Low, Speed::High);
    let cs = Output::new(p.PE11, Level::Low, Speed::High);
    let _lcd_led = Output::new(p.PE10, Level::Low, Speed::Low);

    let config: spi::Config = Default::default();

    let spi = Spi::new_txonly(p.SPI4, p.PE12, p.PE14, p.DMA1_CH0, config);
    let spi_bus = Mutex::<NoopRawMutex, _>::new(spi);
    let spi_dev = embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice::new(&spi_bus, cs);
    let mut display: ST7735<Display160x80Type1, _, _> = ST7735::new(spi_dev, dc);

    display.init(&mut Delay).await.unwrap();
    display.clear(Rgb565::BLACK).await.unwrap();

    let fb = Framebuffer::new();
    let shared_fb = SHARED_FB.init(Mutex::new(fb));
    
    // Lock the framebuffer once for the backend lifetime
    let mut fb_guard = shared_fb.lock().await;

    // Capture raw pointers to bypass borrow checker in the loop
    let fb_ptr = fb_guard.data().as_ptr();
    let fb_len = fb_guard.data().len();

    let backend_config = EmbeddedBackendConfig {
        flush_callback: Box::new(|_| {}), // Do not clear the framebuffer here
        font_regular: embedded_graphics_unicodefonts::mono_7x13_atlas(),
        ..Default::default()
    };

    let backend = EmbeddedBackend::new(&mut *fb_guard, backend_config);
    let mut terminal = Terminal::new(backend).unwrap();

    // Initial clear
    terminal.clear().ok();

    loop {
        terminal.draw(draw).unwrap();

        // Create a temporary slice from raw parts to send to display.
        // Safety: `terminal.draw` is complete, so no concurrent writes occur during this read.
        let fb_slice = unsafe { 
            core::slice::from_raw_parts(fb_ptr, fb_len) 
        };

        display.write_framebuffer(fb_slice).await.unwrap();

        info!("Heap used: {} free: {}", HEAP.used(), HEAP.free());
        Timer::after_secs(1).await;
    }
}

// ---- New data and functions for the Scatter Chart example ----

// Three different sets of data
const DATA_1: &[(f64, f64)] = &[(0.0, 1.0), (1.0, 3.0), (2.0, 0.5), (5.0, 4.0), (7.0, 6.0), (10.0, 10.0)];
const DATA_2: &[(f64, f64)] = &[(0.5, 5.0), (1.5, 6.5), (2.5, 7.0), (4.0, 8.0), (6.0, 9.0), (9.0, 7.5)];
const DATA_3: &[(f64, f64)] = &[(0.8, 9.0), (3.0, 8.5), (5.5, 6.0), (6.5, 7.0), (8.0, 5.0), (9.5, 6.5)];

/// The main drawing function, now renders the scatter chart
fn draw(frame: &mut Frame) {
    let chart = create_chart();
    frame.render_widget(chart, frame.area());
}

/// Create a scatter chart with three datasets and no outer border.
fn create_chart<'a>() -> Chart<'a> {
    // Dataset 1: Braille marker, Cyan color
    let dataset1 = Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Scatter)
        .style(Style::new().cyan())
        .data(DATA_1);

    // Dataset 2: Dot marker, Yellow color
    let dataset2 = Dataset::default()
        .marker(Marker::Dot)
        .graph_type(GraphType::Scatter)
        .style(Style::new().yellow())
        .data(DATA_2);

    // Dataset 3: Block marker, Magenta color
    let dataset3 = Dataset::default()
        .marker(Marker::Block)
        .graph_type(GraphType::Scatter)
        .style(Style::new().magenta())
        .data(DATA_3);

    // Create X axis with labels for scale
    let x_axis = Axis::default()
        .style(Style::new().gray())
        .bounds([0.0, 10.0])
        .labels(vec![
            Span::from("0"),
            Span::from("5"),
            Span::from("10"),
        ]);

    // Create Y axis with labels for scale
    let y_axis = Axis::default()
        .style(Style::new().gray())
        .bounds([0.0, 10.0])
        .labels(vec![
            Span::from("0"),
            Span::from("5"),
            Span::from("10"),
        ]);

    // Create the chart with all three datasets, without the outer block
    Chart::new(vec![dataset1, dataset2, dataset3])
        .x_axis(x_axis)
        .y_axis(y_axis)
}
