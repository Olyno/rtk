//! Deterministic markdown compressor for agent instruction files.
//!
//! Inspired by vidanov/context-compress: strip formatting, collapse blanks,
//! and replace verbose phrases with shorthand. Code blocks are preserved.
//! Used by `rtk init --compress` when writing instruction files.

use regex::Regex;
use std::sync::LazyLock;

static BOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*(.+?)\*\*").unwrap());
static ITALIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*(.+?)\*").unwrap());
static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());
static BLANK_RUNS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

/// Verbose phrase → shorthand, applied in order.
static SHORTHANDS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?i)\bfor example\b", "e.g."),
        (r"(?i)\bthat is\b", "i.e."),
        (r"(?i)\bin order to\b", "to"),
        (r"(?i)\bmake sure to\b", ""),
        (r"(?i)\bplease note that\b", ""),
        (r"(?i)\bit is important to\b", ""),
        (r"(?i)\byou should\b", ""),
        (r"(?i)\bdo not\b", "don't"),
        (r"(?i)\bwill not\b", "won't"),
        (r"(?i)\bcannot\b", "can't"),
        (r"(?i)\bis not\b", "isn't"),
    ]
    .into_iter()
    .map(|(pat, repl)| (Regex::new(pat).unwrap(), repl))
    .collect()
});

/// Compress a markdown document with the deterministic compressor.
///
/// Code blocks (fenced with ```) are preserved.
pub fn compress_markdown(text: &str) -> String {
    compress_markdown_deterministic(text)
}

/// Strip formatting, collapse blanks, and apply shorthand replacements.
/// Code blocks (fenced with ```) are preserved.
fn compress_markdown_deterministic(text: &str) -> String {
    let mut out = String::new();
    let mut in_code_block = false;

    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if in_code_block {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let mut line = line.to_string();
        line = strip_formatting(&line);
        line = shorthand(&line);
        out.push_str(&line);
        out.push('\n');
    }

    collapse_blanks(&out)
}

fn strip_formatting(line: &str) -> String {
    let out = BOLD.replace_all(line, "$1");
    let out = ITALIC.replace_all(&out, "$1");
    INLINE_CODE.replace_all(&out, "$1").to_string()
}

fn shorthand(line: &str) -> String {
    let mut out = line.to_string();
    for (pat, repl) in SHORTHANDS.iter() {
        out = pat.replace_all(&out, *repl).to_string();
    }
    out
}

fn collapse_blanks(text: &str) -> String {
    BLANK_RUNS.replace_all(text, "\n\n").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_bold_italic() {
        let input = "**Usage**: Token-optimized *CLI proxy*";
        assert_eq!(
            compress_markdown(input),
            "Usage: Token-optimized CLI proxy\n"
        );
    }

    #[test]
    fn test_shorthand() {
        let input = "For example, in order to run tests, make sure to use rtk.";
        let out = compress_markdown(input);
        assert!(out.contains("e.g."), "{out}");
        assert!(out.contains("to run tests"), "{out}");
        assert!(!out.contains("make sure to"), "{out}");
    }

    #[test]
    fn test_preserves_code_blocks() {
        let input = "```bash\nrtk cargo test\n```\n";
        let out = compress_markdown(input);
        assert!(out.contains("```bash"), "{out}");
        assert!(out.contains("rtk cargo test"), "{out}");
    }

    #[test]
    fn test_collapse_blanks() {
        let input = "a\n\n\n\nb";
        assert_eq!(collapse_blanks(input), "a\n\nb");
    }
}
