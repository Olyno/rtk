# RTK - Rust Token Killer

**Usage**: Token-optimized CLI proxy (60-90% savings on dev operations)

## Meta Commands (always use rtk directly)

```bash
rtk gain              # Show token savings analytics
rtk gain --history    # Show command usage history with savings
rtk discover          # Analyze Claude Code history for missed opportunities
rtk expand <hash>     # Resolve a §ref:<hash>§ token to full/cached output
rtk proxy <cmd>       # Execute raw command without filtering (for debugging)
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

## Installation Verification

```bash
rtk --version         # Should show: rtk X.Y.Z
rtk gain              # Should work (not "command not found")
which rtk             # Verify correct binary
```

⚠️ **Name collision**: If `rtk gain` fails, you may have reachingforthejack/rtk (Rust Type Kit) installed instead.

## Hook-Based Usage

All other commands are automatically rewritten by the Claude Code hook.
Example: `git status` → `rtk git status` (transparent, 0 tokens overhead)

Refer to CLAUDE.md for full command reference.
