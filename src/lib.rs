mod icmp;
mod ipv4;

#[cfg(unix)]
mod linux;

#[cfg(unix)]
pub use linux::{ping_with_interval, Pinger};

#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use linux::{ping_with_interval, Pinger};
