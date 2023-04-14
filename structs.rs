use std::{collections::HashMap, fmt::Display, str::FromStr};

use anyhow::anyhow;
use lib::structs::ShareData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct DiscordTokenForm {
    pub client_id: String,
    pub client_secret: String,
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct DiscordTokenResponse {
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: u64,
    // pub refresh_token: String,
    // pub scope: String,
}

#[derive(Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    // pub username: String,
    // pub discriminator: String,
    // pub avatar: String,
    // pub verified: bool,
    // pub email: String,
    // pub flags: u64,
    // pub banner: String,
    // pub accent_color: u64,
    // pub premium_type: u64,
    // pub public_flags: u64,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Timed<T> {
    pub thing: T,
    pub time: u64,
}

impl<T> Timed<T> {
    pub fn split(self) -> (T, u64) {
        (self.thing, self.time)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StringData {
    pub string: String,
    pub available_shares: u64,
    pub value: f64,
    pub history: Vec<Timed<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserData {
    pub id: String,
    pub cash: f64,
    pub shares: HashMap<String, ShareData>,
    pub share_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSafe {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bearer {
    pub token: String,
}

impl FromStr for Bearer {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // formatted as  format!("Bearer {}", token) so we need to remove the `Bearer ` part
        let s = s
            .split("Bearer ")
            .nth(1)
            .ok_or_else(|| anyhow!("Invalid Bearer token"))?;
        Ok(Self {
            token: s.to_string(),
        })
    }
}

impl Display for Bearer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bearer {}", self.token)
    }
}
