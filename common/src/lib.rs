use sha2::Digest;

pub mod structs;

pub fn format_seconds(s: u64) -> String {
    let mut s = s;
    let mut out = String::new();
    let days = s / 86400;
    s -= days * 86400;
    let hours = s / 3600;
    s -= hours * 3600;
    let minutes = s / 60;
    s -= minutes * 60;
    let seconds = s;
    if days > 0 {
        out.push_str(&format!("{days}d "));
    }
    if hours > 0 {
        out.push_str(&format!("{hours}h "));
    }
    if minutes > 0 {
        out.push_str(&format!("{minutes}m "));
    }
    if seconds > 0 {
        out.push_str(&format!("{seconds}s "));
    }
    out.trim().to_string()
}

// hash_with_salt and hash_invitation are gone. They were sha256(value || salt) over small
// enumerable inputs, recoverable by brute force from a database dump. See database::keys.

pub fn hash_file(data: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("{result:x}")
}

