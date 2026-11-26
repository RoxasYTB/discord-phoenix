use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::codec::CodecOffer;
use crate::{Error, Result};

pub mod op {
    pub const IDENTIFY: u8 = 0;
    pub const SELECT_PROTOCOL: u8 = 1;
    pub const READY: u8 = 2;
    pub const HEARTBEAT: u8 = 3;
    pub const SESSION_DESCRIPTION: u8 = 4;
    pub const SPEAKING: u8 = 5;
    pub const HEARTBEAT_ACK: u8 = 6;
    pub const RESUME: u8 = 7;
    pub const HELLO: u8 = 8;
    pub const RESUMED: u8 = 9;
    pub const CLIENTS_CONNECT: u8 = 11;
    pub const VIDEO: u8 = 12;
    pub const MEDIA_SINK_WANTS: u8 = 15;
    pub const CLIENT_DISCONNECT: u8 = 13;
    pub const FLAGS: u8 = 18;
    pub const PLATFORM: u8 = 20;
    pub const DAVE_PREPARE_TRANSITION: u8 = 21;
    pub const DAVE_EXECUTE_TRANSITION: u8 = 22;
    pub const DAVE_TRANSITION_READY: u8 = 23;
    pub const DAVE_PREPARE_EPOCH: u8 = 24;
    pub const DAVE_MLS_EXTERNAL_SENDER: u8 = 25;
    pub const DAVE_MLS_KEY_PACKAGE: u8 = 26;
    pub const DAVE_MLS_PROPOSALS: u8 = 27;
    pub const DAVE_MLS_COMMIT_WELCOME: u8 = 28;
    pub const DAVE_MLS_ANNOUNCE_COMMIT_TRANSITION: u8 = 29;
    pub const DAVE_MLS_WELCOME: u8 = 30;
    pub const DAVE_MLS_INVALID_COMMIT_WELCOME: u8 = 31;
}

#[derive(Debug, Clone)]
pub struct VoiceConnectInfo {
    pub endpoint: String,
    pub server_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
    pub max_dave_protocol_version: u16,
    pub stream: bool,
}

impl VoiceConnectInfo {
    pub fn url(&self) -> String {
        let host = self.endpoint.trim_start_matches("wss://").trim_end_matches('/');
        format!("wss://{host}/?v=8")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    #[serde(rename = "type")]
    pub kind: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StreamDescriptor {
    #[serde(rename = "type")]
    pub kind: String,
    pub rid: String,
    #[serde(default)]
    pub ssrc: u32,
    #[serde(default)]
    pub rtx_ssrc: u32,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub quality: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_framerate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_resolution: Option<Resolution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VideoPayload {
    #[serde(default)]
    pub user_id: Option<String>,
    pub audio_ssrc: u32,
    pub video_ssrc: u32,
    pub rtx_ssrc: u32,
    #[serde(default)]
    pub streams: Vec<StreamDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Ready {
    pub ssrc: u32,
    pub ip: String,
    pub port: u16,
    pub modes: Vec<String>,
    #[serde(default)]
    pub streams: Vec<StreamDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SessionDescription {
    pub mode: String,
    pub secret_key: Vec<u8>,
    #[serde(default)]
    pub dave_protocol_version: u16,
    #[serde(default)]
    pub audio_codec: Option<String>,
    #[serde(default)]
    pub video_codec: Option<String>,
}

pub mod speaking {
    pub const MICROPHONE: u32 = 1 << 0;
    pub const SOUNDSHARE: u32 = 1 << 1;
    pub const PRIORITY: u32 = 1 << 2;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveServerMessage {
    PrepareTransition { transition_id: u16, protocol_version: u16 },
    ExecuteTransition { transition_id: u16 },
    PrepareEpoch { epoch: u64, protocol_version: u16 },
    ExternalSender(Vec<u8>),
    Proposals(Vec<u8>),
    AnnounceCommit { transition_id: u16, commit: Vec<u8> },
    Welcome { transition_id: u16, welcome: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerMessage {
    Hello { heartbeat_interval_ms: f64 },
    Ready(Ready),
    SessionDescription(SessionDescription),
    HeartbeatAck { nonce: u64 },
    Resumed,
    Speaking { user_id: Option<String>, ssrc: u32, speaking: u32 },
    ClientsConnect { user_ids: Vec<String> },
    ClientDisconnect { user_id: String },
    Video(VideoPayload),
    Dave(DaveServerMessage),
    Other { op: u8, d: Value },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub seq: Option<u64>,
    pub msg: ServerMessage,
}

fn u16_field(d: &Value, k: &str) -> u16 {
    d.get(k).and_then(Value::as_u64).unwrap_or(0) as u16
}

pub fn decode_text(text: &str) -> Result<Decoded> {
    let v: Value = serde_json::from_str(text)?;
    let opn = v.get("op").and_then(Value::as_u64).ok_or(Error::Protocol("missing op".into()))?;
    let op = u8::try_from(opn).map_err(|_| Error::Protocol("op out of range".into()))?;
    let seq = v.get("seq").and_then(Value::as_u64);
    let d = v.get("d").cloned().unwrap_or(Value::Null);
    let msg = match op {
        op::HELLO => ServerMessage::Hello {
            heartbeat_interval_ms: d.get("heartbeat_interval").and_then(Value::as_f64).ok_or(Error::Protocol("hello".into()))?,
        },
        op::READY => ServerMessage::Ready(serde_json::from_value(d)?),
        op::SESSION_DESCRIPTION => ServerMessage::SessionDescription(serde_json::from_value(d)?),
        op::HEARTBEAT_ACK => ServerMessage::HeartbeatAck {
            nonce: d.get("t").and_then(Value::as_u64).or_else(|| d.as_u64()).unwrap_or(0),
        },
        op::RESUMED => ServerMessage::Resumed,
        op::SPEAKING => ServerMessage::Speaking {
            user_id: d.get("user_id").and_then(Value::as_str).map(str::to_string),
            ssrc: d.get("ssrc").and_then(Value::as_u64).unwrap_or(0) as u32,
            speaking: d.get("speaking").and_then(Value::as_u64).unwrap_or(0) as u32,
        },
        op::CLIENTS_CONNECT => ServerMessage::ClientsConnect {
            user_ids: serde_json::from_value(d.get("user_ids").cloned().unwrap_or(Value::Array(vec![])))?,
        },
        op::CLIENT_DISCONNECT => ServerMessage::ClientDisconnect {
            user_id: d.get("user_id").and_then(Value::as_str).unwrap_or_default().to_string(),
        },
        op::VIDEO => ServerMessage::Video(serde_json::from_value(d)?),
        op::DAVE_PREPARE_TRANSITION => ServerMessage::Dave(DaveServerMessage::PrepareTransition {
            transition_id: u16_field(&d, "transition_id"),
            protocol_version: u16_field(&d, "protocol_version"),
        }),
        op::DAVE_EXECUTE_TRANSITION => ServerMessage::Dave(DaveServerMessage::ExecuteTransition {
            transition_id: u16_field(&d, "transition_id"),
        }),
        op::DAVE_PREPARE_EPOCH => ServerMessage::Dave(DaveServerMessage::PrepareEpoch {
            epoch: d.get("epoch").and_then(Value::as_u64).unwrap_or(0),
            protocol_version: u16_field(&d, "protocol_version"),
        }),
        op => ServerMessage::Other { op, d },
    };
    Ok(Decoded { seq, msg })
}

pub fn decode_binary(b: &[u8]) -> Result<Decoded> {
    if b.len() < 3 {
        return Err(Error::Malformed("binary frame too short"));
    }
    let seq = u16::from_be_bytes([b[0], b[1]]) as u64;
    let opcode = b[2];
    let p = &b[3..];
    let tid = |p: &[u8]| -> Result<(u16, Vec<u8>)> {
        if p.len() < 2 {
            return Err(Error::Malformed("transition id"));
        }
        Ok((u16::from_be_bytes([p[0], p[1]]), p[2..].to_vec()))
    };
    let msg = match opcode {
        op::DAVE_MLS_EXTERNAL_SENDER => DaveServerMessage::ExternalSender(p.to_vec()),
        op::DAVE_MLS_PROPOSALS => DaveServerMessage::Proposals(p.to_vec()),
        op::DAVE_MLS_ANNOUNCE_COMMIT_TRANSITION => {
            let (transition_id, commit) = tid(p)?;
            DaveServerMessage::AnnounceCommit { transition_id, commit }
        }
        op::DAVE_MLS_WELCOME => {
            let (transition_id, welcome) = tid(p)?;
            DaveServerMessage::Welcome { transition_id, welcome }
        }
        other => return Err(Error::Protocol(format!("unknown binary opcode {other}"))),
    };
    Ok(Decoded { seq: Some(seq), msg: ServerMessage::Dave(msg) })
}

pub fn encode_binary(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(payload.len() + 1);
    v.push(opcode);
    v.extend_from_slice(payload);
    v
}

pub fn text(op: u8, d: Value) -> String {
    json!({ "op": op, "d": d }).to_string()
}

pub fn identify(info: &VoiceConnectInfo) -> String {
    text(
        op::IDENTIFY,
        json!({
            "server_id": info.server_id,
            "user_id": info.user_id,
            "session_id": info.session_id,
            "token": info.token,
            "video": true,
            "streams": [{ "type": if info.stream { "screen" } else { "video" }, "rid": "100", "quality": 100 }],
            "max_dave_protocol_version": info.max_dave_protocol_version,
        }),
    )
}

pub fn resume(info: &VoiceConnectInfo, seq_ack: Option<u64>) -> String {
    text(
        op::RESUME,
        json!({
            "server_id": info.server_id,
            "session_id": info.session_id,
            "token": info.token,
            "seq_ack": seq_ack.map(|s| s as i64).unwrap_or(-1),
        }),
    )
}

pub fn heartbeat(nonce: u64, seq_ack: Option<u64>) -> String {
    text(op::HEARTBEAT, json!({ "t": nonce, "seq_ack": seq_ack.map(|s| s as i64).unwrap_or(-1) }))
}

pub fn select_protocol(address: &str, port: u16, mode: &str, codecs: &[CodecOffer]) -> String {
    text(
        op::SELECT_PROTOCOL,
        json!({
            "protocol": "udp",
            "data": { "address": address, "port": port, "mode": mode },
            "codecs": codecs,
        }),
    )
}

pub fn speaking(flags: u32, ssrc: u32) -> String {
    text(op::SPEAKING, json!({ "speaking": flags, "delay": 0, "ssrc": ssrc }))
}

pub fn video(p: &VideoPayload) -> Result<String> {
    Ok(text(op::VIDEO, serde_json::to_value(p)?))
}

pub fn media_sink_wants(wants: &Value) -> String {
    text(op::MEDIA_SINK_WANTS, wants.clone())
}

pub fn dave_transition_ready(transition_id: u16) -> String {
    text(op::DAVE_TRANSITION_READY, json!({ "transition_id": transition_id }))
}

pub fn dave_invalid_commit_welcome(transition_id: u16) -> String {
    text(op::DAVE_MLS_INVALID_COMMIT_WELCOME, json!({ "transition_id": transition_id }))
}

pub fn close_code_resumable(code: u16) -> bool {
    !matches!(code, 1000 | 4003 | 4004 | 4005 | 4006 | 4009 | 4011 | 4012 | 4014 | 4016 | 4021 | 4022)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_ready_and_binary() {
        let r = decode_text(r#"{"op":2,"seq":3,"d":{"ssrc":1,"ip":"1.2.3.4","port":5,"modes":["aead_aes256_gcm_rtpsize"],"streams":[{"type":"video","rid":"100","ssrc":2,"rtx_ssrc":3,"active":false,"quality":100}]}}"#).unwrap();
        assert_eq!(r.seq, Some(3));
        let ServerMessage::Ready(ready) = r.msg else { panic!("not ready") };
        assert_eq!(ready.streams[0].rtx_ssrc, 3);
        let b = decode_binary(&[0, 9, 29, 0, 7, 0xaa]).unwrap();
        assert_eq!(b.seq, Some(9));
        assert_eq!(b.msg, ServerMessage::Dave(DaveServerMessage::AnnounceCommit { transition_id: 7, commit: vec![0xaa] }));
        assert!(decode_binary(&[0, 1]).is_err());
    }

    #[test]
    fn media_sink_wants_is_op15() {
        let v: Value = serde_json::from_str(&media_sink_wants(&json!({"any": 90, "123": 0}))).unwrap();
        assert_eq!(v["op"], 15);
        assert_eq!(v["d"]["any"], 90);
        assert_eq!(v["d"]["123"], 0);
    }

    #[test]
    fn video_op_serializes_streams() {
        let p = VideoPayload {
            user_id: None,
            audio_ssrc: 1,
            video_ssrc: 2,
            rtx_ssrc: 3,
            streams: vec![StreamDescriptor {
                kind: "video".into(),
                rid: "100".into(),
                ssrc: 2,
                rtx_ssrc: 3,
                active: true,
                quality: 100,
                max_bitrate: Some(2_500_000),
                max_framerate: Some(30),
                max_resolution: Some(Resolution { kind: "fixed".into(), width: 1280, height: 720 }),
            }],
        };
        let s = video(&p).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["op"], 12);
        assert_eq!(v["d"]["streams"][0]["max_resolution"]["width"], 1280);
        assert_eq!(v["d"]["streams"][0]["type"], "video");
    }

    #[test]
    fn url_building() {
        let i = VoiceConnectInfo {
            endpoint: "c-abc.discord.media:443".into(),
            server_id: "1".into(),
            user_id: "2".into(),
            session_id: "s".into(),
            token: "t".into(),
            max_dave_protocol_version: 0,
            stream: false,
        };
        assert_eq!(i.url(), "wss://c-abc.discord.media:443/?v=8");
    }
}
