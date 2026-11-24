use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("websocket: {0}")]
    WebSocket(#[from] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("gateway closed (code {code:?}): {reason}")]
    Closed { code: Option<u16>, reason: String },
    #[error("crypto failure: {0}")]
    Crypto(&'static str),
    #[error("malformed packet: {0}")]
    Malformed(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("encoder error: {0}")]
    Encoder(String),
    #[error("no usable codec left for stream")]
    NoCodecAvailable,
    #[error("mls: {0}")]
    Mls(String),
    #[error("timed out: {0}")]
    Timeout(&'static str),
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebSocket(Box::new(e))
    }
}
