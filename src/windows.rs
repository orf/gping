use crate::Pinger;
use dns_lookup::lookup_host;
use std::{net::IpAddr, sync::mpsc, thread, time::Duration};
use tokio::{sync::oneshot, time};
use winping::{AsyncPinger as WinPinger, Buffer};

#[derive(Default)]
pub struct WindowsPinger {
    interval: Duration,
    interface: Option<String>,
}

impl WindowsPinger {
    pub fn start(&self, target: String) -> Result<Pinger> {
        let interval = self.interval;
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

        let (tx, rx) = mpsc::channel();
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
                            match pinger.send(parsed_ip, buffer).await.result {
                                Ok(rtt) => {
                                    if tx.send(Ok(Duration::from_millis(rtt as u64))).is_err() {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    tx.send(Err(e.to_string())).ok();
                                }
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

    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    pub fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }
}
