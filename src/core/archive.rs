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
pub struct ArchiveEntry {
    /// The original (unfiltered) command.
    pub command: String,
    /// The full raw output.
    pub raw_output: String,
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
    hasher.update(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    let full = format!("{:x}", hasher.finalize());
    full[..8].to_string()
}

/// Archive a command output. Returns the archive ID if the output was large
/// enough to warrant archiving, None otherwise.
///
/// The caller should append a `§ref:<id>§` token to the filtered output.
pub fn archive(command: &str, raw: &str, filtered: &str) -> Option<String> {
    if raw.len() < MIN_ARCHIVE_SIZE {
        return None;
    }

    // Only archive when the raw output is materially larger than the filtered
    // output. If filtering did not compress much, the ref token is overhead.
    let filtered_len = filtered.len().max(1);
    if raw.len() < filtered_len * 2 && filtered.len() >= 100 {
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
        command,
        raw_size: raw_output.len(),
        raw_output,
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

/// Compact reference token that can be emitted instead of a verbose hint.
/// LLMs can resolve it with `rtk expand <id>`.
pub fn ref_token(id: &str) -> String {
    format!("§ref:{}§", id)
}

/// Format the archive hint for appending to filtered output.
/// Kept ultra-compact: the LLM learns the meaning from agent instructions.
pub fn archive_hint(id: &str, _raw_size: usize, _filtered_size: usize) -> String {
    ref_token(id)
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

    #[test]
    fn test_ref_token_format() {
        assert_eq!(ref_token("abc123"), "§ref:abc123§");
    }

    #[test]
    fn test_archive_hint_is_ref_token() {
        let hint = archive_hint("deadbeef", 1000, 100);
        assert_eq!(hint, "§ref:deadbeef§");
    }
}
