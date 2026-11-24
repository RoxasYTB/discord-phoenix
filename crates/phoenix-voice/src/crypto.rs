use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionMode {
    Aes256GcmRtpSize,
    XChaCha20Poly1305RtpSize,
}

impl EncryptionMode {
    pub fn name(self) -> &'static str {
        match self {
            EncryptionMode::Aes256GcmRtpSize => "aead_aes256_gcm_rtpsize",
            EncryptionMode::XChaCha20Poly1305RtpSize => "aead_xchacha20_poly1305_rtpsize",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "aead_aes256_gcm_rtpsize" => Some(Self::Aes256GcmRtpSize),
            "aead_xchacha20_poly1305_rtpsize" => Some(Self::XChaCha20Poly1305RtpSize),
            _ => None,
        }
    }

    pub fn choose(server_modes: &[String]) -> Option<Self> {
        [Self::Aes256GcmRtpSize, Self::XChaCha20Poly1305RtpSize]
            .into_iter()
            .find(|m| server_modes.iter().any(|s| s == m.name()))
    }

    fn nonce_len(self) -> usize {
        match self {
            EncryptionMode::Aes256GcmRtpSize => 12,
            EncryptionMode::XChaCha20Poly1305RtpSize => 24,
        }
    }
}

enum Cipher {
    Aes(Box<Aes256Gcm>),
    XChaCha(Box<XChaCha20Poly1305>),
}

pub struct TransportCipher {
    mode: EncryptionMode,
    cipher: Cipher,
    counter: u32,
}

pub const TAG_LEN: usize = 16;
pub const NONCE_SUFFIX_LEN: usize = 4;

impl TransportCipher {
    pub fn new(mode: EncryptionMode, secret_key: &[u8]) -> Result<Self> {
        if secret_key.len() != 32 {
            return Err(Error::Crypto("secret key must be 32 bytes"));
        }
        let cipher = match mode {
            EncryptionMode::Aes256GcmRtpSize => Cipher::Aes(Box::new(
                Aes256Gcm::new_from_slice(secret_key).map_err(|_| Error::Crypto("key"))?,
            )),
            EncryptionMode::XChaCha20Poly1305RtpSize => Cipher::XChaCha(Box::new(
                XChaCha20Poly1305::new_from_slice(secret_key).map_err(|_| Error::Crypto("key"))?,
            )),
        };
        Ok(Self { mode, cipher, counter: 0 })
    }

    pub fn mode(&self) -> EncryptionMode {
        self.mode
    }

    pub fn set_counter(&mut self, c: u32) {
        self.counter = c;
    }

    fn full_nonce(&self, suffix: [u8; 4]) -> Vec<u8> {
        let mut n = vec![0u8; self.mode.nonce_len()];
        n[..4].copy_from_slice(&suffix);
        n
    }

    fn seal(&self, nonce: &[u8], aad: &[u8], msg: &[u8]) -> Result<Vec<u8>> {
        let p = Payload { msg, aad };
        match &self.cipher {
            Cipher::Aes(c) => c.encrypt(nonce.into(), p),
            Cipher::XChaCha(c) => c.encrypt(nonce.into(), p),
        }
        .map_err(|_| Error::Crypto("encrypt"))
    }

    fn open(&self, nonce: &[u8], aad: &[u8], msg: &[u8]) -> Result<Vec<u8>> {
        let p = Payload { msg, aad };
        match &self.cipher {
            Cipher::Aes(c) => c.decrypt(nonce.into(), p),
            Cipher::XChaCha(c) => c.decrypt(nonce.into(), p),
        }
        .map_err(|_| Error::Crypto("decrypt / authentication failed"))
    }

    pub fn encrypt_rtp(&mut self, packet: &[u8]) -> Result<Vec<u8>> {
        let aad_len = rtp_aad_len(packet)?;
        self.encrypt_with_aad(packet, aad_len)
    }

    pub fn encrypt_rtcp(&mut self, packet: &[u8]) -> Result<Vec<u8>> {
        if packet.len() < 8 {
            return Err(Error::Malformed("rtcp too short"));
        }
        self.encrypt_with_aad(packet, 8)
    }

    fn encrypt_with_aad(&mut self, packet: &[u8], aad_len: usize) -> Result<Vec<u8>> {
        let suffix = self.counter.to_be_bytes();
        self.counter = self.counter.wrapping_add(1);
        let nonce = self.full_nonce(suffix);
        let (aad, body) = packet.split_at(aad_len);
        let ct = self.seal(&nonce, aad, body)?;
        let mut out = Vec::with_capacity(aad_len + ct.len() + 4);
        out.extend_from_slice(aad);
        out.extend_from_slice(&ct);
        out.extend_from_slice(&suffix);
        Ok(out)
    }

    pub fn decrypt_rtp(&self, packet: &[u8]) -> Result<Vec<u8>> {
        let aad_len = rtp_aad_len(packet)?;
        self.decrypt_with_aad(packet, aad_len)
    }

    pub fn decrypt_rtcp(&self, packet: &[u8]) -> Result<Vec<u8>> {
        self.decrypt_with_aad(packet, 8)
    }

    fn decrypt_with_aad(&self, packet: &[u8], aad_len: usize) -> Result<Vec<u8>> {
        if packet.len() < aad_len + TAG_LEN + NONCE_SUFFIX_LEN {
            return Err(Error::Malformed("encrypted packet too short"));
        }
        let (rest, suffix) = packet.split_at(packet.len() - NONCE_SUFFIX_LEN);
        let mut s = [0u8; 4];
        s.copy_from_slice(suffix);
        let nonce = self.full_nonce(s);
        let (aad, ct) = rest.split_at(aad_len);
        let pt = self.open(&nonce, aad, ct)?;
        let mut out = Vec::with_capacity(aad_len + pt.len());
        out.extend_from_slice(aad);
        out.extend_from_slice(&pt);
        Ok(out)
    }
}

pub fn rtp_aad_len(packet: &[u8]) -> Result<usize> {
    if packet.len() < 12 {
        return Err(Error::Malformed("rtp too short"));
    }
    let cc = (packet[0] & 0x0f) as usize;
    let mut len = 12 + 4 * cc;
    if packet[0] & 0x10 != 0 {
        len += 4;
    }
    if packet.len() < len {
        return Err(Error::Malformed("rtp header truncated"));
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0)).collect()
    }

    #[test]
    fn aes256gcm_known_vector() {
        let c = Aes256Gcm::new_from_slice(&[0u8; 32]).unwrap_or_else(|_| panic!("key"));
        let out = c.encrypt((&[0u8; 12]).into(), &[0u8; 16][..]).unwrap_or_default();
        assert_eq!(out, hex("cea7403d4d606b6e074ec5d3baf39d18d0d1c8a799996bf0265b98b5d48ab919"));
    }

    #[test]
    fn rtpsize_roundtrip_both_modes() {
        let mut pkt = vec![0x90, 0x78, 0x00, 0x01, 0, 0, 0, 10, 0, 0, 0, 42];
        pkt.extend_from_slice(&[0xbe, 0xde, 0x00, 0x01]);
        pkt.extend_from_slice(&[0x10, 0xaa, 0, 0]);
        pkt.extend_from_slice(b"opus payload");
        for mode in [EncryptionMode::Aes256GcmRtpSize, EncryptionMode::XChaCha20Poly1305RtpSize] {
            let key = [7u8; 32];
            let mut tx = TransportCipher::new(mode, &key).unwrap_or_else(|e| panic!("{e}"));
            tx.set_counter(0x01020304);
            let enc = tx.encrypt_rtp(&pkt).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(&enc[..16], &pkt[..16], "header+ext header stays clear");
            assert_eq!(&enc[enc.len() - 4..], &[1, 2, 3, 4]);
            assert_eq!(enc.len(), pkt.len() + 20);
            let rx = TransportCipher::new(mode, &key).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(rx.decrypt_rtp(&enc).unwrap_or_default(), pkt);
            let mut bad = enc.clone();
            bad[3] ^= 1;
            assert!(rx.decrypt_rtp(&bad).is_err());
        }
    }

    #[test]
    fn rtcp_roundtrip() {
        let mut tx = TransportCipher::new(EncryptionMode::Aes256GcmRtpSize, &[1; 32]).unwrap_or_else(|e| panic!("{e}"));
        let pkt = [0x80, 200, 0, 6, 0, 0, 0, 1, 9, 9, 9, 9];
        let e = tx.encrypt_rtcp(&pkt).unwrap_or_default();
        assert_eq!(tx.decrypt_rtcp(&e).unwrap_or_default(), pkt);
    }

    #[test]
    fn choose_mode() {
        let m = vec!["aead_xchacha20_poly1305_rtpsize".to_string(), "aead_aes256_gcm_rtpsize".into()];
        assert_eq!(EncryptionMode::choose(&m), Some(EncryptionMode::Aes256GcmRtpSize));
        assert_eq!(EncryptionMode::choose(&["xsalsa20_poly1305".into()]), None);
    }
}
