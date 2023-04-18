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
                let filetype = file
                    .extension
                    .split('/')
                    .last()
                    .ok_or(anyhow!("Invalid file type, mimetype is effed"))?;
                let universalfolderpath = format!("/files/{}/", file.extension);
                let mut universalfilepath = format!("{}{}.{}", universalfolderpath, id, filetype);

                let mut diskfilepath =
                    format!("{}{}", env!("FILE_STORAGE_PATH"), universalfilepath.clone());

                println!("{}", diskfilepath);
                println!("{}", universalfilepath);
                println!("{}", universalfolderpath);
                {
                    // disc actions
                    let folders = format!("{}{}", env!("FILE_STORAGE_PATH"), universalfolderpath);
                    tokio::fs::create_dir_all(folders).await?;

                    let mut num = 0;
                    while tokio::fs::metadata(diskfilepath.clone()).await.is_ok() {
                        num += 1;
                        universalfilepath =
                            format!("/files/{}/{}{num}.{}", file.extension, id, file.extension);
                        diskfilepath =
                            format!("{}{}", env!("FILE_STORAGE_PATH"), universalfilepath.clone());
                    }
                    tokio::fs::write(diskfilepath.clone(), file.data).await?;
                }
                // tokio spawn blocking thread to create thumbnails
                let handle = tokio::task::spawn(async move {
                    let thumbpath = format!("{diskfilepath}-thumb.jpg");
                    // use the ffmpeg command `ffmpeg -i {file} -r 1 -vf scale=80:-2 -frames:v 1 {id}.jpg -y` to create a thumbnail for the file. return a result of the filepath OR an error if it is not created. so we can delete the file and reject the post with an invalid file error
                    let output = tokio::process::Command::new("ffmpeg")
                        .args(["-i", &diskfilepath])
                        .args(["-r", "1"])
                        .args(["-vf", "scale=80:-2"])
                        .args(["-frames:v", "1"])
                        .arg(&thumbpath)
                        .arg("-y")
                        .output()
                        .await;

                    if output.is_ok() {
                        // check if the file was created
                        if tokio::fs::metadata(thumbpath.clone()).await.is_ok() {
                            Ok(thumbpath)
                        } else {
                            Err(diskfilepath)
                        }
                    } else {
                        Err(diskfilepath)
                    }
                });
                let path = match handle.await? {
                    Ok(path) => path,
                    Err(f) => {
                        // remove the file
                        tokio::fs::remove_file(f).await?;
                        return Err(anyhow!("Invalid file"));
                    }
                };
                println!("Thumbnail created at {path}");
                Ok(universalfilepath)
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
