use crate::target::{IPVersion, Target};
use crate::PingCreationError;
use crate::{extract_regex, PingOptions, PingResult, Pinger};
use lazy_regex::*;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use winping::{Buffer, Pinger as WinPinger};

pub static RE: Lazy<Regex> = lazy_regex!(r"(?ix-u)time=(?P<ms>\d+)(?:\.(?P<ns>\d+))?");

pub struct WindowsPinger {
    options: PingOptions,
}

impl Pinger for WindowsPinger {
    fn from_options(options: PingOptions) -> Result<Self, PingCreationError> {
        Ok(Self { options })
    }

    fn parse_fn(&self) -> fn(String) -> Option<PingResult> {
        |line| {
            if line.contains("timed out") || line.contains("failure") {
                return Some(PingResult::Timeout(line));
            }
            extract_regex(&RE, line)
        }
    }

    fn ping_args(&self) -> (&str, Vec<String>) {
        unimplemented!("ping_args for WindowsPinger is not implemented")
    }

    fn start(&self) -> Result<mpsc::Receiver<PingResult>, PingCreationError> {
        let interval = self.options.interval;
        let parsed_ip = match &self.options.target {
            Target::IP(ip) => ip.clone(),
            Target::Hostname { domain, version } => {
                let ips = (domain.as_str(), 0).to_socket_addrs()?;
                let selected_ips: Vec<_> = if *version == IPVersion::Any {
                    ips.collect()
                } else {
                    ips.into_iter()
                        .filter(|addr| {
                            if *version == IPVersion::V6 {
                                matches!(addr.ip(), IpAddr::V6(_))
                            } else {
                                matches!(addr.ip(), IpAddr::V4(_))
                            }
                        })
                        .collect()
                };
                if selected_ips.is_empty() {
                    return Err(PingCreationError::HostnameError(domain.clone()).into());
                }
                selected_ips[0].ip()
            }
        };

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let pinger = WinPinger::new().expect("Failed to create a WinPinger instance");
            let mut buffer = Buffer::new();
            loop {
                match pinger.send(parsed_ip.clone(), &mut buffer) {
                    Ok(rtt) => {
                        if tx
                            .send(PingResult::Pong(
                                Duration::from_millis(rtt as u64),
                                "".to_string(),
                            ))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => {
                        // Fuck it. All errors are timeouts. Why not.
                        if tx.send(PingResult::Timeout("".to_string())).is_err() {
                            break;
                        }
                    }
                }
                thread::sleep(interval);
            }
        });

        Ok(rx)
    }
}
