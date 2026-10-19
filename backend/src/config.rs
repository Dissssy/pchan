//! Runtime configuration, loaded from the environment (and `backend/.env`).
//!
//! Secrets live here rather than in `.cargo/config.toml` `[env]` for two reasons: compile-time
//! `env!()` bakes them into the shipped binary, and rotating any of them currently requires a
//! full recompile.
//!
//! Everything is read and validated once at startup so a misconfiguration is a refusal to boot
//! rather than a failure part-way through a request.

// Consumed by the Auth0 login flow; the scaffolding lands first so configuration can be
// prepared and validated independently of that work.
#![allow(dead_code)]

use anyhow::{bail, Context as _, Result};

#[derive(Debug)]
pub struct Config {
    pub auth0: Auth0,
    pub keys: Keys,
    /// Public origin, e.g. `https://pchan.p51.nl`. No trailing slash.
    pub public_url: String,
    /// Whether to mark the session cookie `Secure`. Must be true in production; settable to
    /// false only so the site can be exercised over plain HTTP locally.
    pub cookie_secure: bool,
    pub vapid: Vapid,
}

/// The key pair the server signs web pushes with.
///
/// Only the private half is configured. The public half is what the browser needs to subscribe
/// (`applicationServerKey`), and it is worked out from the private one, so the two cannot be
/// set to values that do not belong together, and there is nothing second to rotate.
#[derive(Debug)]
pub struct Vapid {
    /// `VAPID_PRIVATE_KEY`: the P-256 private key, base64url without padding.
    pub private_key: String,
    /// The matching public key in the form `pushManager.subscribe` takes.
    pub public_key: String,
}

impl Vapid {
    pub fn from_private(private_key: &str) -> Result<Self> {
        use base64::Engine as _;
        let private_key = private_key.trim();
        // The library panics, rather than failing, on a key of the wrong length, so a typo here
        // would otherwise crash startup with an unhelpful message. A P-256 scalar is 32 bytes.
        let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(private_key)
            .map_err(|e| anyhow::anyhow!("VAPID_PRIVATE_KEY is not base64url ({e})"))?;
        if raw.len() != 32 {
            bail!(
                "VAPID_PRIVATE_KEY decodes to {} bytes, not 32. It must be the PRIVATE key (the \
                 public one is 65 bytes), base64url without padding, as printed by \
                 `npx web-push generate-vapid-keys`",
                raw.len()
            );
        }
        let builder = web_push::VapidSignatureBuilder::from_base64_no_sub(private_key).map_err(|e| {
            anyhow::anyhow!(
                "VAPID_PRIVATE_KEY is not a usable P-256 private key ({e}); it is the base64url \
                 (no padding) form, as printed by `npx web-push generate-vapid-keys`"
            )
        })?;
        Ok(Self {
            private_key: private_key.to_owned(),
            public_key: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(builder.get_public_key()),
        })
    }
}

#[derive(Debug)]
pub struct Auth0 {
    /// Tenant domain, e.g. `example.eu.auth0.com`. No scheme.
    pub domain: String,
    pub client_id: String,
    pub client_secret: String,
}

impl Auth0 {
    pub fn authorize_url(&self) -> String {
        format!("https://{}/authorize", self.domain)
    }
    pub fn token_url(&self) -> String {
        format!("https://{}/oauth/token", self.domain)
    }
    pub fn jwks_url(&self) -> String {
        format!("https://{}/.well-known/jwks.json", self.domain)
    }
    pub fn issuer(&self) -> String {
        format!("https://{}/", self.domain)
    }
    /// Ends the Auth0 session itself, then sends the browser to `return_to`, which must be
    /// listed under the application's Allowed Logout URLs or Auth0 shows an error page.
    pub fn logout_url(&self, return_to: &str, encode: impl Fn(&str) -> String) -> String {
        format!(
            "https://{}/v2/logout?client_id={}&returnTo={}",
            self.domain,
            encode(&self.client_id),
            encode(return_to),
        )
    }
    pub fn redirect_uri(&self, public_url: &str) -> String {
        format!("{public_url}/login/callback")
    }
}

/// Keyed-hash secrets. These are held outside the database on purpose: with them, a database
/// dump links a join request to a member and a member to their posts; without them, it does
/// not. Treat them as equivalent in sensitivity to the identity table itself.
#[derive(Debug)]
pub struct Keys {
    /// HMAC key for `identities.identity_hash`.
    pub identity: Vec<u8>,
    /// HMAC key for `join_requests.request_hash`. MUST differ from `identity`.
    pub request: Vec<u8>,
    /// HMAC key for `posts.actual_author`.
    pub post_author: Vec<u8>,
    /// HMAC key for `user_tags.invite_hash` and `generated_by`.
    pub invite: Vec<u8>,
    /// Encryption key for stored push subscriptions.
    pub push_encryption: Vec<u8>,
}

static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();

/// Read and validate configuration. Call once, early in `main`, before anything serves.
pub fn init() -> Result<()> {
    let cfg = Config::load()?;

    // The database crate computes every keyed hash, so it needs the key material. Skipping
    // this makes the first keyed hash panic at request time instead of failing at startup,
    // which reaches the user as a 502 from the reverse proxy.
    database::keys::init(database::keys::Keyring {
        identity: cfg.keys.identity.clone(),
        request: cfg.keys.request.clone(),
        post_author: cfg.keys.post_author.clone(),
        invite: cfg.keys.invite.clone(),
        push_encryption: cfg.keys.push_encryption.clone(),
    })?;

    let _ = CONFIG.set(cfg);
    Ok(())
}

/// The validated configuration. Panics if [`init`] has not run, which is a programming error
/// rather than a runtime condition.
pub fn get() -> &'static Config {
    CONFIG
        .get()
        .expect("config::init() must be called before config::get()")
}

/// Load the `.env` file into the process environment, if one can be found.
///
/// Values already set in the real environment win, so a deployment can override the file
/// without editing it. Absence of a file is not an error — the environment may be populated
/// by other means.
///
/// Search order, because the binary is launched from several different directories (the repo
/// root during development, `backend/` by the diesel CLI, and `stage/` once deployed):
///
/// 1. `PCHAN_ENV_FILE`, if set — an explicit path always wins.
/// 2. `.env` in the working directory or any parent (dotenvy's own search).
/// 3. `backend/.env` relative to the working directory.
/// 4. `.env` alongside the executable.
///
/// This runs before the logger is initialised, so it reports to stderr. Only paths are
/// printed, never values.
pub fn load_dotenv() {
    if let Ok(path) = std::env::var("PCHAN_ENV_FILE") {
        match dotenvy::from_path(&path) {
            Ok(()) => {
                eprintln!("config: loaded {path}");
                return;
            }
            Err(e) => eprintln!("config: PCHAN_ENV_FILE={path} could not be read: {e}"),
        }
    }

    if let Ok(path) = dotenvy::dotenv() {
        eprintln!("config: loaded {}", path.display());
        return;
    }

    let mut candidates = vec![std::path::PathBuf::from("backend/.env")];
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Into::into)) {
        let dir: std::path::PathBuf = dir;
        candidates.push(dir.join(".env"));
    }

    for candidate in candidates {
        if dotenvy::from_path(&candidate).is_ok() {
            eprintln!("config: loaded {}", candidate.display());
            return;
        }
    }

    eprintln!("config: no .env file found; using the environment as-is");
}

impl Config {
    pub fn load() -> Result<Self> {
        let keys = Keys {
            identity: key("IDENTITY_KEY")?,
            request: key("REQUEST_KEY")?,
            post_author: key("POST_AUTHOR_KEY")?,
            invite: key("INVITE_KEY")?,
            push_encryption: key("PUSH_ENCRYPTION_KEY")?,
        };
        keys.check_distinct()?;

        let public_url = var("PUBLIC_URL")?.trim_end_matches('/').to_owned();
        let cookie_secure = flag("COOKIE_SECURE", true)?;

        if !cookie_secure && public_url.starts_with("https://") {
            bail!(
                "COOKIE_SECURE is false but PUBLIC_URL is https. That would hand out session \
                 cookies usable over plain HTTP; set COOKIE_SECURE=true."
            );
        }

        Ok(Self {
            auth0: Auth0 {
                domain: var("AUTH0_DOMAIN")?
                    .trim_start_matches("https://")
                    .trim_end_matches('/')
                    .to_owned(),
                client_id: var("AUTH0_CLIENT_ID")?,
                client_secret: var("AUTH0_CLIENT_SECRET")?,
            },
            keys,
            public_url,
            cookie_secure,
            vapid: Vapid::from_private(&var("VAPID_PRIVATE_KEY")?)?,
        })
    }
}

impl Keys {
    /// Reusing one value across these defeats the point of having separate keys. In
    /// particular, `identity` and `request` being equal would make `identity_hash` and
    /// `request_hash` equal for the same Auth0 subject, which is exactly the join from a
    /// pending join request to the resulting member that the schema exists to prevent.
    fn check_distinct(&self) -> Result<()> {
        let named: [(&str, &[u8]); 5] = [
            ("IDENTITY_KEY", &self.identity),
            ("REQUEST_KEY", &self.request),
            ("POST_AUTHOR_KEY", &self.post_author),
            ("INVITE_KEY", &self.invite),
            ("PUSH_ENCRYPTION_KEY", &self.push_encryption),
        ];
        for i in 0..named.len() {
            for j in (i + 1)..named.len() {
                if named[i].1 == named[j].1 {
                    bail!(
                        "{} and {} are set to the same value; every key must be independently \
                         generated (openssl rand -hex 32)",
                        named[i].0,
                        named[j].0
                    );
                }
            }
        }
        Ok(())
    }
}

fn var(name: &str) -> Result<String> {
    let value = std::env::var(name)
        .with_context(|| format!("{name} is not set (see backend/.env.example)"))?;
    if value.trim().is_empty() {
        bail!("{name} is set but empty (see backend/.env.example)");
    }
    Ok(value)
}

fn flag(name: &str, default: bool) -> Result<bool> {
    match std::env::var(name) {
        Err(_) => Ok(default),
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            other => bail!("{name} must be true or false, got {other:?}"),
        },
    }
}

fn key(name: &str) -> Result<Vec<u8>> {
    parse_key(name, &var(name)?)
}

fn parse_key(name: &str, raw: &str) -> Result<Vec<u8>> {
    let bytes = hex::decode(raw.trim())
        .with_context(|| format!("{name} must be hex (generate with: openssl rand -hex 32)"))?;
    if bytes.len() < 32 {
        bail!(
            "{name} must be at least 32 bytes, got {}. Generate with: openssl rand -hex 32",
            bytes.len()
        );
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys_from(identity: &[u8], request: &[u8]) -> Keys {
        Keys {
            identity: identity.to_vec(),
            request: request.to_vec(),
            post_author: vec![3; 32],
            invite: vec![4; 32],
            push_encryption: vec![5; 32],
        }
    }

    #[test]
    fn the_public_vapid_key_follows_from_the_private_one() {
        use base64::Engine as _;
        // A throwaway key, not one in use anywhere.
        let private = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode((1u8..=32).collect::<Vec<_>>());
        let vapid = Vapid::from_private(&format!("  {private}\n")).expect("a valid key, whitespace tolerated");
        assert_eq!(vapid.private_key, private);

        // Browsers want an uncompressed P-256 point: 0x04 followed by two 32-byte coordinates.
        let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&vapid.public_key)
            .expect("base64url");
        assert_eq!(public.len(), 65);
        assert_eq!(public[0], 4);
        assert_eq!(vapid.public_key, Vapid::from_private(&private).unwrap().public_key, "stable");
    }

    #[test]
    fn a_bad_vapid_key_is_refused_at_startup() {
        for bad in ["", "not base64!", "AAAA", "BD5mCRfVrSTvzmpHYulsiKI5ogPBC9y_K-JJBVxavAqulOQP94lgFYoEfg93xy7ZPGKapn8jAyamwHukxYaQFHA"] {
            // The last is a *public* key, the easy mistake of pasting the wrong half.
            assert!(Vapid::from_private(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn rejects_reused_key() {
        // identity == request would make identity_hash and request_hash equal for the same
        // Auth0 subject, joining a pending join request straight to the member it created.
        let err = keys_from(&[1; 32], &[1; 32])
            .check_distinct()
            .expect_err("reusing a key must be refused");
        let msg = err.to_string();
        assert!(msg.contains("IDENTITY_KEY") && msg.contains("REQUEST_KEY"), "{msg}");
    }

    #[test]
    fn accepts_distinct_keys() {
        keys_from(&[1; 32], &[2; 32])
            .check_distinct()
            .expect("independently generated keys are fine");
    }

    #[test]
    fn rejects_short_or_malformed_keys() {
        assert!(parse_key("K", &hex::encode([7; 31])).is_err(), "31 bytes is too short");
        assert!(parse_key("K", "nothexatall").is_err(), "non-hex must be refused");
        parse_key("K", &hex::encode([7; 32])).expect("32 bytes of hex is valid");
        parse_key("K", &format!("  {}\n", hex::encode([7; 32]))).expect("surrounding space is ok");
    }
}
