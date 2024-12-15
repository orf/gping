use pinger::{ping, PingOptions};

const LIMIT: usize = 3;

pub fn main() {
    let target = "tomforb.es".to_string();
    let interval = std::time::Duration::from_millis(500);
    let options = PingOptions::new(target, interval, None);
    let stream = ping(options).expect("Error pinging");
    for message in stream.into_iter().take(LIMIT) {
        match message {
            pinger::PingResult::Pong(duration, line) => {
                println!("Duration: {:?}\t\t(raw: {:?})", duration, line)
            }
            pinger::PingResult::Timeout(line) => println!("Timeout! (raw: {line:?})"),
            pinger::PingResult::Unknown(line) => println!("Unknown line: {:?}", line),
            pinger::PingResult::PingExited(code, stderr) => {
                panic!("Ping exited! Code: {:?}. Stderr: {:?}", code, stderr)
            }
        }
    }
}
