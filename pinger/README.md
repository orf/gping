# pinger

> A small cross-platform library to execute the ping command and parse the output.

This crate is primarily built for use with `gping`, but it can also be used as a
standalone library.

This allows you to reliably ping hosts without having to worry about process permissions,
in a cross-platform manner on Windows, Linux and macOS.

## Usage

A full example of using the library can be found in the `examples/` directory, but the
interface is quite simple:

```rust
use std::time::Duration;
use pinger::{ping, PingOptions};

fn ping_google() {
    let options = PingOptions::new("google.com", Duration::from_secs(1), None);
    let stream = ping(options).expect("Error pinging");
    for message in stream {
        match message {
            pinger::PingResult::Pong(duration, _) => {
                println!("Duration: {:?}", duration)
            }
            _ => {} // Handle errors, log ping timeouts, etc.
        }
    }
}
```

## Adding pinger to your project.

`cargo add pinger`
