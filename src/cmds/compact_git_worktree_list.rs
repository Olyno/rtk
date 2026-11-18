//! `git worktree list` output compactor.
//!
//! Keeps the directory path and short SHA, dropping a redundant `[branch]`
//! marker when the directory basename already implies the branch.
//! Inspired by token-savior's GitWorktreeListCompactor.

#![allow(dead_code)]

use regex::Regex;

/// Compact `git worktree list` output.
///
/// Format per line: `<path>  <sha7>  [branch]` or `<path>  <sha7>  (detached)`.
/// Drops `[branch]` when the directory basename already matches the branch,
/// but keeps `(detached)` and other non-branch tails.
pub fn filter_git_worktree_list(output: &str) -> String {
    let split_re = Regex::new(r"\s{2,}").unwrap();
    let branch_re = Regex::new(r"^\[(.+)\]$").unwrap();

    let mut out: Vec<String> = Vec::new();
    for raw in output.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        // Split on 2+ spaces to separate path/sha/tail, preserving path contents.
        let parts: Vec<&str> = split_re.splitn(line.trim(), 3).collect();
        if parts.len() < 2 {
            out.push(line.trim().to_string());
            continue;
        }

        let path = parts[0];
        let sha = parts[1];
        let tail = parts.get(2).copied().unwrap_or("").trim();

        let mut keep_tail = String::new();
        if !tail.is_empty() {
            if let Some(caps) = branch_re.captures(tail) {
                let branch = &caps[1];
                let base = path
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or(path)
                    .to_ascii_lowercase();
                let branch_lower = branch.to_ascii_lowercase();
                if !branch_lower.contains(&base) && !base.contains(&branch_lower) {
                    keep_tail = format!(" [{}]", branch);
                }
            } else if tail == "(detached)" {
                keep_tail = " (detached)".to_string();
            } else {
                keep_tail = format!(" {}", tail);
            }
        }

        out.push(format!("{}  {}{}", path, sha, keep_tail));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tokenizer;

    #[test]
    fn test_filter_worktree_list_drops_redundant_branch() {
        let input = r#"/home/user/rtk  a1b2c3d  [rtk-next]
/home/user/rtk-docs  d4e5f6g  [main]
"#;
        let out = filter_git_worktree_list(input);
        assert_eq!(
            out,
            "/home/user/rtk  a1b2c3d\n/home/user/rtk-docs  d4e5f6g [main]"
        );
    }

    #[test]
    fn test_filter_worktree_list_keeps_detached() {
        let input = "/home/user/rtk  a1b2c3d  (detached)\n";
        let out = filter_git_worktree_list(input);
        assert_eq!(out, "/home/user/rtk  a1b2c3d (detached)");
    }

    #[test]
    fn test_filter_worktree_list_token_savings() {
        let input = r#"/home/user/rtk  a1b2c3d  [rtk-next]
/home/user/rtk-docs  d4e5f6g  [main]
/home/user/legacy  a1b2c3d  [legacy]
"#;
        let out = filter_git_worktree_list(input);
        let saved = tokenizer::estimate_tokens(input) - tokenizer::estimate_tokens(&out);
        assert!(
            saved > 0,
            "expected positive token savings, got {} -> {}",
            tokenizer::estimate_tokens(input),
            tokenizer::estimate_tokens(&out)
        );
        assert!(!out.contains("[rtk-next]"));
        assert!(out.contains("[main]"));
    }
}
