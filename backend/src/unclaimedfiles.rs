use anyhow::{anyhow, Result};
use std::collections::HashMap;
use thumbnailer::{create_thumbnails, ThumbnailSize};

pub struct UnclaimedFiles {
    pub files: HashMap<String, (String, File, tokio::time::Instant)>,
    // pub timeouts: HashMap<String, tokio::time::Instant>,
}

impl UnclaimedFiles {
    pub fn new(files: HashMap<String, (String, File, tokio::time::Instant)>) -> Self {
        Self { files }
    }

    pub async fn add_file(
        &mut self,
        extension: String,
        data: Vec<u8>,
        token: String,
    ) -> Result<String> {
        let file = File::new(extension, data);
        for _ in 0..3 {
            let id = nanoid::nanoid!(16);
            if !self.files.contains_key(&id) {
                self.files
                    .insert(token, (id.clone(), file, tokio::time::Instant::now()));
                return Ok(id);
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Err(anyhow!("Failed to generate unique id, try again later"))
    }

    pub async fn claim_file(&mut self, id: &str, token: String) -> Result<String> {
        match self.files.remove(&token) {
            Some((tid, file, _)) => {
                if tid != id {
                    return Err(anyhow!("Invalid id"));
                }
                let folder_path = format!(
                    "{}/files/full/{}s/",
                    env!("FILE_STORAGE_PATH"),
                    file.extension
                );
                tokio::fs::create_dir_all(folder_path).await?;
                let mut universalpath =
                    format!("/files/full/{}s/{}.{}", file.extension, id, file.extension);
                let mut filepath =
                    format!("{}{}", env!("FILE_STORAGE_PATH"), universalpath.clone());
                let mut num = 0;
                while tokio::fs::metadata(filepath.clone()).await.is_ok() {
                    num += 1;
                    universalpath = format!(
                        "/files/full/{}s/{}{num}.{}",
                        file.extension, id, file.extension
                    );
                    filepath = format!("{}{}", env!("FILE_STORAGE_PATH"), universalpath.clone());
                }
                tokio::fs::write(filepath.clone(), file.data).await?;

                // tokio spawn blocking thread to create thumbnails
                let umm = file.extension.clone();
                let nid = id.to_owned();
                let handle = tokio::task::spawn_blocking(move || {
                    let file = std::fs::File::open(filepath.clone())?;
                    let reader = std::io::BufReader::new(file);
                    let mut thumbnails =
                        create_thumbnails(reader, mime::IMAGE_PNG, [ThumbnailSize::Small])?;
                    let thumb = thumbnails
                        .pop()
                        .ok_or(anyhow!("Failed to create thumbnail for file {}", filepath))?;
                    let mut buf = std::io::Cursor::new(Vec::new());
                    thumb.write_png(&mut buf)?;
                    let newpath = filepath.replace("full", "thumb");
                    std::fs::create_dir_all(newpath.replace(&format!("/{nid}.{umm}"), ""))?;
                    Ok::<(String, Vec<u8>), anyhow::Error>((newpath, buf.into_inner()))
                });
                let (location, data) = handle.await??;
                tokio::fs::write(location, data).await?;

                Ok(universalpath)
            }
            None => Err(anyhow!("File not found")),
        }
    }

    pub async fn trim_files(&mut self) -> Result<()> {
        let mut to_remove = Vec::new();
        for (token, (_, _, timeout)) in self.files.iter() {
            if timeout.elapsed().as_secs() > env!("FILE_LIFESPAN").parse::<u64>()? {
                to_remove.push(token.clone());
            }
        }
        for token in to_remove {
            self.files.remove(&token);
        }
        Ok(())
    }

    pub fn has_pending(&self, token: &str) -> bool {
        self.files.contains_key(token)
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
