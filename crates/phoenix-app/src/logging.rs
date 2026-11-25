use std::borrow::Cow;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use regex::Regex;
use tracing_subscriber::{EnvFilter, fmt::MakeWriter, layer::SubscriberExt, util::SubscriberInitExt};

pub const FILE_NAME: &str = "phoenix.log";
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;
pub const KEEP: usize = 3;
pub const DEFAULT_FILTER: &str = "info";

pub fn log_dir() -> Option<PathBuf> {
    Some(crate::paths::cache_dir()?.join("logs"))
}

pub fn log_file() -> Option<PathBuf> {
    Some(log_dir()?.join(FILE_NAME))
}

const MASK: &str = "[redacted]";

static RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?i)\b((?:set-)?cookie|authorization)\b(\s*[:=]\s*).*", "$1$2[redacted]"),
        (r"(?i)\b(bearer)(\s+)[A-Za-z0-9._~+/=-]{8,}", "$1$2[redacted]"),
        (
            r#"(?i)("?\b(?:token|access_token|refresh_token|password|new_password|ticket|captcha_key|fingerprint|x-fingerprint|x-super-properties|secret|session_id)\b"?)(\s*[:=]\s*)("(?:[^"\\]|\\.)*"|'[^']*'|[^\s,;}\])]+)"#,
            "$1$2\"[redacted]\"",
        ),
        (
            r#"(?i)("?\b(?:content|body|text|caption|description|bio|topic|query|q|nick|global_name|username|display_name|phone|email)\b"?)(\s*[:=]\s*)("(?:[^"\\]|\\.)*"|'[^']*')"#,
            "$1$2\"[redacted]\"",
        ),
        (r"mfa\.[A-Za-z0-9_-]{20,}", MASK),
        (r"[A-Za-z0-9_-]{23,28}\.[A-Za-z0-9_-]{6,7}\.[A-Za-z0-9_-]{27,}", MASK),
        (r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}", "[email]"),
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(p).expect("valid redaction rule"), r))
    .collect()
});

pub fn redact(line: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(line);
    for (re, rep) in RULES.iter() {
        if re.is_match(&out) {
            out = Cow::Owned(re.replace_all(&out, *rep).into_owned());
        }
    }
    out
}

pub struct Rotating {
    dir: PathBuf,
    max: u64,
    keep: usize,
    file: Option<File>,
    len: u64,
}

impl Rotating {
    pub fn open(dir: &Path, max: u64, keep: usize) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let mut r = Self { dir: dir.to_path_buf(), max, keep: keep.max(1), file: None, len: 0 };
        r.reopen()?;
        Ok(r)
    }

    fn path(&self, n: usize) -> PathBuf {
        if n == 0 { self.dir.join(FILE_NAME) } else { self.dir.join(format!("{FILE_NAME}.{n}")) }
    }

    fn reopen(&mut self) -> io::Result<()> {
        let f = OpenOptions::new().create(true).append(true).open(self.path(0))?;
        self.len = f.metadata().map(|m| m.len()).unwrap_or(0);
        self.file = Some(f);
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file = None;
        let _ = std::fs::remove_file(self.path(self.keep - 1));
        for n in (0..self.keep - 1).rev() {
            let from = self.path(n);
            if from.exists() {
                std::fs::rename(&from, self.path(n + 1))?;
            }
        }
        self.reopen()
    }

    pub fn write_line(&mut self, buf: &[u8]) -> io::Result<()> {
        if self.len > 0 && self.len + buf.len() as u64 > self.max {
            self.rotate()?;
        }
        let take = buf.len().min(self.max as usize);
        if let Some(f) = self.file.as_mut() {
            f.write_all(&buf[..take])?;
            self.len += take as u64;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Sink {
    file: Option<Arc<Mutex<Rotating>>>,
    stderr: bool,
}

struct SinkWriter(Sink);

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let clean = redact(&text);
        if let Some(f) = &self.0.file
            && let Ok(mut f) = f.lock()
        {
            let _ = f.write_line(clean.as_bytes());
        }
        if self.0.stderr {
            let _ = io::stderr().write_all(clean.as_bytes());
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Sink {
    type Writer = SinkWriter;
    fn make_writer(&'a self) -> Self::Writer {
        SinkWriter(self.clone())
    }
}

pub fn filter_from(var: Option<&str>) -> EnvFilter {
    var.filter(|v| !v.trim().is_empty())
        .and_then(|v| EnvFilter::try_new(v).ok())
        .unwrap_or_else(|| EnvFilter::new(DEFAULT_FILTER))
}

static INIT: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn init() -> Option<PathBuf> {
    INIT.get_or_init(|| {
        let file = log_dir().and_then(|d| match Rotating::open(&d, MAX_BYTES, KEEP) {
            Ok(r) => Some(Arc::new(Mutex::new(r))),
            Err(e) => {
                eprintln!("discord-phoenix: log file unavailable: {e}");
                None
            }
        });
        let path = file.as_ref().and_then(|_| log_file());
        let sink = Sink { file, stderr: true };
        let layer = tracing_subscriber::fmt::layer().with_ansi(false).with_target(true).with_writer(sink);
        let ok = tracing_subscriber::registry()
            .with(filter_from(std::env::var("RUST_LOG").ok().as_deref()))
            .with(layer)
            .try_init()
            .is_ok();
        if ok {
            install_panic_hook();
            tracing::info!(version = crate::diagnostics::VERSION, commit = crate::diagnostics::COMMIT, "Discord Phoenix starting");
        }
        path
    })
    .clone()
}

fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        tracing::error!(target: "panic", location = %at, "{msg}\n{}", std::backtrace::Backtrace::force_capture());
        prev(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_token() -> String {
        format!("{}.{}.{}", "Q".repeat(26), "x".repeat(6), "z9".repeat(19))
    }

    #[test]
    fn redacts_tokens_everywhere() {
        let token = fake_token();
        let token = token.as_str();
        for line in [
            format!("token={token}"),
            format!("Authorization: {token}"),
            format!("{{\"token\":\"{token}\",\"properties\":{{}}}}"),
            format!("connecting with {token} now"),
            format!("Authorization: Bot {token}"),
            "Bearer abcDEF123456789xyz".to_owned(),
            "mfa.abcdefghijklmnopqrstuvwxyz0123456789".to_owned(),
        ] {
            let out = redact(&line);
            assert!(!out.contains(token), "{out}");
            assert!(!out.contains("abcDEF123456789xyz"), "{out}");
            assert!(!out.contains("mfa.abc"), "{out}");
            assert!(out.contains(MASK), "{out}");
        }
    }

    #[test]
    fn redacts_cookies_emails_and_message_content() {
        let out = redact("set-cookie: __dcfduid=abc123; __sdcfduid=def456; Path=/");
        assert!(!out.contains("abc123") && !out.contains("def456"), "{out}");
        let out = redact("login failed for someone.private+x@example.co.uk");
        assert!(!out.contains("someone.private") && out.contains("[email]"), "{out}");
        let out = redact(r#"dispatch MESSAGE_CREATE {"id":"1","content":"secret \"plans\" tonight","channel_id":"2"}"#);
        assert!(!out.contains("plans") && !out.contains("tonight"), "{out}");
        assert!(out.contains(r#""channel_id":"2""#), "{out}");
        let out = redact(r#"Message { id: 1, content: "hello there", nonce: None }"#);
        assert!(!out.contains("hello there"), "{out}");
        let out = redact("password=hunter2hunter2 ok");
        assert!(!out.contains("hunter2"), "{out}");
        let out = redact(r#"user {"email":"a@b.cd","phone":"+33 6 12 34 56 78"}"#);
        assert!(!out.contains("a@b.cd") && !out.contains("12 34"), "{out}");
    }

    #[test]
    fn leaves_ordinary_lines_alone() {
        for line in [
            "INFO phoenix_api::gateway: gateway connecting url=wss://gateway.discord.gg/?v=9&encoding=json resume=false",
            "WARN phoenix_voice::session: encoder unavailable, trying next codec kind=Video codec=AV1",
            "channel 1403447573389381652 opened in 12.5ms",
        ] {
            assert_eq!(redact(line), line);
        }
    }

    #[test]
    fn rotation_keeps_three_capped_files() {
        let dir = crate::paths::cache_dir().unwrap().join(format!("logs-rot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut r = Rotating::open(&dir, 100, 3).unwrap();
        for i in 0..40 {
            r.write_line(format!("line {i:03} ........\n").as_bytes()).unwrap();
        }
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["phoenix.log", "phoenix.log.1", "phoenix.log.2"]);
        for n in &names {
            assert!(std::fs::metadata(dir.join(n)).unwrap().len() <= 100);
        }
        let newest = std::fs::read_to_string(dir.join("phoenix.log")).unwrap();
        assert!(newest.contains("line 039"));
        drop(r);
        let mut again = Rotating::open(&dir, 100, 3).unwrap();
        again.write_line(b"after restart\n").unwrap();
        assert!(std::fs::read_to_string(dir.join("phoenix.log")).unwrap().contains("after restart"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_dir_is_inside_the_sandbox() {
        let root = phoenix_api::paths::sandbox_root().unwrap();
        let dir = log_dir().unwrap();
        assert!(dir.starts_with(&root), "{dir:?}");
        assert!(dir.ends_with("logs"));
    }

    #[test]
    fn filter_defaults_to_info() {
        use tracing_subscriber::filter::LevelFilter;
        assert_eq!(filter_from(None).max_level_hint(), Some(LevelFilter::INFO));
        assert_eq!(filter_from(Some("  ")).max_level_hint(), Some(LevelFilter::INFO));
        assert_eq!(filter_from(Some("debug")).max_level_hint(), Some(LevelFilter::DEBUG));
        assert_eq!(filter_from(Some("phoenix_api=trace,warn")).max_level_hint(), Some(LevelFilter::TRACE));
    }
}
