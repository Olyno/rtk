//! Filters Dart command output for analyzer, formatter, and tests.

use crate::cmds::dart::flutter_cmd::{summarize_dart_analyze, summarize_flutter_test};
use crate::core::runner;
use anyhow::Result;
use std::process::Command;

pub fn run(args: &[String], verbose: u8) -> Result<i32> {
    let subcommand = args.first().map(String::as_str).unwrap_or("");
    match subcommand {
        "test" => run_test(&args[1..], verbose),
        "analyze" => run_analyze(&args[1..], verbose),
        "format" => run_format(&args[1..], verbose),
        _ => run_passthrough(args, verbose),
    }
}

fn run_test(args: &[String], _verbose: u8) -> Result<i32> {
    let mut cmd = Command::new("dart");
    cmd.arg("test").args(args);
    runner::run_filtered(
        cmd,
        "dart-test",
        &format!("dart test {}", args.join(" ")),
        summarize_flutter_test,
        runner::RunOptions::with_tee("dart-test"),
    )
}

fn run_analyze(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = Command::new("dart");
    cmd.arg("analyze").args(args);
    if verbose > 0 {
        eprintln!("Running: dart analyze {}", args.join(" "));
    }
    runner::run_filtered(
        cmd,
        "dart-analyze",
        &format!("dart analyze {}", args.join(" ")),
        summarize_dart_analyze,
        runner::RunOptions::with_tee("dart-analyze"),
    )
}

fn run_format(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = Command::new("dart");
    cmd.arg("format").args(args);
    if verbose > 0 {
        eprintln!("Running: dart format {}", args.join(" "));
    }
    runner::run_filtered(
        cmd,
        "dart-format",
        &format!("dart format {}", args.join(" ")),
        summarize_dart_format,
        runner::RunOptions::with_tee("dart-format"),
    )
}

fn run_passthrough(args: &[String], verbose: u8) -> Result<i32> {
    let os_args: Vec<std::ffi::OsString> = args.iter().map(Into::into).collect();
    runner::run_passthrough("dart", &os_args, verbose)
}

pub(crate) fn summarize_dart_format(raw: &str) -> String {
    let lines: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let changed: Vec<String> = lines
        .iter()
        .filter(|line| line.starts_with("Formatted ") && !line.contains(" files "))
        .cloned()
        .collect();
    let summary = lines
        .iter()
        .rev()
        .find(|line| line.starts_with("Formatted ") || line.starts_with("Changed "));
    if changed.is_empty() {
        return match summary {
            Some(line) => format!("[ok] {}", line),
            None => "[ok] dart format completed".to_string(),
        };
    }
    let mut output = String::new();
    output.push_str(
        summary
            .map(String::as_str)
            .unwrap_or("dart format changed files"),
    );
    output.push('\n');
    for line in changed.iter().take(20) {
        output.push_str("  ");
        output.push_str(line);
        output.push('\n');
    }
    if changed.len() > 20 {
        output.push_str(&format!("  ... +{} more\n", changed.len() - 20));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_dart_format_without_changed_files() {
        assert_eq!(
            summarize_dart_format("Formatted 3 files (0 changed) in 0.08 seconds.\n"),
            "[ok] Formatted 3 files (0 changed) in 0.08 seconds."
        );
    }

    #[test]
    fn summarizes_dart_format_changed_files() {
        let summary = summarize_dart_format(
            "Formatted lib/a.dart\nFormatted test/a_test.dart\nFormatted 2 files (2 changed) in 0.08 seconds.\n",
        );
        assert!(summary.contains("2 changed"));
        assert!(summary.contains("lib/a.dart"));
    }
}
