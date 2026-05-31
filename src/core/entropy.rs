//! Shannon entropy-based line filtering — identifies and removes low-information
//! lines that carry no unique signal (boilerplate, repeated patterns, empty noise).
//!
//! Modeled after lean-ctx's entropy filter: computes per-line Shannon entropy over
//! character frequencies, then drops lines below a configurable threshold. High-
//! entropy lines (errors, unique identifiers, stack traces) are preserved; low-
//! entropy lines (separators, repeated frames, padding) are dropped.
//!
//! This serves as an intelligent fallback when no dedicated command filter exists —
//! it doesn't need to know the command's output format, just the information
//! density of each line.

use std::collections::HashMap;

/// Default entropy threshold — lines with Shannon entropy below this value are
/// considered low-information and candidates for removal.
#[allow(dead_code)]
const DEFAULT_ENTROPY_THRESHOLD: f64 = 2.5;

/// Result of entropy filtering a block of text.
#[derive(Debug)]
#[allow(dead_code)]
pub struct EntropyResult {
    /// Filtered output (lines above threshold preserved, rest dropped).
    pub output: String,
    /// Number of lines in the original input.
    pub original_lines: usize,
    /// Number of lines kept after filtering.
    pub kept_lines: usize,
    /// Average entropy across all original lines.
    pub avg_entropy: f64,
    /// Percentage of lines kept.
    pub keep_pct: f64,
}

/// Compute Shannon entropy (in bits) for a string over its character frequencies.
///
/// H = -Σ p(x) × log₂(p(x))
///
/// Returns 0.0 for empty strings. Higher values = more unique information.
pub fn shannon_entropy(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }

    let mut freq: HashMap<char, usize> = HashMap::new();
    let total = text.chars().count();

    for c in text.chars() {
        *freq.entry(c).or_default() += 1;
    }

    freq.values().fold(0.0_f64, |acc, &count| {
        let p = count as f64 / total as f64;
        acc - p * p.log2()
    })
}

/// Filter lines by Shannon entropy, keeping only those at or above the threshold.
///
/// Lines with entropy below `threshold` are considered boilerplate/noise and
/// dropped. A line of all dashes (`---`) has entropy 0 and is always dropped.
/// A unique error message has high entropy and is always kept.
#[allow(dead_code)]
pub fn filter_by_entropy(text: &str, threshold: f64) -> EntropyResult {
    let lines: Vec<&str> = text.lines().collect();
    let original_lines = lines.len();

    let mut kept = Vec::with_capacity(original_lines);
    let mut total_entropy = 0.0_f64;

    for line in &lines {
        let entropy = shannon_entropy(line);
        total_entropy += entropy;

        // Always keep empty/whitespace-only lines — they preserve paragraph
        // structure in logs and error output.
        if entropy >= threshold || line.trim().is_empty() {
            kept.push(*line);
        }
    }

    let kept_lines = kept.len();
    let avg_entropy = if original_lines > 0 {
        total_entropy / original_lines as f64
    } else {
        0.0
    };
    let keep_pct = if original_lines > 0 {
        (kept_lines as f64 / original_lines as f64) * 100.0
    } else {
        100.0
    };

    EntropyResult {
        output: kept.join("\n"),
        original_lines,
        kept_lines,
        avg_entropy,
        keep_pct,
    }
}

/// Apply entropy filtering with the default threshold (2.5 bits).
/// This is the recommended entry point for general-purpose noise reduction.
#[allow(dead_code)]
pub fn filter(text: &str) -> EntropyResult {
    filter_by_entropy(text, DEFAULT_ENTROPY_THRESHOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shannon_entropy_empty() {
        assert_eq!(shannon_entropy(""), 0.0);
    }

    #[test]
    fn test_shannon_entropy_single_char() {
        // Single character repeated = entropy 0
        assert_eq!(shannon_entropy("aaaaa"), 0.0);
    }

    #[test]
    fn test_shannon_entropy_unique_chars() {
        // All chars unique = maximum entropy for length
        let e = shannon_entropy("abcdefgh");
        assert!(e > 2.5, "unique chars should have high entropy, got {e}");
    }

    #[test]
    fn test_shannon_entropy_mixed() {
        let e_repeated = shannon_entropy("------");
        let e_unique = shannon_entropy("Error: connection refused on port 8080");
        assert!(e_unique > e_repeated, "unique content should have higher entropy");
    }

    #[test]
    fn test_filter_keeps_high_entropy() {
        let input = "---\nError: something went wrong\n---\nTraceback (most recent call last):\n  File \"app.py\", line 42";
        let result = filter(input);
        assert!(result.output.contains("Error"));
        assert!(result.output.contains("Traceback"));
    }

    #[test]
    fn test_filter_drops_separators() {
        let input = "===================================\nreal content here\n===================================";
        let result = filter(input);
        assert!(result.output.contains("real content"));
        assert_eq!(result.kept_lines, 1); // only the content line stays
    }

    #[test]
    fn test_filter_preserves_empty_lines() {
        let input = "line1\n\nline2";
        let result = filter_by_entropy(input, 1.0); // low threshold for short lines
        assert_eq!(result.kept_lines, 3); // empty line preserved
    }

    #[test]
    fn test_filter_metrics() {
        let input = "---\nunique error message with many different words\n---\n---";
        let result = filter(input);
        assert_eq!(result.original_lines, 4);
        assert!(result.keep_pct < 100.0);
        assert!(result.avg_entropy > 0.0);
    }
}
