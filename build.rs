use std::{collections::HashSet, env, fmt::Write as _, fs, path::PathBuf};

fn main() {
  println!("cargo:rerun-if-changed=config.toml");
  println!("cargo:rerun-if-changed=build.rs");

  let text = fs::read_to_string("config.toml").expect("Error: config.toml missing!");
  let cfg: toml::Table = text.parse().expect("Error: config.toml: invalid TOML!");

  let get = |sec: &str, key: &str| {
    cfg
      .get(sec)
      .and_then(|t| t.get(key))
      .unwrap_or_else(|| panic!("Error: config.toml: [{sec}] {key} missing!"))
      .clone()
  };
  let s = |sec: &str, key: &str| -> String {
    get(sec, key)
      .as_str()
      .unwrap_or_else(|| panic!("Error: [{sec}] {key}: expected a string!"))
      .to_string()
  };
  let i = |sec: &str, key: &str| -> i64 {
    get(sec, key)
      .as_integer()
      .unwrap_or_else(|| panic!("Error: [{sec}] {key}: expected a number!"))
  };
  let opt_i = |sec: &str, key: &str| -> Option<i64> {
    cfg.get(sec).and_then(|t| t.get(key)).map(|value| {
      value
        .as_integer()
        .unwrap_or_else(|| panic!("Error: [{sec}] {key}: expected a number!"))
    })
  };
  let opt_arr = |sec: &str, key: &str| -> Option<Vec<i64>> {
    cfg.get(sec).and_then(|t| t.get(key)).map(|value| {
      value
        .as_array()
        .unwrap_or_else(|| panic!("Error: [{sec}] {key}: expected an array of numbers!"))
        .iter()
        .map(|entry| {
          entry
            .as_integer()
            .unwrap_or_else(|| panic!("Error: [{sec}] {key}: expected an array of numbers!"))
        })
        .collect()
    })
  };

  let mut out = String::new();
  writeln!(out, "pub const WIFI_SSID: &str = {:?};", s("wifi", "ssid")).unwrap();
  writeln!(
    out,
    "pub const WIFI_PASSWORD: &str = {:?};",
    s("wifi", "password")
  )
  .unwrap();
  let cat_name = s("cat", "name");
  writeln!(out, "pub const CAT_NAME: &str = {cat_name:?};").unwrap();
  writeln!(
    out,
    "pub const CAT_NAME_LOWER: &str = {:?};",
    cat_name.to_lowercase()
  )
  .unwrap();

  let eye_activity_duration = opt_i("cat", "eye_activity_duration").unwrap_or(30_000);
  assert!(
    eye_activity_duration >= 0,
    "Error: [cat] eye_activity_duration must not be negative!"
  );
  writeln!(
    out,
    "pub const EYE_ACTIVITY_DURATION_MS: u64 = {eye_activity_duration};\n"
  )
  .unwrap();

  if cfg.get("servo").and_then(|t| t.get("middle")).is_some() {
    panic!("Error: [servo] middle was renamed to middle_angle and is now specified in degrees!");
  }

  let pulse_min_us = opt_i("servo", "pulse_min_us").unwrap_or(800);
  let pulse_max_us = opt_i("servo", "pulse_max_us").unwrap_or(2200);
  assert!(
    pulse_min_us > 0,
    "Error: [servo] pulse_min_us must be greater than 0!"
  );
  assert!(
    pulse_max_us > pulse_min_us,
    "Error: [servo] pulse_max_us must be greater than pulse_min_us!"
  );
  assert!(
    pulse_max_us < 20_000,
    "Error: [servo] pulse_max_us must be shorter than the 20ms PWM period!"
  );
  writeln!(
    out,
    "#[allow(dead_code)]\npub const SERVO_PULSE_MIN_US: u32 = {pulse_min_us};\n"
  )
  .unwrap();
  writeln!(
    out,
    "#[allow(dead_code)]\npub const SERVO_PULSE_MAX_US: u32 = {pulse_max_us};\n"
  )
  .unwrap();

  let angle_min = opt_i("servo", "angle_min").unwrap_or(0);
  let angle_max = opt_i("servo", "angle_max").unwrap_or(180);
  assert!(
    (0..=360).contains(&angle_min),
    "Error: [servo] angle_min must be between 0 and 360!"
  );
  assert!(
    (0..=360).contains(&angle_max),
    "Error: [servo] angle_max must be between 0 and 360!"
  );
  assert!(
    angle_max > angle_min,
    "Error: [servo] angle_max must be greater than angle_min!"
  );
  writeln!(
    out,
    "#[allow(dead_code)]\npub const SERVO_ANGLE_MIN: u16 = {angle_min};\n"
  )
  .unwrap();
  writeln!(
    out,
    "#[allow(dead_code)]\npub const SERVO_ANGLE_MAX: u16 = {angle_max};\n"
  )
  .unwrap();

  let middle_angle = opt_i("servo", "middle_angle").unwrap_or(90);
  assert!(
    angle_min <= middle_angle && middle_angle <= angle_max,
    "Error: [servo] middle_angle must be between angle_min and angle_max!"
  );
  writeln!(
    out,
    "#[allow(dead_code)]\npub const SERVO_MIDDLE_ANGLE: u16 = {middle_angle};\n"
  )
  .unwrap();

  let swing_angles =
    opt_arr("servo", "swing_angles").unwrap_or_else(|| vec![155, 25, 155, 50, 130, 75, 105]);
  assert!(
    !swing_angles.is_empty(),
    "Error: [servo] swing_angles must not be empty!"
  );
  for angle in &swing_angles {
    assert!(
      angle_min <= *angle && *angle <= angle_max,
      "Error: [servo] swing_angles must contain angles between angle_min and angle_max!"
    );
  }
  let angles = swing_angles
    .iter()
    .map(|angle| angle.to_string())
    .collect::<Vec<_>>()
    .join(", ");
  writeln!(
    out,
    "#[allow(dead_code)]\npub const SERVO_SWING_ANGLES: &[u16] = &[{angles}];\n"
  )
  .unwrap();

  let sweep_ms = opt_i("servo", "sweep_ms").unwrap_or(300);
  assert!(
    sweep_ms > 0,
    "Error: [servo] sweep_ms must be greater than 0!"
  );
  writeln!(
    out,
    "#[allow(dead_code)]\npub const SERVO_SWEEP_MS: u64 = {sweep_ms};\n"
  )
  .unwrap();

  let servo_steps = opt_i("servo", "steps").unwrap_or(20);
  assert!(
    servo_steps > 0,
    "Error: [servo] steps must be greater than 0!"
  );
  writeln!(
    out,
    "#[allow(dead_code)]\npub const SERVO_STEPS: u64 = {servo_steps};\n"
  )
  .unwrap();

  let led_brightness = opt_i("cat", "led_brightness").unwrap_or(100);
  assert!(
    (0..=100).contains(&led_brightness),
    "Error: [cat] led_brightness must be between 0 and 100!"
  );
  writeln!(out, "pub const LED_BRIGHTNESS: u8 = {led_brightness};\n").unwrap();

  println!("cargo:rustc-check-cfg=cfg(actuator_magnet)");
  println!("cargo:rustc-check-cfg=cfg(actuator_servo)");
  match s("cat", "active_actuator").to_ascii_lowercase().as_str() {
    "both" => {
      println!("cargo:rustc-cfg=actuator_magnet");
      println!("cargo:rustc-cfg=actuator_servo");
    }
    "magnet" => println!("cargo:rustc-cfg=actuator_magnet"),
    "servo" => println!("cargo:rustc-cfg=actuator_servo"),
    _ => panic!("Error: [cat] active_actuator must be one of \"both\", \"magnet\", \"servo\"!"),
  }

  let url = s("mqtt", "url");
  let rest = url
    .strip_prefix("mqtt://")
    .or_else(|| url.strip_prefix("tcp://"))
    .unwrap_or_else(|| panic!("Error: MQTT URL must start with mqtt://!"))
    .trim_end_matches('/');
  let (host, port) = match rest.rsplit_once(':') {
    Some((h, p)) => (
      h,
      p.parse::<u16>().expect("Error: invalid port for MQTT URL!"),
    ),
    None => (rest, 1883),
  };
  writeln!(out, "pub const MQTT_HOST: &str = {host:?};").unwrap();
  writeln!(out, "pub const MQTT_PORT: u16 = {port};").unwrap();

  let keepalive_ms = opt_i("mqtt", "keepalive_ms").unwrap_or(30_000);
  assert!(
    keepalive_ms > 0,
    "Error: [mqtt] keepalive_ms must be greater than 0!"
  );
  writeln!(out, "pub const MQTT_KEEPALIVE_MS: u64 = {keepalive_ms};").unwrap();

  let mut used = HashSet::new();
  for key in [
    "led_builtin",
    "eye_leds",
    "blink",
    "i2c_sda",
    "i2c_scl",
    "pwm",
    "adc",
  ] {
    let n = i("pins", key);
    assert!(
      (0..=23).contains(&n),
      "Error: pins.{key}: GPIO number must be between 0 and 23!"
    );
    assert!(
      n != 12 && n != 13,
      "Error: pins.{key}: GPIOs 12 and 13 are reserved for USB!"
    );
    assert!(
      used.insert(n),
      "Error: pins.{key}: GPIO{n} specified multiple times!"
    );
    writeln!(
      out,
      "#[allow(unused_macros)]\nmacro_rules! pin_{key} {{ ($p:expr) => {{ $p.GPIO{n} }}; }}"
    )
    .unwrap();
  }

  fs::write(
    PathBuf::from(env::var("OUT_DIR").unwrap()).join("config.rs"),
    out,
  )
  .unwrap();
}
