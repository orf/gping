use crate::{extract_regex, PingCreationError, PingOptions, PingResult, Pinger};
use lazy_regex::*;

pub static RE: Lazy<Regex> = lazy_regex!(r"time=(?:(?P<ms>[0-9]+).(?P<ns>[0-9]+)\s+ms)");

pub struct BSDPinger {
    options: PingOptions,
}

pub(crate) fn parse_bsd(line: String) -> Option<PingResult> {
    if line.starts_with("PING ") {
        return None;
    }
    if line.starts_with("Request timeout") {
        return Some(PingResult::Timeout(line));
    }
    extract_regex(&RE, line)
}

impl Pinger for BSDPinger {
    fn from_options(options: PingOptions) -> Result<Self, PingCreationError>
    where
        Self: Sized,
    {
        Ok(Self { options })
    }

    fn parse_fn(&self) -> fn(String) -> Option<PingResult> {
        parse_bsd
    }

    fn ping_args(&self) -> (&str, Vec<String>) {
        let mut args = vec![format!(
            "-i{:.1}",
            self.options.interval.as_millis() as f32 / 1_000_f32
        )];
        if let Some(interface) = &self.options.interface {
            args.push("-I".into());
            args.push(interface.clone());
        }
        if let Some(raw_args) = &self.options.raw_arguments {
            args.extend(raw_args.iter().cloned());
        }
        args.push(self.options.target.to_string());
        ("ping", args)
    }
}
