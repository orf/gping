use pinger::ping_with_interval;

pub fn main() {
    let target = "tomforb.es".to_string();
    let interval = std::time::Duration::from_secs(1);
    let stream = ping_with_interval(target, interval, None).expect("Error pinging");
    for message in stream {
        match message {
            pinger::PingResult::Pong(duration, line) => {
                println!("Duration: {:?}\t\t(raw: {:?})", duration, line)
            }
            pinger::PingResult::Timeout(line) => println!("Timeout! (raw: {line:?})"),
            pinger::PingResult::Unknown(line) => println!("Unknown line: {:?}", line),
            pinger::PingResult::PingExited(code, stderr) => {
                println!("Ping exited! Code: {:?}. Stderr: {:?}", code, stderr)
            }
        }
    }
}
