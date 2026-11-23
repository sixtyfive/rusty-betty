use super::*;

const HEALTHY: Health = Health::new(true, true, true);

fn settle(led: &mut StatusLed) {
  assert_eq!(led.next(HEALTHY, 0).color, Some(STATUS_BOOT_MQTT));
  assert_eq!(led.next(HEALTHY, STATUS_OK_HOLD_MS).color, None);
}

fn frames_during(led: &mut StatusLed, health: Health, start_ms: u64, ms: u64) -> Vec<StatusFrame> {
  let mut frames = Vec::new();
  let mut now = start_ms;
  while now < start_ms + ms {
    let frame = led.next(health, now);
    now += frame.wait_ms;
    frames.push(frame);
  }
  frames
}

#[test]
fn status_led_shows_boot_progress_colors() {
  let mut led = StatusLed::new();

  assert_eq!(
    led.next(Health::new(false, false, false), 0).color,
    Some(STATUS_BOOT_WIFI)
  );
  assert_eq!(
    led.next(Health::new(true, false, false), 500).color,
    Some(STATUS_BOOT_DHCP)
  );
  assert_eq!(
    led.next(Health::new(true, true, false), 1_000).color,
    Some(STATUS_BOOT_MQTT)
  );
}

#[test]
fn status_led_switches_off_after_healthy_hold() {
  let mut led = StatusLed::new();

  assert_eq!(led.next(HEALTHY, 0).color, Some(STATUS_BOOT_MQTT));
  assert_eq!(
    led.next(HEALTHY, STATUS_OK_HOLD_MS - 1).color,
    Some(STATUS_BOOT_MQTT)
  );
  assert_eq!(led.next(HEALTHY, STATUS_OK_HOLD_MS).color, None);
  assert_eq!(led.next(HEALTHY, STATUS_OK_HOLD_MS * 10).color, None);
}

#[test]
fn status_led_restarts_hold_after_a_boot_fault() {
  let mut led = StatusLed::new();

  led.next(HEALTHY, 0);
  led.next(HEALTHY, 10_000);

  assert_eq!(
    led.next(Health::new(false, false, false), 10_001).color,
    Some(STATUS_BOOT_WIFI)
  );

  assert_eq!(led.next(HEALTHY, 15_000).color, Some(STATUS_BOOT_MQTT));
  assert_eq!(led.next(HEALTHY, 34_999).color, Some(STATUS_BOOT_MQTT));
  assert_eq!(led.next(HEALTHY, 35_000).color, None);
}

#[test]
fn status_led_blinks_the_matching_fault_code() {
  for (health, expected) in [
    (Health::new(false, true, true), 1),
    (Health::new(true, false, true), 2),
    (Health::new(true, true, false), 3),
  ] {
    let mut led = StatusLed::new();
    settle(&mut led);

    let blink_ms = STATUS_BLINK_ON_MS + STATUS_BLINK_OFF_MS;
    let cycle_ms = expected * blink_ms + STATUS_BLINK_GAP_MS;
    let frames = frames_during(&mut led, health, STATUS_OK_HOLD_MS, cycle_ms);
    let reds = frames
      .iter()
      .filter(|frame| frame.color == Some(STATUS_FAULT_RED))
      .count();

    assert_eq!(reds, expected as usize, "for {health:?}");
  }
}

#[test]
fn status_led_blink_cycles_repeat() {
  let mut led = StatusLed::new();
  settle(&mut led);

  let health = Health::new(true, true, false);
  let cycle_ms = 3 * (STATUS_BLINK_ON_MS + STATUS_BLINK_OFF_MS) + STATUS_BLINK_GAP_MS;
  let frames = frames_during(&mut led, health, STATUS_OK_HOLD_MS, cycle_ms * 2);
  let reds = frames
    .iter()
    .filter(|frame| frame.color == Some(STATUS_FAULT_RED))
    .count();

  assert_eq!(reds, 6);
}

#[test]
fn status_led_switches_off_again_after_recovery() {
  let mut led = StatusLed::new();
  settle(&mut led);

  let down = Health::new(true, true, false);
  assert_eq!(
    led.next(down, STATUS_OK_HOLD_MS).color,
    Some(STATUS_FAULT_RED)
  );
  assert_eq!(led.next(HEALTHY, STATUS_OK_HOLD_MS + 100).color, None);
}

#[test]
fn status_led_does_not_blink_during_boot() {
  let mut led = StatusLed::new();

  let frames = frames_during(&mut led, Health::new(false, false, false), 0, 5_000);

  assert!(frames
    .iter()
    .all(|frame| frame.color == Some(STATUS_BOOT_WIFI)));
}
