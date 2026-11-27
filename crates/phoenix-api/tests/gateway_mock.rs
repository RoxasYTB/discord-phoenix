#![allow(clippy::result_large_err)]

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use phoenix_api::gateway::zlib::ZlibEncoder;
use phoenix_api::gateway::{GatewayConfig, GatewayEvent};
use phoenix_api::models::Snowflake;
use phoenix_api::Gateway;
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

struct Conn {
    ws: WebSocketStream<TcpStream>,
    z: ZlibEncoder,
    query: String,
}

impl Conn {
    async fn accept(l: &TcpListener) -> Conn {
        let (tcp, _) = timeout(Duration::from_secs(10), l.accept()).await.expect("no connection").unwrap();
        let mut query = String::new();
        let ws = tokio_tungstenite::accept_hdr_async(tcp, |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp| {
            query = req.uri().query().unwrap_or("").to_owned();
            Ok(resp)
        })
        .await
        .unwrap();
        Conn { ws, z: ZlibEncoder::new(), query }
    }

    async fn send(&mut self, v: Value) {
        let data = self.z.encode(v.to_string().as_bytes());
        let (a, b) = data.split_at(data.len() / 2);
        self.ws.send(Message::Binary(a.to_vec().into())).await.unwrap();
        self.ws.send(Message::Binary(b.to_vec().into())).await.unwrap();
    }

    async fn hello(&mut self, interval_ms: u64) {
        self.send(json!({"op": 10, "d": {"heartbeat_interval": interval_ms}, "s": null, "t": null})).await;
    }

    async fn recv_non_heartbeat(&mut self, ack: bool) -> Value {
        loop {
            let v = self.recv_any().await.expect("connection closed");
            if v["op"] == 40 {
                if ack {
                    self.send(json!({"op": 11})).await;
                }
                continue;
            }
            return v;
        }
    }

    async fn recv_any(&mut self) -> Option<Value> {
        loop {
            match timeout(Duration::from_secs(10), self.ws.next()).await.expect("recv timeout") {
                Some(Ok(Message::Text(t))) => return Some(serde_json::from_str(&t).unwrap()),
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return None,
                Some(Ok(_)) => {}
            }
        }
    }

    async fn recv_close(&mut self) -> Option<u16> {
        loop {
            match timeout(Duration::from_secs(10), self.ws.next()).await.expect("close timeout") {
                Some(Ok(Message::Close(f))) => return f.map(|f| f.code.into()),
                None | Some(Err(_)) => return None,
                Some(Ok(_)) => {}
            }
        }
    }
}

async fn wait_for(rx: &mut UnboundedReceiver<GatewayEvent>, pred: impl Fn(&GatewayEvent) -> bool) -> GatewayEvent {
    loop {
        let ev = timeout(Duration::from_secs(10), rx.recv()).await.expect("event timeout").expect("events closed");
        if pred(&ev) {
            return ev;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hello_identify_ready_heartbeat_reconnect_resume() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = format!("ws://{}", listener.local_addr().unwrap());

    let mut cfg = GatewayConfig::new("user-token");
    cfg.url = addr.clone();
    cfg.initial_backoff = Duration::from_millis(10);
    cfg.max_backoff = Duration::from_millis(50);
    let (handle, mut rx) = Gateway::spawn(cfg);

    let mut c = Conn::accept(&listener).await;
    assert!(c.query.contains("v=10") && c.query.contains("compress=zlib-stream"), "{}", c.query);
    c.hello(150).await;
    let ident = c.recv_non_heartbeat(true).await;
    assert_eq!(ident["op"], 2);
    assert_eq!(ident["d"]["token"], "user-token");
    assert!(ident["d"]["capabilities"].as_u64().unwrap() > 0);
    assert_eq!(ident["d"]["properties"]["os"], "Linux");
    assert!(ident["d"].get("intents").is_none());
    let ready = std::fs::read_to_string(format!("{}/tests/fixtures/ready_user.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut ready: Value = serde_json::from_str(&ready).unwrap();
    ready["session_id"] = json!("sess-1");
    ready["resume_gateway_url"] = json!(addr);
    c.send(json!({"op": 0, "s": 1, "t": "READY", "d": ready})).await;

    assert!(matches!(wait_for(&mut rx, |e| matches!(e, GatewayEvent::Connected { .. })).await,
        GatewayEvent::Connected { heartbeat_interval } if heartbeat_interval == Duration::from_millis(150)));
    let GatewayEvent::Ready(r) = wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await else { unreachable!() };
    assert_eq!(r.session_id, "sess-1");

    loop {
        let hb = c.recv_any().await.unwrap();
        assert_eq!(hb["op"], 40);
        assert_eq!(hb["d"]["qos"]["ver"], 31);
        c.send(json!({"op": 11})).await;
        if hb["d"]["seq"] == 1 {
            break;
        }
    }
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::HeartbeatAck { .. })).await;

    c.send(json!({"op": 0, "s": 2, "t": "MESSAGE_CREATE", "d": {"id": "9", "channel_id": "8", "content": "yo", "author": {"id": "7", "username": "z"}}})).await;
    let GatewayEvent::MessageCreate(m) = wait_for(&mut rx, |e| matches!(e, GatewayEvent::MessageCreate(_))).await else { unreachable!() };
    assert_eq!(m.content, "yo");

    c.send(json!({"op": 7, "d": null})).await;
    let code = c.recv_close().await;
    assert_ne!(code, Some(1000), "reconnect must not invalidate the session");
    let GatewayEvent::Reconnecting { resume, .. } = wait_for(&mut rx, |e| matches!(e, GatewayEvent::Reconnecting { .. })).await else { unreachable!() };
    assert!(resume);

    let mut c = Conn::accept(&listener).await;
    c.hello(300).await;
    let resume = c.recv_non_heartbeat(false).await;
    assert_eq!(resume, json!({"op": 6, "d": {"token": "user-token", "session_id": "sess-1", "seq": 2}}));
    c.send(json!({"op": 0, "s": 3, "t": "RESUMED", "d": {}})).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Resumed)).await;

    handle.update_voice_state(Some(Snowflake(1)), Some(Snowflake(2)), true, false).unwrap();
    let vs = c.recv_non_heartbeat(false).await;
    assert_eq!(vs["op"], 4);
    assert_eq!(vs["d"], json!({"guild_id": "1", "channel_id": "2", "self_mute": true, "self_deaf": false, "self_video": false}));

    c.send(json!({"op": 1, "d": null})).await;
    assert_eq!(c.recv_any().await.unwrap()["op"], 40);

    let code = c.recv_close().await;
    assert_ne!(code, Some(1000));
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Disconnected { reason, .. } if reason.contains("ACK"))).await;

    let mut c = Conn::accept(&listener).await;
    c.hello(1000).await;
    let resume = c.recv_non_heartbeat(true).await;
    assert_eq!(resume["op"], 6);
    assert_eq!(resume["d"]["seq"], 3);
    c.send(json!({"op": 9, "d": false})).await;
    let ident = c.recv_non_heartbeat(true).await;
    assert_eq!(ident["op"], 2, "op 9 d=false identifies again on the same socket");

    handle.shutdown();
    assert_eq!(c.recv_close().await, Some(1000));
    timeout(Duration::from_secs(5), async {
        while rx.recv().await.is_some() {}
    })
    .await
    .expect("event channel should close after shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fatal_close_stops() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = GatewayConfig::new("bad");
    cfg.url = format!("ws://{}", listener.local_addr().unwrap());
    cfg.compress = false;
    let (_handle, mut rx) = Gateway::spawn(cfg);
    let mut c = Conn::accept(&listener).await;
    assert!(!c.query.contains("compress"));
    c.ws.send(Message::Text(json!({"op": 10, "d": {"heartbeat_interval": 1000}}).to_string().into())).await.unwrap();
    assert_eq!(c.recv_non_heartbeat(true).await["op"], 2);
    c.ws.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
        code: 4004u16.into(),
        reason: "Authentication failed.".into(),
    }))
    .await
    .unwrap();
    let ev = wait_for(&mut rx, |e| matches!(e, GatewayEvent::Fatal { .. })).await;
    assert!(matches!(ev, GatewayEvent::Fatal { code: 4004, .. }));
}

fn ready_payload(session: &str, resume_url: &str) -> Value {
    let ready = std::fs::read_to_string(format!("{}/tests/fixtures/ready_user.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut ready: Value = serde_json::from_str(&ready).unwrap();
    ready["session_id"] = json!(session);
    ready["resume_gateway_url"] = json!(resume_url);
    json!({"op": 0, "s": 1, "t": "READY", "d": ready})
}

fn fast_config(addr: &str) -> GatewayConfig {
    let mut cfg = GatewayConfig::new("user-token");
    cfg.url = addr.to_owned();
    cfg.initial_backoff = Duration::from_millis(5);
    cfg.max_backoff = Duration::from_millis(20);
    cfg
}

async fn bind() -> (TcpListener, String) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = format!("ws://{}", l.local_addr().unwrap());
    (l, addr)
}

async fn ready_conn(l: &TcpListener, session: &str, resume_url: &str) -> Conn {
    let mut c = Conn::accept(l).await;
    c.hello(60_000).await;
    assert_eq!(c.recv_non_heartbeat(true).await["op"], 2);
    c.send(ready_payload(session, resume_url)).await;
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_session_resumable_resumes_on_the_same_socket() {
    let (l, addr) = bind().await;
    let (_h, mut rx) = Gateway::spawn(fast_config(&addr));
    let mut c = ready_conn(&l, "s1", &addr).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await;
    c.send(json!({"op": 9, "d": true})).await;
    let resume = c.recv_non_heartbeat(true).await;
    assert_eq!(resume["op"], 6);
    assert_eq!(resume["d"]["session_id"], "s1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_without_close_drops_the_resume_url_and_resumes_on_the_main_gateway() {
    let (main, main_addr) = bind().await;
    let (alt, alt_addr) = bind().await;
    let (_h, mut rx) = Gateway::spawn(fast_config(&main_addr));
    let mut c = ready_conn(&main, "s1", &alt_addr).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await;
    c.send(json!({"op": 7, "d": null})).await;
    c.recv_close().await;

    let mut c = Conn::accept(&alt).await;
    c.hello(60_000).await;
    assert_eq!(c.recv_non_heartbeat(true).await["op"], 6, "op 7 resumes on resume_gateway_url");
    drop(c);
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Disconnected { .. })).await;

    let mut c = Conn::accept(&main).await;
    c.hello(60_000).await;
    let resume = c.recv_non_heartbeat(true).await;
    assert_eq!(resume["op"], 6, "the session survives a reset");
    c.send(json!({"op": 0, "s": 2, "t": "RESUMED", "d": {}})).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Resumed)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn five_failed_attempts_reset_the_session_and_identify() {
    let (l, addr) = bind().await;
    let (_h, mut rx) = Gateway::spawn(fast_config(&addr));
    let mut c = ready_conn(&l, "s1", &addr).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await;
    c.ws.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 4000u16.into(), reason: "".into() })).await.unwrap();
    drop(c);
    for i in 0..4 {
        let mut c = Conn::accept(&l).await;
        c.hello(60_000).await;
        assert_eq!(c.recv_non_heartbeat(true).await["op"], 6, "attempt {i} still resumes");
        c.ws.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 4000u16.into(), reason: "".into() })).await.unwrap();
    }
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::SessionReset)).await;
    let mut c = Conn::accept(&l).await;
    c.hello(60_000).await;
    assert_eq!(c.recv_non_heartbeat(true).await["op"], 2, "a reset session identifies");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_4007_starts_a_new_session() {
    let (l, addr) = bind().await;
    let (_h, mut rx) = Gateway::spawn(fast_config(&addr));
    let mut c = ready_conn(&l, "s1", &addr).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await;
    c.ws.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 4007u16.into(), reason: "Invalid seq".into() })).await.unwrap();
    let mut c = Conn::accept(&l).await;
    c.hello(60_000).await;
    assert_eq!(c.recv_non_heartbeat(true).await["op"], 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_auth_close_codes_reconnect() {
    let (l, addr) = bind().await;
    let (_h, mut rx) = Gateway::spawn(fast_config(&addr));
    let mut c = ready_conn(&l, "s1", &addr).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await;
    c.ws.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 4008u16.into(), reason: "Rate limited.".into() })).await.unwrap();
    let mut c = Conn::accept(&l).await;
    c.hello(60_000).await;
    assert_eq!(c.recv_non_heartbeat(true).await["op"], 6);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn becoming_active_sends_a_qos_heartbeat_at_once() {
    let (l, addr) = bind().await;
    let mut cfg = fast_config(&addr);
    cfg.qos = phoenix_api::gateway::QosPayload::new(false, false);
    let (h, mut rx) = Gateway::spawn(cfg);
    let mut c = ready_conn(&l, "s1", &addr).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await;
    h.set_activity(true, true).unwrap();
    let hb = c.recv_any().await.unwrap();
    assert_eq!(hb, json!({"op": 40, "d": {"seq": 1, "qos": {"active": true, "ver": 31, "reasons": ["foregrounded", "rtc_connected"]}}}));
    h.set_activity(false, false).unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    c.send(json!({"op": 1, "d": null})).await;
    let hb = c.recv_any().await.unwrap();
    assert_eq!(hb["d"]["qos"]["active"], true, "a beat reports activity seen since the previous one");
    c.send(json!({"op": 1, "d": null})).await;
    let hb = c.recv_any().await.unwrap();
    assert_eq!(hb["d"]["qos"], json!({"active": false, "ver": 31, "reasons": []}));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expedited_heartbeat_without_ack_reconnects() {
    let (l, addr) = bind().await;
    let (h, mut rx) = Gateway::spawn(fast_config(&addr));
    let mut c = ready_conn(&l, "s1", &addr).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await;
    h.expedited_heartbeat(Duration::from_millis(100)).unwrap();
    assert_eq!(c.recv_any().await.unwrap()["op"], 40);
    let code = c.recv_close().await;
    assert_eq!(code, Some(4000));
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Disconnected { reason, .. } if reason.contains("ACK"))).await;
    let mut c = Conn::accept(&l).await;
    c.hello(60_000).await;
    assert_eq!(c.recv_non_heartbeat(true).await["op"], 6);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hello_timeout_reconnects() {
    let (l, addr) = bind().await;
    let mut cfg = fast_config(&addr);
    cfg.hello_timeout = Duration::from_millis(200);
    let (_h, mut rx) = Gateway::spawn(cfg);
    let _silent = Conn::accept(&l).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Disconnected { reason, .. } if reason.contains("HELLO"))).await;
    let mut c = Conn::accept(&l).await;
    c.hello(60_000).await;
    assert_eq!(c.recv_non_heartbeat(true).await["op"], 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn network_changes_send_an_expedited_heartbeat() {
    let (l, addr) = bind().await;
    let (h, mut rx) = Gateway::spawn(fast_config(&addr));
    let mut c = ready_conn(&l, "s1", &addr).await;
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Ready(_))).await;
    h.network_changed(false).unwrap();
    assert_eq!(c.recv_any().await.unwrap()["op"], 40);
    h.network_changed(true).unwrap();
    assert_eq!(c.recv_any().await.unwrap()["op"], 40);
}
