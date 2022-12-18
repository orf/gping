use crate::{Parser, PingResult, Pinger};
use regex::Regex;
use std::time::Duration;

lazy_static! {
    static ref RE: Regex = Regex::new(r"time=(?:(?P<ms>[0-9]+).(?P<ns>[0-9]+)\s+ms)").unwrap();
}

#[derive(Default)]
pub struct MacOSPinger {
    interval: Duration,
    interface: Option<String>,
}

impl Pinger for MacOSPinger {
    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }

    fn ping_args(&self, target: String) -> Vec<String> {
        let mut args = vec![
            format!("-i{:.1}", self.interval.as_millis() as f32 / 1_000_f32),
            target,
        ];
        if let Some(interface) = &self.interface {
            args.push("-b".into());
            args.push(interface.clone());
        }

        args
    }
}

#[derive(Default)]
pub struct MacOSParser {}

impl Parser for MacOSParser {
    fn parse(&self, line: String) -> Option<PingResult> {
        if line.starts_with("PING ") {
            return None;
        }
        if line.starts_with("Request timeout") {
            return Some(PingResult::Timeout(line));
        }
        self.extract_regex(&RE, line)
    }
}
