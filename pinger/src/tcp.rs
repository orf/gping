use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::{Instant};
use std::io::ErrorKind;


use crate::{PingOptions, PingResult, Pinger};

pub struct TcpPinger {
    options: PingOptions,
}

impl Pinger for TcpPinger {
    fn from_options(options: PingOptions) -> Result<Self, crate::PingCreationError> {
        Ok(TcpPinger { options })
    }

    fn parse_fn(&self) -> fn(String) -> Option<PingResult> {
        |_| None // TCP doesn't parse lines
    }

    fn ping_args(&self) -> (&str, Vec<String>) {
        ("tcp", vec![]) // unused
    }

    fn start(&self) -> Result<mpsc::Receiver<PingResult>, crate::PingCreationError> {
        let (tx, rx) = mpsc::channel();
        let options = self.options.clone();

        thread::spawn(move || {
            for _ in 0.. {
                let port = options.port.unwrap_or(80);
                let socket_str = format!("{}:{}", options.target, port);
                let addr = match socket_str.to_socket_addrs() {
                    Ok(mut addrs) => match addrs.next() {
                        Some(a) => a,
                        None => {
                            let _ = tx.send(PingResult::Unknown(
                                "Unable to resolve address".to_string()
                            ));
                            continue;
                        }
                    },
                    Err(e) => {
                        let _ = tx.send(PingResult::Unknown(format!("Resolve error: {}", e)));
                        continue;
                    }
                };

                let start = Instant::now();
                match TcpStream::connect_timeout(&addr, options.interval) {
                    Ok(_) => {
                        let _ = tx.send(PingResult::Pong(start.elapsed(), addr.to_string()));
                    }
                    Err(e) => {
                        //println!("DEBUG: error kind for {}: {:?}", addr, e.kind());
                        let is_rst = matches!(e.kind(), ErrorKind::ConnectionRefused);
                        if is_rst && options.allow_rst { // treat RST as pong default behavior
                            let _ = tx.send(PingResult::Pong(start.elapsed(), addr.to_string()));
                        } else {
                            let _ = tx.send(PingResult::Timeout(addr.to_string()));
                        }
                    }
                }

                thread::sleep(options.interval);
            }
        });

        Ok(rx)
    }
}

