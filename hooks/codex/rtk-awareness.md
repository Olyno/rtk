# RTK - Rust Token Killer (Codex CLI)

**Usage**: Token-optimized CLI proxy for shell commands.

## Rule

Always prefix shell commands with `rtk`.

Examples:

```bash
rtk git status
rtk cargo test
rtk npm run build
rtk pytest -q
```

## Meta Commands

```bash
rtk gain            # Token savings analytics
rtk gain --history  # Recent command savings history
rtk expand <hash>   # Resolve a §ref:<hash>§ token to full/cached output
rtk proxy <cmd>     # Run raw command without filtering
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

## Verification

```bash
rtk --version
rtk gain
which rtk
```
