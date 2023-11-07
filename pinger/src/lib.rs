#[cfg(unix)]
use crate::linux::{detect_linux_ping, LinuxPingType};
/// Pinger
/// This crate exposes a simple function to ping remote hosts across different operating systems.
/// Example:
/// ```no_run
/// use pinger::{ping, PingResult};
///
/// let stream = ping("tomforb.es".to_string(), None).expect("Error pinging");
/// for message in stream {
///     match message {
///         PingResult::Pong(duration, line) => println!("{:?} (line: {})", duration, line),
///         PingResult::Timeout(_) => println!("Timeout!"),
///         PingResult::Unknown(line) => println!("Unknown line: {}", line),
///         PingResult::PingExited(_code, _stderr) => {}
///     }
/// }
/// ```
use anyhow::{Context, Result};
use lazy_regex::Regex;
use std::fmt::Formatter;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use std::{fmt, thread};
use thiserror::Error;

pub mod linux;
// pub mod alpine'
pub mod macos;
#[cfg(windows)]
pub mod windows;

mod bsd;
mod fake;
#[cfg(test)]
mod test;

pub fn run_ping(cmd: &str, args: Vec<String>) -> Result<Child> {
    Command::new(cmd)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Required to ensure that the output is formatted in the way we expect, not
        // using locale specific delimiters.
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .spawn()
        .with_context(|| format!("Failed to run ping with args {:?}", &args))
}

pub trait Pinger {
    type Parser: Parser;

    fn new(interval: Duration, interface: Option<String>) -> Self;

    fn start(&self, target: String) -> Result<mpsc::Receiver<PingResult>> {
        let (tx, rx) = mpsc::channel();
        let (cmd, args) = self.ping_args(target);
        let mut child = run_ping(cmd, args)?;
        let stdout = child.stdout.take().context("child did not have a stdout")?;

        thread::spawn(move || {
            let parser = Self::Parser::default();
            let reader = BufReader::new(stdout).lines();
            for line in reader {
                match line {
                    Ok(msg) => {
                        if let Some(result) = parser.parse(msg) {
                            if tx.send(result).is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            let result = child.wait_with_output().expect("Child wasn't started?");
            let decoded_stderr = String::from_utf8(result.stderr).expect("Error decoding stderr");
            let _ = tx.send(PingResult::PingExited(result.status, decoded_stderr));
        });

        Ok(rx)
    }

    fn ping_args(&self, target: String) -> (&str, Vec<String>) {
        ("ping", vec![target])
    }
}

pub trait Parser: Default {
    fn parse(&self, line: String) -> Option<PingResult>;

    fn extract_regex(&self, regex: &Regex, line: String) -> Option<PingResult> {
        let cap = regex.captures(&line)?;
        let ms = cap
            .name("ms")
            .expect("No capture group named 'ms'")
            .as_str()
            .parse::<u64>()
            .ok()?;
        let ns = match cap.name("ns") {
            None => 0,
            Some(cap) => {
                let matched_str = cap.as_str();
                let number_of_digits = matched_str.len() as u32;
                let fractional_ms = matched_str.parse::<u64>().ok()?;
                fractional_ms * (10u64.pow(6 - number_of_digits))
            }
        };
        let duration = Duration::from_millis(ms) + Duration::from_nanos(ns);
        Some(PingResult::Pong(duration, line))
    }
}

#[derive(Debug)]
pub enum PingResult {
    Pong(Duration, String),
    Timeout(String),
    Unknown(String),
    PingExited(ExitStatus, String),
}

impl fmt::Display for PingResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            PingResult::Pong(duration, _) => write!(f, "{duration:?}"),
            PingResult::Timeout(_) => write!(f, "Timeout"),
            PingResult::Unknown(_) => write!(f, "Unknown"),
            PingResult::PingExited(status, stderr) => write!(f, "Exited({status}, {stderr})"),
        }
    }
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

    #[error("Installed ping is not supported: {alternative}")]
    NotSupported { alternative: String },
}

#[derive(Error, Debug)]
pub enum PingError {
    #[error("Could not detect ping command type")]
    UnsupportedPing(#[from] PingDetectionError),
    #[error("Invalid or unresolvable hostname {0}")]
    HostnameError(String),
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping(addr: String, interface: Option<String>) -> Result<mpsc::Receiver<PingResult>> {
    ping_with_interval(addr, Duration::from_millis(200), interface)
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping_with_interval(
    addr: String,
    interval: Duration,
    interface: Option<String>,
) -> Result<mpsc::Receiver<PingResult>> {
    if std::env::var("PINGER_FAKE_PING")
        .map(|e| e == "1")
        .unwrap_or(false)
    {
        let fake = fake::FakePinger::new(interval, interface);
        return fake.start(addr);
    }

    #[cfg(windows)]
    {
        let p = windows::WindowsPinger::new(interval, interface);
        return p.start(addr);
    }
    #[cfg(unix)]
    {
        if cfg!(target_os = "freebsd")
            || cfg!(target_os = "dragonfly")
            || cfg!(target_os = "openbsd")
            || cfg!(target_os = "netbsd")
        {
            let p = bsd::BSDPinger::new(interval, interface);
            p.start(addr)
        } else if cfg!(target_os = "macos") {
            let p = macos::MacOSPinger::new(interval, interface);
            p.start(addr)
        } else {
            match detect_linux_ping() {
                Ok(LinuxPingType::IPTools) => {
                    let p = linux::LinuxPinger::new(interval, interface);
                    p.start(addr)
                }
                Ok(LinuxPingType::BusyBox) => {
                    let p = linux::AlpinePinger::new(interval, interface);
                    p.start(addr)
                }
                Err(e) => Err(PingError::UnsupportedPing(e))?,
            }
        }
    }
}
