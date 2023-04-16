use anyhow::{anyhow, Result};
use std::collections::HashMap;

pub struct UnclaimedFiles {
    pub files: HashMap<String, File>,
}

impl UnclaimedFiles {
    pub fn new(files: HashMap<String, File>) -> Self {
        Self { files }
    }

    pub async fn add_file(&mut self, extension: String, data: Vec<u8>) -> Result<String> {
        let file = File::new(extension, data);
        for _ in 0..3 {
            let id = nanoid::nanoid!(16);
            if !self.files.contains_key(&id) {
                self.files.insert(id.clone(), file);
                return Ok(id);
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Err(anyhow!("Failed to generate unique id, try again later"))
    }

    pub async fn get_file(&mut self, id: &str) -> Result<File> {
        match self.files.remove(id) {
            Some(file) => Ok(file),
            None => Err(anyhow!("File not found")),
        }
    }
}

pub struct File {
    pub extension: String,
    pub data: Vec<u8>,
}

impl File {
    pub fn new(extension: String, data: Vec<u8>) -> Self {
        Self { extension, data }
    }
}
