use crate::{Parser, PingResult, Pinger};
use lazy_regex::*;
use std::time::Duration;

pub static RE: Lazy<Regex> = lazy_regex!(r"time=(?:(?P<ms>[0-9]+).(?P<ns>[0-9]+)\s+ms)");

pub struct BSDPinger {
    interval: Duration,
    interface: Option<String>,
}

impl Pinger for BSDPinger {
    type Parser = BSDParser;

    fn new(interval: Duration, interface: Option<String>) -> Self {
        Self {
            interface,
            interval,
        }
    }

    fn ping_args(&self, target: String) -> (&str, Vec<String>) {
        let mut args = vec![format!(
            "-i{:.1}",
            self.interval.as_millis() as f32 / 1_000_f32
        )];
        if let Some(interface) = &self.interface {
            args.push("-I".into());
            args.push(interface.clone());
        }
        args.push(target);
        ("ping", args)
    }
}

#[derive(Default)]
pub struct BSDParser {}

impl Parser for BSDParser {
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
