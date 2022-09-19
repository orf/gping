use crate::{run_ping, Parser, PingResult, Pinger};
use anyhow::Context;
use regex::Regex;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Eq, PartialEq)]
pub enum LinuxPingType {
    BusyBox,
    IPTools,
}

#[derive(Error, Debug)]
pub enum PingDetectionError {
    #[error("Could not detect ping. Stderr: {stderr:?}\nStdout: {stdout:?}")]
    UnknownPing {
        stderr: Vec<String>,
        stdout: Vec<String>,
    },
    #[error(transparent)]
    CommandError(#[from] anyhow::Error),
}

pub fn detect_linux_ping() -> Result<LinuxPingType, PingDetectionError> {
    // Err(PingDetectionError::Thing)
    let child = run_ping(vec!["-V".to_string()], true);
    let output = child
        .wait_with_output()
        .context("Error getting ping stdout/stderr")?;
    let stdout = String::from_utf8(output.stdout).context("Error decoding ping stdout")?;
    let stderr = String::from_utf8(output.stderr).context("Error decoding ping stderr")?;

    if stderr.contains("BusyBox") {
        Ok(LinuxPingType::BusyBox)
    } else if stdout.contains("iputils") {
        Ok(LinuxPingType::IPTools)
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
}

impl Pinger for LinuxPinger {
    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    fn ping_args(&self, target: String) -> Vec<String> {
        // The -O flag ensures we "no answer yet" messages from ping
        // See https://superuser.com/questions/270083/linux-ping-show-time-out
        vec![
            "-O".to_string(),
            format!("-i{:.1}", self.interval.as_millis() as f32 / 1_000_f32),
            target,
        ]
    }
}

#[derive(Default)]
pub struct AlpinePinger {
    interval: Duration,
}

// Alpine doesn't support timeout notifications, so we don't add the -O flag here
impl Pinger for AlpinePinger {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_detection() {
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
