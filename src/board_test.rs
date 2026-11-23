#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::{
    analog::adc::{Adc, AdcConfig, Attenuation},
    gpio::{DriveMode, Level, Output, OutputConfig},
    i2c::master::{Config as I2cConfig, I2c},
    ledc::{
        channel::{self, ChannelIFace},
        timer::{self, TimerIFace},
        LSGlobalClkSource, Ledc, LowSpeed,
    },
    main,
    rmt::Rmt,
    time::{Instant, Rate},
};
use esp_hal_smartled::{buffer_size, color_order, RmtSmartLeds, WS2812B_TIMING};
use esp_println::println;
use smart_leds::{SmartLedsWrite, RGB8};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let p = esp_hal::init(esp_hal::Config::default());

    let freq = Rate::from_mhz(80);
    let rmt = Rmt::new(p.RMT, freq).unwrap();

    let mut ext = RmtSmartLeds::<{ buffer_size::<RGB8>(2) }, _, RGB8, color_order::Rgb>::new(
        WS2812B_TIMING, rmt.channel0, p.GPIO14, freq,
    )
    .unwrap();
    let mut board = RmtSmartLeds::<{ buffer_size::<RGB8>(1) }, _, RGB8, color_order::Rgb>::new(
        WS2812B_TIMING, rmt.channel1, p.GPIO8, freq,
    )
    .unwrap();

    board.write([RGB8::new(0, 0, 0)]).unwrap();

    let mut gpio22 = Output::new(p.GPIO22, Level::Low, OutputConfig::default());

    let mut i2c = I2c::new(p.I2C0, I2cConfig::default())
        .unwrap()
        .with_sda(p.GPIO3)
        .with_scl(p.GPIO4);

    let mut ledc = Ledc::new(p.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    let mut lstimer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    lstimer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty3Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_mhz(5),
        })
        .unwrap();
    let mut pwm = ledc.channel(channel::Number::Channel0, p.GPIO21);
    pwm.configure(channel::config::Config {
        timer: &lstimer0,
        duty_pct: 50,
        drive_mode: DriveMode::PushPull,
    })
    .unwrap();

    let mut adc_cfg = AdcConfig::new();
    let mut adc_pin = adc_cfg.enable_pin(p.GPIO0, Attenuation::_11dB);
    let mut adc = Adc::new(p.ADC1, adc_cfg);

    let mut last_toggle = 0u64;
    let mut last_win = 0u64;
    let mut win_count = 0u32;
    let mut adc_min = u16::MAX;
    let mut adc_max = 0u16;
    let mut last_lvl = u8::MAX;
    let mut addr = 1u8;
    let mut found = 0u128;
    let mut last_found = u128::MAX;

    loop {
        let now = Instant::now().duration_since_epoch().as_millis();

        if now - last_toggle >= 500 {
            gpio22.toggle();
            last_toggle = now;
        }

        let mut b = [0u8; 1];
        if i2c.read(addr, &mut b).is_ok() {
            found |= 1u128 << addr;
        }
        addr += 1;
        if addr > 126 {
            addr = 1;
            if found != last_found {
                println!("I2C-Geraete (Bitmaske): {:#034x}", found);
                last_found = found;
            }
            found = 0;
        }

        let raw = nb::block!(adc.read_oneshot(&mut adc_pin)).unwrap_or(0);
        adc_min = adc_min.min(raw);
        adc_max = adc_max.max(raw);

        if now - last_win >= 40 {
            last_win = now;
            let pp = adc_max.saturating_sub(adc_min);
            adc_min = u16::MAX;
            adc_max = 0;

            let lvl = (pp / 8).min(120) as u8;

            win_count += 1;
            if win_count % 10 == 0 {
                println!("adc: raw={} pp={} lvl={}", raw, pp, lvl);
            }

            if lvl != last_lvl {
                last_lvl = lvl;
                board.write([RGB8::new(0, lvl, 0)]).unwrap();
                ext.write([RGB8::new(lvl, lvl, lvl), RGB8::new(lvl, 0, 120 - lvl)])
                    .unwrap();
            }
        }
    }
}
