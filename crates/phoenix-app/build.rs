use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

fn main() {
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
