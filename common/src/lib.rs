use sha2::Digest;

pub mod structs;

pub fn format_seconds(s: u64) -> String {
    // format seconds into a human readable string (e.g. 1d 2h 3m 4s)
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

pub fn hash_with_salt(s: &str, salt: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(s.as_bytes());
    hasher.update(salt);
    let result = hasher.finalize();
    format!("{result:x}")
}

pub fn hash_file(data: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("{result:x}")
}
