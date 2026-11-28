use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use super::{Body, RestClient};
use crate::error::Result;

#[derive(Deserialize)]
struct TokenBody {
    token: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginMfaType {
    Totp,
    Sms,
    Backup,
    Webauthn,
}

impl LoginMfaType {
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::Sms => "sms",
            Self::Backup => "backup",
            Self::Webauthn => "webauthn",
        }
    }
}

pub fn login_mfa_path(kind: LoginMfaType) -> String {
    format!("/auth/mfa/{}", kind.api_name())
}

pub fn login_mfa_body(ticket: &str, code: &str, login_instance_id: Option<&str>) -> serde_json::Value {
    json!({
        "code": code,
        "ticket": ticket,
        "login_source": null,
        "gift_code_sku_id": null,
        "login_instance_id": login_instance_id,
    })
}

impl RestClient {
    pub async fn login_mfa_sms_send(&self, ticket: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct P {
            #[serde(default)]
            phone: String,
        }
        let p: P = self.call(Method::POST, "/auth/mfa/sms/send", Body::Json(json!({ "ticket": ticket }))).await?;
        Ok(p.phone)
    }

    pub async fn login_mfa(&self, kind: LoginMfaType, ticket: &str, code: &str, login_instance_id: Option<&str>) -> Result<String> {
        let t: TokenBody =
            self.call(Method::POST, &login_mfa_path(kind), Body::Json(login_mfa_body(ticket, code, login_instance_id))).await?;
        Ok(t.token)
    }

    pub async fn remote_auth_login(&self, ticket: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct E {
            encrypted_token: String,
        }
        let e: E = self
            .call(Method::POST, "/users/@me/remote-auth/login", Body::Json(json!({ "ticket": ticket })))
            .await?;
        Ok(e.encrypted_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientProperties, Error, rest::LoginResponse};
    use serde_json::Value;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    type Seen = Vec<(String, Value)>;

    async fn serve(responses: Vec<(&'static str, &'static str)>) -> (RestClient, tokio::task::JoinHandle<Seen>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut seen = Vec::new();
            for (status, response) in responses {
                let (socket, _) = listener.accept().await.unwrap();
                let mut socket = BufReader::new(socket);
                let mut head = String::new();
                loop {
                    let mut line = String::new();
                    assert_ne!(socket.read_line(&mut line).await.unwrap(), 0);
                    if line == "\r\n" {
                        break;
                    }
                    head.push_str(&line);
                }
                let len = head
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:")?.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0; len];
                socket.read_exact(&mut body).await.unwrap();
                let line = head.lines().next().unwrap_or_default().to_owned();
                seen.push((line, serde_json::from_slice(&body).unwrap_or(Value::Null)));
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response}",
                    response.len()
                );
                socket.get_mut().write_all(reply.as_bytes()).await.unwrap();
            }
            seen
        });
        (RestClient::with_base(None, ClientProperties::linux_desktop(), format!("http://{addr}")), server)
    }

    fn timeout<F: std::future::Future>(f: F) -> impl std::future::Future<Output = F::Output> {
        async move { tokio::time::timeout(std::time::Duration::from_secs(5), f).await.unwrap() }
    }

    #[tokio::test]
    async fn password_login_then_totp_posts_the_client_body() {
        let (rest, server) = serve(vec![
            (
                "200 OK",
                r#"{"user_id":"1","mfa":true,"sms":true,"ticket":"tkt","backup":true,"totp":true,"webauthn":"{\"publicKey\":{}}","login_instance_id":"inst"}"#,
            ),
            ("200 OK", r#"{"token":"fresh","user_settings":{"locale":"fr","theme":"dark"}}"#),
        ])
        .await;
        let res = timeout(rest.login("a@b.c", "pw")).await.unwrap();
        let LoginResponse::MfaRequired { ticket, totp, sms, backup, webauthn, login_instance_id, .. } = res else { panic!() };
        assert_eq!((ticket.as_str(), totp, sms, backup, webauthn), ("tkt", true, true, true, true));
        assert_eq!(login_instance_id.as_deref(), Some("inst"));
        let token = timeout(rest.login_mfa(LoginMfaType::Totp, &ticket, "123456", login_instance_id.as_deref())).await.unwrap();
        assert_eq!(token, "fresh");
        let seen = server.await.unwrap();
        assert!(seen[0].0.starts_with("POST /auth/login "));
        assert!(seen[1].0.starts_with("POST /auth/mfa/totp "));
        assert_eq!(
            seen[1].1,
            json!({"code":"123456","ticket":"tkt","login_source":null,"gift_code_sku_id":null,"login_instance_id":"inst"})
        );
    }

    #[tokio::test]
    async fn backup_and_sms_use_their_own_routes() {
        let (rest, server) = serve(vec![
            ("200 OK", r#"{"phone":"+*******1234"}"#),
            ("200 OK", r#"{"token":"a"}"#),
            ("200 OK", r#"{"token":"b"}"#),
        ])
        .await;
        assert_eq!(timeout(rest.login_mfa_sms_send("tkt")).await.unwrap(), "+*******1234");
        timeout(rest.login_mfa(LoginMfaType::Sms, "tkt", "654321", None)).await.unwrap();
        timeout(rest.login_mfa(LoginMfaType::Backup, "tkt", "abcd1234", None)).await.unwrap();
        let seen = server.await.unwrap();
        assert!(seen[0].0.starts_with("POST /auth/mfa/sms/send "));
        assert_eq!(seen[0].1, json!({"ticket":"tkt"}));
        assert!(seen[1].0.starts_with("POST /auth/mfa/sms "));
        assert!(seen[2].0.starts_with("POST /auth/mfa/backup "));
        assert_eq!(seen[2].1["code"], "abcd1234");
        assert_eq!(seen[2].1["login_instance_id"], Value::Null);
    }

    #[tokio::test]
    async fn mfa_errors_keep_the_server_message() {
        let (rest, server) = serve(vec![
            ("400 Bad Request", r#"{"message":"Invalid two-factor code","code":60008}"#),
            ("400 Bad Request", r#"{"message":"Invalid two-factor auth ticket","code":60006}"#),
            ("400 Bad Request", r#"{"captcha_key":["captcha-required"],"captcha_sitekey":"site","captcha_service":"hcaptcha"}"#),
        ])
        .await;
        let invalid = timeout(rest.login_mfa(LoginMfaType::Totp, "tkt", "000000", None)).await.unwrap_err();
        assert!(matches!(&invalid, Error::Api { status: 400, body, .. } if body.code == 60008 && body.message == "Invalid two-factor code"));
        let expired = timeout(rest.login_mfa(LoginMfaType::Totp, "old", "123456", None)).await.unwrap_err();
        assert!(matches!(&expired, Error::Api { body, .. } if body.code == 60006));
        let captcha = timeout(rest.login_mfa(LoginMfaType::Totp, "tkt", "123456", None)).await.unwrap_err();
        assert!(matches!(captcha, Error::CaptchaRequired { sitekey: Some(s), .. } if s == "site"));
        assert_eq!(server.await.unwrap().len(), 3);
    }

    #[test]
    fn mfa_paths_match_the_client_routes() {
        assert_eq!(login_mfa_path(LoginMfaType::Totp), "/auth/mfa/totp");
        assert_eq!(login_mfa_path(LoginMfaType::Webauthn), "/auth/mfa/webauthn");
    }
}
