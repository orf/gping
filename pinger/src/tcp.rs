use std::io;
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use crate::{PingCreationError, PingMode, PingOptions, PingResult, Pinger, RstBehaviour};

pub struct TcpPinger {
    options: PingOptions,
    rst: RstBehaviour,
    port: u16,
}

impl TcpPinger {
    /// Resolve the target once, up front, so that a bad hostname surfaces as an error
    /// from `start` rather than as an endless stream of failed pings.
    fn resolve(&self) -> Result<SocketAddr, PingCreationError> {
        let hostname = self.options.target.to_string();
        let socket_str = format!("{hostname}:{}", self.port);
        socket_str
            .to_socket_addrs()
            .map_err(|err| PingCreationError::HostnameError {
                hostname: hostname.clone(),
                err,
            })?
            .next()
            .ok_or_else(|| PingCreationError::HostnameError {
                hostname,
                err: io::Error::other("name resolved to no addresses"),
            })
    }
}

impl Pinger for TcpPinger {
    fn from_options(options: PingOptions) -> Result<Self, PingCreationError> {
        match options.mode {
            PingMode::TCP { rst, port } => Ok(TcpPinger {
                rst,
                port: port.unwrap_or(80),
                options,
            }),
            PingMode::ICMP => Err(PingCreationError::InternalError(
                "ICMP ping options passed to TcpPinger".to_string(),
            )),
        }
    }

    fn parse_fn(&self) -> fn(String) -> Option<PingResult> {
        |_| None // TCP doesn't parse lines
    }

    fn ping_args(&self) -> (&str, Vec<String>) {
        ("tcp", vec![]) // unused
    }

    fn start(&self) -> Result<mpsc::Receiver<PingResult>, PingCreationError> {
        let (tx, rx) = mpsc::channel();
        let addr = self.resolve()?;
        let interval = self.options.interval;
        let rst = self.rst;

        thread::spawn(move || {
            loop {
                let start = Instant::now();
                let sent = match TcpStream::connect_timeout(&addr, interval) {
                    Ok(_) => tx.send(PingResult::Pong(start.elapsed(), addr.to_string())),
                    // A RST means the host is up, it just isn't listening on this port
                    Err(e)
                        if rst == RstBehaviour::Pong
                            && e.kind() == ErrorKind::ConnectionRefused =>
                    {
                        tx.send(PingResult::Pong(start.elapsed(), addr.to_string()))
                    }
                    Err(_) => tx.send(PingResult::Timeout(addr.to_string())),
                };

                // The receiver has hung up, so nothing will read any further pings.
                if sent.is_err() {
                    break;
                }

                thread::sleep(interval);
            }
        });

        Ok(rx)
    }
}
