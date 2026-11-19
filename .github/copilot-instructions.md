# Copilot Instructions for rtk

**rtk (Rust Token Killer)** is a CLI proxy that filters and compresses command outputs before they reach an LLM context. It wraps common tools (`git`, `cargo`, `grep`, `pnpm`, `go`, etc.) and outputs condensed summaries instead of raw output. Percentages measure bash output, not billed tokens; RTK ships no tokenizer (`src/core/tracking.rs` estimates `bytes / 4`).

## Using rtk in this session

**Always prefix commands with `rtk` when running shell commands** — this reduces token consumption for every operation you perform.

```bash
# Instead of:              Use:
git status                 rtk git status
git log -10                rtk git log -10
cargo test                 rtk cargo test
cargo clippy --all-targets rtk cargo clippy --all-targets
grep -r "pattern" src/     rtk grep -r "pattern" src/
```

**rtk meta-commands** (always use these directly, no prefix needed):
```bash
rtk gain              # Show token savings analytics
rtk gain --history    # Full command history with per-command savings
rtk discover          # Scan session history for missed rtk opportunities
rtk bypass <cmd>       # Run a command raw (no filtering) but still track it
```

### Expanded command reference

Always prefer `rtk <cmd>` when a filter exists. If you need the raw bytes, use `rtk proxy <cmd>`.

**Context & discovery (run these first in a new session)**
- `rtk index [path]` — build/update the local symbol index.
- `rtk read --mode signatures <file>` — read only function/class signatures.
- `rtk read --mode map <file>` — signatures + imports.

**Compression & references (use before pasting long output)**
- `rtk recall <hash>` — restore full output behind an `rtk recall`-style hint (large outputs are auto-stored).
- `rtk diff <file1> <file2>` — changed-lines-only diff.
- `rtk pipe [-f filter]` — filter stdin through an RTK filter.

**Analytics & trust**
- `rtk gain` — token savings dashboard.
- `rtk gain --history` — per-command history with savings.
- `rtk discover` / `rtk learn` — scan session history for missed RTK opportunities.
- `rtk trust` / `rtk untrust` — authorize/revoke project TOML filters.
- `rtk verify` — validate TOML filter syntax and inline tests.

**Supported ecosystems (prefix with `rtk`)**
Git / forges, Rust, Go, Node, Python, Ruby, .NET, Gradle, Cloud/Ops (docker, kubectl, aws, curl, wget, psql, mvn), and utilities (`ls`, `tree`, `find`, `wc`, `env`, `grep`, `rg`, `json`, `log`, `err`, `summary`, `smart`, `test`, `run`, `proxy`).

**Automation tip**
- `rtk run <cmd>` auto-routes recognized commands to the right filter, so you can default to it when unsure.
- `rtk proxy <cmd>` always runs raw when you need every byte.

**When NOT to use rtk**
- When the raw full output is required: `rtk proxy <cmd>`.

**Verify rtk is installed before starting:**
```bash
rtk --version   # Should print: rtk X.Y.Z
rtk gain        # Should show a dashboard (not "command not found")
```

> Name collision: `rtk gain` failing means you have `reachingforthejack/rtk` (Rust Type Kit) installed instead. Run `which rtk` to check.

## Build, Test & Lint

```bash
cargo build                    # Development build
cargo test                     # All tests
cargo test test_name           # Single test
cargo test module::tests::     # Module tests
cargo test -- --nocapture      # With stdout

# Pre-commit gate (must all pass before any PR)
cargo fmt --all --check && cargo clippy --all-targets && cargo test

bash scripts/test-all.sh       # Smoke tests (requires installed binary)
```

PRs target the **`develop`** branch, not `main`. All commits require a DCO sign-off (`git commit -s`).

## Architecture

rtk routes CLI commands via a Clap `Commands` enum in `main.rs` to specialized filter modules in `src/cmds/*/`, each executing the underlying command and compressing output. Token savings are tracked in SQLite via `src/core/tracking.rs`.

For full details see [ARCHITECTURE.md](../docs/contributing/ARCHITECTURE.md) and [docs/contributing/TECHNICAL.md](../docs/contributing/TECHNICAL.md). Module responsibilities are documented in each folder's `README.md` and each file's `//!` doc header.

## Key Conventions

- **Error handling**: `anyhow::Result` with `.context("description")?` — no bare `?`, no `unwrap()` in production. Filters must fall back to raw command on error.
- **Regex**: Use `LazyLock<Regex>` for fixed patterns reused across calls; keep runtime-dependent patterns local.
- **Testing**: Unit tests inside modules (`#[cfg(test)] mod tests`). Fixtures in `tests/fixtures/`. Token savings assertions with `count_tokens()`.
- **Exit codes**: Preserve the underlying command's exit code via `std::process::exit(code)`.
- **Performance**: Startup <10ms (no async runtime), binary <5MB stripped.
- **Branch naming**: `fix(scope):`, `feat(scope):`, `chore(scope):` where scope is the affected component.

For the full contribution workflow, design philosophy, and new-filter checklist, see [CONTRIBUTING.md](../CONTRIBUTING.md).
