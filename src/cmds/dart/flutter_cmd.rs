//! Filters Flutter command output for analyzer and test runs.

use crate::core::runner;
use anyhow::Result;
use std::process::Command;

pub fn run(args: &[String], verbose: u8) -> Result<i32> {
    let subcommand = args.first().map(String::as_str).unwrap_or("");
    match subcommand {
        "test" => run_test(&args[1..], verbose),
        "analyze" => run_analyze(&args[1..], verbose),
        _ => run_passthrough(args, verbose),
    }
}

fn run_test(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = Command::new("flutter");
    cmd.arg("test").args(args);
    if verbose > 0 {
        eprintln!("Running: flutter test {}", args.join(" "));
    }
    runner::run_filtered(
        cmd,
        "flutter",
        &format!("flutter test {}", args.join(" ")),
        summarize_flutter_test,
        runner::RunOptions::with_tee("flutter-test"),
    )
}

fn run_analyze(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = Command::new("flutter");
    cmd.arg("analyze").args(args);
    if verbose > 0 {
        eprintln!("Running: flutter analyze {}", args.join(" "));
    }
    runner::run_filtered(
        cmd,
        "flutter",
        &format!("flutter analyze {}", args.join(" ")),
        summarize_dart_analyze,
        runner::RunOptions::with_tee("flutter-analyze"),
    )
}

fn run_passthrough(args: &[String], verbose: u8) -> Result<i32> {
    let os_args: Vec<std::ffi::OsString> = args.iter().map(Into::into).collect();
    runner::run_passthrough("flutter", &os_args, verbose)
}

pub(crate) fn summarize_flutter_test(raw: &str) -> String {
    let clean_lines: Vec<String> = raw
        .lines()
        .map(strip_ansi)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    let mut failures = Vec::new();
    let mut errors = Vec::new();
    let mut summary = None;
    let mut passed = false;

    for line in &clean_lines {
        if is_flutter_test_success_summary(line) {
            summary = Some(line.clone());
            passed = true;
            continue;
        }
        if is_flutter_test_failure_summary(line) {
            summary = Some(line.clone());
            failures.push(line.clone());
            continue;
        }
        if is_flutter_test_failure_status(line) {
            failures.push(line.clone());
        } else if is_flutter_test_error_detail(line) {
            errors.push(line.clone());
        }
    }

    if passed {
        return format!("[ok] {}", summary.expect("success summary is set"));
    }

    if failures.is_empty() && errors.is_empty() {
        return match summary {
            Some(summary_line) => format!("[ok] {}", summary_line),
            None => compact_tail("[ok] flutter test completed", &clean_lines),
        };
    }

    let mut output = String::from("[FAIL] flutter test\n");
    if let Some(summary_line) = summary {
        output.push_str(&format!("summary: {}\n", summary_line));
    }
    append_limited(&mut output, "failures", &failures, 12);
    append_limited(&mut output, "errors", &errors, 12);
    output
}

pub(crate) fn summarize_dart_analyze(raw: &str) -> String {
    let clean_lines: Vec<String> = raw
        .lines()
        .map(strip_ansi)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    let mut diagnostics = Vec::new();
    let mut summary = None;
    let mut passed = false;
    for line in &clean_lines {
        let lower = line.to_lowercase();
        if lower.contains("no issues found") {
            summary = Some(line.clone());
            passed = true;
            continue;
        }
        if lower.contains("issue found") || lower.contains("issues found") {
            summary = Some(line.clone());
        }
        if lower.starts_with("error")
            || lower.starts_with("warning")
            || lower.starts_with("info")
            || line.contains(" • ")
        {
            diagnostics.push(line.clone());
        }
    }

    if passed {
        return format!("[ok] {}", summary.expect("success summary is set"));
    }

    if diagnostics.is_empty() {
        return match summary {
            Some(summary_line) => format!("[ok] {}", summary_line),
            None => compact_tail("[ok] analyze completed", &clean_lines),
        };
    }

    let mut output = String::new();
    output.push_str(match summary {
        Some(ref summary_line) => summary_line,
        None => "[FAIL] analyze diagnostics",
    });
    output.push('\n');
    append_limited(&mut output, "diagnostics", &diagnostics, 20);
    output
}

fn is_flutter_test_success_summary(line: &str) -> bool {
    line.contains("All tests passed!")
}

fn is_flutter_test_failure_summary(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("some tests failed")
        || lower.contains("test failed.")
        || lower.contains("tests failed.")
        || lower.contains("failed to load")
}

fn is_flutter_test_failure_status(line: &str) -> bool {
    line.contains("[E]") || looks_like_flutter_progress_failure(line)
}

fn looks_like_flutter_progress_failure(line: &str) -> bool {
    let Some(after_passed) = line.split('+').nth(1) else {
        return false;
    };
    let Some(failed_count) = after_passed.split('-').nth(1) else {
        return false;
    };
    failed_count
        .trim_start()
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit() && ch != '0')
}

fn is_flutter_test_error_detail(line: &str) -> bool {
    line.starts_with("Error:")
        || line.starts_with("Exception:")
        || line.starts_with("Expected:")
        || line.starts_with("Actual:")
        || line.starts_with("package:")
        || line.contains("Test failed. See exception logs above.")
}

fn append_limited(output: &mut String, label: &str, lines: &[String], limit: usize) {
    if lines.is_empty() {
        return;
    }
    output.push_str(&format!("{}:\n", label));
    for line in lines.iter().take(limit) {
        output.push_str("  ");
        output.push_str(line);
        output.push('\n');
    }
    if lines.len() > limit {
        output.push_str(&format!("  ... +{} more\n", lines.len() - limit));
    }
}

fn compact_tail(prefix: &str, lines: &[String]) -> String {
    let tail = lines
        .iter()
        .rev()
        .take(3)
        .cloned()
        .collect::<Vec<String>>()
        .into_iter()
        .rev()
        .collect::<Vec<String>>();
    if tail.is_empty() {
        return prefix.to_string();
    }
    format!("{}: {}", prefix, tail.join(" | "))
}

fn strip_ansi(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_passing_flutter_test() {
        let raw = "00:00 +0: loading test/foo_test.dart\n00:00 +1: All tests passed!\n";
        assert_eq!(
            summarize_flutter_test(raw),
            "[ok] 00:00 +1: All tests passed!"
        );
    }

    #[test]
    fn summarizes_passing_flutter_test_names_with_error_words() {
        let raw = "\
00:00 +0: loading test/services/channel_lifecycle_actions_test.dart
00:00 +1: createChannel surfaces actionable non-JSON group engine errors
00:00 +2: formats API errors with the matching operation label
00:00 +2: All tests passed!
";
        assert_eq!(
            summarize_flutter_test(raw),
            "[ok] 00:00 +2: All tests passed!"
        );
    }

    #[test]
    fn summarizes_failing_flutter_test() {
        let raw = "00:00 +0 -1: Some test [E]\n  Expected: <1>\n    Actual: <2>\n00:00 +0 -1: Some tests failed.\n";
        let summary = summarize_flutter_test(raw);
        assert!(summary.contains("[FAIL] flutter test"));
        assert!(summary.contains("Some test [E]"));
    }

    #[test]
    fn summarizes_flutter_load_failures() {
        let raw = "00:00 +0 -1: loading test/foo_test.dart [E]\nFailed to load \"test/foo_test.dart\": Compilation failed\n00:00 +0 -1: Some tests failed.\n";
        let summary = summarize_flutter_test(raw);
        assert!(summary.contains("[FAIL] flutter test"));
        assert!(summary.contains("Failed to load"));
    }

    #[test]
    fn summarizes_clean_analyze() {
        assert_eq!(
            summarize_dart_analyze("Analyzing 3 items...\nNo issues found! (ran in 1.0s)\n"),
            "[ok] No issues found! (ran in 1.0s)"
        );
    }

    #[test]
    fn summarizes_analyze_diagnostics() {
        let raw = "warning • Unused import • lib/foo.dart:1:8 • unused_import\n1 issue found.\n";
        let summary = summarize_dart_analyze(raw);
        assert!(summary.contains("1 issue found."));
        assert!(summary.contains("Unused import"));
    }
}
