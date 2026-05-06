# RTK — Rust Token Killer

**Usage**: Token-optimized CLI proxy. Commands are automatically rewritten
by the RTK extension before they execute — no changes to your workflow needed.

## Meta Commands

```bash
rtk gain              # Show token savings analytics
rtk gain --history    # Show recent command savings history
rtk proxy <cmd>       # Run raw command without filtering (bypass RTK)
```

## Verification

```bash
rtk --version         # Should show: rtk X.Y.Z
rtk gain              # Should work (not "command not found")
which rtk             # Verify correct binary in PATH
```

⚠️ **Name collision**: Two packages share the name `rtk`. If `rtk gain`
fails, you may have `reachingforthejack/rtk` (Rust Type Kit) installed
instead. See the RTK README for installation instructions.

## How It Works

The RTK extension intercepts every bash tool call and transparently rewrites
commands to their token-optimized equivalents before execution:

```
git status          →  rtk git status      (~70% fewer tokens)
cargo test          →  rtk cargo test      (~90% fewer tokens)
npm run build       →  rtk npm run build   (~80% fewer tokens)
```

Rewrites are invisible — the LLM sees only the filtered output.
