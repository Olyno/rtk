//! Output archive — saves large filtered outputs to disk and provides
//! a retrieval mechanism via `rtk expand <id>`.
//!
//! Modeled after lean-ctx's archive system: compressed/filtered outputs are
//! shown to the LLM, but the full detailed output is archived and retrievable
//! on-demand. This gives agents confidence to accept aggressive compression
//! knowing the complete output is never lost.
//!
//! Archive entries are stored in `~/.local/share/rtk/archive/` with
//! auto-expiry after 1 hour (configurable).

use super::constants::RTK_DATA_DIR;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Minimum output size to trigger archiving (bytes). Smaller outputs are
/// already cheap enough to send in full.
const MIN_ARCHIVE_SIZE: usize = 512;

/// Archive entries expire after this duration.
const ARCHIVE_TTL: Duration = Duration::from_secs(3600);

/// An archived output that can be retrieved with `rtk expand <id>`.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ArchiveEntry {
    /// Short ID for retrieval (first 8 chars of SHA-256).
    pub id: String,
    /// The original (unfiltered) command.
    pub command: String,
    /// The full raw output.
    pub raw_output: String,
    /// The filtered/compressed version that was shown.
    pub filtered_output: String,
    /// When this entry was stored.
    pub stored_at: SystemTime,
    /// Size of the raw output in bytes.
    pub raw_size: usize,
}

/// Build the archive directory: `~/.local/share/rtk/archive/`
fn archive_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join(RTK_DATA_DIR).join("archive"))
}

/// Generate a short archive ID from command + raw output.
fn archive_id(command: &str, raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(command.as_bytes());
    hasher.update(raw.as_bytes());
    hasher.update(&SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_le_bytes());
    let full = format!("{:x}", hasher.finalize());
    full[..8].to_string()
}

/// Archive a command output. Returns the archive ID if the output was large
/// enough to warrant archiving, None otherwise.
///
/// The caller should append `[rtk: full output archived, rtk expand <id> to view]`
/// to the filtered output.
pub fn archive(command: &str, raw: &str, _filtered: &str) -> Option<String> {
    if raw.len() < MIN_ARCHIVE_SIZE {
        return None;
    }

    let dir = archive_dir()?;
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }

    let id = archive_id(command, raw);
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Format: stored_at_secs\n<command>\n<raw output>
    let payload = format!("{}\n{}\n{}", now_secs, command, raw);

    let path = dir.join(&id);
    if std::fs::write(&path, &payload).is_err() {
        return None;
    }

    // Cleanup old entries
    cleanup_old(&dir);

    Some(id)
}

/// Retrieve an archived output by ID. Returns None if not found or expired.
pub fn retrieve(id: &str) -> Option<ArchiveEntry> {
    let dir = archive_dir()?;
    let path = dir.join(id);

    let content = std::fs::read_to_string(&path).ok()?;
    let mut parts = content.splitn(3, '\n');
    let stored_secs: u64 = parts.next()?.parse().ok()?;
    let command = parts.next()?.to_string();
    let raw_output = parts.next()?.to_string();

    let stored_at = SystemTime::UNIX_EPOCH + Duration::from_secs(stored_secs);
    let age = SystemTime::now()
        .duration_since(stored_at)
        .unwrap_or(Duration::MAX);

    if age > ARCHIVE_TTL {
        let _ = std::fs::remove_file(&path);
        return None;
    }

    Some(ArchiveEntry {
        id: id.to_string(),
        command,
        raw_size: raw_output.len(),
        raw_output,
        filtered_output: String::new(), // not stored
        stored_at,
    })
}

/// Remove expired archive entries.
fn cleanup_old(dir: &std::path::Path) {
    let now = SystemTime::now();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if now.duration_since(modified).unwrap_or(Duration::ZERO) > ARCHIVE_TTL {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

/// Format the archive hint for appending to filtered output.
pub fn archive_hint(id: &str, raw_size: usize, filtered_size: usize) -> String {
    let saved = if raw_size > filtered_size {
        raw_size - filtered_size
    } else {
        0
    };
    format!(
        "[rtk: full output archived ({}→{} bytes, -{}%), rtk expand {} to view]",
        raw_size,
        filtered_size,
        if raw_size > 0 { saved * 100 / raw_size } else { 0 },
        id
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_small_output_skipped() {
        let result = archive("echo hi", "hi", "hi");
        assert!(result.is_none(), "small output should not be archived");
    }

    #[test]
    fn test_archive_large_output() {
        let raw = "x".repeat(1024);
        let filtered = "x".repeat(10);
        let result = archive("echo large", &raw, &filtered);
        assert!(result.is_some(), "large output should be archived");
        let id = result.unwrap();

        let entry = retrieve(&id);
        assert!(entry.is_some(), "should be retrievable");
        assert_eq!(entry.unwrap().command, "echo large");
    }

    #[test]
    fn test_retrieve_nonexistent() {
        assert!(retrieve("deadbeef").is_none());
    }

    #[test]
    fn test_archive_id_unique() {
        let id1 = archive_id("cmd", "out1");
        std::thread::sleep(std::time::Duration::from_millis(1));
        let id2 = archive_id("cmd", "out1");
        assert_ne!(id1, id2, "IDs should differ due to nanosecond timestamp");
    }
}
