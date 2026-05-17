//! Filters npm output and auto-injects the "run" subcommand when appropriate.
//!
//! Also detects common scripts (test→vitest, build→tsc) and routes through
//! their specialized parsers for better token savings.

use crate::core::runner;
use crate::core::utils::resolved_command;
use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter};
use anyhow::Result;
use serde::Deserialize;

/// Known npm subcommands that should NOT get "run" injected.
/// Shared between production code and tests to avoid drift.
const NPM_SUBCOMMANDS: &[&str] = &[
    "install", "i", "ci", "uninstall", "remove", "rm", "update", "up", "list", "ls", "outdated",
    "init", "create", "publish", "pack", "link", "audit", "fund", "exec", "explain", "why",
    "search", "view", "info", "show", "config", "set", "get", "cache", "prune", "dedupe",
    "doctor", "help", "version", "prefix", "root", "bin", "bugs", "docs", "home", "repo",
    "ping", "whoami", "token", "profile", "team", "access", "owner", "deprecate", "dist-tag",
    "star", "stars", "login", "logout", "adduser", "unpublish", "pkg", "diff", "rebuild",
    "test", "t", "start", "stop", "restart",
];

/// Script routing hint parsed from package.json
enum ScriptRouter {
    Vitest,
    Tsc,
}

/// Parse package.json scripts and detect routing for a script name
fn detect_route(script_name: &str) -> Option<ScriptRouter> {
    #[derive(Deserialize)]
    struct PackageJson {
        scripts: Option<std::collections::HashMap<String, String>>,
    }

    let content = std::fs::read_to_string("package.json").ok()?;
    let pkg: PackageJson = serde_json::from_str(&content).ok()?;
    let scripts = pkg.scripts?;
    let script = scripts.get(script_name)?;

    let trimmed = script.trim();

    if trimmed.contains("vitest") {
        return Some(ScriptRouter::Vitest);
    }
    if trimmed.starts_with("tsc") || trimmed.contains("tsc ") || trimmed == "tsc" {
        return Some(ScriptRouter::Tsc);
    }

    None
}

/// Try to parse npm script output with a specialized parser.
/// Returns Some(filtered_output) on success, None to fall through to generic filter.
fn try_specialized_parse(raw_output: &str, route: &ScriptRouter) -> Option<String> {
    match route {
        ScriptRouter::Vitest => {
            let result = crate::cmds::js::vitest_cmd::VitestParser::parse(raw_output);
            match result {
                ParseResult::Full(data) | ParseResult::Degraded(data, _) => {
                    Some(data.format(FormatMode::Compact))
                }
                ParseResult::Passthrough(_) => None,
            }
        }
        ScriptRouter::Tsc => {
            // tsc output is mostly errors/warnings — just strip npm boilerplate
            // and keep the tsc diagnostics intact (they're already concise)
            None
        }
    }
}

pub fn run(args: &[String], verbose: u8, skip_env: bool) -> Result<i32> {
    // Determine if this is "npm run <script>" or another npm subcommand (install, list, etc.)
    // Only inject "run" when args look like a script name, not a known npm subcommand.
    let first_arg = args.first().map(|s| s.as_str());
    let is_run_explicit = first_arg == Some("run");
    let is_npm_subcommand = first_arg
        .map(|a| NPM_SUBCOMMANDS.contains(&a) || a.starts_with('-'))
        .unwrap_or(false);

    let mut effective_args: Vec<String> = Vec::with_capacity(args.len() + 1);
    if is_run_explicit || is_npm_subcommand {
        effective_args.extend_from_slice(args);
    } else {
        // "rtk npm build" → "npm run build" (assume script name)
        effective_args.push("run".to_string());
        effective_args.extend_from_slice(args);
    }

    // Detect script routing
    let script_name = if is_run_explicit && args.len() > 1 {
        Some(args[1].as_str())
    } else if is_npm_subcommand && !first_arg.map(|a| a.starts_with('-')).unwrap_or(false) {
        // Check if this subcommand also exists as a script (e.g., "npm test")
        // NPM_SUBCOMMANDS includes lifecycle scripts like test, start, stop
        first_arg
    } else {
        None
    };

    let route = script_name.and_then(detect_route);

    run_filtered("npm", &effective_args, verbose, skip_env, route.as_ref())
}

/// Run an npx tool through the same filtered pipeline as `npm`.
pub fn exec(args: &[String], verbose: u8, skip_env: bool) -> Result<i32> {
    run_filtered("npx", args, verbose, skip_env, None)
}

/// Shared command-execution path for `run` (npm) and `exec` (npx).
fn run_filtered(
    name: &str,
    args: &[String],
    verbose: u8,
    skip_env: bool,
    route: Option<&ScriptRouter>,
) -> Result<i32> {
    let mut cmd = resolved_command(name);
    for arg in args {
        cmd.arg(arg);
    }

    if skip_env {
        cmd.env("SKIP_ENV_VALIDATION", "1");
    }

    let args_display = args.join(" ");
    if verbose > 0 {
        eprintln!("Running: {} {}", name, args_display);
    }

    let options = runner::RunOptions::default();

    // If we have a specialized route, use the npm boilerplate filter first,
    // then apply the specialized parser
    if let Some(r) = route {
        if verbose > 0 {
            eprintln!("  (routed through {:?} parser)", std::mem::discriminant(r));
        }
        // Still run through npm filter to strip boilerplate, then apply specialized parser
        let specialized_filter = move |output: &str| {
            let stripped = filter_npm_output(output);
            // Try specialized parser, fall back to stripped output
            try_specialized_parse(&stripped, r).unwrap_or(stripped)
        };
        runner::run_filtered(cmd, name, &args_display, specialized_filter, options)
    } else {
        runner::run_filtered(cmd, name, &args_display, filter_npm_output, options)
    }
}

/// Filter npm run output - strip boilerplate, progress bars, npm WARN
fn filter_npm_output(output: &str) -> String {
    let mut result = Vec::new();

    for line in output.lines() {
        // Skip npm boilerplate
        if line.starts_with('>') && line.contains('@') {
            continue;
        }
        // Skip npm lifecycle scripts
        if line.trim_start().starts_with("npm WARN") {
            continue;
        }
        if line.trim_start().starts_with("npm notice") {
            continue;
        }
        // Skip progress indicators
        if line.contains("⸩") || line.contains("⸨") || line.contains("...") && line.len() < 10 {
            continue;
        }
        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        result.push(line.to_string());
    }

    if result.is_empty() {
        "ok".to_string()
    } else {
        result.join("\n")
    }
}
