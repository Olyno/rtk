//! NPM/Yarn/pnpm `list` output compactor.
//!
//! Flattens a dependency tree to top-level packages only and trims verbose
//! semver strings to `major.minor`. Inspired by token-savior's NpmListCompactor.

#![allow(dead_code)]

use regex::Regex;

/// Compact npm/yarn/pnpm list output.
///
/// - Keeps the project header line (first non-empty line), dropping the path.
/// - Keeps only top-level tree entries (`├──` / `└──`).
/// - Drops nested dependencies and trailing status tokens (`deduped`, `invalid`).
/// - Trims semver strings to `major.minor`.
pub fn filter_npm_list(output: &str) -> String {
    let top_level_re = Regex::new(r"^[├└][─-]{2}\s+(\S.*)$").unwrap();
    let ver_re = Regex::new(r"^(\d+)\.(\d+)(?:\.\d+)?(?:[-+][\w.]+)?$").unwrap();

    let mut out: Vec<String> = Vec::new();
    let mut first = true;

    for raw in output.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        if first {
            first = false;
            // Keep the project header line (e.g. `myapp@1.0.0 /path`), drop path.
            if !line.starts_with(['├', '└', '│', ' ']) {
                out.push(line.split_whitespace().next().unwrap_or(line).to_string());
                continue;
            }
        }
        first = false;

        let Some(caps) = top_level_re.captures(line) else {
            continue;
        };
        let entry = caps[1].trim();

        // Take the first token and strip trailing status words.
        let first_token = entry.split_whitespace().next().unwrap_or(entry);
        let trimmed = trim_version(first_token, &ver_re);
        out.push(trimmed);
    }

    out.join("\n")
}

/// Trim a `name@major.minor.patch...` string to `name@major.minor`.
/// Passes through if there is no `@` or the version does not match semver.
fn trim_version(name_at_ver: &str, ver_re: &Regex) -> String {
    let Some(at_pos) = name_at_ver.rfind('@') else {
        return name_at_ver.to_string();
    };
    let (head, ver) = name_at_ver.split_at(at_pos);
    // The `ver` includes the leading '@'.
    let version_part = &ver[1..];

    // Scoped package edge case: `@scope/pkg@1.2.3`.
    if head.is_empty() {
        return name_at_ver.to_string();
    }

    if let Some(caps) = ver_re.captures(version_part) {
        format!("{}@{}.{}", head, &caps[1], &caps[2])
    } else {
        name_at_ver.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tokenizer;

    #[test]
    fn test_filter_npm_list_top_level_only() {
        let input = r#"myapp@1.0.0 /home/user/myapp
├── react@18.2.0
│   loose-envify@1.4.0
├── lodash@4.17.21 deduped
└── typescript@5.1.6 invalid
"#;
        let out = filter_npm_list(input);
        assert_eq!(out, "myapp@1.0.0\nreact@18.2\nlodash@4.17\ntypescript@5.1");
    }

    #[test]
    fn test_filter_npm_list_keeps_scoped_packages() {
        let input = r#"myapp@1.0.0 /home/user/myapp
├── @types/react@18.2.14
└── @scope/pkg@1.2.3-canary.1
"#;
        let out = filter_npm_list(input);
        assert_eq!(out, "myapp@1.0.0\n@types/react@18.2\n@scope/pkg@1.2");
    }

    #[test]
    fn test_filter_npm_list_no_header() {
        let input = r#"├── react@18.2.0
└── lodash@4.17.21
"#;
        let out = filter_npm_list(input);
        assert_eq!(out, "react@18.2\nlodash@4.17");
    }

    #[test]
    fn test_filter_npm_list_token_savings() {
        let input = r#"myapp@1.0.0 /home/user/myapp
├── react@18.2.0
│   loose-envify@1.4.0
│   object-assign@4.1.1
├── react-dom@18.2.0
│   loose-envify@1.4.0 deduped
│   object-assign@4.1.1 deduped
│   scheduler@0.23.0
└── typescript@5.1.6
"#;
        let out = filter_npm_list(input);
        let saved = tokenizer::estimate_tokens(input) - tokenizer::estimate_tokens(&out);
        assert!(
            saved > 0,
            "expected token savings, got {} -> {}",
            tokenizer::estimate_tokens(input),
            tokenizer::estimate_tokens(&out)
        );
        assert!(out.contains("react@18.2"));
        assert!(out.contains("react-dom@18.2"));
        assert!(out.contains("typescript@5.1"));
        assert!(!out.contains("loose-envify"));
        assert!(!out.contains("scheduler"));
    }
}
