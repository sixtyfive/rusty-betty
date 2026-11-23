use embassy_time::{Duration, Timer};
use esp_hal::{gpio::interconnect::PeripheralOutput, rmt::TxChannelCreator, time::Rate, Async};
use esp_hal_smartled::{buffer_size, color_order, RmtSmartLeds, WS2812B_TIMING};
use smart_leds::{SmartLedsWriteAsync, RGB8};

use betty_core::led::scale;
pub use betty_core::led::{
  color_by_name, wink_phases, Fade, Health, Pulse, StatusLed, BLACK, BLUE, GREEN, PULSE_MAX,
  WINK_CYCLES, YELLOW,
};

const LED_COUNT: usize = 2;

const FADE_OUT_STEP: u8 = 8;
const FADE_OUT_INTERVAL: Duration = Duration::from_millis(25);

pub struct Eyes<'d> {
  leds: RmtSmartLeds<'d, { buffer_size::<RGB8>(LED_COUNT) }, Async, RGB8, color_order::Rgb>,
  saved: RGB8,
  brightness: u8,
}

impl<'d> Eyes<'d> {
  pub fn new<Ch, P>(channel: Ch, pin: P, freq: Rate) -> Self
  where
    Ch: TxChannelCreator<'d, Async>,
    P: PeripheralOutput<'d>,
  {
    let leds = RmtSmartLeds::<{ buffer_size::<RGB8>(LED_COUNT) }, _, RGB8, color_order::Rgb>::new(
      WS2812B_TIMING,
      channel,
      pin,
      freq,
    )
    .unwrap();

    Self {
      leds,
      saved: GREEN,
      brightness: (crate::LED_BRIGHTNESS as u16 * 255 / 100) as u8,
    }
  }

  pub async fn set_color(&mut self, color: RGB8) {
    self.saved = color;
    self.write(color).await;
  }

  pub async fn write_brightness(&mut self, color: RGB8, brightness: u8) {
    self.write(scale(color, brightness)).await;
  }

  pub async fn show(&mut self) {
    self.write(self.saved).await;
  }

  pub async fn black(&mut self) {
    self.write(BLACK).await;
  }

  pub async fn wink(&mut self, total: Duration) {
    let cycle_ms = total.as_millis() / WINK_CYCLES as u64;
    let phases = wink_phases(cycle_ms, crate::WINK_OFF_MS, crate::WINK_ON_MS);

    for _ in 0..WINK_CYCLES {
      for (phase, &ms) in phases.iter().enumerate() {
        if phase % 2 == 0 {
          self.show().await;
        } else {
          self.write(BLACK).await;
        }
        Timer::after(Duration::from_millis(ms)).await;
      }
    }
  }

  pub async fn fade_out(&mut self) {
    let color = self.saved;
    let mut brightness = PULSE_MAX;
    while brightness > FADE_OUT_STEP {
      brightness -= FADE_OUT_STEP;
      self.write_brightness(color, brightness).await;
      Timer::after(FADE_OUT_INTERVAL).await;
    }
    self.black().await;
  }

  async fn write(&mut self, color: RGB8) {
    let color = scale(color, self.brightness);
    let _ = self.leds.write([color; LED_COUNT]).await;
  }
}
