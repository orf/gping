use crate::{Parser, PingResult, Pinger};
use regex::Regex;
use std::time::Duration;

#[derive(Default)]
pub struct LinuxPinger {
    interval: Duration,
}

impl Pinger for LinuxPinger {
    fn ping_args(&self, target: String) -> Vec<String> {
        // The -O flag ensures we "no answer yet" messages from ping
        // See https://superuser.com/questions/270083/linux-ping-show-time-out
        vec![
            "-O".to_string(),
            format!("-i{:.1}", self.interval.as_millis() as f32 / 1_000_f32),
            target,
        ]
    }

    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }
}

lazy_static! {
    static ref UBUNTU_RE: Regex = Regex::new(r"(?i-u)time=(?P<time>\d+(?:\.\d+)?) *ms").unwrap();
}

#[derive(Default)]
pub struct LinuxParser {}

impl Parser for LinuxParser {
    fn parse(&self, line: String) -> Option<PingResult> {
        if line.starts_with("64 bytes from") {
            return self.extract_regex(&UBUNTU_RE, line);
        } else if line.starts_with("no answer yet") {
            return Some(PingResult::Timeout(line));
        }
        None
    }
}
