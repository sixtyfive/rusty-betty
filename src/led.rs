use esp_hal::{gpio::interconnect::PeripheralOutput, rmt::TxChannelCreator, time::Rate, Async};
use esp_hal_smartled::{buffer_size, color_order, RmtSmartLeds, WS2812B_TIMING};
use smart_leds::{SmartLedsWriteAsync, RGB8};

pub const BLACK: RGB8 = RGB8::new(0x00, 0x00, 0x00);
pub const BLUE: RGB8 = RGB8::new(0x00, 0x00, 0xFF);
pub const CYAN: RGB8 = RGB8::new(0x00, 0xFF, 0xFF);
pub const GREEN: RGB8 = RGB8::new(0x00, 0xFF, 0x00);
pub const PINK: RGB8 = RGB8::new(0x80, 0x00, 0x80);
pub const RED: RGB8 = RGB8::new(0xFF, 0x00, 0x00);
pub const WHITE: RGB8 = RGB8::new(0xFF, 0xFF, 0xFF);
pub const YELLOW: RGB8 = RGB8::new(0xFF, 0xFF, 0x00);

const LED_COUNT: usize = 2;

pub struct Eyes<'d> {
    leds: RmtSmartLeds<'d, { buffer_size::<RGB8>(LED_COUNT) }, Async, RGB8, color_order::Rgb>,
    saved: RGB8,
}

impl<'d> Eyes<'d> {
    pub fn new<Ch, P>(channel: Ch, pin: P, freq: Rate) -> Self
    where
        Ch: TxChannelCreator<'d, Async>,
        P: PeripheralOutput<'d>,
    {
        let leds =
            RmtSmartLeds::<{ buffer_size::<RGB8>(LED_COUNT) }, _, RGB8, color_order::Rgb>::new(
                WS2812B_TIMING,
                channel,
                pin,
                freq,
            )
            .unwrap();

        Self { leds, saved: GREEN }
    }

    pub async fn set_color(&mut self, color: RGB8) {
        self.saved = color;
        self.write(color).await;
    }

    pub async fn show(&mut self) {
        self.write(self.saved).await;
    }

    pub async fn black(&mut self) {
        self.write(BLACK).await;
    }

    async fn write(&mut self, color: RGB8) {
        let _ = self.leds.write([color; LED_COUNT]).await;
    }
}

pub fn color_by_name(name: &str) -> Option<RGB8> {
    let name = name.trim();

    if name.eq_ignore_ascii_case("black") {
        Some(BLACK)
    } else if name.eq_ignore_ascii_case("blue") {
        Some(BLUE)
    } else if name.eq_ignore_ascii_case("cyan") {
        Some(CYAN)
    } else if name.eq_ignore_ascii_case("green") {
        Some(GREEN)
    } else if name.eq_ignore_ascii_case("pink") {
        Some(PINK)
    } else if name.eq_ignore_ascii_case("red") {
        Some(RED)
    } else if name.eq_ignore_ascii_case("white") {
        Some(WHITE)
    } else if name.eq_ignore_ascii_case("yellow") {
        Some(YELLOW)
    } else {
        None
    }
}
