#![no_std]
#![no_main]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
#[cfg(all(actuator_magnet, actuator_servo))]
use embassy_futures::join::join;
use embassy_futures::select::{select, Either};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use embassy_time::{Duration, Instant, Timer};
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

/// How often the boot animation updates the eye LEDs.
const BOOT_PULSE_INTERVAL: Duration = Duration::from_millis(25);
/// How long the eyes pulse yellow before showing WiFi/MQTT progress.
const BOOT_YELLOW_PERIOD: Duration = Duration::from_millis(1500);

include!(concat!(env!("OUT_DIR"), "/config.rs"));

mod led;
#[cfg(actuator_magnet)]
mod magnet;
mod mqtt;
#[cfg(actuator_servo)]
mod servo;
mod wifi;

pub static WIFI_UP: AtomicBool = AtomicBool::new(false);
pub static IP_UP: AtomicBool = AtomicBool::new(false);
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
    let boot_started = Instant::now();
    let mut pulse = led::Pulse::new();
    let mut fade = led::Fade::new(led::YELLOW);
    let mut last_phase = u8::MAX;
    let mut booting = true;
    let mut pending = None;

    // Pulse the eyes while booting: yellow first, then blue until an IP address
    // is acquired, then green while waiting for MQTT. Commands are only handled
    // once the color transition has settled, so MQTT coming online cannot cut a
    // fade short.
    loop {
        while booting {
            let (phase, target) = if IP_UP.load(Ordering::Relaxed) {
                (2, led::GREEN)
            } else if boot_started.elapsed() < BOOT_YELLOW_PERIOD {
                (0, led::YELLOW)
            } else {
                (1, led::BLUE)
            };

            let event = if pending.is_none() {
                select(receiver.receive(), Timer::after(BOOT_PULSE_INTERVAL)).await
            } else {
                Timer::after(BOOT_PULSE_INTERVAL).await;
                Either::Second(())
            };

            if let Either::First(command) = event {
                pending = Some(command);
            }

            if phase != last_phase {
                match phase {
                    0 => println!("Eye LEDs pulsing yellow (bootup...)"),
                    1 => println!("Eye LEDs pulsing blue (waiting for WiFi/IP)"),
                    _ => println!("Eye LEDs pulsing green (waiting for MQTT)"),
                }
                last_phase = phase;
            }

            let color = fade.next(target);
            eyes.write_brightness(color, pulse.next()).await;

            if pending.is_some() && fade.is_settled(target) {
                booting = false;
            }
        }

        let command = match pending.take() {
            Some(command) => command,
            None => receiver.receive().await,
        };

        match command {
            Command::Color(color) => {
                eyes.set_color(color).await;
            }
            Command::Wink => {
                eyes.show().await;
                hardware.wave().await;
                eyes.fade_out().await;
            }
        }
    }
}
