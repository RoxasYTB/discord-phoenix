use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Body, RestClient};
use crate::error::Result;
use crate::models::{Channel, ChannelId, Message, MessageId, Snowflake, null_default};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThreadSort {
    #[default]
    LastMessage,
    Created,
}

pub(crate) fn threads_search_path(forum: ChannelId, sort: ThreadSort, tags: &[Snowflake], offset: u32, archived: bool) -> String {
    let by = match sort {
        ThreadSort::LastMessage => "last_message_time",
        ThreadSort::Created => "creation_time",
    };
    let mut s = format!("/channels/{forum}/threads/search?archived={archived}&sort_by={by}&sort_order=desc&limit=25&offset={offset}");
    for t in tags {
        s.push_str(&format!("&tag={t}"));
    }
    s
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ThreadSearch {
    #[serde(default, deserialize_with = "null_default")]
    pub threads: Vec<Channel>,
    #[serde(default, deserialize_with = "null_default")]
    pub first_messages: Vec<Message>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub total_results: u32,
}

pub fn forum_post_body(title: &str, content: &str, tags: &[Snowflake]) -> Value {
    json!({
        "name": title,
        "applied_tags": tags,
        "auto_archive_duration": 4320,
        "message": { "content": content },
    })
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Poll {
    #[serde(default)]
    pub question: PollMedia,
    #[serde(default, deserialize_with = "null_default")]
    pub answers: Vec<PollAnswer>,
    #[serde(default)]
    pub expiry: Option<String>,
    #[serde(default)]
    pub allow_multiselect: bool,
    #[serde(default)]
    pub results: Option<PollResults>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PollMedia {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub emoji: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PollAnswer {
    pub answer_id: u32,
    #[serde(default)]
    pub poll_media: PollMedia,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PollResults {
    #[serde(default)]
    pub is_finalized: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub answer_counts: Vec<PollCount>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PollCount {
    pub id: u32,
    pub count: u32,
    #[serde(default)]
    pub me_voted: bool,
}

impl Poll {
    pub fn from_value(v: &Value) -> Option<Self> {
        serde_json::from_value(v.clone()).ok()
    }
    pub fn count(&self, answer: u32) -> (u32, bool) {
        self.results
            .as_ref()
            .and_then(|r| r.answer_counts.iter().find(|c| c.id == answer))
            .map(|c| (c.count, c.me_voted))
            .unwrap_or((0, false))
    }
    pub fn total(&self) -> u32 {
        self.results.as_ref().map(|r| r.answer_counts.iter().map(|c| c.count).sum()).unwrap_or(0)
    }
}

impl RestClient {
    pub async fn search_threads(&self, forum: ChannelId, sort: ThreadSort, tags: &[Snowflake], offset: u32, archived: bool) -> Result<ThreadSearch> {
        self.call(Method::GET, &threads_search_path(forum, sort, tags, offset, archived), Body::None).await
    }

    pub async fn create_forum_post(&self, forum: ChannelId, title: &str, content: &str, tags: &[Snowflake]) -> Result<Channel> {
        self.call(Method::POST, &format!("/channels/{forum}/threads"), Body::Json(forum_post_body(title, content, tags))).await
    }

    pub async fn vote_poll(&self, channel: ChannelId, message: MessageId, answers: &[u32]) -> Result<()> {
        self.call_empty(
            Method::PUT,
            &format!("/channels/{channel}/polls/{message}/answers/@me"),
            Body::Json(json!({ "answer_ids": answers.iter().map(|a| a.to_string()).collect::<Vec<_>>() })),
        )
        .await
    }

    pub async fn create_poll(&self, channel: ChannelId, question: &str, answers: &[PollAnswerInput], hours: u32, multi: bool) -> Result<Message> {
        self.call(Method::POST, &format!("/channels/{channel}/messages"), Body::Json(poll_body(question, answers, hours, multi))).await
    }

    pub async fn end_poll(&self, channel: ChannelId, message: MessageId) -> Result<Message> {
        self.call(Method::POST, &format!("/channels/{channel}/polls/{message}/expire"), Body::None).await
    }

    pub async fn get_stage_instance(&self, channel: ChannelId) -> Result<Value> {
        self.request_json(Method::GET, &format!("/stage-instances/{channel}"), None).await
    }

    pub async fn stage_request_to_speak(&self, guild: Snowflake, channel: ChannelId, raise: bool, now_rfc3339: &str) -> Result<()> {
        let ts = if raise { json!(now_rfc3339) } else { Value::Null };
        self.call_empty(
            Method::PATCH,
            &format!("/guilds/{guild}/voice-states/@me"),
            Body::Json(json!({ "channel_id": channel, "request_to_speak_timestamp": ts })),
        )
        .await
    }
}

pub type PollAnswerInput = (String, Option<(Option<Snowflake>, String)>);

pub fn poll_body(question: &str, answers: &[PollAnswerInput], hours: u32, multi: bool) -> Value {
    let answers: Vec<Value> = answers
        .iter()
        .map(|(text, emoji)| {
            let mut media = json!({ "text": text });
            if let Some((id, name)) = emoji {
                media["emoji"] = match id {
                    Some(id) => json!({ "id": id.to_string(), "name": name }),
                    None => json!({ "name": name }),
                };
            }
            json!({ "poll_media": media })
        })
        .collect();
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| ((d.as_millis() as u64 - 1_420_070_400_000) << 22).to_string()).unwrap_or_default();
    json!({
        "content": "",
        "nonce": nonce,
        "poll": { "question": { "text": question }, "answers": answers, "allow_multiselect": multi, "duration": hours, "layout_type": 1 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_message_body() {
        let b = poll_body("Q ?", &[("A".into(), None), ("B".into(), Some((None, "👍".into())))], 24, true);
        assert_eq!(b["poll"]["question"]["text"], "Q ?");
        assert_eq!(b["poll"]["answers"][1]["poll_media"]["emoji"]["name"], "👍");
        assert_eq!(b["poll"]["duration"], 24);
        assert_eq!(b["poll"]["allow_multiselect"], true);
        assert_eq!(b["poll"]["layout_type"], 1);
    }

    #[test]
    fn paths_and_bodies() {
        assert_eq!(
            threads_search_path(Snowflake(1), ThreadSort::Created, &[Snowflake(2), Snowflake(3)], 25, false),
            "/channels/1/threads/search?archived=false&sort_by=creation_time&sort_order=desc&limit=25&offset=25&tag=2&tag=3"
        );
        let b = forum_post_body("T", "hi", &[Snowflake(4)]);
        assert_eq!(b["applied_tags"][0], "4");
        assert_eq!(b["message"]["content"], "hi");
    }

    #[test]
    fn parse_poll() {
        let p = Poll::from_value(&serde_json::json!({
            "question":{"text":"Q?"},"answers":[{"answer_id":1,"poll_media":{"text":"A"}},{"answer_id":2,"poll_media":{"text":"B"}}],
            "expiry":"2030-01-01T00:00:00Z","allow_multiselect":false,
            "results":{"is_finalized":false,"answer_counts":[{"id":1,"count":3,"me_voted":true}]}})).unwrap();
        assert_eq!(p.question.text.as_deref(), Some("Q?"));
        assert_eq!(p.count(1), (3, true));
        assert_eq!(p.count(2), (0, false));
        assert_eq!(p.total(), 3);
        let s: ThreadSearch = serde_json::from_str(r#"{"threads":[],"first_messages":null,"has_more":true}"#).unwrap();
        assert!(s.has_more);
    }
}
