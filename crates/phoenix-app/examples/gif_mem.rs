use image::AnimationDecoder;

fn pss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/smaps_rollup")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Pss:"))?.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (dir, prefix) = (args.next().expect("dir"), args.next().unwrap_or_default());
    let before = pss_kib();
    let mut held = Vec::new();
    let (mut files, mut file_bytes, mut decoded) = (0, 0u64, 0u64);
    let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) || !name.ends_with(".gif") {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let Ok(dec) = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&bytes)) else { continue };
        let frames: Vec<_> = dec.into_frames().flatten().collect();
        decoded += frames.iter().map(|f| f.buffer().len() as u64).sum::<u64>();
        file_bytes += bytes.len() as u64;
        files += 1;
        held.push(frames);
    }
    let after = pss_kib();
    println!(
        "prefix={prefix} gifs={files} file_mib={:.1} decoded_mib={:.1} pss_delta_mib={:.1}",
        file_bytes as f64 / 1048576.,
        decoded as f64 / 1048576.,
        after.saturating_sub(before) as f64 / 1024.
    );
    drop(held);
}
