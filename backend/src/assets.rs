//! Site assets: banner and spoiler images.
//!
//! These are uploaded at runtime, so they cannot live in `frontend/res/` — that directory is
//! only copied into the served bundle when the frontend is rebuilt. They are written under
//! `FILE_STORAGE_PATH` instead, where the existing file route already serves them. Rows
//! created before this feature point at `/res/...` and keep working untouched.

use anyhow::{anyhow, bail, Result};
use common::structs::{BANNER_SIZE, SPOILER_SIZE};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Banner,
    Spoiler,
}

impl AssetKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Banner => "banner",
            Self::Spoiler => "spoiler",
        }
    }

    fn required_size(self) -> (u32, u32) {
        match self {
            Self::Banner => BANNER_SIZE,
            Self::Spoiler => SPOILER_SIZE,
        }
    }
}

/// Validate, strip and store an uploaded asset, returning the URL path it is served from.
///
/// Dimensions are exact rather than a maximum: a mis-sized banner or spoiler does not scale
/// gracefully, it breaks the layout of every board it appears on.
///
/// Re-encoding also strips metadata, the same as for post uploads — an asset is as capable of
/// carrying EXIF as any other image.
pub async fn store(data: Vec<u8>, kind: AssetKind) -> Result<String> {
    let format = image::guess_format(&data)
        .map_err(|e| anyhow!("That is not an image we can read ({e})."))?;
    let decoded = image::load_from_memory_with_format(&data, format)
        .map_err(|e| anyhow!("That image could not be read ({e})."))?;

    let (want_w, want_h) = kind.required_size();
    let (got_w, got_h) = (image::GenericImageView::width(&decoded), image::GenericImageView::height(&decoded));
    if (got_w, got_h) != (want_w, want_h) {
        bail!("{}s must be exactly {want_w}x{want_h}; that one is {got_w}x{got_h}.", kind.prefix());
    }

    let extension = match format {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::Gif => "gif",
        image::ImageFormat::WebP => "webp",
        other => bail!("{other:?} images are not accepted for site assets."),
    };

    // Re-encode rather than storing the original bytes, which is what drops any metadata.
    let mut out = std::io::Cursor::new(Vec::new());
    decoded
        .write_to(&mut out, format)
        .map_err(|e| anyhow!("That image could not be processed ({e})."))?;

    let name = format!("{}-{}.{extension}", kind.prefix(), uuid::Uuid::new_v4());
    let url_path = format!("/files/assets/{name}");
    let disk_dir = format!("{}/files/assets", env!("FILE_STORAGE_PATH"));
    tokio::fs::create_dir_all(&disk_dir).await?;
    tokio::fs::write(format!("{disk_dir}/{name}"), out.into_inner()).await?;

    Ok(url_path)
}

/// Remove an asset's file. Rows pointing at the bundled `/res/` assets are left alone, since
/// those ship with the frontend rather than being uploaded.
pub async fn remove(url_path: &str) {
    if !url_path.starts_with("/files/assets/") {
        return;
    }
    let disk = format!("{}{url_path}", env!("FILE_STORAGE_PATH"));
    if let Err(e) = tokio::fs::remove_file(&disk).await {
        log::warn!("could not remove asset {url_path}: {e}");
    }
}
