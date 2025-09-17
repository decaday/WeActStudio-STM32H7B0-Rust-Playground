#![no_main]
#![no_std]

// Tested on weact stm32h7b0 board + w25q64 spi flash

use defmt::info;
use embassy_executor::Spawner;
use st7735_lcd::Orientation;
use {defmt_rtt as _, panic_probe as _};

use embassy_stm32::{gpio::{Level, Output, Speed}, spi::{self, Spi}};
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;

use embedded_graphics::{
    pixelcolor::{raw::LittleEndian, Rgb565},
    prelude::*,
};

const IMAGE_WIDTH: u16 = 86;
const IMAGE_HEIGHT: u16 = 64;

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

    let dc = Output::new(p.PE13, Level::Low, Speed::High);
    let cs = Output::new(p.PE11, Level::Low, Speed::High);
    let _lcd_led = Output::new(p.PE10, Level::Low, Speed::Low);

    let config: spi::Config = Default::default();

    let spi = Spi::new_blocking_txonly(p.SPI4, p.PE12, p.PE14, config); // p.DMA1_CH0
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
    // draw ferris
    let image_raw: embedded_graphics::image::ImageRaw<Rgb565, LittleEndian> =
        embedded_graphics::image::ImageRaw::new(
            include_bytes!("../../assets/ferris.raw"),
            IMAGE_WIDTH as u32,
        );

    let image = embedded_graphics::image::Image::new(
        &image_raw, 
        Point {
            x: (160 - IMAGE_WIDTH as i32) / 2,
            y: (80 - IMAGE_HEIGHT as i32) / 2,
        }
    );
    image.draw(&mut disp).unwrap();

    info!("finished");

    loop {}
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