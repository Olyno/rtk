//! Session-aware output cache — skips redundant command execution and filtering
//! when the same command produced identical raw output within a short window.
//!
//! Modeled after lean-ctx's session caching (re-reads cost ~13 tokens instead of
//! thousands). Each cache entry is keyed by `sha256(command || raw_stdout)`, so
//! we only hit when both the command AND its raw output are byte-identical.
//!
//! Expiry: 5 minutes (300s). This is long enough to catch repeated commands in a
//! tight dev loop (build→test→fix→build→test) but short enough that stale cache
//! won't persist across unrelated sessions.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use super::constants::RTK_DATA_DIR;

/// How long a cache entry lives before it is considered stale.
const CACHE_TTL: Duration = Duration::from_secs(300);

/// A cached filtered output.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The already-filtered output.
    pub filtered: String,
    /// Original token count (before filtering).
    pub raw_tokens: usize,
    /// Token count after filtering.
    pub filtered_tokens: usize,
}

/// Build the cache directory: `~/.local/share/rtk/cache/`
fn cache_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join(RTK_DATA_DIR).join("cache"))
}

/// Compute the cache key for a command + its raw output.
pub fn cache_key(command: &str, raw_stdout: &str, raw_stderr: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(command.as_bytes());
    hasher.update(raw_stdout.as_bytes());
    hasher.update(raw_stderr.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Try to load a cached entry by key. Returns `None` if the entry doesn't exist
/// or has expired.
pub fn load_cached(key: &str) -> Option<CacheEntry> {
    let dir = cache_dir()?;
    let path = dir.join(key);

    let content = std::fs::read_to_string(&path).ok()?;

    // Format: stored_at_secs\nraw_tokens\nfiltered_tokens\n<filtered output>
    let mut lines = content.lines();
    let stored_secs: u64 = lines.next()?.parse().ok()?;
    let raw_tokens: usize = lines.next()?.parse().ok()?;
    let filtered_tokens: usize = lines.next()?.parse().ok()?;
    // The rest (joined by \n) is the filtered output.
    let filtered = lines.collect::<Vec<_>>().join("\n");

    let stored_at = SystemTime::UNIX_EPOCH + Duration::from_secs(stored_secs);
    let age = SystemTime::now()
        .duration_since(stored_at)
        .unwrap_or(Duration::MAX);

    if age > CACHE_TTL {
        // Stale — clean up
        let _ = std::fs::remove_file(&path);
        return None;
    }

    Some(CacheEntry {
        filtered,
        raw_tokens,
        filtered_tokens,
    })
}

/// Store a filtered output in the cache.
pub fn store_cached(key: &str, raw: &str, filtered: &str) {
    let dir = match cache_dir() {
        Some(d) => d,
        None => return,
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }

    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let raw_tokens = crate::core::tokenizer::estimate_tokens(raw);
    let filtered_tokens = crate::core::tokenizer::estimate_tokens(filtered);

    let payload = format!(
        "{}\n{}\n{}\n{}",
        now_secs, raw_tokens, filtered_tokens, filtered
    );

    // Atomic write: tmp + rename
    let tmp = dir.join(format!(".{key}.tmp"));
    let dest = dir.join(key);
    if std::fs::write(&tmp, &payload).is_ok() {
        let _ = std::fs::rename(&tmp, &dest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_different_commands() {
        let k1 = cache_key("git status", "output1", "");
        let k2 = cache_key("git diff", "output1", "");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_different_outputs() {
        let k1 = cache_key("cargo test", "ok", "");
        let k2 = cache_key("cargo test", "fail", "");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_same_inputs() {
        let k1 = cache_key("git log -5", "abc\ndef", "");
        let k2 = cache_key("git log -5", "abc\ndef", "");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_cache_store_and_load() {
        let cmd = "echo test-cache-store-and-load";
        let raw = "hello world";
        let filtered = "hl";
        let key = cache_key(cmd, raw, "");

        store_cached(&key, raw, filtered);
        let entry = load_cached(&key);

        assert!(entry.is_some(), "entry should be loadable immediately");
        let entry = entry.unwrap();
        assert_eq!(entry.filtered, filtered);
        // "hl" = 2 chars → either 1 token (chars/4 fallback) or tiktoken count
        assert!(entry.filtered_tokens > 0);
    }

    #[test]
    fn test_cache_miss_unknown_key() {
        assert!(load_cached("definitely-not-a-valid-key-12345").is_none());
    }
}
