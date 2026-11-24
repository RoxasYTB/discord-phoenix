use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::UdpSocket;

use crate::crypto::TransportCipher;
use crate::{Error, Result};

pub const DISCOVERY_LEN: usize = 74;

pub fn discovery_request(ssrc: u32) -> [u8; DISCOVERY_LEN] {
    let mut b = [0u8; DISCOVERY_LEN];
    b[0..2].copy_from_slice(&1u16.to_be_bytes());
    b[2..4].copy_from_slice(&70u16.to_be_bytes());
    b[4..8].copy_from_slice(&ssrc.to_be_bytes());
    b
}

pub fn discovery_response(ssrc: u32, ip: &str, port: u16) -> Result<[u8; DISCOVERY_LEN]> {
    let mut b = [0u8; DISCOVERY_LEN];
    b[0..2].copy_from_slice(&2u16.to_be_bytes());
    b[2..4].copy_from_slice(&70u16.to_be_bytes());
    b[4..8].copy_from_slice(&ssrc.to_be_bytes());
    let ipb = ip.as_bytes();
    if ipb.len() > 63 {
        return Err(Error::Malformed("ip too long"));
    }
    b[8..8 + ipb.len()].copy_from_slice(ipb);
    b[72..74].copy_from_slice(&port.to_be_bytes());
    Ok(b)
}

pub fn parse_discovery_response(b: &[u8]) -> Result<(String, u16)> {
    if b.len() < DISCOVERY_LEN || u16::from_be_bytes([b[0], b[1]]) != 2 {
        return Err(Error::Malformed("ip discovery response"));
    }
    let addr = &b[8..72];
    let end = addr.iter().position(|&c| c == 0).unwrap_or(addr.len());
    let ip = std::str::from_utf8(&addr[..end]).map_err(|_| Error::Malformed("ip discovery utf8"))?;
    Ok((ip.to_string(), u16::from_be_bytes([b[72], b[73]])))
}

pub struct MediaSocket {
    socket: UdpSocket,
    cipher: Option<TransportCipher>,
}

impl MediaSocket {
    pub async fn connect(server: SocketAddr) -> Result<Self> {
        let bind: SocketAddr = if server.is_ipv4() { ([0, 0, 0, 0], 0).into() } else { ([0u16; 8], 0).into() };
        let socket = UdpSocket::bind(bind).await?;
        socket.connect(server).await?;
        Ok(Self { socket, cipher: None })
    }

    pub async fn discover(&self, ssrc: u32) -> Result<(String, u16)> {
        let req = discovery_request(ssrc);
        let mut buf = [0u8; 128];
        for _ in 0..3 {
            self.socket.send(&req).await?;
            if let Ok(r) = tokio::time::timeout(Duration::from_secs(1), self.socket.recv(&mut buf)).await {
                let n = r?;
                if let Ok(v) = parse_discovery_response(&buf[..n]) {
                    return Ok(v);
                }
            }
        }
        Err(Error::Timeout("ip discovery"))
    }

    pub fn set_cipher(&mut self, c: TransportCipher) {
        self.cipher = Some(c);
    }

    pub async fn send_rtp(&mut self, packet: &[u8]) -> Result<()> {
        let c = self.cipher.as_mut().ok_or(Error::Protocol("no session description yet".into()))?;
        let enc = c.encrypt_rtp(packet)?;
        self.socket.send(&enc).await?;
        Ok(())
    }

    pub async fn send_rtcp(&mut self, packet: &[u8]) -> Result<()> {
        let c = self.cipher.as_mut().ok_or(Error::Protocol("no session description yet".into()))?;
        let enc = c.encrypt_rtcp(packet)?;
        self.socket.send(&enc).await?;
        Ok(())
    }

    pub async fn recv(&self) -> Result<Incoming> {
        let mut buf = vec![0u8; 2048];
        let n = self.socket.recv(&mut buf).await?;
        buf.truncate(n);
        classify_and_decrypt(self.cipher.as_ref(), &buf)
    }
}

#[derive(Debug)]
pub enum Incoming {
    Rtp(Vec<u8>),
    Rtcp(Vec<u8>),
}

pub fn classify_and_decrypt(cipher: Option<&TransportCipher>, buf: &[u8]) -> Result<Incoming> {
    let c = cipher.ok_or(Error::Protocol("no session description yet".into()))?;
    if buf.len() < 2 || buf[0] >> 6 != 2 {
        return Err(Error::Malformed("not rtp/rtcp"));
    }
    if (192..=223).contains(&buf[1]) {
        Ok(Incoming::Rtcp(c.decrypt_rtcp(buf)?))
    } else {
        Ok(Incoming::Rtp(c.decrypt_rtp(buf)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_roundtrip() {
        let r = discovery_response(42, "203.0.113.7", 50004).unwrap();
        assert_eq!(parse_discovery_response(&r).unwrap(), ("203.0.113.7".into(), 50004));
        let q = discovery_request(42);
        assert_eq!(&q[..8], &[0, 1, 0, 70, 0, 0, 0, 42]);
    }

    #[tokio::test]
    async fn discovery_against_local_server() {
        let srv = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = srv.local_addr().unwrap();
        tokio::spawn(async move {
            let mut b = [0u8; 128];
            if let Ok((n, from)) = srv.recv_from(&mut b).await {
                assert_eq!(n, DISCOVERY_LEN);
                let ssrc = u32::from_be_bytes([b[4], b[5], b[6], b[7]]);
                let r = discovery_response(ssrc, &from.ip().to_string(), from.port()).unwrap();
                let _ = srv.send_to(&r, from).await;
            }
        });
        let s = MediaSocket::connect(addr).await.unwrap();
        let (ip, _port) = s.discover(7).await.unwrap();
        assert_eq!(ip, "127.0.0.1");
    }
}
