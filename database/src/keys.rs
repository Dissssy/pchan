//! Keyed hashes for every value that could otherwise link a person to their activity.
//!
//! The old scheme was `sha256(value ‖ salt)` with the salt compiled into the binary. That is
//! recoverable by brute force from a database dump alone: the inputs (a Discord snowflake, a
//! board id, a post id) come from small enumerable sets. These are HMACs under keys held
//! outside the database, so a dump on its own links nothing.
//!
//! Two properties this module exists to guarantee:
//!
//! * **`identity` and `request` are separate keys.** Both hash the same Auth0 subject. If they
//!   were one key the two hashes would be equal, and anyone holding the database could join a
//!   pending join request to the member it eventually created — the exact correlation the
//!   schema is built to prevent.
//! * **Every use is domain-separated.** A label is mixed into each hash so the same key
//!   material cannot produce a value that is meaningful in another context.
//!
//! This crate is not a dependency of the frontend, so none of this reaches the WASM bundle.

use anyhow::{bail, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::OnceLock;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Raw key material. Validation (length, distinctness) happens where it is read from the
/// environment; this type is the handoff into the database layer.
pub struct Keyring {
    pub identity: Vec<u8>,
    pub request: Vec<u8>,
    pub post_author: Vec<u8>,
    pub invite: Vec<u8>,
    pub push_encryption: Vec<u8>,
}

static KEYRING: OnceLock<Keyring> = OnceLock::new();

/// Install the process keyring. Call once during startup, before serving.
pub fn init(keyring: Keyring) -> Result<()> {
    if KEYRING.set(keyring).is_err() {
        bail!("keyring was already initialised");
    }
    Ok(())
}

fn keyring() -> &'static Keyring {
    KEYRING
        .get()
        .expect("database::keys::init() must be called before any keyed hash is computed")
}

/// Domain-separated HMAC-SHA256, hex encoded.
///
/// The domain label is length-prefixed so that no combination of label and payload can be
/// confused with a different combination.
fn mac(key: &[u8], domain: &str, parts: &[&[u8]]) -> String {
    let mut m = <HmacSha256 as Mac>::new_from_slice(key)
        .expect("HMAC accepts keys of any length");
    m.update(&(domain.len() as u64).to_be_bytes());
    m.update(domain.as_bytes());
    for part in parts {
        m.update(&(part.len() as u64).to_be_bytes());
        m.update(part);
    }
    hex::encode(m.finalize().into_bytes())
}

/// `identities.identity_hash` — resolves an Auth0 subject to a member.
pub fn identity_hash(subject: &str) -> String {
    mac(&keyring().identity, "identity:v1", &[subject.as_bytes()])
}

/// `join_requests.request_hash` / `request_blocks.request_hash`.
///
/// Deliberately keyed differently from [`identity_hash`] so the two cannot be correlated.
pub fn request_hash(subject: &str) -> String {
    mac(&keyring().request, "request:v1", &[subject.as_bytes()])
}

/// `posts.actual_author` — proves authorship for deletion without storing who wrote what.
///
/// Keyed on the member's rotatable `author_seed` rather than their id, so rotating the seed
/// severs the link to everything they have already posted.
pub fn post_author_hash(author_seed: &str, post_id: i64) -> String {
    mac(
        &keyring().post_author,
        "post-author:v1",
        &[author_seed.as_bytes(), &post_id.to_be_bytes()],
    )
}

/// `user_tags.invite_hash` — a member's access grant for one board.
///
/// Keyed on the member id, not the author seed: rotating a posting identity must not cost
/// someone the boards they were invited to.
pub fn invite_hash(member_id: &Uuid, board_id: i64) -> String {
    mac(
        &keyring().invite,
        "invite:v1",
        &[member_id.as_bytes(), &board_id.to_be_bytes()],
    )
}

/// What a browser's memory of "which posts are mine" is tied to.
///
/// Keyed on the member and on their current author seed: a different member signing in on the
/// same browser, or this member starting a new posting identity, both change it, and the
/// browser throws the memory away. It is only ever sent to the member it describes.
pub fn authored_cache_key(member_id: &Uuid, author_seed: &str) -> String {
    mac(
        &keyring().invite,
        "authored-cache:v1",
        &[member_id.as_bytes(), author_seed.as_bytes()],
    )[..16]
        .to_owned()
}

/// MAC for the short-lived join ticket, which carries a request hash across the one round
/// trip between logging in and submitting a join request (there is no session yet).
///
/// Keyed with the request key under its own domain label, so a ticket cannot be substituted
/// for a `request_hash` or vice versa.
pub fn ticket_mac(request_hash: &str, expiry: i64) -> String {
    mac(
        &keyring().request,
        "join-ticket:v1",
        &[request_hash.as_bytes(), &expiry.to_be_bytes()],
    )
}

/// Hash a session cookie for storage in `sessions.token_hash`.
///
/// Unkeyed on purpose. The token is 32 bytes from the OS CSPRNG, so there is no guessable
/// input to protect — this exists so that a database dump does not contain usable live
/// credentials, not to resist a dictionary attack.
pub fn session_token_hash(token: &str) -> String {
    use sha2::Digest as _;
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// `member_push.endpoint_hash` — lets a subscribe find, and take over, the row another member
/// left for the same browser.
///
/// Keyed with the push encryption key under its own domain label rather than a sixth key: it
/// protects the same value and has the same lifetime, and the label keeps the two uses apart.
pub fn push_endpoint_hash(endpoint: &str) -> String {
    mac(
        &keyring().push_encryption,
        "push-endpoint:v1",
        &[endpoint.as_bytes()],
    )
}

/// Encrypt a push subscription for storage.
///
/// A push endpoint URL is a stable per-browser identifier. Stored in the clear, two member
/// rows sharing one would prove the same person is behind both, which defeats the point of
/// members being mutually unlinkable. Output is `nonce ‖ ciphertext`.
pub fn seal_push(plaintext: &[u8]) -> Result<Vec<u8>> {
    use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
    use chacha20poly1305::{ChaCha20Poly1305, Key};

    let key = Key::from_slice(&keyring().push_encryption[..32]);
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let mut out = nonce.to_vec();
    match cipher.encrypt(&nonce, plaintext) {
        Ok(mut ct) => {
            out.append(&mut ct);
            Ok(out)
        }
        Err(_) => bail!("could not encrypt push subscription"),
    }
}

/// Reverse of [`seal_push`].
pub fn open_push(sealed: &[u8]) -> Result<Vec<u8>> {
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

    if sealed.len() < 12 {
        bail!("stored push subscription is truncated");
    }
    let (nonce, ciphertext) = sealed.split_at(12);
    let key = Key::from_slice(&keyring().push_encryption[..32]);
    let cipher = ChaCha20Poly1305::new(key);
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| anyhow::anyhow!("could not decrypt push subscription"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keyring() -> Keyring {
        Keyring {
            identity: vec![1; 32],
            request: vec![2; 32],
            post_author: vec![3; 32],
            invite: vec![4; 32],
            push_encryption: vec![5; 32],
        }
    }

    // The whole module shares one process-wide OnceLock, so the ordering-sensitive checks live
    // in a single test.
    #[test]
    fn hashes_are_separated_and_stable() {
        let _ = init(test_keyring());

        let subject = "oauth2|discord|123456789";

        // The property the join queue depends on: same input, different keys, different hash.
        assert_ne!(
            identity_hash(subject),
            request_hash(subject),
            "identity and request hashes must not be correlatable"
        );

        // Deterministic, or nobody could log in twice.
        assert_eq!(identity_hash(subject), identity_hash(subject));
        assert_ne!(identity_hash(subject), identity_hash("oauth2|discord|987"));

        // Hex-encoded SHA256 output.
        assert_eq!(identity_hash(subject).len(), 64);

        // Rotating the seed must orphan previous posts.
        assert_ne!(
            post_author_hash("seed-a", 1),
            post_author_hash("seed-b", 1),
            "a rotated author_seed must not still match old posts"
        );
        // Per-post salting: one member's two posts must not be linkable to each other.
        assert_ne!(post_author_hash("seed-a", 1), post_author_hash("seed-a", 2));

        let member = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        assert_ne!(invite_hash(&member, 1), invite_hash(&member, 2));
        assert_ne!(invite_hash(&member, 1), invite_hash(&other, 1));
        assert_eq!(invite_hash(&member, 1), invite_hash(&member, 1));
    }

    /// The authorship check runs this once per post asked about, so how cheap it is decides how
    /// large a batch the endpoint can safely take. Kept as a test so the figure stays honest.
    #[test]
    fn checking_authorship_is_cheap() {
        let _ = init(test_keyring());
        let started = std::time::Instant::now();
        let mut last = String::new();
        for post in 0..10_000i64 {
            last = post_author_hash("seed-for-timing", post);
        }
        let per_check = started.elapsed() / 10_000;
        eprintln!("post_author_hash: {per_check:?} each");
        assert_eq!(last.len(), 64);
        // Generous: it is microseconds. This only guards against it becoming expensive.
        assert!(per_check < std::time::Duration::from_micros(500), "{per_check:?}");
    }

    #[test]
    fn the_authored_cache_key_follows_the_member_and_their_seed() {
        let _ = init(test_keyring());
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let key = authored_cache_key(&a, "seed");
        assert_eq!(key, authored_cache_key(&a, "seed"));
        assert_ne!(key, authored_cache_key(&b, "seed"), "another member");
        assert_ne!(key, authored_cache_key(&a, "new seed"), "a new posting identity");
        assert_eq!(key.len(), 16);
    }

    #[test]
    fn push_endpoint_hash_is_stable_and_distinct() {
        let _ = init(test_keyring());
        let a = push_endpoint_hash("https://fcm.googleapis.com/fcm/send/a");
        assert_eq!(a, push_endpoint_hash("https://fcm.googleapis.com/fcm/send/a"));
        assert_ne!(a, push_endpoint_hash("https://fcm.googleapis.com/fcm/send/b"));
    }

    #[test]
    fn push_subscriptions_round_trip() {
        let _ = init(test_keyring());
        let plain = br#"{"endpoint":"https://fcm.googleapis.com/fcm/send/abc"}"#;

        let sealed = seal_push(plain).expect("seal");
        assert_ne!(&sealed[..], &plain[..], "must not be stored in the clear");
        assert_eq!(open_push(&sealed).expect("open"), plain);

        // Fresh nonce per call, so identical subscriptions do not produce identical rows.
        assert_ne!(seal_push(plain).unwrap(), seal_push(plain).unwrap());

        // Tampering is rejected rather than silently decrypting to garbage.
        let mut tampered = sealed.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(open_push(&tampered).is_err());
        assert!(open_push(b"short").is_err());
    }

    #[test]
    fn length_prefixing_prevents_boundary_confusion() {
        let _ = init(test_keyring());
        // Without length prefixes, ("ab", "c") and ("a", "bc") would hash identically.
        assert_ne!(
            mac(&[0; 32], "d", &[b"ab", b"c"]),
            mac(&[0; 32], "d", &[b"a", b"bc"])
        );
    }
}
