use anyhow::{anyhow, Result};
use std::collections::HashMap;

pub struct UnclaimedFiles {
    pub files: HashMap<String, (String, File, tokio::time::Instant)>,
    // pub timeouts: HashMap<String, tokio::time::Instant>,
}

impl UnclaimedFiles {
    pub fn new(files: HashMap<String, (String, File, tokio::time::Instant)>) -> Self {
        Self { files }
    }

    pub async fn add_file(&mut self, file: File, token: String) -> Result<String> {
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

                let universalfolderpath = format!("/files/{}/", file.mimetype);
                let mut universalfilepath = format!("{universalfolderpath}{id}.{}", file.extension);

                let mut diskfilepath =
                    format!("{}{}", env!("FILE_STORAGE_PATH"), universalfilepath.clone());

                // replace the file extension with the .tmp.{ext}
                // let mut tempfilepath = diskfilepath.clone().replace(
                //     &format!(".{}", file.extension),
                //     &format!(".tmp.{}", file.extension),
                // );
                // println!("{}", diskfilepath);
                // println!("{}", universalfilepath);
                // println!("{}", universalfolderpath);
                {
                    // disc actions
                    let folders = format!("{}{}", env!("FILE_STORAGE_PATH"), universalfolderpath);
                    tokio::fs::create_dir_all(folders).await?;

                    let mut num = 0;
                    while tokio::fs::metadata(diskfilepath.clone()).await.is_ok() {
                        num += 1;
                        universalfilepath =
                            format!("/files/{}/{}{}.{}", file.mimetype, id, num, file.extension);
                        diskfilepath =
                            format!("{}{}", env!("FILE_STORAGE_PATH"), universalfilepath.clone());
                        // tempfilepath = diskfilepath.clone().replace(
                        //     &format!(".{}", file.extension),
                        //     &format!(".tmp.{}", file.extension),
                        // );
                    }
                    tokio::fs::write(diskfilepath.clone(), file.data).await?;
                }

                // if i can figure it out i want to attempt to reencode the files (to their original file type) to fix any potential issues with the file. this is a very low priority task as it's not my fault if the user uploads a broken file

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
    pub mimetype: String,
    pub data: Vec<u8>,
}

impl File {
    pub fn new(extension: String, mimetype: String, data: Vec<u8>) -> Self {
        Self {
            extension,
            data,
            mimetype,
        }
    }

    pub fn builder() -> FileBuilder {
        FileBuilder::new()
    }
}

#[derive(Default)]
pub struct FileBuilder {
    extension: Option<String>,
    mimetype: Option<String>,
    data: Option<Vec<u8>>,
}

impl FileBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extension(mut self, extension: String) -> Self {
        self.extension = Some(extension);
        self
    }

    pub fn mimetype(mut self, mimetype: String) -> Self {
        self.mimetype = Some(mimetype);
        self
    }

    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = Some(data);
        self
    }

    pub fn build(self) -> Result<File> {
        Ok(File::new(
            self.extension.ok_or(anyhow!("Missing extension"))?,
            self.mimetype.ok_or(anyhow!("Missing mimetype"))?,
            self.data.ok_or(anyhow!("Missing data"))?,
        ))
    }
}
