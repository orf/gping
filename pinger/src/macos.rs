use crate::{Parser, PingResult, Pinger};
use lazy_regex::*;
use std::net::Ipv6Addr;
use std::time::Duration;

pub static RE: Lazy<Regex> = lazy_regex!(r"time=(?:(?P<ms>[0-9]+).(?P<ns>[0-9]+)\s+ms)");

pub struct MacOSPinger {
    interval: Duration,
    interface: Option<String>,
}

impl Pinger for MacOSPinger {
    type Parser = MacOSParser;

    fn new(interval: Duration, interface: Option<String>) -> Self {
        Self {
            interval,
            interface,
        }
    }

    fn ping_args(&self, target: String) -> (&str, Vec<String>) {
        let cmd = match target.parse::<Ipv6Addr>() {
            Ok(_) => "ping6",
            Err(_) => "ping",
        };
        let mut args = vec![
            format!("-i{:.1}", self.interval.as_millis() as f32 / 1_000_f32),
            target,
        ];
        if let Some(interface) = &self.interface {
            args.push("-b".into());
            args.push(interface.clone());
        }

        (cmd, args)
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
