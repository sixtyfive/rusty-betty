use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};

const HELP_PULSES: u64 = 10;

fn help_delay_us(i: u64) -> u64 {
  100_000 * (i + 2) / 2
}

pub struct Magnet<'d> {
  pin: Output<'d>,
  duration: Duration,
  sleep_after_wave: Duration,
  needs_help: bool,
}

impl<'d> Magnet<'d> {
  pub fn new(pin: impl OutputPin + 'd, needs_help: bool) -> Self {
    Self {
      pin: Output::new(pin, Level::Low, OutputConfig::default()),
      duration: Duration::from_secs(2),
      sleep_after_wave: Duration::from_secs(3),
      needs_help,
    }
  }

  pub async fn wave(&mut self) {
    if self.needs_help {
      for i in 0..HELP_PULSES {
        let delay_us = help_delay_us(i);
        self.pin.set_high();
        Timer::after(Duration::from_micros(delay_us * 9 / 5)).await;
        self.pin.set_low();
        Timer::after(Duration::from_micros(delay_us * 3 / 4)).await;
      }
    } else {
      self.pin.set_high();
      Timer::after(self.duration).await;
      self.pin.set_low();
      Timer::after(self.sleep_after_wave).await;
    }
  }
}
