use std::{str::FromStr, sync::Arc};

use notify::Watcher as _;
use rand::rngs::ThreadRng;
use rsa::traits::PrivateKeyParts;
use tokio::sync::Mutex;

lazy_static::lazy_static!(
    pub static ref TRIM_TIME: u64 = env!("TRIM_TIME").parse::<u64>().expect("TRIM_TIME must be a valid u64");
    pub static ref TOKEN: String = env!("TOKEN").to_owned();
    pub static ref HASH_SALT: String = env!("HASH_SALT").to_owned();
    pub static ref DELETE_TIME: u64 = env!("DELETE_TIME").parse::<u64>().expect("DELETE_TIME must be a valid u64");
    pub static ref TOKEN_SALT: String = env!("TOKEN_SALT").to_owned();
    pub static ref KNOWN_SCRAPERS: Vec<&'static str> = vec!["Mozilla/5.0 (compatible; Discordbot/2.0; +https://discordapp.com)"];
    pub static ref BASE_THUMBNAIL: &'static [u8] = include_bytes!(env!("BASE_THUMBNAIL_PATH"));
    pub static ref BASE_THUMBNAIL_LARGE: &'static [u8] = include_bytes!(env!("BASE_THUMBNAIL_LARGE_PATH"));
    pub static ref FONT: &'static [u8] = include_bytes!(env!("FONT_PATH"));
    pub static ref RANDOM_KEY: &'static [u8] = Box::leak(Box::new(nanoid::nanoid!(32))).as_bytes();
    pub static ref HMAC_KEY_GENERATOR: HmacKeyGenerator = HmacKeyGenerator::new(env!("HMAC_KEY_PATH"));
);

#[cfg(feature = "base64_no_pad")]
pub const BASE64_ENGINE: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

#[cfg(not(feature = "base64_no_pad"))]
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE;

pub const FILE_SHARE_DURATION: u64 = 60 * 60 * 24 * 7; // 1 week in seconds

pub struct HmacKeyGenerator {
    _watcher: notify::RecommendedWatcher,
    key: Arc<Mutex<Vec<u8>>>,
}

impl HmacKeyGenerator {
    pub fn new(file_path: &'static str) -> Self {
        let mut rng = rand::thread_rng();
        let key = Arc::new(Mutex::new({
            // attempt to retrieve from disc, if failure, generate a new key and write to disc, if writing fails panic
            match std::fs::read(file_path) {
                Ok(key) => key,
                Err(_) => generate_and_write_key(file_path, &mut rng)
                    .expect("Failed to write HMAC key to disc"),
            }
        }));

        let watcher = {
            let key = Arc::clone(&key);
            let the_file_path = std::path::PathBuf::from_str(file_path)
                .expect("Failed to convert HMAC key path to PathBuf");
            let mut watcher = notify::recommended_watcher(move |res| {
                if let Ok(notify::Event {
                    kind: notify::EventKind::Remove(_),
                    paths: _,
                    attrs: _,
                }) = res
                {
                    println!("HMAC key file removed, generating new key");
                    // ensure the key file no longer exists
                    if std::fs::metadata(file_path).is_ok() {
                        println!("HMAC key file still exists, removing");
                        let _ = std::fs::remove_file(file_path);
                    }
                    let mut key = key.blocking_lock();
                    *key = generate_and_write_key(file_path, &mut rand::thread_rng())
                        .expect("Failed to write HMAC key to disc");
                    println!("New HMAC key generated");
                } else {
                    println!("Failed to handle event: {:?}", res);
                }
            })
            .expect("Failed to create watcher");

            watcher
                .watch(the_file_path.as_path(), notify::RecursiveMode::NonRecursive)
                .expect("Failed to watch HMAC key file");

            watcher
        };

        Self {
            _watcher: watcher, // just held so it doesn't get dropped
            key,
        }
    }
    pub async fn get_key(&self) -> Vec<u8> {
        self.key.lock().await.clone()
    }
}

fn generate_and_write_key(file_path: &str, rng: &mut ThreadRng) -> std::io::Result<Vec<u8>> {
    let key = rsa::RsaPrivateKey::new(rng, 4096)
        .expect("Failed to generate RSA key")
        .d()
        .to_bytes_be();
    std::fs::write(file_path, &key)?;
    Ok(key)
}
