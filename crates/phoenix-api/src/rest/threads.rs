use reqwest::Method;
use serde::Deserialize;
use serde_json::Value;

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{Channel, ChannelId, null_default};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Public,
    JoinedPrivate,
}

pub(crate) fn archived_threads_path(channel: ChannelId, kind: ArchiveKind, before: Option<&str>, limit: u32) -> String {
    let base = match kind {
        ArchiveKind::Public => format!("/channels/{channel}/threads/archived/public"),
        ArchiveKind::JoinedPrivate => format!("/channels/{channel}/users/@me/threads/archived/private"),
    };
    let mut s = format!("{base}?limit={limit}");
    if let Some(b) = before {
        s.push_str("&before=");
        s.push_str(&urlencoding_min(b));
    }
    s
}

fn urlencoding_min(s: &str) -> String {
    s.replace('+', "%2B").replace(':', "%3A")
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ArchivedThreads {
    #[serde(default, deserialize_with = "null_default")]
    pub threads: Vec<Channel>,
    #[serde(default, deserialize_with = "null_default")]
    pub members: Vec<Value>,
    #[serde(default)]
    pub has_more: bool,
}

impl ArchivedThreads {
    pub fn cursor(&self) -> Option<&str> {
        self.threads.iter().filter_map(|t| t.thread_metadata.as_ref()?.archive_timestamp.as_deref()).min()
    }
}

pub fn merge_archived(mut a: Vec<Channel>, b: Vec<Channel>) -> Vec<Channel> {
    for t in b {
        if !a.iter().any(|x| x.id == t.id) {
            a.push(t);
        }
    }
    let ts = |c: &Channel| c.thread_metadata.as_ref().and_then(|m| m.archive_timestamp.clone()).unwrap_or_default();
    a.sort_by_key(|c| std::cmp::Reverse(ts(c)));
    a
}

impl RestClient {
    pub async fn archived_threads(&self, channel: ChannelId, kind: ArchiveKind, before: Option<&str>) -> Result<ArchivedThreads> {
        self.call(Method::GET, &archived_threads_path(channel, kind, before, 25), Body::None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Snowflake;

    #[test]
    fn member_preview_parses() {
        let a: ArchivedThreads = serde_json::from_str(
            r#"{"threads":[{"id":"7","type":11,"member_count":5,"member_ids_preview":["1","2","3"]},
              {"id":"8","type":11,"member_ids_preview":null}],"members":[],"has_more":false}"#,
        )
        .unwrap();
        assert_eq!(a.threads[0].member_count, Some(5));
        assert_eq!(a.threads[0].member_ids_preview, vec![Snowflake(1), Snowflake(2), Snowflake(3)]);
        assert!(a.threads[1].member_ids_preview.is_empty());
    }

    #[test]
    fn paths() {
        assert_eq!(archived_threads_path(Snowflake(5), ArchiveKind::Public, None, 25), "/channels/5/threads/archived/public?limit=25");
        assert_eq!(
            archived_threads_path(Snowflake(5), ArchiveKind::JoinedPrivate, Some("2025-06-28T10:00:00+00:00"), 25),
            "/channels/5/users/@me/threads/archived/private?limit=25&before=2025-06-28T10%3A00%3A00%2B00%3A00"
        );
    }

    #[test]
    fn parse_and_merge() {
        let p: ArchivedThreads = serde_json::from_str(
            r#"{"threads":[
              {"id":"1","type":11,"name":"old","thread_metadata":{"archived":true,"archive_timestamp":"2025-01-01T00:00:00+00:00"}},
              {"id":"2","type":11,"name":"new","thread_metadata":{"archived":true,"archive_timestamp":"2025-06-01T00:00:00+00:00"}}],
              "members":null,"has_more":true}"#,
        )
        .unwrap();
        assert!(p.has_more);
        assert_eq!(p.cursor(), Some("2025-01-01T00:00:00+00:00"));
        let q: ArchivedThreads = serde_json::from_str(
            r#"{"threads":[{"id":"3","type":12,"name":"priv","thread_metadata":{"archived":true,"archive_timestamp":"2025-03-01T00:00:00+00:00"}},
              {"id":"2","type":11,"name":"new"}],"has_more":false}"#,
        )
        .unwrap();
        let m = merge_archived(p.threads, q.threads);
        assert_eq!(m.iter().map(|c| c.id.0).collect::<Vec<_>>(), [2, 3, 1]);
    }
}
