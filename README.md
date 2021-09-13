# Control your Elgato Keylight with Rust!

This library allows you to easily control your Elgato Keylight.

-   Simple wrapper that also handles caching of the lamp state for using in applications.
-   Supports **zeroconf** to discover your lights from name, instead of requiring you to know the IP.

[crates.io](https://crates.io/crates/elgato-keylight) |
[docs.rs](https://docs.rs/elgato-keylight/latest/elgato-keylight/)

## Usage

You can test the library easy by opening `examples/from_name.rs`. `from_name.rs` and `from_ip.rs` has identical functionality, only differing in how they connect.

```rust
//Lookup lamp by name (using zeroconf)
let mut kl = KeyLight::new_from_name("Key Light Left", None).await?;

//Turn on the light
kl.set_power(true).await?;

//Set brightness to 30
kl.set_brightness(30).await?;
```

## Contributing

Just open a PR LUL

## License

All under MIT
