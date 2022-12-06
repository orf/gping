#[cfg(unix)]
use crate::linux::{detect_linux_ping, LinuxPingType, PingDetectionError};
/// Pinger
/// This crate exposes a simple function to ping remote hosts across different operating systems.
/// Example:
/// ```no_run
/// use pinger::{ping, PingResult};
///
/// let stream = ping("tomforb.es".to_string()).expect("Error pinging");
/// for message in stream {
///     match message {
///         PingResult::Pong(duration, line) => println!("{:?} (line: {})", duration, line),
///         PingResult::Timeout(_) => println!("Timeout!"),
///         PingResult::Unknown(line) => println!("Unknown line: {}", line),
///     }
/// }
/// ```
use anyhow::Result;
use regex::Regex;
use std::fmt::Formatter;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use std::{fmt, thread};
use thiserror::Error;

#[macro_use]
extern crate lazy_static;

pub mod linux;
// pub mod alpine'
pub mod macos;
#[cfg(windows)]
pub mod windows;

mod bsd;
#[cfg(test)]
mod test;

pub fn run_ping(args: Vec<String>, capture_stdout: bool) -> Child {
    Command::new("ping")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(if capture_stdout {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        // Required to ensure that the output is formatted in the way we expect, not
        // using locale specific delimiters.
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .spawn()
        .expect("Failed to run ping")
}

pub trait Pinger: Default {
    fn start<P>(&self, target: String) -> Result<mpsc::Receiver<PingResult>>
    where
        P: Parser,
    {
        let (tx, rx) = mpsc::channel();
        let args = self.ping_args(target);

        thread::spawn(move || {
            let mut child = run_ping(args, false);
            let parser = P::default();
            let stdout = child.stdout.take().expect("child did not have a stdout");
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
        });

        Ok(rx)
    }

    fn set_interval(&mut self, interval: Duration);

    fn ping_args(&self, target: String) -> Vec<String> {
        vec![target]
    }
}

// Default empty implementation of a pinger.
#[derive(Default)]
pub struct SimplePinger {}

impl Pinger for SimplePinger {
    fn set_interval(&mut self, _interval: Duration) {}
}

pub trait Parser: Default {
    fn parse(&self, line: String) -> Option<PingResult>;

    fn extract_regex(&self, regex: &Regex, line: String) -> Option<PingResult> {
        let cap = regex.captures(&line)?;
        let time = cap
            .name("time")
            .expect("No capture group named 'time'")
            .as_str()
            .parse::<f32>()
            .expect("time cannot be parsed as f32");
        let duration = Duration::from_micros((time * 1000f32) as u64);
        Some(PingResult::Pong(duration, line))
    }
}

#[derive(Debug)]
pub enum PingResult {
    Pong(Duration, String),
    Timeout(String),
    Unknown(String),
}

impl fmt::Display for PingResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            PingResult::Pong(duration, _) => write!(f, "{:?}", duration),
            PingResult::Timeout(_) => write!(f, "Timeout"),
            PingResult::Unknown(_) => write!(f, "Unknown"),
        }
    }
}

#[derive(Error, Debug)]
pub enum PingError {
    #[error("Could not detect ping command type")]
    UnsupportedPing(#[from] PingDetectionError),
    #[error("Invalid or unresolvable hostname {0}")]
    HostnameError(String),
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping(addr: String) -> Result<mpsc::Receiver<PingResult>> {
    ping_with_interval(addr, Duration::from_millis(200))
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping_with_interval(addr: String, interval: Duration) -> Result<mpsc::Receiver<PingResult>> {
    #[cfg(windows)]
    {
        let mut p = windows::WindowsPinger::default();
        p.set_interval(interval);
        return p.start::<windows::WindowsParser>(addr);
    }
    #[cfg(unix)]
    {
        if cfg!(target_os = "freebsd")
            || cfg!(target_os = "dragonfly")
            || cfg!(target_os = "openbsd")
            || cfg!(target_os = "netbsd")
        {
            let mut p = bsd::BSDPinger::default();
            p.set_interval(interval);
            p.start::<bsd::BSDParser>(addr)
        } else if cfg!(target_os = "macos") {
            let mut p = macos::MacOSPinger::default();
            p.set_interval(interval);
            p.start::<macos::MacOSParser>(addr)
        } else {
            match detect_linux_ping() {
                Ok(LinuxPingType::IPTools) => {
                    let mut p = linux::LinuxPinger::default();
                    p.set_interval(interval);
                    p.start::<linux::LinuxParser>(addr)
                }
                Ok(LinuxPingType::BusyBox) => {
                    let mut p = linux::AlpinePinger::default();
                    p.set_interval(interval);
                    p.start::<linux::LinuxParser>(addr)
                }
                Err(e) => Err(PingError::UnsupportedPing(e))?,
            }
        }
    }
}
