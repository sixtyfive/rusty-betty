use super::*;

const NAMED_COLORS: [(&str, RGB8); 8] = [
  ("black", BLACK),
  ("blue", BLUE),
  ("cyan", CYAN),
  ("green", GREEN),
  ("pink", PINK),
  ("red", RED),
  ("white", WHITE),
  ("yellow", YELLOW),
];

#[test]
fn pulse_starts_rising() {
  let mut pulse = Pulse::new();
  let first = pulse.next();
  let second = pulse.next();

  assert_eq!(first, PULSE_MIN + PULSE_STEP);
  assert!(second > first);
}

#[test]
fn pulse_stays_within_bounds_and_reverses() {
  let mut pulse = Pulse::new();
  let mut previous = pulse.next();
  let mut saw_max = previous == PULSE_MAX;
  let mut saw_min = false;

  for _ in 0..200 {
    let current = pulse.next();
    assert!((PULSE_MIN..=PULSE_MAX).contains(&current));
    assert!(current.abs_diff(previous) <= PULSE_STEP);
    saw_max |= current == PULSE_MAX;
    saw_min |= current == PULSE_MIN;
    previous = current;
  }

  assert!(saw_max, "pulse should reach full brightness");
  assert!(saw_min, "pulse should return to its minimum brightness");
}

#[test]
fn pulse_ramps_up_then_down() {
  let mut pulse = Pulse::new();
  let mut values = [0u8; 200];
  for value in &mut values {
    *value = pulse.next();
  }

  let peak = values
    .iter()
    .position(|&value| value == PULSE_MAX)
    .expect("pulse should reach its peak");
  let trough = values[peak..]
    .iter()
    .position(|&value| value == PULSE_MIN)
    .map(|offset| peak + offset)
    .expect("pulse should return to its minimum after the peak");

  assert!(
    values[..peak].windows(2).all(|pair| pair[0] <= pair[1]),
    "brightness should not decrease on the way up"
  );
  assert!(
    values[peak..=trough]
      .windows(2)
      .all(|pair| pair[0] >= pair[1]),
    "brightness should not increase on the way down"
  );
}

#[test]
fn fade_moves_toward_target_without_overshooting() {
  let mut fade = Fade::new(BLACK);
  let mut previous = BLACK;

  for _ in 0..100 {
    let current = fade.next(WHITE);
    for (from, to) in [
      (previous.r, current.r),
      (previous.g, current.g),
      (previous.b, current.b),
    ] {
      assert!(to >= from);
      assert!(to - from <= FADE_STEP);
    }
    previous = current;
  }

  assert!(fade.is_settled(WHITE));
}

#[test]
fn fade_descends_to_darker_target() {
  let mut fade = Fade::new(WHITE);
  let mut previous = WHITE;

  for _ in 0..100 {
    let current = fade.next(BLACK);
    assert!(current.r <= previous.r);
    assert!(previous.r - current.r <= FADE_STEP);
    previous = current;
  }

  assert!(fade.is_settled(BLACK));
}

#[test]
fn approach_clamps_at_target() {
  assert_eq!(approach(0, 255), FADE_STEP);
  assert_eq!(approach(250, 255), 255);
  assert_eq!(approach(10, 0), 4);
  assert_eq!(approach(5, 5), 5);
}

#[test]
fn blink_closes_fast_in_coarse_steps() {
  let values: Vec<u8> = blink_close().collect();
  assert_eq!(values, [255, 204, 153, 102, 51, 0]);
}

#[test]
fn blink_opens_slower_in_finer_steps() {
  let values: Vec<u8> = blink_open().collect();
  assert_eq!(
    values,
    [0, 25, 50, 75, 100, 125, 150, 175, 200, 225, 250, 255]
  );
  assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn blink_ramps_cover_all_brightness_values() {
  let close: Vec<u8> = blink_close().collect();
  let open: Vec<u8> = blink_open().collect();

  assert_eq!(close.first(), Some(&PULSE_MAX));
  assert_eq!(close.last(), Some(&0));
  assert_eq!(open.first(), Some(&0));
  assert_eq!(open.last(), Some(&PULSE_MAX));
}

#[test]
fn scale_at_full_brightness_keeps_color() {
  let color = RGB8::new(12, 34, 56);
  assert_eq!(scale(color, 255), color);
}

#[test]
fn scale_at_zero_brightness_is_black() {
  assert_eq!(scale(WHITE, 0), BLACK);
}

#[test]
fn scale_is_proportional() {
  assert_eq!(scale(RGB8::new(255, 128, 0), 50), RGB8::new(50, 25, 0));
}

#[test]
fn colors_are_resolved_case_insensitively_and_trimmed() {
  for (name, expected) in NAMED_COLORS {
    assert_eq!(color_by_name(name), Some(expected));
    assert_eq!(
      color_by_name(&format!("  {}  ", name.to_uppercase())),
      Some(expected)
    );
  }
}

#[test]
fn unknown_colors_are_rejected() {
  assert_eq!(color_by_name("magenta"), None);
  assert_eq!(color_by_name(""), None);
  assert_eq!(color_by_name("greenish"), None);
}
