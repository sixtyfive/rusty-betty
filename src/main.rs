#![no_std]
#![no_main]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, rmt::Rmt, time::Rate, timer::timg::TimerGroup};
use esp_hal_smartled::{buffer_size, color_order, RmtSmartLeds, WS2812B_TIMING};
use smart_leds::{SmartLedsWrite, RGB8};

esp_bootloader_esp_idf::esp_app_desc!();

include!(concat!(env!("OUT_DIR"), "/config.rs"));

mod mqtt;
mod wifi;

pub static WIFI_UP: AtomicBool = AtomicBool::new(false);
pub static MQTT_UP: AtomicBool = AtomicBool::new(false);

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 36 * 1024);

    let timg0 = TimerGroup::new(p.TIMG0);
    esp_rtos::start(timg0.timer0, p.FROM_CPU_INTR0);

    let freq = Rate::from_mhz(80);
    let rmt = Rmt::new(p.RMT, freq).unwrap();
    let mut led = RmtSmartLeds::<{ buffer_size::<RGB8>(1) }, _, RGB8, color_order::Rgb>::new(
        WS2812B_TIMING,
        rmt.channel0,
        pin_onboard_led!(p),
        freq,
    )
    .unwrap();

    let stack = wifi::start(spawner, p.WIFI);
    spawner.spawn(mqtt::task(stack).unwrap());

    loop {
        let color = if MQTT_UP.load(Ordering::Relaxed) {
            RGB8::new(0, 20, 0)
        } else if WIFI_UP.load(Ordering::Relaxed) {
            RGB8::new(20, 8, 0)
        } else {
            RGB8::new(20, 0, 0)
        };
        led.write([color]).unwrap();
        Timer::after(Duration::from_millis(250)).await;
    }
}
