use embassy_time::{Duration, Timer};
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

// Keep a little glow at the bottom of the pulse instead of fading to black.
const PULSE_MIN: u8 = 12;
const PULSE_MAX: u8 = 255;
const PULSE_STEP: u8 = 8;

// Maximum color-channel change per animation frame, for smooth fades.
const FADE_STEP: u8 = 6;

// Fade-out steps and timing after a wink.
const FADE_OUT_STEP: u8 = 8;
const FADE_OUT_INTERVAL: Duration = Duration::from_millis(25);

/// Triangle-wave brightness generator for pulsing the eyes.
pub struct Pulse {
    brightness: u8,
    rising: bool,
}

impl Pulse {
    pub const fn new() -> Self {
        Self {
            brightness: PULSE_MIN,
            rising: true,
        }
    }

    /// Returns the next brightness, ramping up to full and back down again.
    pub fn next(&mut self) -> u8 {
        if self.rising {
            self.brightness = self.brightness.saturating_add(PULSE_STEP).min(PULSE_MAX);
            if self.brightness == PULSE_MAX {
                self.rising = false;
            }
        } else {
            self.brightness = self.brightness.saturating_sub(PULSE_STEP).max(PULSE_MIN);
            if self.brightness == PULSE_MIN {
                self.rising = true;
            }
        }
        self.brightness
    }
}

/// Smoothes color changes by easing each channel toward a target color.
pub struct Fade {
    color: RGB8,
}

impl Fade {
    pub const fn new(color: RGB8) -> Self {
        Self { color }
    }

    /// Returns the next color on the way to `target`.
    pub fn next(&mut self, target: RGB8) -> RGB8 {
        self.color = RGB8::new(
            approach(self.color.r, target.r),
            approach(self.color.g, target.g),
            approach(self.color.b, target.b),
        );
        self.color
    }

    /// Returns whether the fade has reached `target`.
    pub fn is_settled(&self, target: RGB8) -> bool {
        self.color.r == target.r && self.color.g == target.g && self.color.b == target.b
    }
}

fn approach(from: u8, to: u8) -> u8 {
    let step = from.abs_diff(to).min(FADE_STEP);
    if from < to {
        from + step
    } else {
        from - step
    }
}

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

    /// Writes `color` at the given brightness without changing the saved color.
    pub async fn write_brightness(&mut self, color: RGB8, brightness: u8) {
        self.write(scale(color, brightness)).await;
    }

    pub async fn show(&mut self) {
        self.write(self.saved).await;
    }

    pub async fn black(&mut self) {
        self.write(BLACK).await;
    }

    /// Fades the saved color out to black instead of switching it off abruptly.
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
        let _ = self.leds.write([color; LED_COUNT]).await;
    }
}

fn scale(color: RGB8, brightness: u8) -> RGB8 {
    let b = brightness as u16;
    RGB8::new(
        (color.r as u16 * b / 255) as u8,
        (color.g as u16 * b / 255) as u8,
        (color.b as u16 * b / 255) as u8,
    )
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
