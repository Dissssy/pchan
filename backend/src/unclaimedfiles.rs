use anyhow::{anyhow, Result};
use common::structs::ClaimedFileInfo;
use imageproc::drawing::draw_text_mut;
use std::collections::HashMap;


pub struct UnclaimedFiles {
    pub files: HashMap<String, (String, File, tokio::time::Instant)>,
}

impl UnclaimedFiles {
    pub fn new(files: HashMap<String, (String, File, tokio::time::Instant)>) -> Self {
        Self { files }
    }

    pub async fn add_file(&mut self, file: File, viewer: &database::Viewer) -> Result<String> {
        for _ in 0..3 {
            let id = nanoid::nanoid!(16);
            if let std::collections::hash_map::Entry::Vacant(e) =
                self.files.entry(viewer.id.to_string())
            {
                e.insert((id.clone(), file, tokio::time::Instant::now()));
                return Ok(id);
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Err(anyhow!("Failed to generate unique id, try again later"))
    }

    pub async fn claim_file(
        &mut self,
        createfile: &common::structs::CreateFile,
        viewer: &database::Viewer,
        is_thread_post: bool,
    ) -> Result<ClaimedFile> {
        match self.files.remove(&viewer.id.to_string()) {
            Some((tid, file, _)) => {
                if tid != createfile.id {
                    return Err(anyhow!("Invalid id"));
                }

                if file.mimetype.split('/').next() == Some("application") {
                    return Err(anyhow!("Invalid file type"));
                }

                // Strip embedded metadata before the bytes are ever stored. Phone photos
                // carry GPS coordinates, camera serial numbers and timestamps in EXIF, and
                // uploads were previously written verbatim and served to every visitor — on
                // a board whose whole point is that posts are not attributable.
                //
                // Deliberately fails the upload rather than storing an unstripped file: a
                // privacy control that silently falls open is worse than one that refuses.
                let file = File {
                    data: strip_metadata(file.data, &file.mimetype, &file.extension).await?,
                    ..file
                };

                let universalfolderpath = format!("/files/{}/", file.mimetype);
                let mut universalfilepath = format!(
                    "{}{}.{}",
                    universalfolderpath, createfile.id, file.extension
                );

                let mut diskfilepath =
                    format!("{}{}", env!("FILE_STORAGE_PATH"), universalfilepath.clone());

                let filehash = common::hash_file(&file.data);
                {
                    let folders = format!("{}{}", env!("FILE_STORAGE_PATH"), universalfolderpath);
                    tokio::fs::create_dir_all(folders).await?;

                    let mut num = 0;
                    while tokio::fs::metadata(diskfilepath.clone()).await.is_ok() {
                        num += 1;
                        universalfilepath = format!(
                            "/files/{}/{}{}.{}",
                            file.mimetype, createfile.id, num, file.extension
                        );
                        diskfilepath =
                            format!("{}{}", env!("FILE_STORAGE_PATH"), universalfilepath.clone());
                    }
                    tokio::fs::write(diskfilepath.clone(), file.data).await?;
                }

                let ext = file.extension.to_uppercase();
                let handle = tokio::task::spawn(async move {
                    let thumbpath = format!("{diskfilepath}-thumb.jpg");
                    let output = tokio::process::Command::new("ffmpeg")
                        .args(["-i", &diskfilepath])
                        .args(["-r", "1"])
                        .args(if is_thread_post {
                            ["-vf", "scale=200:-2"]
                        } else {
                            ["-vf", "scale=80:-2"]
                        })
                        .args(["-frames:v", "1"])
                        .arg(&thumbpath)
                        .arg("-y")
                        .output()
                        .await;

                    if output.is_ok() {
                        if tokio::fs::metadata(thumbpath.clone()).await.is_ok() {
                            Ok(thumbpath)
                        } else {
                            // FFMPEG failed to create thumbnail, manually create one with the file type printed over our default thumbnail

                            let mut img = match image::load_from_memory(if is_thread_post {
                                *crate::statics::BASE_THUMBNAIL_LARGE
                            } else {
                                *crate::statics::BASE_THUMBNAIL
                            }) {
                                Ok(img) => img,
                                Err(e) => {
                                    log::error!("Failed to load base thumbnail: {e:?}");
                                    return Err(diskfilepath);
                                }
                            };

                            let font = match ab_glyph::FontVec::try_from_vec(
                                (*crate::statics::FONT).to_vec(),
                            ) {
                                Ok(f) => f,
                                Err(e) => {
                                    log::error!("Failed to load font: {e:?}");
                                    return Err(diskfilepath);
                                }
                            };

                            let scale = if is_thread_post {
                                ab_glyph::PxScale { x: 75.0, y: 75.0 }
                            } else {
                                ab_glyph::PxScale { x: 30.0, y: 30.0 }
                            };

                            // draw_text_mut(canvas, color, x, y, scale, font, text)
                            draw_text_mut(
                                &mut img,
                                image::Rgba([127, 127, 127, 255]),
                                if is_thread_post { 13 } else { 5 },
                                if is_thread_post { 50 } else { 20 },
                                scale,
                                &font,
                                &ext,
                            );

                            match img.save(&thumbpath) {
                                Ok(_) => Ok(thumbpath),
                                Err(e) => {
                                    log::error!("Failed to save thumbnail: {e:?}");
                                    Err(diskfilepath)
                                }
                            }
                        }
                    } else {
                        Err(diskfilepath)
                    }
                });
                let path = match handle.await? {
                    Ok(path) => path,
                    Err(f) => {
                        tokio::fs::remove_file(f).await?;
                        return Err(anyhow!("Invalid file"));
                    }
                };
                Ok(ClaimedFile {
                    info: ClaimedFileInfo {
                        path: universalfilepath,
                        thumbnail: path,
                        spoiler: createfile.spoiler,
                    },
                    hash: filehash,
                })
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

    pub fn has_pending(&self, viewer: &database::Viewer) -> bool {
        self.files.contains_key(&viewer.id.to_string())
    }
}

/// What claiming an upload produces.
///
/// The content hash is kept beside the client-facing info rather than inside it: it is used
/// for duplicate detection, and publishing it would let anyone holding a copy of a file
/// obtained elsewhere hash it and locate the exact post.
pub struct ClaimedFile {
    pub info: ClaimedFileInfo,
    pub hash: String,
}

/// Remove embedded metadata from an upload.
///
/// Images are decoded and re-encoded, which cannot carry metadata through. That is lossy for
/// JPEG, but it is the only guaranteed strip, and an imageboard re-encodes for thumbnails
/// anyway. GIF is passed through: the format has no EXIF, and decoding would flatten an
/// animation to a single frame.
///
/// Everything else (video, audio) goes through ffmpeg with `-map_metadata -1 -c copy`, which
/// drops container tags without re-encoding the stream.
async fn strip_metadata(data: Vec<u8>, mimetype: &str, extension: &str) -> Result<Vec<u8>> {
    let kind = mimetype.split('/').next().unwrap_or("");
    let ext = extension.to_ascii_lowercase();

    if kind == "image" {
        // GIF has no EXIF chunk and re-encoding would lose the animation.
        if ext == "gif" {
            return Ok(data);
        }

        let format = image::guess_format(&data)
            .map_err(|e| anyhow!("Could not identify that image ({e}); it was not accepted."))?;
        let decoded = image::load_from_memory_with_format(&data, format)
            .map_err(|e| anyhow!("Could not read that image ({e}); it was not accepted."))?;

        let mut out = std::io::Cursor::new(Vec::new());
        decoded
            .write_to(&mut out, format)
            .map_err(|e| anyhow!("Could not re-encode that image ({e}); it was not accepted."))?;
        return Ok(out.into_inner());
    }

    // ffmpeg needs real files, and it is already a hard dependency for thumbnails.
    let dir = std::env::temp_dir();
    let stamp = uuid::Uuid::new_v4();
    let input = dir.join(format!("pchan-in-{stamp}.{ext}"));
    let output = dir.join(format!("pchan-out-{stamp}.{ext}"));

    tokio::fs::write(&input, &data).await?;

    let result = tokio::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(&input)
        .args(["-map_metadata", "-1", "-c", "copy"])
        .arg(&output)
        .arg("-y")
        .output()
        .await;

    let stripped = match result {
        Ok(out) if out.status.success() => tokio::fs::read(&output).await.ok(),
        Ok(out) => {
            log::warn!(
                "ffmpeg could not strip metadata: {}",
                String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
            );
            None
        }
        Err(e) => {
            log::warn!("could not run ffmpeg to strip metadata: {e}");
            None
        }
    };

    let _ = tokio::fs::remove_file(&input).await;
    let _ = tokio::fs::remove_file(&output).await;

    stripped.ok_or_else(|| anyhow!("Could not process that file; it was not accepted."))
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
