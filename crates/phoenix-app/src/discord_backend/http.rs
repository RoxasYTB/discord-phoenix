use futures::{AsyncReadExt, FutureExt, future::BoxFuture};
use gpui::http_client::{self, AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};

const ALLOWED_HOSTS: &[&str] = &[
    "cdn.discordapp.com",
    "media.discordapp.net",
    "images-ext-1.discordapp.net",
    "images-ext-2.discordapp.net",
    "cdn.jsdelivr.net",
    "static.klipy.com",
    "media.tenor.com",
    "media.giphy.com",
    "i.scdn.co",
];

pub fn host_allowed(url: &str) -> bool {
    Url::parse(url).ok().is_some_and(|u| {
        u.scheme() == "https"
            && u.host_str().is_some_and(|h| {
                ALLOWED_HOSTS.contains(&h)
                    || (h == "discord.com" && u.path().starts_with("/assets/"))
            })
    })
}

pub struct CdnHttpClient {
    client: reqwest::Client,
    rt: tokio::runtime::Handle,
    user_agent: HeaderValue,
}

impl CdnHttpClient {
    pub fn new(rt: tokio::runtime::Handle, user_agent: &str) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .use_rustls_tls()
            .pool_max_idle_per_host(8)
            .build()
            .expect("reqwest client");
        Self {
            client,
            rt,
            user_agent: HeaderValue::from_str(user_agent).unwrap_or(HeaderValue::from_static("discord-phoenix")),
        }
    }
}

impl HttpClient for CdnHttpClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(&self, req: Request<AsyncBody>) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let url = req.uri().to_string();
        if !host_allowed(&url) {
            return async move { Err(anyhow::anyhow!("blocked non-Discord media host")) }.boxed();
        }
        let (parts, mut body) = req.into_parts();
        let client = self.client.clone();
        let rt = self.rt.clone();
        async move {
            let mut bytes = Vec::new();
            body.read_to_end(&mut bytes).await?;
            let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())?;
            let task = rt.spawn(async move {
                let mut rb = client.request(method, &url);
                for (k, v) in parts.headers.iter() {
                    rb = rb.header(k.as_str(), v.as_bytes());
                }
                if !bytes.is_empty() {
                    rb = rb.body(bytes);
                }
                let resp = rb.send().await?;
                let status = resp.status().as_u16();
                let headers: Vec<(String, Vec<u8>)> =
                    resp.headers().iter().map(|(k, v)| (k.as_str().to_owned(), v.as_bytes().to_vec())).collect();
                let body = resp.bytes().await?;
                Ok::<_, reqwest::Error>((status, headers, body))
            });
            let (status, headers, body) = task.await??;
            let mut b = http_client::http::Response::builder().status(status);
            for (k, v) in headers {
                b = b.header(k, v);
            }
            Ok(b.body(AsyncBody::from(body.to_vec()))?)
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::host_allowed;

    #[test]
    fn only_discord_media_hosts() {
        assert!(host_allowed("https://cdn.discordapp.com/avatars/1/a.png?size=128"));
        assert!(host_allowed("https://media.discordapp.net/attachments/1/2/x.png?width=10&height=10"));
        assert!(!host_allowed("http://cdn.discordapp.com/x.png"));
        assert!(!host_allowed("https://evil.example/x.png"));
        assert!(host_allowed("https://static.klipy.com/ii/abc/80/03/X.gif"));
        assert!(!host_allowed("https://cdn.discordapp.com.evil.example/x.png"));
        assert!(host_allowed("https://i.scdn.co/image/ab67616d0000b2739e1cfc756886ac782e363d79"));
    }
}
