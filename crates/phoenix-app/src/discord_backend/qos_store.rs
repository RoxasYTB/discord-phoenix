use std::collections::BTreeMap;

const FILE: &str = "derived_qos.json";

pub(super) fn token_user(token: &str) -> Option<u64> {
    use base64::Engine;
    let head = token.split('.').next()?.trim_end_matches('=').replace('+', "-").replace('/', "_");
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(head).ok()?;
    std::str::from_utf8(&raw).ok()?.parse().ok()
}

fn update(map: &mut BTreeMap<String, String>, user: u64, data: Option<&str>) -> bool {
    let key = user.to_string();
    match data {
        Some(d) if map.get(&key).map(String::as_str) != Some(d) => {
            map.insert(key, d.to_owned());
            true
        }
        Some(_) => false,
        None => map.remove(&key).is_some(),
    }
}

fn read() -> BTreeMap<String, String> {
    crate::paths::config_file(FILE).and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub(super) fn load(token: &str) -> Option<String> {
    read().remove(&token_user(token)?.to_string())
}

pub(super) fn save(user: u64, data: Option<&str>) {
    let mut map = read();
    if !update(&mut map, user, data) {
        return;
    }
    let Some(path) = crate::paths::config_file(FILE) else { return };
    let Ok(bytes) = serde_json::to_vec(&map) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    if let Ok(mut f) = o.open(&path) {
        let _ = std::io::Write::write_all(&mut f, &bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_id_comes_from_the_first_token_segment() {
        assert_eq!(token_user("MTIzNDU2Nzg5MDEyMzQ1Njc4.GAAAAA.fake-signature"), Some(123456789012345678));
        assert_eq!(token_user("MTIzNDU2Nzg5MDEyMzQ1Njc4OQ==.x.y"), Some(1234567890123456789));
        assert_eq!(token_user("not a token"), None);
    }

    #[test]
    fn ready_without_a_token_drops_the_stored_one() {
        let mut m = BTreeMap::new();
        assert!(update(&mut m, 7, Some("CgEB")));
        assert!(!update(&mut m, 7, Some("CgEB")));
        assert!(update(&mut m, 8, Some("CgEC")));
        assert!(update(&mut m, 7, None));
        assert!(!update(&mut m, 7, None));
        assert_eq!(m.into_iter().collect::<Vec<_>>(), vec![("8".to_owned(), "CgEC".to_owned())]);
    }
}
