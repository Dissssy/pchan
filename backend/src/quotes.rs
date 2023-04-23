use anyhow::Result;
use rand::seq::SliceRandom;

pub struct Quotes {
    quotes: Vec<String>,
}

impl Quotes {
    pub fn load(path: String) -> Result<Self> {
        let quotes = std::fs::read_to_string(path)?;
        let quotes = quotes.lines().map(|x| x.to_string()).collect();
        Ok(Self { quotes })
    }

    pub fn random_quote(&self) -> String {
        self.quotes
            .choose(&mut rand::thread_rng())
            .unwrap_or(&"No quotes found".to_string())
            .to_string()
    }
}
