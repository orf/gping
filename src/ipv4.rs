use std::error::Error;

const MINIMUM_PACKET_SIZE: usize = 20;

#[derive(Debug, PartialEq)]
pub enum IpV4Protocol {
    Icmp,
}

impl IpV4Protocol {
    fn decode(data: u8) -> Option<Self> {
        match data {
            1 => Some(IpV4Protocol::Icmp),
            _ => None,
        }
    }
}

pub struct IpV4Packet<'a> {
    pub protocol: IpV4Protocol,
    pub data: &'a [u8],
}

impl<'a> IpV4Packet<'a> {
    pub fn decode(data: &'a [u8]) -> Result<Self, Box<dyn Error>> {
        if data.len() < MINIMUM_PACKET_SIZE {
            return Err("Too Small Header".into());
        }
        let byte0 = data[0];
        let version = (byte0 & 0xf0) >> 4;
        let header_size = 4 * ((byte0 & 0x0f) as usize);

        if version != 4 {
            return Err("Invalid Version".into());
        }

        if data.len() < header_size {
            return Err("Invalid Header Size".into());
        }

        let protocol = match IpV4Protocol::decode(data[9]) {
            Some(protocol) => protocol,
            None => return Err("Unknown Protocol".into()),
        };

        Ok(Self {
            protocol,
            data: &data[header_size..],
        })
    }
}
