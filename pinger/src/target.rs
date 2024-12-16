use std::fmt;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum IPVersion {
    V4,
    V6,
    Any,
}

#[derive(Debug, Clone)]
pub enum Target {
    IP(IpAddr),
    Hostname { domain: String, version: IPVersion },
}

impl Target {
    pub fn is_ipv6(&self) -> bool {
        match self {
            Target::IP(ip) => ip.is_ipv6(),
            Target::Hostname { version, .. } => *version == IPVersion::V6,
        }
    }

    pub fn new_any(value: impl ToString) -> Self {
        let value = value.to_string();
        if let Ok(ip) = value.parse::<IpAddr>() {
            return Self::IP(ip);
        }
        Self::Hostname {
            domain: value,
            version: IPVersion::Any,
        }
    }

    pub fn new_ipv4(value: impl ToString) -> Self {
        let value = value.to_string();
        if let Ok(ip) = value.parse::<Ipv4Addr>() {
            return Self::IP(IpAddr::V4(ip));
        }
        Self::Hostname {
            domain: value.to_string(),
            version: IPVersion::V4,
        }
    }

    pub fn new_ipv6(value: impl ToString) -> Self {
        let value = value.to_string();
        if let Ok(ip) = value.parse::<Ipv6Addr>() {
            return Self::IP(IpAddr::V6(ip));
        }
        Self::Hostname {
            domain: value.to_string(),
            version: IPVersion::V6,
        }
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Target::IP(v) => Display::fmt(&v, f),
            Target::Hostname { domain, .. } => Display::fmt(&domain, f),
        }
    }
}
