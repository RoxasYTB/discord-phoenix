use std::time::Duration;

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct ApiErrorBody {
    #[serde(default)]
    pub code: u32,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub errors: Option<serde_json::Value>,
    #[serde(default)]
    pub captcha_key: Option<Vec<String>>,
    #[serde(default)]
    pub captcha_sitekey: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP {status}: {} (code {})", body.message, body.code)]
    Api { status: u16, body: Box<ApiErrorBody>, raw: String },
    #[error("rate limited for {retry_after:?} (global: {global})")]
    RateLimited { retry_after: Duration, global: bool },
    #[error("captcha required (sitekey {sitekey:?})")]
    CaptchaRequired { sitekey: Option<String>, body: Box<ApiErrorBody> },
    #[error("transport: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("websocket: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    #[error("zlib: {0}")]
    Zlib(String),
    #[error("gateway closed with fatal code {code}: {reason}")]
    FatalClose { code: u16, reason: String },
    #[error("gateway protocol: {0}")]
    Protocol(String),
    #[error("gateway task stopped")]
    Closed,
}

impl Error {
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Error::Api { status: 401, .. })
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
