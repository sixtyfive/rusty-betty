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

pub const STATUS_OK_HOLD_MS: u64 = 20_000;

const STATUS_POLL_MS: u64 = 250;

const STATUS_BLINK_ON_MS: u64 = 50;
const STATUS_BLINK_OFF_MS: u64 = 250;
const STATUS_BLINK_GAP_MS: u64 = 1_500;

const STATUS_BOOT_WIFI: RGB8 = RGB8::new(0x14, 0x00, 0x00);
const STATUS_BOOT_DHCP: RGB8 = RGB8::new(0x14, 0x08, 0x00);
const STATUS_BOOT_MQTT: RGB8 = RGB8::new(0x00, 0x14, 0x00);
const STATUS_FAULT_RED: RGB8 = RGB8::new(0x40, 0x00, 0x00);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
  pub wifi: bool,
  pub ip: bool,
  pub mqtt: bool,
}

impl Health {
  pub const fn new(wifi: bool, ip: bool, mqtt: bool) -> Self {
    Self { wifi, ip, mqtt }
  }

  pub const fn fault(self) -> Option<Fault> {
    if !self.wifi {
      Some(Fault::Wifi)
    } else if !self.ip {
      Some(Fault::Dhcp)
    } else if !self.mqtt {
      Some(Fault::Mqtt)
    } else {
      None
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
  Wifi,
  Dhcp,
  Mqtt,
}

impl Fault {
  pub const fn blinks(self) -> u8 {
    match self {
      Fault::Wifi => 1,
      Fault::Dhcp => 2,
      Fault::Mqtt => 3,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusFrame {
  pub color: Option<RGB8>,
  pub wait_ms: u64,
}

pub struct StatusLed {
  settled: bool,
  healthy_since: Option<u64>,
  fault: Option<Fault>,
  fault_since: Option<u64>,
}

impl Default for StatusLed {
  fn default() -> Self {
    Self::new()
  }
}

impl StatusLed {
  pub const fn new() -> Self {
    Self {
      settled: false,
      healthy_since: None,
      fault: None,
      fault_since: None,
    }
  }

  pub fn next(&mut self, health: Health, now_ms: u64) -> StatusFrame {
    if let Some(fault) = health.fault() {
      self.healthy_since = None;
      if self.fault != Some(fault) {
        self.fault = Some(fault);
        self.fault_since = Some(now_ms);
      }

      if self.settled {
        let elapsed = now_ms.saturating_sub(self.fault_since.unwrap_or(now_ms));
        return blink_frame(fault, elapsed);
      }

      return StatusFrame {
        color: Some(boot_color(health)),
        wait_ms: STATUS_POLL_MS,
      };
    }

    self.fault = None;
    self.fault_since = None;

    let healthy_since = *self.healthy_since.get_or_insert(now_ms);
    if self.settled || now_ms.saturating_sub(healthy_since) >= STATUS_OK_HOLD_MS {
      self.settled = true;
      StatusFrame {
        color: None,
        wait_ms: STATUS_POLL_MS,
      }
    } else {
      StatusFrame {
        color: Some(STATUS_BOOT_MQTT),
        wait_ms: STATUS_POLL_MS,
      }
    }
  }
}

const fn boot_color(health: Health) -> RGB8 {
  if !health.wifi {
    STATUS_BOOT_WIFI
  } else if !health.ip {
    STATUS_BOOT_DHCP
  } else {
    STATUS_BOOT_MQTT
  }
}

fn blink_frame(fault: Fault, elapsed_ms: u64) -> StatusFrame {
  let blink_ms = STATUS_BLINK_ON_MS + STATUS_BLINK_OFF_MS;
  let train_ms = fault.blinks() as u64 * blink_ms;
  let cycle_ms = train_ms + STATUS_BLINK_GAP_MS;
  let position = elapsed_ms % cycle_ms;

  if position < train_ms {
    let phase = position % blink_ms;
    if phase < STATUS_BLINK_ON_MS {
      StatusFrame {
        color: Some(STATUS_FAULT_RED),
        wait_ms: STATUS_BLINK_ON_MS - phase,
      }
    } else {
      StatusFrame {
        color: None,
        wait_ms: blink_ms - phase,
      }
    }
  } else {
    StatusFrame {
      color: None,
      wait_ms: cycle_ms - position,
    }
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

#[cfg(test)]
mod tests;
