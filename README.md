# Rusty Betty ₍^. .^₎⟆

A rust-based Winkekatze for [Winkekatze24](https://www.winkekatze24.de/). More specifically, a cat for the _Winke Vier.Miau_ board (any hardware revision), which in turn is based on the [WaveShare ESP32-C6-Zero mini development board](https://www.waveshare.com/esp32-c6-zero.htm?sku=27035).

## Building

You'll need rustup. On ArchLinux-based Linux distros, that's just `sudo pacman -Sy rustup`. Then:

```
cargo run --release
```

Yes, that's it. The first time around, it'll download a shit-ton of stuff, though.
