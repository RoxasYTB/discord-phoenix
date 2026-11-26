use reqwest::Method;
use serde_json::Value;

use super::RestClient;
use crate::error::Result;

pub const ANNOUNCEMENT_COMPOSER: &str = "2026-02-announcement-composer";
pub const SOUNDBOARD_NEW_SOUNDS_BADGE: &str = "2026-10-soundboard-new-sounds-badge";
pub const A11Y_TARGET_SIZE: &str = "2026-09-a11y-chassis-target-size";

pub fn murmur3(key: &str) -> u32 {
    let data = key.as_bytes();
    let (c1, c2) = (0xcc9e_2d51u32, 0x1b87_3593u32);
    let mut h = 0u32;
    let (chunks, tail) = data.as_chunks::<4>();
    for c in chunks {
        let k = u32::from_le_bytes(*c).wrapping_mul(c1).rotate_left(15).wrapping_mul(c2);
        h = (h ^ k).rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    let mut k = 0u32;
    for (i, b) in tail.iter().enumerate() {
        k ^= u32::from(*b) << (8 * i);
    }
    if !tail.is_empty() {
        h ^= k.wrapping_mul(c1).rotate_left(15).wrapping_mul(c2);
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

pub fn assignments(v: &Value) -> Vec<(u32, i64)> {
    v.get("assignments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let a = a.as_array()?;
            let hash = u32::try_from(a.first()?.as_u64()?).ok()?;
            Some((hash, a.get(2)?.as_i64()?))
        })
        .collect()
}

pub fn apex_assignments(v: &Value) -> Vec<(u32, i64)> {
    v.get("assignments")
        .and_then(|a| a.get("1"))
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|units| units.values())
        .filter_map(|u| u.get("assignments").and_then(Value::as_array))
        .flatten()
        .filter_map(|a| {
            let a = a.as_array()?;
            let hash = u32::try_from(a.first()?.as_u64()?).ok()?;
            Some((hash, a.get(1)?.as_i64()?))
        })
        .collect()
}

static APEX: std::sync::RwLock<Vec<(u32, i64)>> = std::sync::RwLock::new(Vec::new());

pub fn set_apex(v: Option<&Value>) {
    let list = v.map(apex_assignments).unwrap_or_default();
    if let Ok(mut a) = APEX.write() {
        *a = list;
    }
}

pub fn apex() -> Vec<(u32, i64)> {
    APEX.read().map(|a| a.clone()).unwrap_or_default()
}

pub fn bucket(list: &[(u32, i64)], name: &str) -> i64 {
    let h = murmur3(name);
    list.iter().find(|(x, _)| *x == h).map_or(0, |(_, b)| *b)
}

impl RestClient {
    pub async fn user_experiments(&self) -> Result<Vec<(u32, i64)>> {
        let mut out = apex();
        match self.call::<Value>(Method::GET, "/experiments", super::Body::None).await {
            Ok(v) => out.extend(assignments(&v)),
            Err(e) if out.is_empty() => return Err(e),
            Err(_) => {}
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apex_assignments_read_the_user_unit_variant() {
        let v = serde_json::json!({"assignments": {"1": {"42": {"evaluation_id": "x", "assignments": [[3012809029u64, 1, 0, 2], [2094783262u64, 1, 2, 1], [7, 0, 0, 3]]}}, "2": {"9": {"assignments": [[5, 1, 0, 1]]}}}});
        let list = apex_assignments(&v);
        assert_eq!(list, vec![(3_012_809_029, 1), (2_094_783_262, 1), (7, 0)]);
        assert_eq!(murmur3(A11Y_TARGET_SIZE), 3_012_809_029);
        assert_eq!(murmur3(SOUNDBOARD_NEW_SOUNDS_BADGE), 2_094_783_262);
        assert_eq!(bucket(&list, A11Y_TARGET_SIZE), 1);
    }

    #[test]
    fn hash_matches_the_client_murmur3() {
        assert_eq!(murmur3(ANNOUNCEMENT_COMPOSER), 2_988_495_041);
        assert_eq!(murmur3(""), 0);
    }

    #[test]
    fn assignments_read_hash_and_bucket() {
        let v: Value = serde_json::from_str(r#"{"assignments":[[3013881204,5,11,-1,1,4765,0,0,null],[2988495041,1,1,-1,0,10,0,0]]}"#).unwrap();
        let list = assignments(&v);
        assert_eq!(list, vec![(3_013_881_204, 11), (2_988_495_041, 1)]);
        assert_eq!(bucket(&list, ANNOUNCEMENT_COMPOSER), 1);
        assert_eq!(bucket(&list[..1], ANNOUNCEMENT_COMPOSER), 0);
    }
}
