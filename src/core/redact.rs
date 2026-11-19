//! Secret and PII redaction for command output.
//!
//! Inspired by tok's SecretDetector and axiom-cli's privacy shield:
//! command outputs often contain tokens, keys, and credentials that are both
//! sensitive and token-expensive. Replacing them with short placeholders
//! reduces risk and saves context tokens.

use regex::Regex;
use std::sync::LazyLock;

/// Generic high-entropy secret patterns.
static PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        (
            "AWS_SECRET_ACCESS_KEY",
            Regex::new(r"(?i)aws_secret_access_key\s*=\s*[A-Za-z0-9/+=]{40}").unwrap(),
        ),
        (
            "AWS_ACCESS_KEY_ID",
            Regex::new(r"(?i)aws_access_key_id\s*=\s*[A-Z0-9]{20}").unwrap(),
        ),
        (
            "SLACK_TOKEN",
            Regex::new(r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}(-[a-zA-Z0-9]{24})?").unwrap(),
        ),
        (
            "GITHUB_TOKEN",
            Regex::new(r"gh[ps]_[a-zA-Z0-9]{36}").unwrap(),
        ),
        ("OPENAI_API_KEY", Regex::new(r"sk-[a-zA-Z0-9]{48}").unwrap()),
        (
            "ANTHROPIC_API_KEY",
            Regex::new(r"sk-ant-[a-zA-Z0-9]{32,}").unwrap(),
        ),
        (
            "JWT",
            Regex::new(r"eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*").unwrap(),
        ),
        (
            "PASSWORD",
            Regex::new(r#"(?i)(password|passwd|pwd)\s*[:=]\s*[^\s'"]{8,}"#).unwrap(),
        ),
        (
            "PRIVATE_KEY",
            Regex::new(r"-----BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----").unwrap(),
        ),
    ]
});

/// Redact known secret patterns in text, replacing them with `<redacted:<kind>>`.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for (kind, re) in PATTERNS.iter() {
        out = re
            .replace_all(&out, format!("<redacted:{kind}>"))
            .to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_aws_key() {
        let input = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let out = redact(input);
        assert!(out.contains("<redacted:AWS_ACCESS_KEY_ID>"), "{out}");
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"), "{out}");
    }

    #[test]
    fn test_redact_password() {
        let input = "password=super_secret_value_123";
        let out = redact(input);
        assert!(out.contains("<redacted:PASSWORD>"), "{out}");
        assert!(!out.contains("super_secret_value_123"), "{out}");
    }

    #[test]
    fn test_redact_detects_jwt() {
        let input = "token: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let out = redact(input);
        assert!(out.contains("<redacted:JWT>"), "{out}");
        assert!(!out.contains("eyJhbGci"), "{out}");
    }

    #[test]
    fn test_redact_leaves_plain_text() {
        let input = "hello world";
        assert_eq!(redact(input), input);
    }
}
