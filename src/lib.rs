use std::{error::Error, sync::mpsc, thread::JoinHandle, time::Duration};
use tokio::sync::oneshot;

mod icmp;
mod ipv4;

#[cfg(unix)]
pub mod linux;

#[cfg(windows)]
pub mod windows;

pub struct Pinger {
    pub channel: mpsc::Receiver<Result<Duration, String>>,
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

/// Start pinging a an address. The address can be either a hostname or an IP address.
pub fn ping_with_interval(
    addr: String,
    interval: Duration,
    interface: Option<String>,
) -> Result<Pinger, Box<dyn Error>> {
    #[cfg(windows)]
    {
        let mut p = windows::WindowsPinger::default();
        p.set_interval(interval);
        p.set_interface(interface);
        p.start(addr)
    }
    #[cfg(unix)]
    {
        let mut p = linux::LinuxPinger::default();
        p.set_interval(interval);
        p.set_interface(interface);
        p.start(addr)
    }
}
