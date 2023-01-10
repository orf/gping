use crate::{run_ping, Parser, PingDetectionError, PingResult, Pinger};
use anyhow::Context;
use regex::Regex;
use std::time::Duration;

#[derive(Debug, Eq, PartialEq)]
pub enum LinuxPingType {
    BusyBox,
    IPTools,
}

pub fn detect_linux_ping() -> Result<LinuxPingType, PingDetectionError> {
    let child = run_ping("ping", vec!["-V".to_string()])?;
    let output = child
        .wait_with_output()
        .context("Error getting ping stdout/stderr")?;
    let stdout = String::from_utf8(output.stdout).context("Error decoding ping stdout")?;
    let stderr = String::from_utf8(output.stderr).context("Error decoding ping stderr")?;

    if stderr.contains("BusyBox") {
        Ok(LinuxPingType::BusyBox)
    } else if stdout.contains("iputils") {
        Ok(LinuxPingType::IPTools)
    } else if stdout.contains("inetutils") {
        Err(PingDetectionError::NotSupported {
            alternative: "Please use iputils ping, not inetutils.".to_string(),
        })
    } else {
        let first_two_lines_stderr: Vec<String> =
            stderr.lines().take(2).map(str::to_string).collect();
        let first_two_lines_stout: Vec<String> =
            stdout.lines().take(2).map(str::to_string).collect();
        Err(PingDetectionError::UnknownPing {
            stdout: first_two_lines_stout,
            stderr: first_two_lines_stderr,
        })
    }
}

#[derive(Default)]
pub struct LinuxPinger {
    interval: Duration,
    interface: Option<String>,
}

impl Pinger for LinuxPinger {
    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }

    fn ping_args(&self, target: String) -> (&str, Vec<String>) {
        // The -O flag ensures we "no answer yet" messages from ping
        // See https://superuser.com/questions/270083/linux-ping-show-time-out
        let mut args = vec![
            "-O".to_string(),
            format!("-i{:.1}", self.interval.as_millis() as f32 / 1_000_f32),
        ];
        if let Some(interface) = &self.interface {
            args.push("-I".into());
            args.push(interface.clone());
        }
        args.push(target);
        ("ping", args)
    }
}

#[derive(Default)]
pub struct AlpinePinger {
    interval: Duration,
    interface: Option<String>,
}

// Alpine doesn't support timeout notifications, so we don't add the -O flag here
impl Pinger for AlpinePinger {
    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }
}

lazy_static! {
    static ref UBUNTU_RE: Regex =
        Regex::new(r"(?i-u)time=(?P<ms>\d+)(?:\.(?P<ns>\d+))? *ms").unwrap();
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

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_detection() {
        use super::*;
        use os_info::Type;
        let ping_type = detect_linux_ping().expect("Error getting ping");
        match os_info::get().os_type() {
            Type::Alpine => {
                assert_eq!(ping_type, LinuxPingType::BusyBox)
            }
            Type::Ubuntu => {
                assert_eq!(ping_type, LinuxPingType::IPTools)
            }
            _ => {}
        }
    }
}
