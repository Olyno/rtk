//! `pip show` / `pip list` output compactor.
//!
//! Strips deprecation/warning headers, upgrade-pip nags, and table rule lines
//! while preserving the package metadata or list rows. Inspired by token-savior's
//! PipListCompactor.

#![allow(dead_code)]

use regex::Regex;

/// Compact `pip show` (or text `pip list`) output.
///
/// Drops:
/// - `DEPRECATION:` / `WARNING:` lines
/// - lines containing the upgrade-pip recommendation
/// - table rule lines like `---- ----`
pub fn filter_pip_show(output: &str) -> String {
    let rule_re = Regex::new(r"^-{2,}(\s+-{2,})+$").unwrap();

    let mut out: Vec<String> = Vec::new();
    for raw in output.lines() {
        let line = raw.trim_end();
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        if stripped.starts_with("DEPRECATION:") || stripped.starts_with("WARNING:") {
            continue;
        }
        if stripped.contains("pip install --upgrade pip") {
            continue;
        }
        if rule_re.is_match(stripped) {
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tracking as tokenizer;

    #[test]
    fn test_filter_pip_show_strips_noise() {
        let input = r#"DEPRECATION: Loading pip from a virtual environment
Name: requests
Version: 2.31.0
Summary: Python HTTP for Humans.
WARNING: You are using pip version 21.0; however, version 24.0 is available.
You should consider upgrading via the '/venv/bin/python -m pip install --upgrade pip' command.
Home-page: https://requests.readthedocs.io
"#;
        let out = filter_pip_show(input);
        assert!(out.contains("Name: requests"));
        assert!(out.contains("Version: 2.31.0"));
        assert!(!out.contains("DEPRECATION"));
        assert!(!out.contains("WARNING"));
        assert!(!out.contains("upgrade pip"));
    }

    #[test]
    fn test_filter_pip_list_strips_rule_line() {
        let input = r#"Package    Version
----------- -------
requests   2.31.0
urllib3    2.0.7
"#;
        let out = filter_pip_show(input);
        assert!(out.contains("Package    Version"));
        assert!(out.contains("requests   2.31.0"));
        assert!(!out.contains("----------- -------"));
    }

    #[test]
    fn test_filter_pip_show_token_savings() {
        let input = r#"DEPRECATION: Loading pip from a virtual environment
Name: requests
Version: 2.31.0
Summary: Python HTTP for Humans.
WARNING: You are using pip version 21.0; however, version 24.0 is available.
You should consider upgrading via the '/venv/bin/python -m pip install --upgrade pip' command.
Home-page: https://requests.readthedocs.io
Author: Kenneth Reitz
"#;
        let out = filter_pip_show(input);
        let saved = tokenizer::estimate_tokens(input) - tokenizer::estimate_tokens(&out);
        assert!(
            saved > 0,
            "expected token savings, got {} -> {}",
            tokenizer::estimate_tokens(input),
            tokenizer::estimate_tokens(&out)
        );
    }
}
