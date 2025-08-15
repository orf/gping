/// Pinger
/// This crate exposes a simple function to ping remote hosts across different operating systems.
/// Example:
/// ```no_run
/// use std::time::Duration;
/// use pinger::{ping, PingResult, PingOptions};
/// let options = PingOptions::new("tomforb.es".to_string(), Duration::from_secs(1), None);
/// let stream = ping(options).expect("Error pinging");
/// for message in stream {
///     match message {
///         PingResult::Pong(duration, line) => println!("{:?} (line: {})", duration, line),
///         PingResult::Timeout(_) => println!("Timeout!"),
///         PingResult::Unknown(line) => println!("Unknown line: {}", line),
///         PingResult::PingExited(_code, _stderr) => {}
///     }
/// }
/// ```
use lazy_regex::Regex;
use std::ffi::OsStr;
use std::fmt::{Debug, Formatter};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use std::{fmt, io, thread};
use target::Target;
use thiserror::Error;

#[cfg(unix)]
pub mod linux;
#[cfg(unix)]
pub mod macos;
#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
mod bsd;
#[cfg(feature = "fake-ping")]
mod fake;
mod target;
#[cfg(test)]
mod test;

#[derive(Debug, Clone)]
pub struct PingOptions {
    pub target: Target,
    pub interval: Duration,
    pub interface: Option<String>,
    pub raw_arguments: Option<Vec<String>>,
}

impl PingOptions {
    pub fn with_raw_arguments(mut self, raw_arguments: Vec<impl ToString>) -> Self {
        self.raw_arguments = Some(
            raw_arguments
                .into_iter()
                .map(|item| item.to_string())
                .collect(),
        );
        self
    }
}

impl PingOptions {
    pub fn from_target(target: Target, interval: Duration, interface: Option<String>) -> Self {
        Self {
            target,
            interval,
            interface,
            raw_arguments: None,
        }
    }
    pub fn new(target: impl ToString, interval: Duration, interface: Option<String>) -> Self {
        Self::from_target(Target::new_any(target), interval, interface)
    }

    pub fn new_ipv4(target: impl ToString, interval: Duration, interface: Option<String>) -> Self {
        Self::from_target(Target::new_ipv4(target), interval, interface)
    }

    pub fn new_ipv6(target: impl ToString, interval: Duration, interface: Option<String>) -> Self {
        Self::from_target(Target::new_ipv6(target), interval, interface)
    }
}

pub fn run_ping(
    cmd: impl AsRef<OsStr> + Debug,
    args: Vec<impl AsRef<OsStr> + Debug>,
) -> Result<Child, PingCreationError> {
    Ok(Command::new(cmd.as_ref())
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Required to ensure that the output is formatted in the way we expect, not
        // using locale specific delimiters.
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .spawn()?)
}

pub(crate) fn extract_regex(regex: &Regex, line: String) -> Option<PingResult> {
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

pub trait Pinger: Send + Sync {
    fn from_options(options: PingOptions) -> std::result::Result<Self, PingCreationError>
    where
        Self: Sized;

    fn parse_fn(&self) -> fn(String) -> Option<PingResult>;

    fn ping_args(&self) -> (&str, Vec<String>);

    fn start(&self) -> Result<mpsc::Receiver<PingResult>, PingCreationError> {
        let (tx, rx) = mpsc::channel();
        let (cmd, args) = self.ping_args();

        let mut child = run_ping(cmd, args)?;
        let stdout = child.stdout.take().expect("child did not have a stdout");

        let parse_fn = self.parse_fn();

        thread::spawn(move || {
            let reader = BufReader::new(stdout).lines();
            for line in reader {
                match line {
                    Ok(msg) => {
                        if let Some(result) = parse_fn(msg) {
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
pub enum PingCreationError {
    #[error("Could not detect ping. Stderr: {stderr:?}\nStdout: {stdout:?}")]
    UnknownPing {
        stderr: Vec<String>,
        stdout: Vec<String>,
    },
    #[error("Error spawning ping: {0}")]
    SpawnError(#[from] io::Error),

    #[error("Installed ping is not supported: {alternative}")]
    NotSupported { alternative: String },

    #[error("Invalid or unresolvable hostname {0}")]
    HostnameError(String),
}

pub fn get_pinger(options: PingOptions) -> std::result::Result<Arc<dyn Pinger>, PingCreationError> {
    #[cfg(feature = "fake-ping")]
    if std::env::var("PINGER_FAKE_PING")
        .map(|e| e == "1")
        .unwrap_or_default()
    {
        return Ok(Arc::new(fake::FakePinger::from_options(options)?));
    }

    #[cfg(windows)]
    {
        return Ok(Arc::new(windows::WindowsPinger::from_options(options)?));
    }
    #[cfg(unix)]
    {
        if cfg!(target_os = "freebsd")
            || cfg!(target_os = "dragonfly")
            || cfg!(target_os = "openbsd")
            || cfg!(target_os = "netbsd")
        {
            Ok(Arc::new(bsd::BSDPinger::from_options(options)?))
        } else if cfg!(target_os = "macos") {
            Ok(Arc::new(macos::MacOSPinger::from_options(options)?))
        } else {
            Ok(Arc::new(linux::LinuxPinger::from_options(options)?))
        }
    }
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping(
    options: PingOptions,
) -> std::result::Result<mpsc::Receiver<PingResult>, PingCreationError> {
    let pinger = get_pinger(options)?;
    pinger.start()
}
