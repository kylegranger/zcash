use rand::{thread_rng, Rng};

use std::{
    io::{self, Cursor, Read, Write},
    net::{IpAddr::*, Ipv6Addr, SocketAddr},
};

pub mod addr;
pub use addr::Addr;

pub mod version;
pub use version::Version;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Nonce(u64);

impl Default for Nonce {
    fn default() -> Self {
        Self(thread_rng().gen())
    }
}

impl Nonce {
    pub fn encode(&self, buffer: &mut Vec<u8>) -> io::Result<()> {
        buffer.write_all(&self.0.to_le_bytes())?;

        Ok(())
    }

    pub fn decode(bytes: &mut Cursor<&[u8]>) -> io::Result<Self> {
        let nonce = u64::from_le_bytes(read_n_bytes(bytes)?);

        Ok(Self(nonce))
    }
}

struct VarInt(usize);

impl VarInt {
    fn encode(&self, buffer: &mut Vec<u8>) -> io::Result<usize> {
        // length of the payload to be written.
        let l = self.0;
        let bytes_written = match l {
            0x0000_0000..=0x0000_00fc => {
                buffer.write_all(&[l as u8])?;
                1 // bytes written
            }
            0x0000_00fd..=0x0000_ffff => {
                buffer.write_all(&[0xfdu8])?;
                buffer.write_all(&(l as u16).to_le_bytes())?;
                3
            }
            0x0001_0000..=0xffff_ffff => {
                buffer.write_all(&[0xfeu8])?;
                buffer.write_all(&(l as u32).to_le_bytes())?;
                5
            }
            _ => {
                buffer.write_all(&[0xffu8])?;
                buffer.write_all(&(l as u64).to_le_bytes())?;
                9
            }
        };

        Ok(bytes_written)
    }

    fn decode(bytes: &mut Cursor<&[u8]>) -> io::Result<Self> {
        let flag = u8::from_le_bytes(read_n_bytes(bytes)?);

        let len = match flag {
            len @ 0x00..=0xfc => len as u64,
            0xfd => u16::from_le_bytes(read_n_bytes(bytes)?) as u64,
            0xfe => u32::from_le_bytes(read_n_bytes(bytes)?) as u64,
            0xff => u64::from_le_bytes(read_n_bytes(bytes)?) as u64,
        };

        Ok(VarInt(len as usize))
    }
}

fn write_addr(buffer: &mut Vec<u8>, (services, addr): (u64, SocketAddr)) -> io::Result<()> {
    buffer.write_all(&services.to_le_bytes())?;

    let (ip, port) = match addr {
        SocketAddr::V4(v4) => (v4.ip().to_ipv6_mapped(), v4.port()),
        SocketAddr::V6(v6) => (*v6.ip(), v6.port()),
    };

    buffer.write_all(&ip.octets())?;
    buffer.write_all(&port.to_be_bytes())?;

    Ok(())
}

fn write_string(buffer: &mut Vec<u8>, s: &str) -> io::Result<usize> {
    // Bitcoin "CompactSize" encoding.
    let l = s.len();
    let cs_len = match l {
        0x0000_0000..=0x0000_00fc => {
            buffer.write_all(&[l as u8])?;
            1 // bytes written
        }
        0x0000_00fd..=0x0000_ffff => {
            buffer.write_all(&[0xfdu8])?;
            buffer.write_all(&(l as u16).to_le_bytes())?;
            3
        }
        0x0001_0000..=0xffff_ffff => {
            buffer.write_all(&[0xfeu8])?;
            buffer.write_all(&(l as u32).to_le_bytes())?;
            5
        }
        _ => {
            buffer.write_all(&[0xffu8])?;
            buffer.write_all(&(l as u64).to_le_bytes())?;
            9
        }
    };

    buffer.write_all(s.as_bytes())?;

    Ok(l + cs_len)
}

fn decode_addr(bytes: &mut Cursor<&[u8]>) -> io::Result<(u64, SocketAddr)> {
    let services = u64::from_le_bytes(read_n_bytes(bytes)?);

    let mut octets = [0u8; 16];
    bytes.read_exact(&mut octets)?;
    let v6_addr = Ipv6Addr::from(octets);

    let ip_addr = match v6_addr.to_ipv4() {
        Some(v4_addr) => V4(v4_addr),
        None => V6(v6_addr),
    };

    let port = u16::from_be_bytes(read_n_bytes(bytes)?);

    Ok((services, SocketAddr::new(ip_addr, port)))
}

fn decode_string(bytes: &mut Cursor<&[u8]>) -> io::Result<String> {
    let flag = u8::from_le_bytes(read_n_bytes(bytes)?);

    let len = match flag {
        len @ 0x00..=0xfc => len as u64,
        0xfd => u16::from_le_bytes(read_n_bytes(bytes)?) as u64,
        0xfe => u32::from_le_bytes(read_n_bytes(bytes)?) as u64,
        0xff => u64::from_le_bytes(read_n_bytes(bytes)?),
    };

    let mut buffer = vec![0u8; len as usize];
    bytes.read_exact(&mut buffer)?;
    Ok(String::from_utf8(buffer).expect("invalid utf-8"))
}

fn read_n_bytes<const N: usize>(bytes: &mut Cursor<&[u8]>) -> io::Result<[u8; N]> {
    let mut buffer = [0u8; N];
    bytes.read_exact(&mut buffer)?;

    Ok(buffer)
}