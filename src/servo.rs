use embassy_time::{Duration, Instant, Timer};
use embedded_hal::pwm::SetDutyCycle;
use esp_hal::{
  gpio::DriveMode,
  ledc::{
    channel::{self, Channel, ChannelIFace},
    timer::TimerIFace,
    LowSpeed,
  },
};

use crate::{
  SERVO_ANGLE_MAX, SERVO_ANGLE_MIN, SERVO_MIDDLE_ANGLE, SERVO_PULSE_MAX_US, SERVO_PULSE_MIN_US,
  SERVO_STEPS, SERVO_SWEEP_MS, SERVO_SWING_ANGLES,
};

pub struct Servo<'a> {
  channel: Channel<'a, LowSpeed>,
  max_duty: u16,
  period_us: u32,
}

impl<'a> Servo<'a> {
  pub fn new(mut channel: Channel<'a, LowSpeed>, timer: &'a dyn TimerIFace<LowSpeed>) -> Self {
    let period_us = 1_000_000 / timer.frequency().max(1);
    let middle_pct = (pulse_us(SERVO_MIDDLE_ANGLE) * 100 / period_us) as u8;

    channel
      .configure(channel::config::Config {
        timer,
        // Hold a valid neutral position from boot on. A duty of 0 means "no
        // pulses", which lets the servo go limp so the arm drops under gravity.
        duty_pct: middle_pct,
        drive_mode: DriveMode::PushPull,
      })
      .unwrap();

    let max_duty = channel.max_duty_cycle();
    let mut servo = Self {
      channel,
      max_duty,
      period_us,
    };

    // The percent-based configure above is coarse, so set the exact angle.
    let duty = servo.raw(SERVO_MIDDLE_ANGLE);
    let _ = servo.channel.set_duty_cycle(duty);

    servo
  }

  pub async fn wave(&mut self) {
    let mut current = self.raw(SERVO_MIDDLE_ANGLE);

    // Visit every configured swing angle in order, then come back to rest.
    for &angle in SERVO_SWING_ANGLES {
      let target = self.raw(angle);
      self.sweep(current, target).await;
      current = target;
    }

    // Settle in the middle and keep holding it instead of going limp.
    let middle = self.raw(SERVO_MIDDLE_ANGLE);
    self.sweep(current, middle).await;
  }

  pub fn wave_duration(&self) -> Duration {
    // One sweep per configured angle, plus one to settle back in the middle.
    let sweeps = SERVO_SWING_ANGLES.len() as u64 + 1;
    Duration::from_millis(sweeps * SERVO_SWEEP_MS)
  }

  /// Moves smoothly from one raw duty value to another over `SERVO_SWEEP_MS`.
  async fn sweep(&mut self, from: u16, to: u16) {
    let steps = SERVO_STEPS.max(1);
    let start = Instant::now();

    for step in 1..=steps {
      let duty = from as i32 + (to as i32 - from as i32) * step as i32 / steps as i32;
      let _ = self.channel.set_duty_cycle(duty as u16);

      let elapsed_ms = SERVO_SWEEP_MS * step / steps;
      Timer::at(start + Duration::from_millis(elapsed_ms)).await;
    }
  }

  fn raw(&self, angle: u16) -> u16 {
    (self.max_duty as u32 * pulse_us(angle) / self.period_us) as u16
  }
}

/// Converts an angle into the pulse width that the servo expects.
fn pulse_us(angle: u16) -> u32 {
  let angle_span = (SERVO_ANGLE_MAX - SERVO_ANGLE_MIN) as u32;
  let pulse_span = SERVO_PULSE_MAX_US - SERVO_PULSE_MIN_US;

  SERVO_PULSE_MIN_US + (angle as u32 - SERVO_ANGLE_MIN as u32) * pulse_span / angle_span
}
