use std::sync::Arc;

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Body, CreateMessage, FileUpload, RestClient, fill_nonce};
use crate::error::{ApiErrorBody, Error, Result};
use crate::models::{ChannelId, Message};

pub type Progress = Arc<dyn Fn(u64, u64) -> bool + Send + Sync>;

pub const UPLOAD_CANCELLED: &str = "upload cancelled";

pub const UPLOAD_CHUNK: usize = 4 << 20;

pub const ADAPTIVE_CHUNK: usize = usize::MAX;

const CHUNK_UNIT: usize = 256 << 10;

const PROGRESS_STEPS: usize = 16;

pub fn adaptive_chunk(len: usize) -> usize {
    (len.div_ceil(PROGRESS_STEPS).div_ceil(CHUNK_UNIT) * CHUNK_UNIT).clamp(CHUNK_UNIT, UPLOAD_CHUNK)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CloudUpload {
    #[serde(default, deserialize_with = "id_string")]
    pub id: String,
    pub upload_url: String,
    pub upload_filename: String,
}

fn id_string<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    })
}

const MIB: u64 = 1 << 20;
pub const MAX_ATTACHMENTS: usize = 10;
pub const MAX_TOTAL_SIZE: u64 = 500 * MIB;

pub fn max_file_size<'a>(premium_type: Option<u8>, guild: Option<(Option<u8>, impl IntoIterator<Item = &'a str>)>) -> u64 {
    let user = match premium_type {
        Some(1) | Some(3) => 50 * MIB,
        Some(2) => 500 * MIB,
        _ => 10 * MIB,
    };
    let guild = guild.map_or(10 * MIB, |(tier, features)| {
        let mut g = match tier {
            Some(3) => 100 * MIB,
            Some(2) => 50 * MIB,
            _ => 10 * MIB,
        };
        for f in features {
            match f {
                "MAX_FILE_SIZE_100_MB" => g = g.max(100 * MIB),
                "MAX_FILE_SIZE_50_MB" => g = g.max(50 * MIB),
                _ => {}
            }
        }
        g
    });
    user.max(guild)
}

pub fn message_upload_limit(file_limit: u64) -> u64 {
    file_limit.max(20 * MIB)
}

pub fn content_type_for(filename: &str) -> Option<&'static str> {
    let ext = filename.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" | "jfif" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "7z" => "application/x-7z-compressed",
        "gz" => "application/gzip",
        "json" => "application/json",
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "rs" => "text/rust",
        _ => return None,
    })
}

fn content_type(f: &FileUpload) -> Option<String> {
    f.content_type.clone().or_else(|| content_type_for(&f.filename).map(str::to_owned))
}

pub const FLAG_IS_COMPONENTS_V2: u64 = 1 << 15;

pub fn unique_filenames(files: &mut [FileUpload]) {
    let mut taken: std::collections::HashSet<String> = files.iter().map(|f| f.filename.clone()).collect();
    let mut seen = std::collections::HashSet::new();
    for f in files.iter_mut() {
        if seen.insert(f.filename.clone()) {
            continue;
        }
        let (stem, ext) = match f.filename.rfind('.') {
            Some(i) if i > 0 => f.filename.split_at(i),
            _ => (f.filename.as_str(), ""),
        };
        let mut n = 1;
        let mut name = format!("{stem}_{n}{ext}");
        while taken.contains(&name) {
            n += 1;
            name = format!("{stem}_{n}{ext}");
        }
        taken.insert(name.clone());
        seen.insert(name.clone());
        f.filename = name;
    }
}

fn is_media(f: &FileUpload) -> bool {
    content_type(f).is_some_and(|t| t.starts_with("image/") || t.starts_with("video/"))
}

fn gallery_item(f: &FileUpload) -> Value {
    let mut item = json!({ "media": { "url": format!("attachment://{}", f.filename) }, "spoiler": f.spoiler });
    if let Some(d) = f.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        item["description"] = json!(d);
    }
    item
}

pub fn announcement_components(content: &str, files: &[FileUpload], hero: usize) -> Vec<Value> {
    let Some(header) = files.get(hero) else { return Vec::new() };
    let mut out = vec![json!({ "type": 12, "id": 82733, "items": [{ "media": { "url": format!("attachment://{}", header.filename) }, "spoiler": false }] })];
    if !content.is_empty() {
        out.push(json!({ "type": 10, "id": 82744, "content": content }));
    }
    let rest: Vec<&FileUpload> = files.iter().enumerate().filter(|(i, _)| *i != hero).map(|(_, f)| f).collect();
    let media: Vec<Value> = rest.iter().copied().filter(|f| is_media(f)).map(gallery_item).collect();
    if !media.is_empty() {
        out.push(json!({ "type": 12, "id": 82755, "items": media }));
    }
    for (t, f) in rest.iter().copied().filter(|f| !is_media(f)).enumerate() {
        out.push(json!({ "type": 13, "id": 82766 + t, "file": { "url": format!("attachment://{}", f.filename) }, "spoiler": f.spoiler }));
    }
    out
}

pub fn attachment_slots_body(files: &[FileUpload]) -> Value {
    let slots: Vec<Value> = files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut s = json!({
                "filename": f.filename,
                "file_size": f.bytes.len(),
                "id": i.to_string(),
                "is_clip": false,
            });
            if let Some(ct) = content_type(f) {
                s["original_content_type"] = json!(ct);
            }
            s
        })
        .collect();
    json!({ "files": slots })
}

pub fn message_attachments(files: &[FileUpload], uploaded: &[CloudUpload]) -> Value {
    Value::Array(
        files
            .iter()
            .zip(uploaded)
            .enumerate()
            .map(|(i, (f, u))| {
                let mut a = json!({
                    "id": i.to_string(),
                    "filename": f.filename,
                    "uploaded_filename": u.upload_filename,
                });
                if let Some(ct) = content_type(f) {
                    a["original_content_type"] = json!(ct);
                }
                if let Some(d) = f.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
                    a["description"] = json!(d);
                }
                if f.spoiler {
                    a["is_spoiler"] = json!(true);
                }
                if let Some(d) = f.duration_secs {
                    a["duration_secs"] = json!(d);
                }
                if let Some(w) = &f.waveform {
                    a["waveform"] = json!(w);
                }
                a
            })
            .collect(),
    )
}

pub fn chunk_ranges(len: usize, chunk: usize) -> Vec<Option<(usize, usize, String)>> {
    if len <= chunk || chunk == 0 {
        return vec![None];
    }
    (0..len)
        .step_by(chunk)
        .map(|start| {
            let end = (start + chunk).min(len) - 1;
            Some((start, end, format!("bytes {start}-{end}/{len}")))
        })
        .collect()
}

pub fn upload_url_allowed(url: &str, base: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else { return false };
    if u.scheme() == "https"
        && u.host_str().is_some_and(|h| h == "storage.googleapis.com" || h.ends_with(".storage.googleapis.com"))
    {
        return true;
    }
    url::Url::parse(base).is_ok_and(|b| b.origin() == u.origin())
}

fn put_error(status: StatusCode, bytes: &[u8]) -> Error {
    let raw = String::from_utf8_lossy(bytes).chars().take(300).collect::<String>();
    Error::Api {
        status: status.as_u16(),
        body: Box::new(ApiErrorBody { message: "attachment upload failed".into(), ..Default::default() }),
        raw,
    }
}

impl RestClient {
    pub async fn request_upload_urls(&self, channel: ChannelId, files: &[FileUpload]) -> Result<Vec<CloudUpload>> {
        #[derive(Deserialize)]
        struct Resp {
            attachments: Vec<CloudUpload>,
        }
        let r: Resp = self
            .call(Method::POST, &format!("/channels/{channel}/attachments"), Body::Json(attachment_slots_body(files)))
            .await?;
        let mut out = Vec::with_capacity(files.len());
        for (i, _) in files.iter().enumerate() {
            let id = i.to_string();
            let slot = r.attachments.iter().find(|a| a.id == id).or_else(|| r.attachments.get(i));
            out.push(slot.cloned().ok_or_else(|| Error::Protocol(format!("no upload slot for attachment {i}")))?);
        }
        Ok(out)
    }

    pub async fn put_upload(&self, url: &str, bytes: &[u8], chunk: usize, on_sent: &(dyn Fn(u64) -> bool + Send + Sync)) -> Result<()> {
        if !upload_url_allowed(url, &self.inner.base) {
            return Err(Error::Protocol("upload URL outside Cloud Storage".into()));
        }
        let http = reqwest::Client::builder()
            .user_agent(self.inner.props.user_agent())
            .use_rustls_tls()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        for range in chunk_ranges(bytes.len(), chunk) {
            let (part, header) = match &range {
                None => (bytes, None),
                Some((s, e, h)) => (&bytes[*s..=*e], Some(h.clone())),
            };
            let mut req = http
                .put(url)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .header(reqwest::header::CONTENT_LENGTH, part.len())
                .body(part.to_vec());
            if let Some(h) = header {
                req = req.header(reqwest::header::CONTENT_RANGE, h);
            }
            let resp = req.send().await?;
            let status = resp.status();
            let last = range.as_ref().is_none_or(|(_, e, _)| e + 1 == bytes.len());
            let ok = status.is_success() || (status.as_u16() == 308 && !last);
            if !ok {
                let body = resp.bytes().await.unwrap_or_default();
                return Err(put_error(status, &body));
            }
            if !on_sent(range.as_ref().map_or(bytes.len(), |(_, e, _)| e + 1) as u64) {
                return Err(Error::Protocol(UPLOAD_CANCELLED.into()));
            }
        }
        Ok(())
    }

    pub async fn send_message_with_uploads(
        &self,
        channel: ChannelId,
        mut msg: CreateMessage,
        files: Vec<FileUpload>,
        progress: Option<Progress>,
    ) -> Result<Message> {
        self.send_message_with_uploads_chunked(channel, &mut msg, files, progress, UPLOAD_CHUNK).await
    }

    pub async fn upload_files(
        &self,
        channel: ChannelId,
        files: &[FileUpload],
        progress: Option<Progress>,
        chunk: usize,
    ) -> Result<Vec<CloudUpload>> {
        let total: u64 = files.iter().map(|f| f.bytes.len() as u64).sum();
        let report = |sent: u64| progress.as_ref().is_none_or(|p| p(sent.min(total), total));
        if !report(0) {
            return Err(Error::Protocol(UPLOAD_CANCELLED.into()));
        }
        let slots = self.request_upload_urls(channel, files).await?;
        let mut done = 0u64;
        for (f, slot) in files.iter().zip(&slots) {
            let base = done;
            let chunk = if chunk == ADAPTIVE_CHUNK { adaptive_chunk(f.bytes.len()) } else { chunk };
            self.put_upload(&slot.upload_url, &f.bytes, chunk, &|n| report(base + n)).await?;
            done += f.bytes.len() as u64;
        }
        Ok(slots)
    }

    pub async fn upload_one(&self, channel: ChannelId, file: &FileUpload, on_sent: &(dyn Fn(u64) -> bool + Send + Sync)) -> Result<CloudUpload> {
        let slot = self.request_upload_urls(channel, std::slice::from_ref(file)).await?.remove(0);
        self.put_upload(&slot.upload_url, &file.bytes, adaptive_chunk(file.bytes.len()), on_sent).await?;
        Ok(slot)
    }

    pub async fn send_message_with_uploaded(&self, channel: ChannelId, msg: &mut CreateMessage, files: &[FileUpload], slots: &[CloudUpload]) -> Result<Message> {
        fill_nonce(msg);
        msg.attachments = message_attachments(files, slots).as_array().cloned().unwrap_or_default();
        let body = serde_json::to_value(&*msg)?;
        self.call(Method::POST, &format!("/channels/{channel}/messages"), Body::Json(body)).await
    }

    #[doc(hidden)]
    pub async fn send_message_with_uploads_chunked(
        &self,
        channel: ChannelId,
        msg: &mut CreateMessage,
        files: Vec<FileUpload>,
        progress: Option<Progress>,
        chunk: usize,
    ) -> Result<Message> {
        let slots = self.upload_files(channel, &files, progress, chunk).await?;
        fill_nonce(msg);
        msg.attachments = message_attachments(&files, &slots).as_array().cloned().unwrap_or_default();
        let body = serde_json::to_value(&*msg)?;
        self.call(Method::POST, &format!("/channels/{channel}/messages"), Body::Json(body)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Snowflake;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn file(name: &str, len: usize) -> FileUpload {
        FileUpload { filename: name.into(), bytes: vec![7; len], ..Default::default() }
    }

    #[test]
    fn duplicate_filenames_get_a_suffix_like_the_client() {
        let mut files = vec![file("a.png", 1), file("a.png", 1), file("a_1.png", 1), file("a.png", 1), file("noext", 1), file("noext", 1)];
        unique_filenames(&mut files);
        let names: Vec<&str> = files.iter().map(|f| f.filename.as_str()).collect();
        assert_eq!(names, ["a.png", "a_2.png", "a_1.png", "a_3.png", "noext", "noext_1"]);
    }

    #[test]
    fn announcement_header_image_is_a_media_gallery() {
        let mut spoiler = file("b.mp4", 1);
        spoiler.spoiler = true;
        spoiler.description = Some(" alt ".into());
        let files = [file("x.png", 1), file("hero.jpg", 1), spoiler, file("doc.pdf", 1), file("raw.bin", 1)];
        let c = announcement_components("# Title\nbody", &files, 1);
        assert_eq!(
            Value::Array(c),
            json!([
                { "type": 12, "id": 82733, "items": [{ "media": { "url": "attachment://hero.jpg" }, "spoiler": false }] },
                { "type": 10, "id": 82744, "content": "# Title\nbody" },
                { "type": 12, "id": 82755, "items": [
                    { "media": { "url": "attachment://x.png" }, "spoiler": false },
                    { "media": { "url": "attachment://b.mp4" }, "spoiler": true, "description": "alt" },
                ] },
                { "type": 13, "id": 82766, "file": { "url": "attachment://doc.pdf" }, "spoiler": false },
                { "type": 13, "id": 82767, "file": { "url": "attachment://raw.bin" }, "spoiler": false },
            ])
        );
        assert_eq!(announcement_components("", &files[1..2], 0).len(), 1);
    }

    #[test]
    fn slots_body_matches_the_client() {
        let b = attachment_slots_body(&[file("shot.png", 667), file("notes.bin", 3)]);
        assert_eq!(
            b,
            json!({"files":[
                {"filename":"shot.png","file_size":667,"id":"0","is_clip":false,"original_content_type":"image/png"},
                {"filename":"notes.bin","file_size":3,"id":"1","is_clip":false}
            ]})
        );
    }

    #[test]
    fn message_attachments_carry_spoiler_and_alt_text() {
        let mut a = file("cat.JPG", 10);
        a.spoiler = true;
        a.description = Some("  a cat  ".into());
        let b = file("b.txt", 1);
        let up = |n: &str| CloudUpload { id: String::new(), upload_url: String::new(), upload_filename: n.into() };
        let v = message_attachments(&[a, b], &[up("u1/cat.JPG"), up("u2/b.txt")]);
        assert_eq!(
            v,
            json!([
                {"id":"0","filename":"cat.JPG","uploaded_filename":"u1/cat.JPG","original_content_type":"image/jpeg","description":"a cat","is_spoiler":true},
                {"id":"1","filename":"b.txt","uploaded_filename":"u2/b.txt","original_content_type":"text/plain"}
            ])
        );
    }

    #[test]
    fn voice_message_attachment_carries_duration_and_waveform() {
        let mut a = file("voice-message.ogg", 10);
        a.content_type = Some("audio/ogg".into());
        a.duration_secs = Some(3.52);
        a.waveform = Some("AAEC".into());
        let up = CloudUpload { id: "0".into(), upload_url: String::new(), upload_filename: "u/voice-message.ogg".into() };
        assert_eq!(
            message_attachments(&[a], &[up]),
            json!([{"id":"0","filename":"voice-message.ogg","uploaded_filename":"u/voice-message.ogg","original_content_type":"audio/ogg","duration_secs":3.52,"waveform":"AAEC"}])
        );
    }

    #[test]
    fn size_limits_follow_nitro_and_boosts() {
        let none: Option<(Option<u8>, Vec<&str>)> = None;
        assert_eq!(max_file_size(None, none.clone()), 10 * MIB);
        assert_eq!(max_file_size(Some(2), none.clone()), 500 * MIB);
        assert_eq!(max_file_size(Some(3), none.clone()), 50 * MIB);
        assert_eq!(max_file_size(Some(0), Some((Some(2), vec![]))), 50 * MIB);
        assert_eq!(max_file_size(None, Some((Some(1), vec!["MAX_FILE_SIZE_100_MB"]))), 100 * MIB);
        assert_eq!(max_file_size(Some(2), Some((Some(3), vec![]))), 500 * MIB);
        assert_eq!(message_upload_limit(10 * MIB), 20 * MIB);
        assert_eq!(message_upload_limit(500 * MIB), 500 * MIB);
    }

    #[test]
    fn adaptive_chunks_move_the_bar_under_4_mib() {
        assert_eq!(adaptive_chunk(100), 256 << 10);
        assert_eq!(chunk_ranges(100 << 10, adaptive_chunk(100 << 10)).len(), 1);
        assert_eq!(chunk_ranges(1 << 20, adaptive_chunk(1 << 20)).len(), 4);
        assert_eq!(chunk_ranges(4 << 20, adaptive_chunk(4 << 20)).len(), 16);
        assert_eq!(adaptive_chunk(100 << 20), UPLOAD_CHUNK);
        assert_eq!(adaptive_chunk(10 << 20) % (256 << 10), 0);
    }

    #[test]
    fn chunking() {
        assert_eq!(chunk_ranges(10, 16), vec![None]);
        assert_eq!(chunk_ranges(16, 16), vec![None]);
        let r = chunk_ranges(40, 16);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].as_ref().unwrap().2, "bytes 0-15/40");
        assert_eq!(r[2].as_ref().unwrap().2, "bytes 32-39/40");
    }

    #[test]
    fn upload_hosts() {
        let base = "https://discord.com/api/v10";
        assert!(upload_url_allowed("https://discord-attachments-uploads-prd.storage.googleapis.com/x/y.png?sig=1", base));
        assert!(!upload_url_allowed("http://discord-attachments-uploads-prd.storage.googleapis.com/x", base));
        assert!(!upload_url_allowed("https://evil.example/storage.googleapis.com", base));
        assert!(upload_url_allowed("http://127.0.0.1:9/up", "http://127.0.0.1:9"));
        assert!(!upload_url_allowed("http://127.0.0.1:10/up", "http://127.0.0.1:9"));
    }

    async fn serve_one(listener: &tokio::net::TcpListener, status: &str, extra: &str, body: &str) -> (String, Vec<u8>) {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 65536];
        let (head, len) = loop {
            let n = sock.read(&mut chunk).await.unwrap();
            buf.extend_from_slice(&chunk[..n]);
            if let Some(h) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buf[..h]).into_owned();
                let len = head
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                    .unwrap_or(0);
                if buf.len() >= h + 4 + len {
                    break (head, len);
                }
            }
            assert!(n > 0, "connection closed early");
        };
        let start = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let req_body = buf[start..start + len].to_vec();
        let resp = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        sock.write_all(resp.as_bytes()).await.unwrap();
        (head, req_body)
    }

    #[tokio::test]
    async fn upload_flow_order_headers_and_bodies() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let slots = format!(
                r#"{{"attachments":[{{"id":0,"upload_url":"http://{addr}/up/abc/pic.png?sig=x","upload_filename":"abc/pic.png"}}]}}"#
            );
            let a = serve_one(&listener, "200 OK", "", &slots).await;
            let p1 = serve_one(&listener, "308 Resume Incomplete", "range: bytes=0-15\r\n", "").await;
            let p2 = serve_one(&listener, "200 OK", "", "").await;
            let msg = r#"{"id":"99","channel_id":"7","content":"hi","author":{"id":"1","username":"me"},"timestamp":"2026-11-24T15:00:00+00:00","attachments":[]}"#;
            let m = serve_one(&listener, "200 OK", "", msg).await;
            vec![a, p1, p2, m]
        });
        let rest = RestClient::with_base(Some("test-token".into()), crate::ClientProperties::linux_desktop(), format!("http://{addr}"));
        let seen = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let s2 = seen.clone();
        let progress: Progress = Arc::new(move |a, b| {
            s2.lock().push((a, b));
            true
        });
        let mut f = file("pic.png", 20);
        f.spoiler = true;
        let mut msg = CreateMessage::text("hi");
        let m = rest.send_message_with_uploads_chunked(Snowflake(7), &mut msg, vec![f], Some(progress), 16).await.unwrap();
        assert_eq!(m.id, Snowflake(99));
        let reqs = server.await.unwrap();

        let (head, body) = &reqs[0];
        assert!(head.starts_with("POST /channels/7/attachments "), "{head}");
        assert!(head.to_ascii_lowercase().contains("authorization: test-token"));
        let v: Value = serde_json::from_slice(body).unwrap();
        assert_eq!(v["files"][0]["file_size"], 20);
        assert_eq!(v["files"][0]["original_content_type"], "image/png");

        for (i, range) in [(1, "bytes 0-15/20"), (2, "bytes 16-19/20")] {
            let (head, body) = &reqs[i];
            let lower = head.to_ascii_lowercase();
            assert!(head.starts_with("PUT /up/abc/pic.png?sig=x "), "{head}");
            assert!(lower.contains("content-type: application/octet-stream"));
            assert!(lower.contains(&format!("content-range: {range}")), "{head}");
            assert!(!lower.contains("authorization"), "the PUT must not carry the token");
            assert!(!lower.contains("x-super-properties"));
            assert_eq!(body.len(), if i == 1 { 16 } else { 4 });
        }

        let (head, body) = &reqs[3];
        assert!(head.starts_with("POST /channels/7/messages "), "{head}");
        assert!(head.to_ascii_lowercase().contains("content-type: application/json"));
        let v: Value = serde_json::from_slice(body).unwrap();
        assert_eq!(v["content"], "hi");
        assert!(v["nonce"].is_string());
        assert_eq!(
            v["attachments"],
            json!([{"id":"0","filename":"pic.png","uploaded_filename":"abc/pic.png","original_content_type":"image/png","is_spoiler":true}])
        );
        assert_eq!(*seen.lock(), vec![(0, 20), (16, 20), (20, 20)]);
    }

    #[tokio::test]
    async fn failed_put_posts_no_message() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let slots = format!(r#"{{"attachments":[{{"id":"0","upload_url":"http://{addr}/up/x","upload_filename":"x"}}]}}"#);
            serve_one(&listener, "200 OK", "", &slots).await;
            serve_one(&listener, "403 Forbidden", "", "denied").await;
            tokio::time::timeout(std::time::Duration::from_millis(300), listener.accept()).await.is_err()
        });
        let rest = RestClient::with_base(Some("t".into()), crate::ClientProperties::linux_desktop(), format!("http://{addr}"));
        let err = rest.send_message_with_uploads(Snowflake(7), CreateMessage::text(""), vec![file("a.png", 3)], None).await;
        assert!(matches!(err, Err(Error::Api { status: 403, .. })));
        assert!(server.await.unwrap(), "a message was posted after a failed upload");
    }

    #[tokio::test]
    async fn cancel_stops_before_the_message() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let slots = format!(r#"{{"attachments":[{{"id":"0","upload_url":"http://{addr}/up/x","upload_filename":"x"}}]}}"#);
            serve_one(&listener, "200 OK", "", &slots).await;
            serve_one(&listener, "308 Resume Incomplete", "", "").await;
            tokio::time::timeout(std::time::Duration::from_millis(300), listener.accept()).await.is_err()
        });
        let rest = RestClient::with_base(Some("t".into()), crate::ClientProperties::linux_desktop(), format!("http://{addr}"));
        let progress: Progress = Arc::new(|sent, _| sent == 0);
        let mut msg = CreateMessage::text("");
        let err = rest.send_message_with_uploads_chunked(Snowflake(7), &mut msg, vec![file("a.png", 40)], Some(progress), 16).await;
        assert!(matches!(err, Err(Error::Protocol(ref m)) if m == UPLOAD_CANCELLED));
        assert!(server.await.unwrap(), "something was sent after the cancel");
    }

    #[test]
    fn rejects_foreign_upload_urls() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let rest = RestClient::with_base(Some("t".into()), crate::ClientProperties::linux_desktop(), "http://127.0.0.1:9");
        let r = rt.block_on(rest.put_upload("https://example.com/x", b"abc", UPLOAD_CHUNK, &|_| true));
        assert!(matches!(r, Err(Error::Protocol(_))));
    }
}
