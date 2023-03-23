use crate::{
    icmp::{EchoReply, EchoRequest, IcmpV4, IcmpV6, ICMP_HEADER_SIZE},
    ipv4::IpV4Packet,
};
use caps::{CapSet, Capability};
use dns_lookup::lookup_host;
use rand::random;
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::HashMap,
    error::Error,
    io::Read,
    mem,
    net::{IpAddr, SocketAddr},
    os::fd::{AsRawFd, RawFd},
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const TOKEN_SIZE: usize = 24;
const ECHO_REQUEST_BUFFER_SIZE: usize = ICMP_HEADER_SIZE + TOKEN_SIZE;
type Token = [u8; TOKEN_SIZE];

const IN_FLIGHT_TO_RETAIN: usize = 10;

fn create_pipe() -> (RawFd, RawFd) {
    let mut fds = [0, 0];
    unsafe {
        libc::pipe(fds.as_mut_ptr());
    }

    (fds[0], fds[1])
}

pub struct Pinger {
    pub channel: mpsc::Receiver<Duration>,
    run: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    send_pipe_fd: RawFd,
}

impl Drop for Pinger {
    fn drop(&mut self) {
        self.run.store(false, Ordering::SeqCst);

        if self.send_pipe_fd >= 0 {
            unsafe {
                libc::write(
                    self.send_pipe_fd,
                    "wakeup".as_ptr() as *const libc::c_void,
                    6,
                );
            }
        }

        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
        unsafe {
            if self.send_pipe_fd >= 0 {
                libc::close(self.send_pipe_fd);
                self.send_pipe_fd = -1
            }
        }
    }
}

impl Pinger {
    pub fn new(
        addr: String,
        interval: Duration,
        interface: Option<String>,
    ) -> Result<Pinger, Box<dyn Error>> {
        let run = Arc::new(AtomicBool::new(true));

        let addr = match addr.parse::<IpAddr>() {
            Err(_) => {
                let ips = lookup_host(&addr)?;
                if ips.is_empty() {
                    Err(format!("Unknown host: {addr}"))
                } else {
                    Ok(ips[0])
                }
            }
            Ok(addr) => Ok(addr),
        }?;

        let (read_fd, write_fd) = create_pipe();

        let (tx, rx) = mpsc::sync_channel(16);

        let thread = thread::spawn({
            let run = run.clone();
            move || {
                raise_cap_net_raw_to_effective();

                let mut socket = match if addr.is_ipv4() {
                    Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))
                } else {
                    Socket::new(Domain::IPV6, Type::RAW, Some(Protocol::ICMPV6))
                } {
                    Ok(s) => s,
                    Err(err) => {
                        log::error!("Failed create raw socket: {}", err);
                        return;
                    }
                };

                if addr.is_ipv4() {
                    socket.set_ttl(64).ok();
                } else {
                    socket.set_unicast_hops_v6(64).ok();
                }

                socket.set_nonblocking(true).ok();

                if let Some(interface) = interface {
                    socket.bind_device(Some(interface.as_bytes())).ok();
                }

                let mut in_flight_table = HashMap::new();

                let mut read_fds = unsafe { mem::zeroed() };

                let mut last_send = Instant::now()
                    .checked_sub(interval)
                    .unwrap_or_else(Instant::now);

                while run.load(Ordering::SeqCst) {
                    unsafe {
                        let socket_fd = socket.as_raw_fd();

                        libc::FD_ZERO(&mut read_fds);
                        libc::FD_SET(read_fd, &mut read_fds);
                        libc::FD_SET(socket_fd, &mut read_fds);

                        let next = last_send + interval;

                        if let Some(wait) = next.checked_duration_since(Instant::now()) {
                            let mut timeout = libc::timeval {
                                tv_sec: wait.as_secs() as _,
                                tv_usec: wait.subsec_micros() as _,
                            };

                            let max_fd = read_fd.max(socket_fd);

                            libc::select(
                                max_fd + 1,
                                &mut read_fds,
                                null_mut(),
                                null_mut(),
                                &mut timeout,
                            );
                        }
                    }

                    if last_send.elapsed() > interval {
                        last_send = Instant::now();

                        let mut buffer = [0; ECHO_REQUEST_BUFFER_SIZE];

                        let payload: Token = random();

                        let request = EchoRequest {
                            ident: random(),
                            seq_cnt: 1,
                            payload: &payload,
                        };

                        if addr.is_ipv4() {
                            request.encode::<IcmpV4>(&mut buffer);
                        } else {
                            request.encode::<IcmpV6>(&mut buffer);
                        };

                        let dest = SocketAddr::new(addr, 0);

                        socket.send_to(&buffer, &dest.into()).ok();
                        let send_time = Instant::now();

                        {
                            in_flight_table.insert(payload, send_time);

                            while in_flight_table.len() > IN_FLIGHT_TO_RETAIN {
                                let oldest = in_flight_table
                                    .iter()
                                    .max_by(|(_, at), (_, bt)| at.elapsed().cmp(&bt.elapsed()))
                                    .map(|(k, _)| k.to_owned())
                                    .unwrap();

                                in_flight_table.remove(&oldest);
                            }
                        }
                    }

                    {
                        let mut buffer: [u8; 2048] = [0; 2048];
                        if let Ok(amt) = socket.read(&mut buffer) {
                            if addr.is_ipv4() {
                                if let Ok(packet) = IpV4Packet::decode(&buffer[..amt]) {
                                    if let Ok(reply) = EchoReply::decode::<IcmpV4>(packet.data) {
                                        if let Ok(token) = Token::try_from(reply.payload) {
                                            if let Some(send_time) = in_flight_table.remove(&token)
                                            {
                                                tx.try_send(send_time.elapsed()).ok();
                                            }
                                        }
                                    }
                                };
                            } else if let Ok(reply) = EchoReply::decode::<IcmpV6>(&buffer[..amt]) {
                                if let Ok(token) = Token::try_from(reply.payload) {
                                    if let Some(send_time) = in_flight_table.remove(&token) {
                                        tx.try_send(send_time.elapsed()).ok();
                                    }
                                }
                            }
                        }
                    }
                }

                unsafe {
                    libc::close(read_fd);
                }

                drop_cap_net_raw_caps_from_effective();
            }
        });

        Ok(Pinger {
            channel: rx,
            run,
            thread: Some(thread),
            send_pipe_fd: write_fd,
        })
    }
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping_with_interval(
    addr: impl Into<String>,
    interval: Duration,
    interface: Option<impl Into<String>>,
) -> Result<Pinger, Box<dyn Error>> {
    Pinger::new(addr.into(), interval, interface.map(|s| s.into()))
}

pub fn raise_cap_net_raw_to_effective() {
    if let Err(err) = caps::raise(None, CapSet::Effective, Capability::CAP_NET_RAW) {
        log::error!("Failed to raise CAP_NET_RAW to effective: {}", err);
    }
}

pub fn drop_cap_net_raw_caps_from_effective() {
    if let Err(err) = caps::drop(None, CapSet::Effective, Capability::CAP_NET_RAW) {
        log::error!("Failed to drop CAP_NET_RAW from effective: {}", err);
    }
}
