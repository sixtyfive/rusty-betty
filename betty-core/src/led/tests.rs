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
fn wink_phases_preserve_cycle_length() {
    let phases = wink_phases(4000, 300, 500);
    assert_eq!(phases.iter().sum::<u64>(), 4000);
    assert_eq!(phases[1], 300);
    assert_eq!(phases[2], 500);
    assert_eq!(phases[3], 300);
    assert_eq!(phases[0], phases[4]);
}

#[test]
fn wink_phases_shrink_ons_when_off_grows() {
    let short_off = wink_phases(4000, 100, 500);
    let long_off = wink_phases(4000, 500, 500);
    assert_eq!(short_off.iter().sum::<u64>(), 4000);
    assert_eq!(long_off.iter().sum::<u64>(), 4000);
    assert!(short_off[0] > long_off[0]);
}

#[test]
fn wink_phases_middle_on_is_configurable() {
    let short_mid = wink_phases(4000, 300, 200);
    let long_mid = wink_phases(4000, 300, 800);
    assert_eq!(short_mid[2], 200);
    assert_eq!(long_mid[2], 800);
    assert!(short_mid[0] > long_mid[0]);
    assert_eq!(short_mid.iter().sum::<u64>(), 4000);
    assert_eq!(long_mid.iter().sum::<u64>(), 4000);
}

#[test]
fn wink_phases_clamp_oversized_off() {
    let phases = wink_phases(1000, 10_000, 500);
    assert_eq!(phases, [0, 500, 0, 500, 0]);
    assert_eq!(phases.iter().sum::<u64>(), 1000);
}

#[test]
fn wink_phases_clamp_oversized_middle_on() {
    let phases = wink_phases(1000, 100, 10_000);
    assert_eq!(phases, [0, 100, 800, 100, 0]);
    assert_eq!(phases.iter().sum::<u64>(), 1000);
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
