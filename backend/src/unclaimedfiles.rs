use anyhow::{anyhow, Result};
use std::collections::HashMap;

pub struct UnclaimedFiles {
    pub files: HashMap<String, File>,
    pub timeouts: HashMap<String, tokio::time::Instant>,
}

impl UnclaimedFiles {
    pub fn new(files: HashMap<String, File>, timeouts: HashMap<String, tokio::time::Instant>) -> Self {
        Self { files, timeouts }
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

    pub async fn claim_file(&mut self, id: &str) -> Result<String> {
        match self.files.remove(id) {
            Some(file) => {
                let universalpath = format!("/{}s/{}.{}", file.extension, id, file.extension);
                tokio::fs::write(format!("{}{}", std::env::var("UPLOAD_FOLDER")?, universalpath.clone()), file.data).await?;
                Ok(universalpath)
            }
            None => Err(anyhow!("File not found")),
        }
    }

    pub async fn trim_files(&mut self) -> Result<()> {
        let mut to_remove = Vec::new();
        for (id, timeout) in self.timeouts.iter() {
            if timeout.elapsed().as_secs() > env!("FILE_LIFESPAN").parse::<u64>()? {
                to_remove.push(id.clone());
            }
        }
        for id in to_remove {
            self.files.remove(&id);
            self.timeouts.remove(&id);
        }
        Ok(())
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
