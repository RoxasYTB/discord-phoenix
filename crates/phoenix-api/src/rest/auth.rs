use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use super::{Body, RestClient};
use crate::error::Result;

#[derive(Deserialize)]
struct TokenBody {
    token: String,
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

    pub async fn login_mfa_sms(&self, ticket: &str, code: &str) -> Result<String> {
        let t: TokenBody = self
            .call(
                Method::POST,
                "/auth/mfa/sms",
                Body::Json(json!({ "code": code, "ticket": ticket, "login_source": null, "gift_code_sku_id": null })),
            )
            .await?;
        Ok(t.token)
    }

    pub async fn login_mfa_backup(&self, ticket: &str, code: &str) -> Result<String> {
        let code: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        self.login_mfa_totp(ticket, &code).await
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
