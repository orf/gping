use crate::Pinger;
use dns_lookup::lookup_host;
use std::{net::IpAddr, sync::mpsc, thread, time::Duration};
use tokio::{sync::oneshot, time};
use winping::{AsyncPinger as WinPinger, Buffer};

pub struct Pinger {
    pub channel: mpsc::Receiver<Duration>,
    ping_thread: Option<(oneshot::Sender<()>, JoinHandle<()>)>,
}

impl Drop for Pinger {
    fn drop(&mut self) {
        if let Some((notify_exit_sender, thread)) = self.ping_thread.take() {
            notify_exit_sender.send(()).unwrap();
            thread.join().unwrap();
        }
    }
}

impl Pinger {
    pub fn new(
        &self,
        addr: String,
        interval: Duration,
        interface: Option<String>,
    ) -> Result<Pinger> {
        let parsed_ip: IpAddr = match target.parse() {
            Err(_) => {
                let things = lookup_host(target.as_str())?;
                if things.is_empty() {
                    Err(format!("Unknown host: {target}"))
                } else {
                    Ok(things[0])
                }
            }
            Ok(addr) => Ok(addr),
        }?;

        let (tx, rx) = mpsc::sync_channel(16);
        let (notify_exit_sender, exit_receiver) = oneshot::channel();
        let ping_thread = Some((
            notify_exit_sender,
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
                            if let Ok(rtt) = pinger.send(parsed_ip, buffer).await.result {
                                tx.try_send(Duration::from_millis(rtt as u64)).ok();
                            }
                            time::sleep(interval).await;
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
}

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping_with_interval(
    addr: impl Into<String>,
    interval: Duration,
    interface: Option<impl Into<String>>,
) -> Result<Pinger, Box<dyn Error>> {
    Pinger::new(addr.into(), interval, interface.map(|s| s.into()))
}
