//! Filters Mix command output for ExUnit, formatter, and compile runs.

use crate::core::runner;
use anyhow::Result;
use std::process::Command;

pub fn run(args: &[String], verbose: u8) -> Result<i32> {
    let subcommand = args.first().map(String::as_str).unwrap_or("");
    match subcommand {
        "test" => run_test(&args[1..], verbose),
        "format" => run_format(&args[1..], verbose),
        "compile" => run_compile(&args[1..], verbose),
        _ => run_passthrough(args, verbose),
    }
}

fn run_test(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = Command::new("mix");
    cmd.arg("test").args(args);
    if verbose > 0 {
        eprintln!("Running: mix test {}", args.join(" "));
    }
    runner::run_filtered_with_exit(
        cmd,
        "mix",
        &format!("test {}", args.join(" ")),
        summarize_mix_test,
        runner::RunOptions::with_tee("mix-test"),
    )
}

fn run_format(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = Command::new("mix");
    cmd.arg("format").args(args);
    if verbose > 0 {
        eprintln!("Running: mix format {}", args.join(" "));
    }
    runner::run_filtered_with_exit(
        cmd,
        "mix",
        &format!("format {}", args.join(" ")),
        summarize_mix_format,
        runner::RunOptions::with_tee("mix-format"),
    )
}

fn run_compile(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = Command::new("mix");
    cmd.arg("compile").args(args);
    if verbose > 0 {
        eprintln!("Running: mix compile {}", args.join(" "));
    }
    runner::run_filtered_with_exit(
        cmd,
        "mix",
        &format!("compile {}", args.join(" ")),
        summarize_mix_compile,
        runner::RunOptions::with_tee("mix-compile"),
    )
}

fn run_passthrough(args: &[String], verbose: u8) -> Result<i32> {
    let os_args: Vec<std::ffi::OsString> = args.iter().map(Into::into).collect();
    runner::run_passthrough("mix", &os_args, verbose)
}

pub(crate) fn summarize_mix_test(raw: &str, exit_code: i32) -> String {
    let lines = clean_lines(raw);

    if let Some(summary) = detect_database_unavailable(&lines) {
        return summary;
    }
    if let Some(summary) = detect_mix_pubsub_failure(&lines) {
        return summary;
    }
    if let Some(summary) = detect_compile_failure(&lines, "mix test") {
        return summary;
    }

    let final_summary = lines.iter().rev().find(|line| is_exunit_summary(line));
    if exit_code == 0 {
        if let Some(summary) = final_summary {
            return format!("[ok] mix test: {}", summary);
        }
        return compact_tail("[ok] mix test completed", &lines);
    }

    let mut output = String::from("[FAIL] mix test");
    if let Some(summary) = final_summary {
        output.push_str(&format!("\nsummary: {}", summary));
    }

    let failures = collect_exunit_failures(&lines);
    append_limited_blocks(&mut output, "failures", &failures, 6);
    if failures.is_empty() {
        append_limited(&mut output, "tail", &meaningful_tail(&lines, 12), 12);
    }
    output
}

pub(crate) fn summarize_mix_format(raw: &str, exit_code: i32) -> String {
    let lines = clean_lines(raw);
    if exit_code == 0 && lines.is_empty() {
        return "[ok] mix format".to_string();
    }
    if exit_code == 0 {
        return compact_tail("[ok] mix format", &lines);
    }
    let mut output = String::from("[FAIL] mix format");
    append_limited(&mut output, "output", &meaningful_tail(&lines, 20), 20);
    output
}

pub(crate) fn summarize_mix_compile(raw: &str, exit_code: i32) -> String {
    let lines = clean_lines(raw);
    if let Some(summary) = detect_compile_failure(&lines, "mix compile") {
        return summary;
    }

    let warnings = collect_warning_blocks(&lines);
    if exit_code == 0 && warnings.is_empty() {
        return "[ok] mix compile".to_string();
    }

    let mut output = if exit_code == 0 {
        format!("[ok] mix compile: {} warning(s)", warnings.len())
    } else {
        format!("[FAIL] mix compile: {} warning(s)", warnings.len())
    };
    append_limited_blocks(&mut output, "warnings", &warnings, 8);
    if warnings.is_empty() {
        append_limited(&mut output, "tail", &meaningful_tail(&lines, 12), 12);
    }
    output
}

fn clean_lines(raw: &str) -> Vec<String> {
    raw.lines()
        .map(strip_ansi)
        .map(|line| line.trim_end().to_string())
        .filter(|line| !is_shell_noise(line))
        .filter(|line| !line.trim().is_empty())
        .filter(|line| !is_mix_compile_noise(line))
        .filter(|line| !is_exunit_progress(line))
        .collect()
}

fn is_shell_noise(line: &str) -> bool {
    line.contains("Unable to open session log file")
        || line.contains("Can't create the symlink for multishells")
        || line.contains("starship::print")
        || line.contains("Under a 'dumb' terminal")
}

fn is_mix_compile_noise(line: &str) -> bool {
    let trimmed = line.trim();
    (trimmed.starts_with("Compiling ") && trimmed.contains(" file"))
        || trimmed.starts_with("Generated ")
}

fn is_exunit_progress(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| matches!(ch, '.' | '*' | ' ' | '\r'))
}

fn is_exunit_summary(line: &str) -> bool {
    (line.contains(" test, ") || line.contains(" tests, "))
        && (line.contains(" failure") || line.contains(" failures"))
}

fn detect_database_unavailable(lines: &[String]) -> Option<String> {
    let has_database_create_failure = lines
        .iter()
        .any(|line| line.contains("database") && line.contains("couldn't be created"));
    let connection_refused = lines
        .iter()
        .find(|line| line.contains("connection refused") || line.contains(":econnrefused"));
    if has_database_create_failure || connection_refused.is_some() {
        let root = connection_refused
            .cloned()
            .or_else(|| {
                lines
                    .iter()
                    .find(|line| line.contains("couldn't be created"))
                    .cloned()
            })
            .unwrap_or_else(|| "database unavailable".to_string());
        return Some(format!(
            "[FAIL] mix test: database unavailable\nroot: {}\nhint: check PGPORT/DATABASE_URL or start test Postgres",
            root.trim()
        ));
    }
    None
}

fn detect_mix_pubsub_failure(lines: &[String]) -> Option<String> {
    let failed_pubsub = lines
        .iter()
        .any(|line| line.contains("failed to start Mix.PubSub"));
    if !failed_pubsub {
        return None;
    }
    let root = lines
        .iter()
        .find(|line| line.contains("failed to open a TCP socket"))
        .cloned()
        .unwrap_or_else(|| "failed to start Mix.PubSub".to_string());
    Some(format!(
        "[FAIL] mix test: Mix PubSub could not start\nroot: {}",
        root.trim()
    ))
}

fn detect_compile_failure(lines: &[String], label: &str) -> Option<String> {
    let compile_error_index = lines.iter().position(|line| {
        line.starts_with("** (CompileError)")
            || line.starts_with("** (Mix)")
            || line.starts_with("error:")
    });
    compile_error_index.map(|index| {
        let details: Vec<String> = lines.iter().skip(index).take(10).cloned().collect();
        let mut output = format!("[FAIL] {}: compile/runtime error", label);
        append_limited(&mut output, "details", &details, 10);
        output
    })
}

fn collect_exunit_failures(lines: &[String]) -> Vec<Vec<String>> {
    let mut failures = Vec::new();
    let mut current = Vec::new();

    for line in lines {
        if is_failure_start(line) {
            if !current.is_empty() {
                failures.push(current);
            }
            current = vec![line.clone()];
            continue;
        }
        if !current.is_empty() {
            if is_exunit_summary(line) || line.starts_with("Finished in ") {
                failures.push(current);
                current = Vec::new();
            } else {
                current.push(line.clone());
            }
        }
    }

    if !current.is_empty() {
        failures.push(current);
    }

    failures
        .into_iter()
        .map(|block| block.into_iter().take(14).collect())
        .collect()
}

fn is_failure_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some((number, rest)) = trimmed.split_once(')') else {
        return false;
    };
    !number.is_empty()
        && number.chars().all(|ch| ch.is_ascii_digit())
        && rest.trim_start().starts_with("test ")
}

fn collect_warning_blocks(lines: &[String]) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();

    for line in lines {
        if line.trim_start().starts_with("warning:") {
            if !current.is_empty() {
                blocks.push(current);
            }
            current = vec![line.clone()];
            continue;
        }
        if !current.is_empty() {
            if line.trim_start().starts_with("warning:") {
                blocks.push(current);
                current = vec![line.clone()];
            } else if current.len() < 8 {
                current.push(line.clone());
            }
        }
    }

    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn meaningful_tail(lines: &[String], count: usize) -> Vec<String> {
    lines
        .iter()
        .rev()
        .take(count)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn compact_tail(prefix: &str, lines: &[String]) -> String {
    if lines.is_empty() {
        return prefix.to_string();
    }
    let mut output = prefix.to_string();
    append_limited(&mut output, "tail", &meaningful_tail(lines, 8), 8);
    output
}

fn append_limited(output: &mut String, title: &str, lines: &[String], limit: usize) {
    if lines.is_empty() {
        return;
    }
    output.push_str(&format!("\n{}:", title));
    for line in lines.iter().take(limit) {
        output.push_str(&format!("\n{}", line));
    }
    if lines.len() > limit {
        output.push_str(&format!("\n... +{} more", lines.len() - limit));
    }
}

fn append_limited_blocks(output: &mut String, title: &str, blocks: &[Vec<String>], limit: usize) {
    if blocks.is_empty() {
        return;
    }
    output.push_str(&format!("\n{}:", title));
    for block in blocks.iter().take(limit) {
        output.push('\n');
        output.push_str(&block.join("\n"));
    }
    if blocks.len() > limit {
        output.push_str(&format!("\n... +{} more", blocks.len() - limit));
    }
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code_ch in chars.by_ref() {
                if code_ch.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_test_success_is_one_line() {
        let raw = "Running ExUnit with seed: 97109, max_cases: 32\n\n....\nFinished in 1.1 seconds (0.8s async, 0.2s sync)\n304 tests, 0 failures\n";
        assert_eq!(
            summarize_mix_test(raw, 0),
            "[ok] mix test: 304 tests, 0 failures"
        );
    }

    #[test]
    fn mix_test_database_unavailable_is_compact() {
        let raw = "** (Mix) The database for GroupEngine.Repo couldn't be created: killed\nPostgrex.Protocol failed to connect: ** (DBConnection.ConnectionError) tcp connect (localhost:5432): connection refused - :econnrefused\n";
        let filtered = summarize_mix_test(raw, 1);
        assert!(filtered.contains("[FAIL] mix test: database unavailable"));
        assert!(filtered.contains("connection refused"));
        assert!(filtered.contains("check PGPORT/DATABASE_URL"));
    }

    #[test]
    fn mix_test_pubsub_failure_is_compact() {
        let raw = "** (RuntimeError) failed to start Mix.PubSub, reason: {{:shutdown, {:failed_to_start_child, Mix.PubSub.Subscriber, {%Mix.Error{message: \"failed to open a TCP socket in Mix.Sync.PubSub.subscribe/1, reason: :eperm\", mix: 1}, []}}}}";
        let filtered = summarize_mix_test(raw, 1);
        assert!(filtered.contains("[FAIL] mix test: Mix PubSub could not start"));
        assert!(filtered.contains(":eperm"));
    }

    #[test]
    fn mix_test_failure_keeps_failure_block() {
        let raw = "Running ExUnit with seed: 1, max_cases: 32\n\n  1) test rejects bad token (MyAppTest)\n     test/my_app_test.exs:42\n     Assertion with == failed\n     code: assert left == right\n\nFinished in 0.1 seconds\n1 test, 1 failure\n";
        let filtered = summarize_mix_test(raw, 2);
        assert!(filtered.contains("summary: 1 test, 1 failure"));
        assert!(filtered.contains("test/my_app_test.exs:42"));
        assert!(filtered.contains("Assertion with == failed"));
    }

    #[test]
    fn mix_compile_success_strips_noise() {
        let raw = "Compiling 2 files (.ex)\nGenerated group_engine app\n";
        assert_eq!(summarize_mix_compile(raw, 0), "[ok] mix compile");
    }

    #[test]
    fn mix_compile_keeps_warnings() {
        let raw = "Compiling 1 file (.ex)\nwarning: variable \"conn\" is unused\n  lib/router.ex:42\nGenerated app\n";
        let filtered = summarize_mix_compile(raw, 0);
        assert!(filtered.contains("[ok] mix compile: 1 warning(s)"));
        assert!(filtered.contains("variable \"conn\" is unused"));
    }

    #[test]
    fn mix_format_empty_success() {
        assert_eq!(summarize_mix_format("", 0), "[ok] mix format");
    }
}
