pub mod frame;
pub mod mls;
pub mod openmls_session;
pub mod ratchet;
pub mod session;

pub const MAX_PROTOCOL_VERSION: u16 = 1;

pub const EXPORTER_LABEL: &str = "Discord Secure Frames v0";

pub fn displayable_code(data: &[u8], len: usize, group: usize) -> Option<String> {
    if group == 0 || group > 8 || !len.is_multiple_of(group) || data.len() < len {
        return None;
    }
    let modulus = 10u64.pow(group as u32);
    let mut out = String::with_capacity(len);
    for chunk in data[..len].chunks(group) {
        let v = chunk.iter().fold(0u64, |acc, b| (acc << 8) | *b as u64) % modulus;
        out.push_str(&format!("{v:0width$}", width = group));
    }
    Some(out)
}

pub fn privacy_code(epoch_authenticator: &[u8]) -> Option<String> {
    let code = displayable_code(epoch_authenticator, 30, 5)?;
    Some(code.as_bytes().chunks(5).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displayable_code_groups() {
        let data: Vec<u8> = (0..32).collect();
        let c = displayable_code(&data, 30, 5).unwrap();
        assert_eq!(c.len(), 30);
        assert_eq!(&c[..5], "09060");
        assert!(c.bytes().all(|b| b.is_ascii_digit()));
        assert_eq!(privacy_code(&data).unwrap().split(' ').count(), 6);
        assert!(displayable_code(&data[..10], 30, 5).is_none());
    }
}
