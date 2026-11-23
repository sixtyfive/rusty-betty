#![no_std]
#![no_main]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
#[cfg(all(actuator_magnet, actuator_servo))]
use embassy_futures::join::join;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
#[cfg(actuator_servo)]
use esp_hal::ledc::{
    channel,
    timer::{self, TimerIFace},
    LSGlobalClkSource, Ledc, LowSpeed,
};
use esp_hal::{clock::CpuClock, rmt::Rmt, time::Rate, timer::timg::TimerGroup};
use esp_hal_smartled::{buffer_size, color_order, RmtSmartLeds, WS2812B_TIMING};
use esp_println::println;
use smart_leds::{SmartLedsWriteAsync, RGB8};
#[cfg(actuator_servo)]
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();

include!(concat!(env!("OUT_DIR"), "/config.rs"));

mod led;
#[cfg(actuator_magnet)]
mod magnet;
mod mqtt;
#[cfg(actuator_servo)]
mod servo;
mod wifi;

pub static WIFI_UP: AtomicBool = AtomicBool::new(false);
pub static MQTT_UP: AtomicBool = AtomicBool::new(false);

pub enum Command {
    Wink,
    Color(RGB8),
}

pub static COMMANDS: Channel<CriticalSectionRawMutex, Command, 8> = Channel::new();

#[cfg(actuator_servo)]
static SERVO_TIMER: StaticCell<timer::Timer<'static, LowSpeed>> = StaticCell::new();

struct CatHardware {
    #[cfg(actuator_magnet)]
    magnet: magnet::Magnet<'static>,
    #[cfg(actuator_servo)]
    servo: servo::Servo<'static>,
}

impl CatHardware {
    async fn wave(&mut self) {
        #[cfg(all(actuator_magnet, actuator_servo))]
        join(self.magnet.wave(), self.servo.wave()).await;
        #[cfg(all(actuator_magnet, not(actuator_servo)))]
        self.magnet.wave().await;
        #[cfg(all(actuator_servo, not(actuator_magnet)))]
        self.servo.wave().await;
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 36 * 1024);

    let timg0 = TimerGroup::new(p.TIMG0);
    esp_rtos::start(timg0.timer0, p.FROM_CPU_INTR0);

    let freq = Rate::from_mhz(80);
    let rmt = Rmt::new(p.RMT, freq).unwrap().into_async();
    let mut status_led =
        RmtSmartLeds::<{ buffer_size::<RGB8>(1) }, _, RGB8, color_order::Rgb>::new(
            WS2812B_TIMING,
            rmt.channel0,
            pin_onboard_led!(p),
            freq,
        )
        .unwrap();
    let eyes = led::Eyes::new(rmt.channel1, pin_ext_leds!(p), freq);

    #[cfg(actuator_magnet)]
    let magnet = magnet::Magnet::new(pin_blink!(p), false);

    #[cfg(actuator_servo)]
    let servo = {
        let mut ledc = Ledc::new(p.LEDC);
        ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
        let servo_timer = SERVO_TIMER.init(ledc.timer::<LowSpeed>(timer::Number::Timer0));
        servo_timer
            .configure(timer::config::Config {
                duty: timer::config::Duty::Duty14Bit,
                clock_source: timer::LSClockSource::APBClk,
                frequency: Rate::from_hz(50),
            })
            .unwrap();
        servo::Servo::new(
            ledc.channel(channel::Number::Channel0, pin_pwm!(p)),
            servo_timer,
        )
    };

    let hardware = CatHardware {
        #[cfg(actuator_magnet)]
        magnet,
        #[cfg(actuator_servo)]
        servo,
    };

    spawner.spawn(cat_task(eyes, hardware, COMMANDS.receiver()).unwrap());

    let stack = wifi::start(spawner, p.WIFI);
    spawner.spawn(mqtt::task(stack, COMMANDS.sender()).unwrap());

    let mut last_status = u8::MAX;
    loop {
        let (status, color) = if MQTT_UP.load(Ordering::Relaxed) {
            (2, RGB8::new(0, 20, 0))
        } else if WIFI_UP.load(Ordering::Relaxed) {
            (1, RGB8::new(20, 8, 0))
        } else {
            (0, RGB8::new(20, 0, 0))
        };

        if status != last_status {
            match status {
                0 => println!("Built-in LED red (waiting for WiFi)"),
                1 => println!("Built-in LED yellow (waiting for MQTT)"),
                _ => println!("Built-in LED green (online)"),
            }
            last_status = status;
        }

        status_led.write([color]).await.unwrap();
        Timer::after(Duration::from_millis(250)).await;
    }
}

#[embassy_executor::task]
async fn cat_task(
    mut eyes: led::Eyes<'static>,
    mut hardware: CatHardware,
    receiver: Receiver<'static, CriticalSectionRawMutex, Command, 8>,
) -> ! {
    println!("Eye LEDs yellow (bootup...)");
    eyes.set_color(led::YELLOW).await;

    loop {
        match receiver.receive().await {
            Command::Color(color) => {
                println!("Eye LEDs: rgb({}, {}, {})", color.r, color.g, color.b);
                eyes.set_color(color).await;
            }
            Command::Wink => {
                eyes.show().await;
                hardware.wave().await;
                eyes.black().await;
            }
        }
    }
}
