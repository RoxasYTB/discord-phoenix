use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

fn le16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn le32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn res_entry(out: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    le32(out, data.len() as u32);
    le32(out, 32);
    le16(out, 0xFFFF);
    le16(out, kind);
    le16(out, 0xFFFF);
    le16(out, name);
    le32(out, 0);
    le16(out, flags);
    le16(out, if kind == 0 { 0 } else { 0x0409 });
    le32(out, 0);
    le32(out, 0);
    out.extend_from_slice(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn icon_res(ico: &[u8]) -> Vec<u8> {
    let u16_at = |i: usize| u16::from_le_bytes([ico[i], ico[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([ico[i], ico[i + 1], ico[i + 2], ico[i + 3]]);
    assert!(u16_at(0) == 0 && u16_at(2) == 1, "packaging/icons/discord-phoenix.ico is not an icon file");
    let count = u16_at(4);
    let mut out = Vec::new();
    res_entry(&mut out, 0, 0, 0, &[]);
    let mut group = Vec::new();
    le16(&mut group, 0);
    le16(&mut group, 1);
    le16(&mut group, count);
    for i in 0..count {
        let e = 6 + i as usize * 16;
        let (len, off) = (u32_at(e + 8) as usize, u32_at(e + 12) as usize);
        res_entry(&mut out, 3, i + 1, 0x1010, &ico[off..off + len]);
        group.extend_from_slice(&ico[e..e + 12]);
        le16(&mut group, i + 1);
    }
    res_entry(&mut out, 14, 1, 0x1030, &group);
    out
}

fn windows_icon() {
    let ico = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/icons/discord-phoenix.ico");
    println!("cargo:rerun-if-changed={ico}");
    let Ok(bytes) = std::fs::read(ico) else { return };
    let Ok(out_dir) = std::env::var("OUT_DIR") else { return };
    let res = std::path::Path::new(&out_dir).join("discord-phoenix.res");
    std::fs::write(&res, icon_res(&bytes)).expect("write the Windows icon resource");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        if std::env::var("PROFILE").as_deref() == Ok("debug") {
            println!("cargo:rustc-link-arg-bin=discord-phoenix=/STACK:33554432");
        }
        println!("cargo:rustc-link-arg-bin=discord-phoenix={}", res.display());
    }
}

fn main() {
    windows_icon();
    println!("cargo:rerun-if-env-changed=PHOENIX_COMMIT");
    for p in ["HEAD", "logs/HEAD", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--path-format=absolute", "--git-path", p]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    let commit = std::env::var("PHOENIX_COMMIT")
        .ok()
        .filter(|c| !c.trim().is_empty())
        .or_else(|| git(&["rev-parse", "--short=8", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=PHOENIX_COMMIT={commit}");
}
