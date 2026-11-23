use esp_hal::{gpio::interconnect::PeripheralOutput, rmt::TxChannelCreator, time::Rate, Async};
use esp_hal_smartled::{buffer_size, color_order, RmtSmartLeds, WS2812B_TIMING};
use smart_leds::{SmartLedsWriteAsync, RGB8};

pub use betty_core::led_builtin::{Health, StatusLed};

const LED_COUNT: usize = 1;
const OFF: RGB8 = RGB8::new(0x00, 0x00, 0x00);

pub struct BuiltinLed<'d> {
  leds: RmtSmartLeds<'d, { buffer_size::<RGB8>(LED_COUNT) }, Async, RGB8, color_order::Rgb>,
  shown: Option<RGB8>,
}

impl<'d> BuiltinLed<'d> {
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

    Self { leds, shown: None }
  }

  /// Shows `color`, or switches the LED off when `None` is given. Repeated
  /// calls with the same value do not touch the LED again.
  pub async fn show(&mut self, color: Option<RGB8>) {
    if color == self.shown {
      return;
    }

    let _ = self.leds.write([color.unwrap_or(OFF)]).await;
    self.shown = color;
  }
}
