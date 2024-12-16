use crate::bsd::parse_bsd;
use crate::{PingCreationError, PingOptions, PingResult, Pinger};
use lazy_regex::*;

pub static RE: Lazy<Regex> = lazy_regex!(r"time=(?:(?P<ms>[0-9]+).(?P<ns>[0-9]+)\s+ms)");

pub struct MacOSPinger {
    options: PingOptions,
}

impl Pinger for MacOSPinger {
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
        let cmd = if self.options.target.is_ipv6() {
            "ping6"
        } else {
            "ping"
        };
        let mut args = vec![
            format!(
                "-i{:.1}",
                self.options.interval.as_millis() as f32 / 1_000_f32
            ),
            self.options.target.to_string(),
        ];
        if let Some(interface) = &self.options.interface {
            args.push("-b".into());
            args.push(interface.clone());
        }

        (cmd, args)
    }
}
