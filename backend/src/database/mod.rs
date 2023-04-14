use anyhow::{anyhow, Result};

pub mod schema;

pub struct DBConnection {
    valid_users: Vec<String>,
}

impl DBConnection {
    pub fn new(ip: String, port: u64, user: String, password: String, table: String) -> Self {
        let postgres_url = format!(
            "postgres://{}:{}@{}:{}/{}",
            user, password, ip, port, "postgres"
        );
        Self {
            valid_users: vec![],
        }
    }
    pub async fn open(&mut self) -> Result<()> {
        Ok(())
    }
    pub async fn close(&mut self) -> Result<()> {
        Ok(())
    }
    pub async fn trim(&mut self) -> Result<()> {
        Ok(())
    }
    pub async fn is_auth(&mut self, token: String) -> Result<bool> {
        Ok(self.valid_users.contains(&token))
    }
    pub async fn add_auth(&mut self, token: String) -> Result<()> {
        self.valid_users.push(token);
        Ok(())
    }
    pub async fn sync_auth(&mut self, tokens: Vec<String>) -> Result<()> {
        self.valid_users = tokens;
        Ok(())
    }
}
