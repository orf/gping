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
use os_info::Type;
use regex::Regex;
use std::fmt::Formatter;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
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

#[cfg(test)]
mod test;

pub trait Pinger: Default {
    fn start<P>(&self, target: String) -> Result<mpsc::Receiver<PingResult>>
    where
        P: Parser,
    {
        let (tx, rx) = mpsc::channel();
        let args = self.ping_args(target);

        thread::spawn(move || {
            let mut child = Command::new("ping")
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                // Required to ensure that the output is formatted in the way we expect, not
                // using locale specific delimiters.
                .env("LANG", "C")
                .env("LC_ALL", "C")
                .spawn()
                .expect("Failed to run ping");
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
    #[error("Unsupported OS {0}")]
    UnsupportedOS(String),
    #[error("Invalid or unresolvable hostname {0}")]
    HostnameError(String),
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping(addr: String) -> Result<mpsc::Receiver<PingResult>> {
    ping_with_interval(addr, Duration::from_millis(200))
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping_with_interval(addr: String, interval: Duration) -> Result<mpsc::Receiver<PingResult>> {
    let os_type = os_info::get().os_type();
    match os_type {
        #[cfg(windows)]
        Type::Windows => {
            let mut p = windows::WindowsPinger::default();
            p.set_interval(interval);
            p.start::<windows::WindowsParser>(addr)
        }
        Type::Amazon
        | Type::Arch
        | Type::CentOS
        | Type::Debian
        | Type::EndeavourOS
        | Type::Fedora
        | Type::Linux
        | Type::NixOS
        | Type::Manjaro
        | Type::Mint
        | Type::openSUSE
        | Type::OracleLinux
        | Type::Redhat
        | Type::RedHatEnterprise
        | Type::SUSE
        | Type::Ubuntu
        | Type::Pop
        | Type::Solus
        | Type::Raspbian
        | Type::Android => {
            let mut p = linux::LinuxPinger::default();
            p.set_interval(interval);
            p.start::<linux::LinuxParser>(addr)
        }
        Type::Alpine => {
            let mut p = linux::AlpinePinger::default();
            p.set_interval(interval);
            p.start::<linux::LinuxParser>(addr)
        }
        Type::Macos => {
            let mut p = macos::MacOSPinger::default();
            p.set_interval(interval);
            p.start::<macos::MacOSParser>(addr)
        }
        _ => Err(PingError::UnsupportedOS(os_type.to_string()).into()),
    }
}
