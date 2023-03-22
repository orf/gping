use crate::{
    icmp::{EchoReply, EchoRequest, IcmpV4, IcmpV6, ICMP_HEADER_SIZE},
    ipv4::IpV4Packet,
    Pinger,
};
use rand::random;
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::HashMap,
    error::Error,
    io::Read,
    net::ToSocketAddrs,
    sync::{mpsc, Arc, Mutex},
    thread::{self},
    time::{Duration, Instant},
};
use tokio::{sync::oneshot, time};

const TOKEN_SIZE: usize = 24;
const ECHO_REQUEST_BUFFER_SIZE: usize = ICMP_HEADER_SIZE + TOKEN_SIZE;
type Token = [u8; TOKEN_SIZE];

const IN_FLIGHT_TO_RETAIN: usize = 10;

#[derive(Default)]
pub struct LinuxPinger {
    interval: Duration,
    interface: Option<String>,
}

impl LinuxPinger {
    pub fn start(&self, target: String) -> Result<Pinger, Box<dyn Error>> {
        let interval = self.interval;

        let dest = target
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| format!("Invalid adder: {target}"))?;

        let socket = if dest.is_ipv4() {
            Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))?
        } else {
            Socket::new(Domain::IPV6, Type::RAW, Some(Protocol::ICMPV6))?
        };

        if dest.is_ipv4() {
            socket.set_ttl(64)?;
        } else {
            socket.set_unicast_hops_v6(64)?;
        }

        socket.set_write_timeout(Some(Duration::from_millis(100)))?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        let mut read_socket = socket.try_clone()?;
        let write_socket = socket.try_clone()?;

        let (tx, rx) = mpsc::channel();

        let in_flight_table = Arc::new(Mutex::new(HashMap::new()));

        let (notify_exit_sender, exit_receiver) = oneshot::channel();
        let ping_thread = Some((
            notify_exit_sender,
            thread::spawn({
                move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();

                    runtime.spawn({
                        let in_flight_table = in_flight_table.clone();

                        async move {
                            loop {
                                let mut buffer = [0; ECHO_REQUEST_BUFFER_SIZE];

                                let payload: Token = random();

                                let request = EchoRequest {
                                    ident: random(),
                                    seq_cnt: 1,
                                    payload: &payload,
                                };

                                if dest.is_ipv4() {
                                    request.encode::<IcmpV4>(&mut buffer);
                                } else {
                                    request.encode::<IcmpV6>(&mut buffer);
                                };

                                write_socket.send_to(&buffer, &dest.into()).ok();
                                let send_time = Instant::now();

                                {
                                    let mut lock = in_flight_table.lock().unwrap();
                                    lock.insert(payload, send_time);

                                    while lock.len() > IN_FLIGHT_TO_RETAIN {
                                        let oldest = lock
                                            .iter()
                                            .max_by(|(_, at), (_, bt)| {
                                                at.elapsed().cmp(&bt.elapsed())
                                            })
                                            .map(|(k, _)| k.to_owned())
                                            .unwrap();

                                        lock.remove(&oldest);
                                    }
                                }

                                time::sleep(interval).await;
                            }
                        }
                    });

                    runtime.spawn({
                        let in_flight_table = in_flight_table.clone();

                        let handle_reply = move |reply: EchoReply| {
                            let mut lock = in_flight_table.lock().unwrap();

                            if let Ok(token) = Token::try_from(reply.payload) {
                                if let Some(send_time) = lock.remove(&token) {
                                    tx.send(Ok(send_time.elapsed())).ok();
                                }
                            }
                        };

                        async move {
                            loop {
                                let mut buffer: [u8; 2048] = [0; 2048];
                                if let Ok(amt) = read_socket.read(&mut buffer) {
                                    if dest.is_ipv4() {
                                        if let Ok(packet) = IpV4Packet::decode(&buffer[..amt]) {
                                            if let Ok(reply) =
                                                EchoReply::decode::<IcmpV4>(packet.data)
                                            {
                                                handle_reply(reply);
                                            }
                                        };
                                    } else if let Ok(reply) =
                                        EchoReply::decode::<IcmpV6>(&buffer[..amt])
                                    {
                                        handle_reply(reply);
                                    }
                                }
                            }
                        }
                    });

                    runtime.block_on(async move {
                        let _ = exit_receiver.await;
                    });
                }
            }),
        ));
        Ok(Pinger {
            channel: rx,
            ping_thread,
        })
    }

    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    pub fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }
}
