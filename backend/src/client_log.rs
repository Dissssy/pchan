//! Errors reported by the web client, deduplicated into one JSON file.
//!
//! The point is to be able to say "look at the log" instead of describing a crash, so the file
//! is plain, readable JSON, written to a path the operator chooses.
//!
//! Errors from a frontend build that is no longer the one being served are moved to a sibling
//! `…archive.json` file. Whatever is still broken reports again under the new build, so the main
//! file stays about what is true now, while a rare crash's history is not thrown away.
//!
//! What it deliberately does not hold: who sent a report. The handler that feeds this never
//! reads the viewer, so there is nothing to store. Reports are folded into one entry per
//! distinct error with a count, and the only times kept are the first and last occurrence to
//! the minute. A per-report log would be a record of when each member's browser was active,
//! which is the sort of thing this project does not keep.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Distinct errors kept. Past this, new ones are counted but not stored.
const MAX_ENTRIES: usize = 500;
const MAX_MESSAGE: usize = 1_000;
const MAX_STACK: usize = 4_000;
const MAX_SHORT: usize = 120;
/// Entries kept from older builds.
const MAX_ARCHIVED: usize = 200;

/// What the page sends.
#[derive(Debug, Clone, Deserialize)]
pub struct ClientErrorReport {
    /// "panic", "error" or "rejection".
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub stack: Option<String>,
    /// The page type, with board and thread ids already replaced by placeholders.
    #[serde(default)]
    pub route: Option<String>,
    /// Which frontend build produced it.
    #[serde(default)]
    pub build: Option<String>,
    /// Browser family and major version only.
    #[serde(default)]
    pub browser: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    id: String,
    kind: String,
    message: String,
    stack: Option<String>,
    route: Option<String>,
    build: Option<String>,
    browser: Option<String>,
    count: u64,
    first_seen: String,
    last_seen: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FileData {
    /// The frontend build being served when this was written, so a reader can tell which
    /// entries are about it.
    #[serde(default)]
    current_build: Option<String>,
    /// Reports for new errors that arrived once the file was full.
    #[serde(default)]
    dropped: u64,
    entries: Vec<Entry>,
}

pub struct ClientLog {
    path: PathBuf,
    entries: HashMap<String, Entry>,
    dropped: u64,
    current_build: Option<String>,
    archive_path: PathBuf,
    archive: HashMap<String, Entry>,
    archive_dirty: bool,
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Collapse every run of digits, so that two reports of the same error that differ only in a
/// number (a count, an index, a line in a different build) still land on one entry.
fn normalise(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_digits = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('N');
            }
            in_digits = true;
        } else {
            in_digits = false;
            out.push(c);
        }
    }
    out
}

fn fingerprint(r: &ClientErrorReport) -> String {
    let mut hasher = Sha256::new();
    hasher.update(r.kind.as_bytes());
    hasher.update([0]);
    hasher.update(normalise(&r.message).as_bytes());
    hasher.update([0]);
    // The top of the stack identifies where it came from; the rest is mostly framework.
    for line in r.stack.as_deref().unwrap_or("").lines().take(4) {
        hasher.update(normalise(line.trim()).as_bytes());
        hasher.update([b'\n']);
    }
    hasher.update([0]);
    hasher.update(r.build.as_deref().unwrap_or("").as_bytes());
    hex::encode(hasher.finalize())[..12].to_owned()
}

fn now_minute() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%MZ").to_string()
}

impl ClientLog {
    /// Open the log at `path`, picking up what an earlier run recorded.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let archive_path = path.with_extension("archive.json");
        let read = |p: &Path| {
            std::fs::read_to_string(p)
                .ok()
                .and_then(|text| match serde_json::from_str::<FileData>(&text) {
                    Ok(data) => Some(data),
                    Err(e) => {
                        log::warn!("ignoring unreadable client error log {}: {e}", p.display());
                        None
                    }
                })
                .unwrap_or_default()
        };
        let data = read(&path);
        let archived = read(&archive_path);
        Self {
            path,
            dropped: data.dropped,
            current_build: data.current_build,
            entries: data.entries.into_iter().map(|e| (e.id.clone(), e)).collect(),
            archive_path,
            archive: archived.entries.into_iter().map(|e| (e.id.clone(), e)).collect(),
            archive_dirty: false,
        }
    }

    /// Fold `report` in and save. `current_build` is the frontend build being served right now
    /// (the full hash); entries from any other build are moved to the archive.
    pub async fn record(&mut self, report: ClientErrorReport, current_build: Option<&str>) -> Result<()> {
        let id = fingerprint(&report);
        let now = now_minute();

        if let Some(entry) = self.entries.get_mut(&id) {
            entry.count += 1;
            entry.last_seen = now;
        } else if self.entries.len() >= MAX_ENTRIES {
            self.dropped += 1;
        } else {
            self.entries.insert(
                id.clone(),
                Entry {
                    id,
                    kind: truncate(&report.kind, 20),
                    message: truncate(&report.message, MAX_MESSAGE),
                    stack: report.stack.as_deref().map(|s| truncate(s, MAX_STACK)),
                    route: report.route.as_deref().map(|s| truncate(s, MAX_SHORT)),
                    build: report.build.as_deref().map(|s| truncate(s, MAX_SHORT)),
                    browser: report.browser.as_deref().map(|s| truncate(s, MAX_SHORT)),
                    count: 1,
                    first_seen: now.clone(),
                    last_seen: now,
                },
            );
        }

        if let Some(current) = current_build {
            self.current_build = Some(current.to_owned());
            self.archive_other_builds(current);
        }
        self.save().await
    }

    /// Bring the log in line with the build being served, without waiting for a report.
    pub async fn sync_build(&mut self, current: &str) -> Result<()> {
        self.current_build = Some(current.to_owned());
        self.archive_other_builds(current);
        self.save().await
    }

    /// Move entries from builds other than `current` into the archive. An entry with no build
    /// recorded is kept: there is nothing to say it is out of date.
    fn archive_other_builds(&mut self, current: &str) {
        let stale: Vec<String> = self
            .entries
            .values()
            .filter(|e| {
                e.build
                    .as_deref()
                    .is_some_and(|b| !b.is_empty() && !current.starts_with(b))
            })
            .map(|e| e.id.clone())
            .collect();

        for id in stale {
            let Some(entry) = self.entries.remove(&id) else {
                continue;
            };
            match self.archive.get_mut(&id) {
                Some(existing) => {
                    existing.count += entry.count;
                    existing.first_seen = existing.first_seen.clone().min(entry.first_seen);
                    existing.last_seen = existing.last_seen.clone().max(entry.last_seen);
                }
                None => {
                    self.archive.insert(id, entry);
                }
            }
            self.archive_dirty = true;
        }

        // Oldest out first.
        while self.archive.len() > MAX_ARCHIVED {
            let Some(oldest) = self
                .archive
                .values()
                .min_by(|a, b| a.last_seen.cmp(&b.last_seen))
                .map(|e| e.id.clone())
            else {
                break;
            };
            self.archive.remove(&oldest);
            self.archive_dirty = true;
        }
    }

    async fn save(&mut self) -> Result<()> {
        let sorted = |map: &HashMap<String, Entry>| {
            let mut entries: Vec<Entry> = map.values().cloned().collect();
            // Most recent first: that is the one being asked about.
            entries.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then(b.count.cmp(&a.count)));
            entries
        };
        let text = serde_json::to_string_pretty(&FileData {
            current_build: self.current_build.clone(),
            dropped: self.dropped,
            entries: sorted(&self.entries),
        })?;
        write_atomically(&self.path, &text).await?;

        if self.archive_dirty {
            let text = serde_json::to_string_pretty(&FileData {
                current_build: None,
                dropped: 0,
                entries: sorted(&self.archive),
            })?;
            write_atomically(&self.archive_path, &text).await?;
            self.archive_dirty = false;
        }
        Ok(())
    }
}

/// Write to a temporary file and rename it over the target, so a reader never sees half a file.
async fn write_atomically(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, text).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(message: &str, stack: &str, build: &str) -> ClientErrorReport {
        ClientErrorReport {
            kind: "panic".into(),
            message: message.into(),
            stack: Some(stack.into()),
            route: Some("/:board/thread/:id".into()),
            build: Some(build.into()),
            browser: Some("Firefox 131".into()),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pchan-client-log-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("logs").join("frontend-errors.json")
    }

    #[test]
    fn numbers_do_not_split_an_error() {
        let a = fingerprint(&report("failed at 12:40", "f@x.js:10:5\ng@x.js:2:2", "abc"));
        let b = fingerprint(&report("failed at 9:7", "f@x.js:99:1\ng@x.js:3:8", "abc"));
        assert_eq!(a, b);
    }

    #[test]
    fn different_errors_and_builds_stay_apart() {
        let base = fingerprint(&report("boom", "f@x.js:1:1", "abc"));
        assert_ne!(base, fingerprint(&report("bang", "f@x.js:1:1", "abc")));
        assert_ne!(base, fingerprint(&report("boom", "h@x.js:1:1", "abc")));
        assert_ne!(base, fingerprint(&report("boom", "f@x.js:1:1", "def")));
    }

    #[tokio::test]
    async fn repeats_are_counted_and_saved_where_they_can_be_read() {
        let path = scratch("repeats");
        let mut log = ClientLog::load(&path);
        for _ in 0..3 {
            log.record(report("boom", "f@x.js:1:1", "abc"), None).await.unwrap();
        }
        log.record(report("other", "g@x.js:1:1", "abc"), None).await.unwrap();

        // A fresh process reads what was written.
        let reread: FileData = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(reread.entries.len(), 2);
        assert_eq!(reread.entries.iter().find(|e| e.message == "boom").unwrap().count, 3);
        assert_eq!(ClientLog::load(&path).entries.len(), 2);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[tokio::test]
    async fn the_file_and_its_fields_are_bounded() {
        let path = scratch("bounds");
        let mut log = ClientLog::load(&path);
        for i in 0..(MAX_ENTRIES + 5) {
            log.record(
                report(&format!("unique error {}", "x".repeat(i + 1)), "f@x.js", "abc"),
                None,
            )
            .await
            .unwrap();
        }
        assert_eq!(log.entries.len(), MAX_ENTRIES);
        assert_eq!(log.dropped, 5);

        let mut huge = report(&"m".repeat(50_000), &"s".repeat(50_000), "abc");
        huge.route = Some("r".repeat(5_000));
        let mut log = ClientLog::load(scratch("bounds2"));
        log.record(huge, None).await.unwrap();
        let entry = log.entries.values().next().unwrap();
        assert_eq!(entry.message.chars().count(), MAX_MESSAGE);
        assert_eq!(entry.stack.as_ref().unwrap().chars().count(), MAX_STACK);
        assert_eq!(entry.route.as_ref().unwrap().chars().count(), MAX_SHORT);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn an_unreadable_file_starts_fresh() {
        let path = scratch("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(ClientLog::load(&path).entries.len(), 0);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[tokio::test]
    async fn old_builds_move_to_the_archive_and_current_ones_stay() {
        let path = scratch("archive");
        let mut log = ClientLog::load(&path);
        let old = "11111111";
        let new_full = "22222222aaaabbbbccccddddeeeeffff";

        log.record(report("boom", "f@x.js:1:1", old), None).await.unwrap();
        let mut unknown = report("no build", "g@x.js:1:1", "x");
        unknown.build = None;
        log.record(unknown, None).await.unwrap();
        assert_eq!(log.entries.len(), 2);

        // The frontend is rebuilt; the next report arrives from the new build.
        log.record(report("boom", "f@x.js:1:1", "22222222"), Some(new_full)).await.unwrap();

        let main: FileData = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(main.current_build.as_deref(), Some(new_full));
        let mut kept: Vec<_> = main.entries.iter().map(|e| e.build.clone()).collect();
        kept.sort();
        assert_eq!(kept, vec![None, Some("22222222".to_owned())]);

        let archive: FileData =
            serde_json::from_str(&std::fs::read_to_string(path.with_extension("archive.json")).unwrap()).unwrap();
        assert_eq!(archive.entries.len(), 1);
        assert_eq!(archive.entries[0].build.as_deref(), Some(old));

        // A stale tab still running the old build goes straight to the archive and merges.
        log.record(report("boom", "f@x.js:1:1", old), Some(new_full)).await.unwrap();
        let archive: FileData =
            serde_json::from_str(&std::fs::read_to_string(path.with_extension("archive.json")).unwrap()).unwrap();
        assert_eq!(archive.entries.len(), 1);
        assert_eq!(archive.entries[0].count, 2);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[tokio::test]
    async fn the_archive_is_bounded() {
        let path = scratch("archive-bounds");
        let mut log = ClientLog::load(&path);
        for i in 0..(MAX_ARCHIVED + 10) {
            log.record(report(&format!("old {}", "x".repeat(i + 1)), "f@x.js", "11111111"), None)
                .await
                .unwrap();
        }
        log.record(report("now", "f@x.js", "22222222"), Some("22222222ffff")).await.unwrap();
        assert_eq!(log.archive.len(), MAX_ARCHIVED);
        assert_eq!(log.entries.len(), 1);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }
}
