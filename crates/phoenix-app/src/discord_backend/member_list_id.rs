//! The id the gateway gives a channel's lazy member list
//! (`GUILD_MEMBER_LIST_UPDATE.id`), computed from the channel's overwrites so
//! a SYNC can be matched to the channel it belongs to.
//!
//! Measured on the real account (2026-11-24, op 37 then the SYNC `id`): one
//! entry per overwrite that touches VIEW_CHANNEL, `allow:<id>` when it allows
//! it, else `deny:<id>`; entries sorted as strings, joined with `,`, hashed with
//! 32-bit MurmurHash3 (seed 0) and printed as an unsigned decimal. No entry at
//! all gives `everyone`. Checked against three lists: 1093928432 (two allows),
//! 2860088392 (one deny), 1037889033 (two allows + one deny, mixed order), and
//! `everyone` for a channel without VIEW overwrites.

use phoenix_api::{Cache, models as api};

/// List id for `channel`; threads use their parent's list.
pub fn for_channel(cache: &Cache, channel: api::ChannelId) -> Option<String> {
    let mut c = cache.channel(channel)?;
    if c.kind.is_thread() {
        c = cache.channel(c.parent_id?)?;
    }
    Some(from_overwrites(&c.permission_overwrites))
}

pub fn from_overwrites(overwrites: &[api::PermissionOverwrite]) -> String {
    let view = api::Permissions::VIEW_CHANNEL;
    let mut parts: Vec<String> = overwrites
        .iter()
        .filter_map(|o| {
            if o.allow.contains(view) {
                Some(format!("allow:{}", o.id.0))
            } else if o.deny.contains(view) {
                Some(format!("deny:{}", o.id.0))
            } else {
                None
            }
        })
        .collect();
    if parts.is_empty() {
        return "everyone".into();
    }
    parts.sort();
    murmur3_32(parts.join(",").as_bytes(), 0).to_string()
}

fn murmur3_32(data: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mix = |k: u32| k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
    let mut h = seed;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        h ^= mix(u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    let tail = chunks.remainder();
    if !tail.is_empty() {
        let k = tail.iter().rev().fold(0u32, |k, b| (k << 8) | u32::from(*b));
        h ^= mix(k);
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use phoenix_api::models::{OverwriteType, Permissions, PermissionOverwrite, Snowflake};

    fn ow(id: u64, allow: u64, deny: u64) -> PermissionOverwrite {
        PermissionOverwrite {
            id: Snowflake(id),
            kind: OverwriteType::Role,
            allow: Permissions::from_bits_retain(allow),
            deny: Permissions::from_bits_retain(deny),
        }
    }

    #[test]
    fn murmur_reference_values() {
        assert_eq!(murmur3_32(b"", 0), 0);
        assert_eq!(murmur3_32(b"hello", 0), 0x248b_fa47);
        assert_eq!(murmur3_32(b"The quick brown fox jumps over the lazy dog", 0), 0x2e4f_f723);
    }

    #[test]
    fn ids_measured_on_the_gateway() {
        let ids = |a: &[PermissionOverwrite]| from_overwrites(a);
        // Role ids and list ids as the gateway answered (2026-11-24); the first
        // pair comes in the opposite order to the sorted one.
        let (everyone, member) = (1_366_311_080_011_501_578, 1_422_967_078_482_350_161);
        assert_eq!(ids(&[ow(member, 137_439_333_376, 0), ow(everyone, 3072, 0)]), "1093928432");
        assert_eq!(ids(&[ow(everyone, 0, 1024)]), "2860088392");
        let (g2, r1, r2) = (1_369_382_919_948_210_248, 1_475_278_046_331_408_384, 1_369_413_809_151_934_648);
        assert_eq!(ids(&[ow(g2, 0, 1024), ow(r1, 1024, 0), ow(r2, 1024, 0)]), "1037889033");
        // Overwrites that leave VIEW_CHANNEL alone don't count.
        assert_eq!(ids(&[ow(member, 2048, 0)]), "everyone");
        assert_eq!(ids(&[]), "everyone");
    }
}
