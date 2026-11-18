//! End-to-end tests for §ref:<hash>§ tokens and `rtk expand`.

use std::io::Write;
use std::process::{Command, Stdio};

fn rtk_stdin(args: &[&str], input: &str) -> (String, String, i32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtk"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtk");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait rtk");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn archived_output_emits_ref_token() {
    // Large JSON input should trigger archiving (>= 512 bytes raw).
    let mut input = String::from("{");
    for i in 0..100 {
        input.push_str(&format!("\"key_{i}\":\"value_{i}\","));
    }
    input.push_str("\"last\":1}");

    let (stdout, _stderr, code) = rtk_stdin(&["json", "-"], &input);
    assert_eq!(code, 0, "rtk json should exit 0");
    assert!(
        stdout.contains("§ref:"),
        "archived output should emit a compact ref token: {stdout}"
    );
}

#[test]
fn expand_resolves_archive_token() {
    let mut input = String::from("{");
    for i in 0..100 {
        input.push_str(&format!("\"key_{i}\":\"value_{i}\","));
    }
    input.push_str("\"last\":1}");

    let (stdout, _, _) = rtk_stdin(&["json", "-"], &input);
    let token_line = stdout
        .lines()
        .find(|l| l.contains("§ref:"))
        .expect("ref token line");
    let id = token_line
        .split("§ref:")
        .nth(1)
        .expect("ref id start")
        .split('§')
        .next()
        .expect("ref id end");

    let (expand_out, _, expand_code) = rtk_stdin(&["expand", id], "");
    assert_eq!(expand_code, 0, "rtk expand should exit 0");
    assert!(
        expand_out.contains(&input),
        "expand should restore original input: {expand_out}"
    );
}
