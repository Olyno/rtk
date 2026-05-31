//! Bounce detection — tracks when users re-run a command unfiltered after
//! receiving a filtered version, signaling that the compression was too
//! aggressive or lost critical information.
//!
//! Modeled after lean-ctx's BounceTracker: records filtered reads and detects
//! subsequent full/raw reads of the same command within a short window. Bounce
//! rates are exposed via `rtk gain --bounce` to help identify filters that
//! need tuning.
//!
//! # Architecture
//!
//! - In-memory ring buffer of recent filtered commands (last 20)
//! - On each tracking call, checks if this is a "bounce" (raw after filtered)
//! - Persists bounce events to SQLite alongside normal tracking data

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A recently filtered command execution.
struct FilteredCall {
    /// The original command (e.g., "git status")
    command: String,
    /// When the filtered output was delivered
    at: Instant,
}

/// Global bounce tracker (lazy-initialized, in-memory).
static BOUNCE_TRACKER: std::sync::LazyLock<Mutex<BounceState>> =
    std::sync::LazyLock::new(|| Mutex::new(BounceState::new()));

/// Maximum number of recent commands to track for bounce detection.
const MAX_RECENT: usize = 20;

/// Window for detecting bounces — if a raw command runs within this time
/// after the same filtered command, it's a bounce.
const BOUNCE_WINDOW: Duration = Duration::from_secs(30);

struct BounceState {
    recent: VecDeque<FilteredCall>,
    /// Total bounce events detected (across all commands)
    total_bounces: u64,
}

impl BounceState {
    fn new() -> Self {
        Self {
            recent: VecDeque::with_capacity(MAX_RECENT),
            total_bounces: 0,
        }
    }

    /// Record that a filtered command was delivered. Called from track().
    fn record_filtered(&mut self, command: &str) {
        if self.recent.len() >= MAX_RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(FilteredCall {
            command: command.to_string(),
            at: Instant::now(),
        });
    }

    /// Check if this raw/passthrough command is a bounce of a recent filtered one.
    /// Returns true if a bounce was detected (and records it).
    fn check_bounce(&mut self, command: &str) -> bool {
        let now = Instant::now();
        // Find the most recent matching filtered command within the bounce window
        let found = self
            .recent
            .iter()
            .rev()
            .any(|fc| fc.command == command && now.duration_since(fc.at) <= BOUNCE_WINDOW);

        if found {
            self.total_bounces += 1;
            // Remove the entry so we don't double-count
            self.recent.retain(|fc| {
                !(fc.command == command && now.duration_since(fc.at) <= BOUNCE_WINDOW)
            });
        }

        found
    }

    #[allow(dead_code)]
    fn total_bounces(&self) -> u64 {
        self.total_bounces
    }
}

/// Called when a filtered command output is delivered.
/// Records the command for future bounce detection.
pub fn record_filtered(command: &str) {
    if let Ok(mut state) = BOUNCE_TRACKER.lock() {
        state.record_filtered(command);
    }
}

/// Check if executing a raw/passthrough command constitutes a bounce
/// of a recently filtered version. If so, returns true and records
/// the bounce event.
pub fn check_bounce(command: &str) -> bool {
    if let Ok(mut state) = BOUNCE_TRACKER.lock() {
        state.check_bounce(command)
    } else {
        false
    }
}

/// Total number of bounce events detected in this session.
#[allow(dead_code)]
pub fn total_bounces() -> u64 {
    if let Ok(state) = BOUNCE_TRACKER.lock() {
        state.total_bounces()
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reset global state between tests (parallel-safe via mutex).
    fn reset() {
        if let Ok(mut state) = BOUNCE_TRACKER.lock() {
            state.recent.clear();
            state.total_bounces = 0;
        }
    }

    #[test]
    fn test_bounce_detection() {
        reset();
        let cmd = "test-bounce-detection-cmd";

        record_filtered(cmd);
        assert!(check_bounce(cmd));
        assert!(!check_bounce(cmd)); // second call: already consumed
    }

    #[test]
    fn test_no_bounce_without_filtered() {
        reset();
        assert!(!check_bounce("test-no-bounce-cmd"));
    }

    #[test]
    fn test_different_command_no_bounce() {
        reset();
        record_filtered("test-status-cmd");
        assert!(!check_bounce("test-diff-cmd"));
    }

    #[test]
    fn test_total_bounces_increments() {
        reset();
        let before = total_bounces();
        record_filtered("test-total-cmd");
        check_bounce("test-total-cmd");
        assert!(total_bounces() > before);
    }
}
