use reqwest::Method;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{Channel, ChannelId};

pub const ANNOUNCEMENT_THREAD: u8 = 10;
pub const PUBLIC_THREAD: u8 = 11;
pub const PRIVATE_THREAD: u8 = 12;

pub fn thread_type(private: bool, parent_is_announcement: bool) -> u8 {
    match (private, parent_is_announcement) {
        (true, _) => PRIVATE_THREAD,
        (false, true) => ANNOUNCEMENT_THREAD,
        (false, false) => PUBLIC_THREAD,
    }
}

pub fn standalone_thread_body(name: &str, kind: u8, auto_archive_minutes: u32) -> Value {
    let name: String = name.trim().chars().take(100).collect();
    json!({
        "name": if name.is_empty() { "New Thread".to_owned() } else { name },
        "type": kind,
        "auto_archive_duration": auto_archive_minutes,
        "location": "Thread Browser Toolbar",
    })
}

impl RestClient {
    pub async fn create_standalone_thread(&self, channel: ChannelId, name: &str, kind: u8, auto_archive_minutes: u32) -> Result<Channel> {
        self.call(
            Method::POST,
            &format!("/channels/{channel}/threads"),
            Body::Json(standalone_thread_body(name, kind, auto_archive_minutes)),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Snowflake;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn types_follow_the_client() {
        assert_eq!(thread_type(false, false), 11);
        assert_eq!(thread_type(false, true), 10);
        assert_eq!(thread_type(true, true), 12);
    }

    #[test]
    fn body_trims_and_defaults_the_name() {
        let b = standalone_thread_body("  release plans  ", PUBLIC_THREAD, 4320);
        assert_eq!(b["name"], "release plans");
        assert_eq!(b["type"], 11);
        assert_eq!(b["auto_archive_duration"], 4320);
        assert_eq!(standalone_thread_body(" ", PRIVATE_THREAD, 60)["name"], "New Thread");
        assert_eq!(standalone_thread_body(&"x".repeat(150), 11, 60)["name"].as_str().unwrap().len(), 100);
    }

    #[tokio::test]
    async fn posts_to_the_threads_endpoint() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = sock.read(&mut chunk).await.unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf);
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                        .unwrap_or(0);
                    if buf.len() >= h + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let body = r#"{"id":"42","type":11,"name":"plans","guild_id":"1","parent_id":"7"}"#;
            let resp = format!("HTTP/1.1 201 Created\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
            sock.write_all(resp.as_bytes()).await.unwrap();
            String::from_utf8(buf).unwrap()
        });
        let rest = RestClient::with_base(Some("test-token".into()), crate::ClientProperties::linux_desktop(), format!("http://{addr}"));
        let ch = rest.create_standalone_thread(Snowflake(7), "plans", PUBLIC_THREAD, 4320).await.unwrap();
        assert_eq!(ch.id, Snowflake(42));
        let req = server.await.unwrap();
        assert!(req.starts_with("POST /channels/7/threads "), "{req}");
        let json: Value = serde_json::from_str(&req[req.find("\r\n\r\n").unwrap() + 4..]).unwrap();
        assert_eq!(json["name"], "plans");
        assert_eq!(json["type"], 11);
    }
}
