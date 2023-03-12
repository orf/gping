#[cfg(unix)]
use crate::linux::{detect_linux_ping};
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
///         PingResult::Failed(_,_) => println!("Failed!"),
///     }
/// }
/// ```
use anyhow::Result;
use regex::Regex;
use std::fmt::Formatter;
use std::sync::mpsc;
use std::time::Duration;
use std::fmt;
use thiserror::Error;
use std::thread::JoinHandle;
use tokio::sync::oneshot;

#[macro_use]
extern crate lazy_static;
extern crate core;

pub mod linux;

#[cfg(windows)]
pub mod windows;


#[cfg(test)]
mod test;

pub struct PhantomPinger {
    pub channel: mpsc::Receiver<PingResult>,
    ping_thread: Option<(oneshot::Sender<()>, JoinHandle<()>)>,
}

impl Drop for PhantomPinger {
    fn drop(&mut self) {
        if let Some((notify_exit_sender, thread)) = self.ping_thread.take() {
            notify_exit_sender.send(()).unwrap();
            thread.join().unwrap();
        }
    }
}


pub trait Pinger: Default {
    fn start<P: Parser>(&self, target: String) -> Result<PhantomPinger>;


    fn set_interval(&mut self, interval: Duration);

    fn set_interface(&mut self, interface: Option<String>);

    fn ping_args(&self, target: String) -> (String, Vec<String>) {
        ("ping".to_string(), vec![target])
    }
}

pub trait Parser: Default + Send {
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
    Failed(String, String),
}

impl fmt::Display for PingResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            PingResult::Pong(duration, _) => write!(f, "{duration:?}"),
            PingResult::Failed(status, stderr) => write!(f, "Exited({status}, {stderr})"),
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
pub fn ping(addr: String, interface: Option<String>) -> Result<PhantomPinger> {
    ping_with_interval(addr, Duration::from_millis(200), interface)
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping_with_interval(
    addr: String,
    interval: Duration,
    interface: Option<String>,
) -> Result<PhantomPinger> {
    #[cfg(windows)]
    {
        let mut p = windows::WindowsPinger::default();
        p.set_interval(interval);
        p.set_interface(interface);
        p.start::<windows::WindowsParser>(addr);
    }
    #[cfg(unix)]
    {
        match detect_linux_ping() {
            Ok(_) => {
                let mut p = linux::LinuxPinger::default();
                p.set_interval(interval);
                p.set_interface(interface);
                p.start::<linux::LinuxParser>(addr)
            }
            Err(e) => Err(PingError::UnsupportedPing(e))?,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    #[test]
    fn test() {
        use super::*;
        let ping_channel = ping_with_interval(
            "8.8.8.9".to_string(),
            Duration::from_millis(200),
            None,
        ).unwrap();
        let mut counter = 0;
        loop {
            if let Ok(result) = ping_channel.channel.try_recv() {
                match result {
                    PingResult::Pong(duration, _) => println!("{:?}", duration.as_millis()),
                    PingResult::Failed(exit_status, err) => println!("{} - {}", exit_status, err)
                }
            }
            counter+=1;
            if counter == 10 {
                break;
            }
            sleep(Duration::from_millis(200));
        }
    }
}
