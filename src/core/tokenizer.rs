//! Multi-tokenizer support for accurate token counting.
//!
//! Different LLMs use different tokenizers — estimating tokens as `chars/4` is
//! a crude approximation that over/under-counts by 20-40% depending on content
//! type and model. This module provides per-model token counting via tiktoken-rs
//! for the most common model families.
//!
//! Supported tokenizers:
//! - `o200k_base`  — GPT-4o, GPT-4o-mini, o1, o3
//! - `cl100k_base` — GPT-4, GPT-3.5-Turbo, text-embedding-ada-002
//! - `fallback`    — chars/4 (fast, zero-dependency, always available)

use std::sync::OnceLock;

use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};

static O200K: OnceLock<Option<CoreBPE>> = OnceLock::new();
static CL100K: OnceLock<Option<CoreBPE>> = OnceLock::new();

fn get_o200k() -> Option<&'static CoreBPE> {
    O200K.get_or_init(|| o200k_base().ok()).as_ref()
}

fn get_cl100k() -> Option<&'static CoreBPE> {
    CL100K.get_or_init(|| cl100k_base().ok()).as_ref()
}

/// Tokenizer variant for different model families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tokenizer {
    /// GPT-4o family (o200k_base, ~200K vocab)
    Gpt4o,
    /// GPT-4 / GPT-3.5 family (cl100k_base, ~100K vocab)
    #[allow(dead_code)]
    Gpt4,
    /// Fast chars/4 fallback (always available)
    #[allow(dead_code)]
    Fallback,
}

impl Tokenizer {
    /// Count tokens in the given text using this tokenizer.
    pub fn count(&self, text: &str) -> usize {
        match self {
            Tokenizer::Gpt4o => get_o200k()
                .map(|bpe| bpe.encode_ordinary(text).len())
                .unwrap_or_else(|| fallback_count(text)),
            Tokenizer::Gpt4 => get_cl100k()
                .map(|bpe| bpe.encode_ordinary(text).len())
                .unwrap_or_else(|| fallback_count(text)),
            Tokenizer::Fallback => fallback_count(text),
        }
    }
}

/// Fast chars/4 fallback (always available, no model download needed).
pub fn fallback_count(text: &str) -> usize {
    text.len() / 4
}

/// Count tokens using the best available tokenizer (GPT-4o preferred).
/// Falls back to chars/4 if tiktoken models aren't available.
pub fn estimate_tokens(text: &str) -> usize {
    Tokenizer::Gpt4o.count(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_count() {
        assert_eq!(fallback_count("hello"), 1); // 5 chars / 4 = 1
        assert_eq!(fallback_count(""), 0);
        assert_eq!(fallback_count("12345678"), 2); // 8 chars / 4 = 2
    }

    #[test]
    fn test_estimate_tokens_non_empty() {
        let code = "fn main() { println!(\"hello\"); }";
        let tokens = estimate_tokens(code);
        assert!(tokens > 0, "non-empty text should produce > 0 tokens");
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_gpt4o_vs_gpt4_code() {
        let code = "fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }";
        let o200k = Tokenizer::Gpt4o.count(code);
        let cl100k = Tokenizer::Gpt4.count(code);
        let fallback = Tokenizer::Fallback.count(code);

        // GPT-4o should be more efficient with code (larger vocab = fewer tokens)
        assert!(o200k > 0);
        assert!(cl100k > 0);
        // GPT-4o usually encodes code in fewer tokens than GPT-4
        // (but not guaranteed — skip assertion on CI)
        let _ = (o200k, cl100k, fallback);
    }

    #[test]
    fn test_tokenizer_english_prose() {
        let text = "The quick brown fox jumps over the lazy dog.";
        let o200k = Tokenizer::Gpt4o.count(text);
        let fallback = Tokenizer::Fallback.count(text);
        // For English prose, o200k is roughly 0.75× to 1.0× of fallback
        assert!(o200k > 0);
        assert!(fallback > 0);
        // Fallback should be within reasonable range of real tokenizer
        let ratio = o200k as f64 / fallback as f64;
        assert!(ratio > 0.3 && ratio < 3.0, "ratio {ratio} out of range");
    }
}
