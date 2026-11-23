use std::{collections::HashSet, env, fmt::Write as _, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=config.toml");
    println!("cargo:rerun-if-changed=build.rs");

    let text = fs::read_to_string("config.toml").expect("Error: config.toml missing!");
    let cfg: toml::Table = text.parse().expect("Error: config.toml: invalid TOML!");

    let get = |sec: &str, key: &str| {
        cfg.get(sec)
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

    let ka = i("mqtt", "keepalive_secs");
    assert!(
        (1..=65535).contains(&ka),
        "Error: MQTT keepalive must be between 1 and 65535!"
    );
    writeln!(out, "pub const MQTT_KEEPALIVE_SECS: u16 = {ka};").unwrap();

    let mut used = HashSet::new();
    for key in [
        "onboard_led",
        "ext_leds",
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
