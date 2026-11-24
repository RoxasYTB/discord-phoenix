use reqwest::Method;

use super::RestClient;
use crate::error::Result;
use crate::models::SoundboardSound;

impl RestClient {
    /// `GET /soundboard-default-sounds` — Discord's built-in Soundboard sounds.
    pub async fn soundboard_default_sounds(&self) -> Result<Vec<SoundboardSound>> {
        self.call(Method::GET, "/soundboard-default-sounds", super::Body::None).await
    }
}
