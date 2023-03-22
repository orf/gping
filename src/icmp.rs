use std::{error::Error, io::Write};

pub const ICMP_HEADER_SIZE: usize = 8;

pub struct IcmpV4;
pub struct IcmpV6;

pub trait Proto {
    const ECHO_REQUEST_TYPE: u8;
    const ECHO_REQUEST_CODE: u8;
    const ECHO_REPLY_TYPE: u8;
    const ECHO_REPLY_CODE: u8;
}

impl Proto for IcmpV4 {
    const ECHO_REQUEST_TYPE: u8 = 8;
    const ECHO_REQUEST_CODE: u8 = 0;
    const ECHO_REPLY_TYPE: u8 = 0;
    const ECHO_REPLY_CODE: u8 = 0;
}

impl Proto for IcmpV6 {
    const ECHO_REQUEST_TYPE: u8 = 128;
    const ECHO_REQUEST_CODE: u8 = 0;
    const ECHO_REPLY_TYPE: u8 = 129;
    const ECHO_REPLY_CODE: u8 = 0;
}

pub struct EchoRequest<'a> {
    pub ident: u16,
    pub seq_cnt: u16,
    pub payload: &'a [u8],
}

impl<'a> EchoRequest<'a> {
    pub fn encode<P: Proto>(&self, buffer: &mut [u8]) {
        buffer[0] = P::ECHO_REQUEST_TYPE;
        buffer[1] = P::ECHO_REQUEST_CODE;

        buffer[4] = (self.ident >> 8) as u8;
        buffer[5] = self.ident as u8;
        buffer[6] = (self.seq_cnt >> 8) as u8;
        buffer[7] = self.seq_cnt as u8;

        (&mut buffer[8..]).write_all(self.payload).unwrap();

        write_checksum(buffer);
    }
}

pub struct EchoReply<'a> {
    pub ident: u16,
    pub seq_cnt: u16,
    pub payload: &'a [u8],
}

impl<'a> EchoReply<'a> {
    pub fn decode<P: Proto>(buffer: &'a [u8]) -> Result<Self, Box<dyn Error>> {
        if buffer.as_ref().len() < ICMP_HEADER_SIZE {
            return Err("Invalid size".into());
        }

        let type_ = buffer[0];
        let code = buffer[1];
        if type_ != P::ECHO_REPLY_TYPE && code != P::ECHO_REPLY_CODE {
            return Err("Invalid Packet".into());
        }

        let ident = (u16::from(buffer[4]) << 8) + u16::from(buffer[5]);
        let seq_cnt = (u16::from(buffer[6]) << 8) + u16::from(buffer[7]);

        let payload = &buffer[ICMP_HEADER_SIZE..];

        Ok(EchoReply {
            ident,
            seq_cnt,
            payload,
        })
    }
}

fn write_checksum(buffer: &mut [u8]) {
    let mut sum = 0u32;
    for word in buffer.chunks(2) {
        let mut part = u16::from(word[0]) << 8;
        if word.len() > 1 {
            part += u16::from(word[1]);
        }
        sum = sum.wrapping_add(u32::from(part));
    }

    while (sum >> 16) > 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    let sum = !sum as u16;

    buffer[2] = (sum >> 8) as u8;
    buffer[3] = (sum & 0xff) as u8;
}
