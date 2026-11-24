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
            if v["op"] == 1 {
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
    cfg.invalid_session_delay = Duration::from_millis(5);
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
        assert_eq!(hb["op"], 1);
        c.send(json!({"op": 11})).await;
        if hb["d"] == 1 {
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
    assert_eq!(c.recv_any().await.unwrap()["op"], 1);

    let code = c.recv_close().await;
    assert_ne!(code, Some(1000));
    wait_for(&mut rx, |e| matches!(e, GatewayEvent::Disconnected { reason, .. } if reason.contains("ACK"))).await;

    let mut c = Conn::accept(&listener).await;
    c.hello(1000).await;
    let resume = c.recv_non_heartbeat(true).await;
    assert_eq!(resume["op"], 6);
    assert_eq!(resume["d"]["seq"], 3);
    c.send(json!({"op": 9, "d": false})).await;
    c.recv_close().await;

    let mut c = Conn::accept(&listener).await;
    c.hello(1000).await;
    let ident = c.recv_non_heartbeat(true).await;
    assert_eq!(ident["op"], 2);

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
