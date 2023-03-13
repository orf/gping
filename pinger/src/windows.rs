use crate::{Parser, PingError, PingResult, Pinger, Pinger};
use anyhow::Result;
use dns_lookup::lookup_host;
use regex::Regex;
use std::{time::Duration, thread, sync::mpsc, net::IpAddr};
use winping::{Buffer, AsyncPinger as WinPinger};
use tokio::{time, sync::oneshot};


lazy_static! {
    static ref RE: Regex = Regex::new(r"(?ix-u)time=(?P<ms>\d+)(?:\.(?P<ns>\d+))?").unwrap();
}

#[derive(Default)]
pub struct WindowsPinger {
    interval: Duration,
    interface: Option<String>,
}

impl PingerTrait for WindowsPinger {
    fn start<P>(&self, target: String) -> Result<Pinger>
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
        let (notify_exit_sender, exit_receiver) = oneshot::channel();
        let ping_thread = Some((notify_exit_sender,
                                thread::spawn({
                                    move || {
                                        let runtime = tokio::runtime::Builder::new_current_thread()
                                            .enable_all()
                                            .build()
                                            .unwrap();

                                        runtime.spawn(async move {
                                            let pinger = WinPinger::new();
                                            loop {
                                                let buffer = Buffer::new();
                                                match pinger.send(parsed_ip, buffer).await.result {
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
                                                    Err(e) => {
                                                        // Fuck it. All errors are timeouts. Why not.
                                                        tx.send(PingResult::Failed("-1".to_string(), e.to_string())).ok();
                                                    }
                                                }
                                                time::sleep(interval).await;
                                            }
                                        });

                                        runtime.block_on(async move {
                                            let _ = exit_receiver.await;
                                        });
                                    }
                                })
        ));
        Ok(Pinger {
            channel: rx,
            ping_thread,
        })
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
            return Some(PingResult::Failed("1".to_string(), line));
        }
        self.extract_regex(&RE, line)
    }
}
