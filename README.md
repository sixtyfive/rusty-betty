# Rusty Betty ₍^. .^₎⟆

A rust-based Winkekatze for [Winkekatze24](https://www.winkekatze24.de/). More specifically, a cat for the _Winke Vier.Miau_ board (any hardware revision), which in turn is based on the [WaveShare ESP32-C6-Zero mini development board](https://www.waveshare.com/esp32-c6-zero.htm?sku=27035).

## Building

You'll need rustup. On ArchLinux-based Linux distros, that's just `sudo pacman -Sy rustup`. Then:

```
rustup target add riscv32imac-unknown-none-elf # only once
cargo run --release
```

Yes, that's it. The first time around, it'll download a shit-ton of stuff, though. One thing: if you only edit config.toml you'll have to `touch build.rs` in order for Cargo to pick up on those changes!

## Status LED Error Codes

The built-in RGB LED next to the Zero's USB-C port will go green if all is well after boot (WiFi, DHCP, MQTT), and if all _stays_ well for 20s, will
turn itself off so as to not shine through the cat body all the time.

If, however, at some point anything goes wrong, it will act as a debugging
helper:

| # of blinks | problem              |
| ----------- | -------------------- |
| 1           | WiFi connection lost |
| 2           | DHCP lease lost      |
| 3           | MQTT connection lost |

## Testing

### Hardware

There is board test code that exercises all peripherals broken out on the
Winke Vier.Miau board in src/board_test.rs.

Build and upload using `cargo run --release --bin board_test`.

### Software

Anything that's hardware-independent (LED pulse, fade, color helpers) isn't in ./src/, but in ./betty-core/src/ so that there can also be tests and the Rust linter leaves me in peace (otherwise it'll complain about a missing test create). You can run the tests like so:

```
cargo test --manifest-path betty-core/Cargo.toml \
    --target x86_64-unknown-linux-gnu --target-dir target/host
```
