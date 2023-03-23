use pinger::ping_with_interval;
use std::time::Duration;

fn main() {
    let pinger = ping_with_interval("google.com", Duration::from_secs(1), Some("enp5s0")).unwrap();

    while let Ok(rtt) = pinger.channel.recv().map_err(|e| panic!("{e}")) {
        println!("Ping: {}", rtt.as_secs_f64() * 1000.0);
    }
}
