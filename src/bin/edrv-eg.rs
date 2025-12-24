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

use edrv_st7735::{Display160x80Type2, ST7735};
use embedded_graphics::{
    framebuffer::{buffer_size, Framebuffer}, 
    pixelcolor::{raw::{LittleEndian, RawU16}, Rgb565}, 
    prelude::*,
    mono_font::{ascii::FONT_8X13, MonoTextStyle},
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::Text,
};

type FramebufferType = Framebuffer<Rgb565, RawU16, LittleEndian, 160, 80, { buffer_size::<Rgb565>(160, 80) }>;
static SHARED_FB: StaticCell<Mutex<CriticalSectionRawMutex, FramebufferType>> = StaticCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
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

    let p = embassy_stm32::init(config);

    let dc = Output::new(p.PE13, Level::Low, Speed::High);
    let cs = Output::new(p.PE11, Level::Low, Speed::High);
    let mut lcd_led = Output::new(p.PE10, Level::Low, Speed::Low);

    let spi_config: spi::Config = Default::default();

    let spi = Spi::new_txonly(p.SPI4, p.PE12, p.PE14, p.DMA1_CH0, spi_config);
    let spi_bus = Mutex::<NoopRawMutex, _>::new(spi);
    let spi_dev = embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice::new(&spi_bus, cs);
    let mut display: ST7735<Display160x80Type2, _, _> = ST7735::new(spi_dev, dc);

    display.init(&mut Delay).await.unwrap();
    display.clear(Rgb565::BLACK).await.unwrap();

    let fb = Framebuffer::new();
    let shared_fb = SHARED_FB.init(Mutex::new(fb));

    let style_text = MonoTextStyle::new(&FONT_8X13, Rgb565::WHITE);
    let style_rect = PrimitiveStyle::with_stroke(Rgb565::GREEN, 1);
    let style_circ = PrimitiveStyle::with_fill(Rgb565::RED);

    loop {
        {
            let mut fb_guard = shared_fb.lock().await;
            
            fb_guard.clear(Rgb565::BLACK).unwrap();

            Text::new("ST7735 Async", Point::new(10, 20), style_text)
                .draw(&mut *fb_guard)
                .unwrap();

            Rectangle::new(Point::new(5, 30), Size::new(150, 40))
                .into_styled(style_rect)
                .draw(&mut *fb_guard)
                .unwrap();

            Circle::new(Point::new(120, 35), 20)
                .into_styled(style_circ)
                .draw(&mut *fb_guard)
                .unwrap();
        }

        {
            let fb_guard = shared_fb.lock().await;
            display.write_framebuffer(fb_guard.data()).await.unwrap();
        }
        info!("Frame drawn");
        // lcd_led.toggle();
        Timer::after_secs(1).await;
    }
}