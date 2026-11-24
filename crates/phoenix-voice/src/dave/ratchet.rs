use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{Error, Result};

pub trait KeyRatchet: Send {
    fn key_for_generation(&mut self, generation: u32) -> Result<[u8; 16]>;
    fn erase_before(&mut self, _generation: u32) {}
}

pub fn expand_with_label(secret: &[u8], label: &str, context: &[u8], len: usize) -> Result<Vec<u8>> {
    let full = format!("MLS 1.0 {label}");
    let mut info = Vec::with_capacity(4 + full.len() + context.len());
    info.extend_from_slice(&(len as u16).to_be_bytes());
    write_varint_vec(&mut info, full.as_bytes())?;
    write_varint_vec(&mut info, context)?;
    hkdf_expand(secret, &info, len)
}

pub fn hkdf_expand(prk: &[u8], info: &[u8], len: usize) -> Result<Vec<u8>> {
    if len > 255 * 32 {
        return Err(Error::Crypto("hkdf length"));
    }
    let mut out = Vec::with_capacity(len);
    let mut t: Vec<u8> = Vec::new();
    let mut i = 1u8;
    while out.len() < len {
        let mut m = <Hmac<Sha256> as Mac>::new_from_slice(prk).map_err(|_| Error::Crypto("hmac key"))?;
        m.update(&t);
        m.update(info);
        m.update(&[i]);
        t = m.finalize().into_bytes().to_vec();
        out.extend_from_slice(&t);
        i = i.wrapping_add(1);
    }
    out.truncate(len);
    Ok(out)
}

fn write_varint_vec(out: &mut Vec<u8>, data: &[u8]) -> Result<()> {
    let l = data.len();
    if l < 64 {
        out.push(l as u8);
    } else if l < 16384 {
        out.extend_from_slice(&((l as u16) | 0x4000).to_be_bytes());
    } else if l < (1 << 30) {
        out.extend_from_slice(&((l as u32) | 0x8000_0000).to_be_bytes());
    } else {
        return Err(Error::Crypto("vector too long"));
    }
    out.extend_from_slice(data);
    Ok(())
}

pub struct HashRatchet {
    next_generation: u32,
    next_secret: Vec<u8>,
    cache: std::collections::BTreeMap<u32, [u8; 16]>,
    max_cached: usize,
}

impl HashRatchet {
    pub fn new(base_secret: &[u8]) -> Self {
        Self { next_generation: 0, next_secret: base_secret.to_vec(), cache: Default::default(), max_cached: 8 }
    }
}

impl KeyRatchet for HashRatchet {
    fn key_for_generation(&mut self, generation: u32) -> Result<[u8; 16]> {
        if let Some(k) = self.cache.get(&generation) {
            return Ok(*k);
        }
        if generation < self.next_generation {
            return Err(Error::Crypto("generation already erased"));
        }
        if generation - self.next_generation > 1024 {
            return Err(Error::Crypto("generation too far ahead"));
        }
        while self.next_generation <= generation {
            let ctx = self.next_generation.to_be_bytes();
            let key = expand_with_label(&self.next_secret, "key", &ctx, 16)?;
            let next = expand_with_label(&self.next_secret, "secret", &ctx, 32)?;
            let mut k = [0u8; 16];
            k.copy_from_slice(&key);
            self.cache.insert(self.next_generation, k);
            self.next_secret = next;
            self.next_generation += 1;
        }
        while self.cache.len() > self.max_cached {
            if let Some((&g, _)) = self.cache.iter().next() {
                self.cache.remove(&g);
            }
        }
        self.cache.get(&generation).copied().ok_or(Error::Crypto("ratchet cache"))
    }

    fn erase_before(&mut self, generation: u32) {
        self.cache.retain(|g, _| *g >= generation);
    }
}

pub struct StaticRatchet(pub [u8; 16]);

impl KeyRatchet for StaticRatchet {
    fn key_for_generation(&mut self, _generation: u32) -> Result<[u8; 16]> {
        Ok(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn hkdf_expand_rfc5869_case1() {
        let prk = hex("077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5");
        let okm = hkdf_expand(&prk, &hex("f0f1f2f3f4f5f6f7f8f9"), 42).unwrap();
        assert_eq!(okm, hex("3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"));
    }

    #[test]
    fn ratchet_deterministic_and_distinct() {
        let mut a = HashRatchet::new(&[3u8; 16]);
        let mut b = HashRatchet::new(&[3u8; 16]);
        let k0 = a.key_for_generation(0).unwrap();
        let k5 = a.key_for_generation(5).unwrap();
        assert_ne!(k0, k5);
        assert_eq!(b.key_for_generation(5).unwrap(), k5);
        assert_eq!(b.key_for_generation(0).unwrap(), k0);
        a.erase_before(5);
        assert!(a.key_for_generation(0).is_err());
    }
}
