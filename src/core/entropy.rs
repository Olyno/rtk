//! Shannon entropy-based line filtering — identifies and removes low-information
//! lines that carry no unique signal (boilerplate, repeated patterns, empty noise).
//!
//! Modeled after lean-ctx's entropy filter: computes per-line Shannon entropy over
//! character frequencies. The `shannon_entropy()` function is the production entry
//! point, used by the TOML filter pipeline's optional `entropy_threshold` stage.
//!
//! A standalone `filter_by_entropy()` API is available in test builds for
//! benchmarking and experimentation.

use std::collections::HashMap;

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

// ── Standalone filtering API (test-only, not wired into production yet) ──

#[cfg(test)]
const DEFAULT_ENTROPY_THRESHOLD: f64 = 2.5;

#[cfg(test)]
#[derive(Debug)]
struct EntropyResult {
    output: String,
    original_lines: usize,
    kept_lines: usize,
    avg_entropy: f64,
    keep_pct: f64,
}

#[cfg(test)]
fn filter_by_entropy(text: &str, threshold: f64) -> EntropyResult {
    let lines: Vec<&str> = text.lines().collect();
    let original_lines = lines.len();

    let mut kept = Vec::with_capacity(original_lines);
    let mut total_entropy = 0.0_f64;

    for line in &lines {
        let entropy = shannon_entropy(line);
        total_entropy += entropy;

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

#[cfg(test)]
fn filter(text: &str) -> EntropyResult {
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
        assert_eq!(shannon_entropy("aaaaa"), 0.0);
    }

    #[test]
    fn test_shannon_entropy_unique_chars() {
        let e = shannon_entropy("abcdefgh");
        assert!(e > 2.5, "unique chars should have high entropy, got {e}");
    }

    #[test]
    fn test_shannon_entropy_mixed() {
        let e_repeated = shannon_entropy("------");
        let e_unique = shannon_entropy("Error: connection refused on port 8080");
        assert!(e_unique > e_repeated);
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
        assert_eq!(result.kept_lines, 1);
    }

    #[test]
    fn test_filter_preserves_empty_lines() {
        let input = "line1\n\nline2";
        let result = filter_by_entropy(input, 1.0);
        assert_eq!(result.kept_lines, 3);
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
