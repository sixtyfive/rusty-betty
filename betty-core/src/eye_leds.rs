use smart_leds::RGB8;

pub const BLACK: RGB8 = RGB8::new(0x00, 0x00, 0x00);
pub const BLUE: RGB8 = RGB8::new(0x00, 0x00, 0xFF);
pub const CYAN: RGB8 = RGB8::new(0x00, 0xFF, 0xFF);
pub const GREEN: RGB8 = RGB8::new(0x00, 0xFF, 0x00);
pub const PINK: RGB8 = RGB8::new(0x80, 0x00, 0x80);
pub const RED: RGB8 = RGB8::new(0xFF, 0x00, 0x00);
pub const WHITE: RGB8 = RGB8::new(0xFF, 0xFF, 0xFF);
pub const YELLOW: RGB8 = RGB8::new(0xFF, 0xFF, 0x00);

const PULSE_MIN: u8 = 12;
pub const PULSE_MAX: u8 = 255;
const PULSE_STEP: u8 = 8;

const FADE_STEP: u8 = 6;

pub const WINK_CYCLES: u32 = 3;

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

pub struct Fade {
  color: RGB8,
}

impl Fade {
  pub const fn new(color: RGB8) -> Self {
    Self { color }
  }

  pub fn next(&mut self, target: RGB8) -> RGB8 {
    self.color = RGB8::new(
      approach(self.color.r, target.r),
      approach(self.color.g, target.g),
      approach(self.color.b, target.b),
    );
    self.color
  }

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

pub fn wink_phases(cycle_ms: u64, off_ms: u64, mid_on_ms: u64) -> [u64; 5] {
  let off_ms = off_ms.min(cycle_ms / 2);
  let mid_on_ms = mid_on_ms.min(cycle_ms - 2 * off_ms);
  let edge = (cycle_ms - 2 * off_ms - mid_on_ms) / 2;
  let middle = cycle_ms - 2 * off_ms - 2 * edge;
  [edge, off_ms, middle, off_ms, edge]
}

pub fn scale(color: RGB8, brightness: u8) -> RGB8 {
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

#[cfg(test)]
mod tests;
