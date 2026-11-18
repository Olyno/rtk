# RTK - Rust Token Killer (Kilo Code)

**Usage**: Token-optimized CLI proxy for shell commands.

## Rule

Always prefix shell commands with `rtk` to minimize token consumption.

Examples:

```bash
rtk git status
rtk cargo test
rtk ls src/
rtk grep "pattern" src/
rtk find "*.rs" .
rtk docker ps
rtk gh pr list
```

## Meta Commands

```bash
rtk gain              # Show token savings
rtk gain --history    # Command history with savings
rtk discover          # Find missed RTK opportunities
rtk expand <hash>     # Resolve a §ref:<hash>§ token to full/cached output
rtk proxy <cmd>       # Run raw (no filtering, for debugging)
```

## Command Reference

Prefer `rtk <cmd>` whenever a filter exists. Use `rtk proxy <cmd>` for raw output.

- **Index / read**: `rtk index`, `rtk read --mode signatures|map`.
- **Compression / refs**: `rtk expand <hash>`, `rtk diff`, `rtk pipe`.
- **Analytics**: `rtk gain`, `rtk gain --history`, `rtk discover`, `rtk learn`, `rtk trust`, `rtk verify`.
- **Ecosystems**: git/gh/glab/gt, cargo, go, npm/pnpm/npx, python (pytest/pip/ruff/mypy), ruby, dotnet, flutter/dart/gradle, docker/kubectl/aws/curl/wget/psql/mvn, mix.
- **Utilities**: `ls`, `tree`, `find`, `wc`, `env`, `grep`, `rg`, `json`, `log`, `err`, `summary`, `smart`, `test`, `run`, `proxy`.

## Smart Pointers

- If RTK output ends with §ref:<hash>§, the full/cached output is one `rtk expand <hash>` away.
- Prefer asking for a specific file/symbol over dumping whole directories.

## Why

RTK filters and compresses command output before it reaches the LLM context, saving 60-90% tokens on common operations. Always use `rtk <cmd>` instead of raw commands.
