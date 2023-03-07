use crate::{Parser, PingError, PingResult, Pinger};
use anyhow::Result;
use dns_lookup::lookup_host;
use regex::Regex;
use std::net::IpAddr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use winping::{Buffer, Pinger as WinPinger};

lazy_static! {
    static ref RE: Regex = Regex::new(r"(?ix-u)time=(?P<ms>\d+)(?:\.(?P<ns>\d+))?").unwrap();
}

#[derive(Default)]
pub struct WindowsPinger {
    interval: Duration,
    interface: Option<String>,
}

impl Pinger for WindowsPinger {
    fn start<P>(&self, target: String) -> Result<mpsc::Receiver<PingResult>>
    where
        P: Parser,
    {
        let interval = self.interval;
        let parsed_ip: IpAddr = match target.parse() {
            Err(_) => {
                let things = lookup_host(target.as_str())?;
                if things.is_empty() {
                    Err(PingError::HostnameError(target))
                } else {
                    Ok(things[0])
                }
            }
            Ok(addr) => Ok(addr),
        }?;

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
                        if tx.send(PingResult::Failed(1, "".to_string())).is_err() {
                            break;
                        }
                    }
                }
                thread::sleep(interval);
            }
        });

        Ok(rx)
    }

    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }
}

#[derive(Default)]
pub struct WindowsParser {}

impl Parser for WindowsParser {
    fn parse(&self, line: String) -> Option<PingResult> {
        if line.contains("timed out") || line.contains("failure") {
            return Some(PingResult::Failed(1,line));
        }
        self.extract_regex(&RE, line)
    }
}
