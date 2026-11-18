//! `git stash list` output compactor.
//!
//! Strips the redundant `WIP on <branch>:` / `On <branch>:` prefix and truncates
//! long stash messages. Inspired by token-savior's GitStashListCompactor while
//! preserving the existing RTK output shape.

#![allow(dead_code)]

/// Compact `git stash list` output.
///
/// Lines like `stash@{0}: WIP on main: abc1234 fix login` become
/// `stash@{0}: abc1234 fix login`. Long messages are truncated to 60 characters
/// after the stash index. Non-matching lines are truncated to 100 characters.
pub fn filter_git_stash_list(output: &str) -> String {
    const MAX_MSG_LEN: usize = 60;
    const MAX_LINE_LEN: usize = 100;

    let mut out: Vec<String> = Vec::new();
    for raw in output.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        // `stash@{n}: <branch prefix>: <sha> <message>`
        // Drop everything between the stash index and the second `:`.
        let Some(colon_pos) = line.find(": ") else {
            out.push(truncate_line(line, MAX_LINE_LEN));
            continue;
        };
        let index = &line[..colon_pos];
        let rest = &line[colon_pos + 2..];
        let message = if let Some(second_colon) = rest.find(": ") {
            rest[second_colon + 2..].trim()
        } else {
            rest.trim()
        };

        out.push(format!(
            "{}: {}",
            index,
            truncate_line(message, MAX_MSG_LEN)
        ));
    }
    out.join("\n")
}

fn truncate_line(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!(
            "{}...",
            text.chars()
                .take(max_len.saturating_sub(3))
                .collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tokenizer;

    #[test]
    fn test_filter_stash_list_strips_branch_prefix() {
        let input =
            "stash@{0}: WIP on main: abc1234 fix login\nstash@{1}: On feature: def5678 wip\n";
        let out = filter_git_stash_list(input);
        assert!(out.contains("stash@{0}: abc1234 fix login"));
        assert!(out.contains("stash@{1}: def5678 wip"));
    }

    #[test]
    fn test_filter_stash_list_truncates_long_message() {
        let input = "stash@{0}: WIP on main: abc1234 This is a very long stash message that describes a complicated work in progress\n";
        let out = filter_git_stash_list(input);
        assert!(out.starts_with("stash@{0}: abc1234 "));
        assert!(out.ends_with("..."));
        assert!(out.chars().count() <= "stash@{0}: ".chars().count() + 60);
    }

    #[test]
    fn test_filter_stash_list_token_savings() {
        let input = r#"stash@{0}: WIP on main: abc1234 This is a very long stash message that describes a complicated work in progress
stash@{1}: On feature: d4e5f6g Another excessively verbose stash description that goes on and on
"#;
        let out = filter_git_stash_list(input);
        let input_tokens = tokenizer::estimate_tokens(input);
        let output_tokens = tokenizer::estimate_tokens(&out);
        let saved = input_tokens.saturating_sub(output_tokens);
        assert!(
            saved > 0,
            "expected positive token savings, got {} -> {}",
            input_tokens,
            output_tokens
        );
        assert!(out.contains("stash@{0}:"));
        assert!(out.contains("stash@{1}:"));
    }
}
