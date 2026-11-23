use smart_leds::RGB8;

pub const STATUS_OK_HOLD_MS: u64 = 20_000;

const STATUS_POLL_MS: u64 = 250;

const STATUS_BLINK_ON_MS: u64 = 50;
const STATUS_BLINK_OFF_MS: u64 = 250;
const STATUS_BLINK_GAP_MS: u64 = 1_500;

const STATUS_BOOT_WIFI: RGB8 = RGB8::new(0x14, 0x00, 0x00);
const STATUS_BOOT_DHCP: RGB8 = RGB8::new(0x14, 0x08, 0x00);
const STATUS_BOOT_MQTT: RGB8 = RGB8::new(0x00, 0x14, 0x00);
const STATUS_FAULT_RED: RGB8 = RGB8::new(0x40, 0x00, 0x00);

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

#[cfg(test)]
mod tests;
