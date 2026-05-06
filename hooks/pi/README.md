# Pi Hooks

> Part of [`hooks/`](../README.md) — see also [`src/hooks/`](../../src/hooks/README.md) for installation code

## Specifics

- TypeScript extension using Pi's `ExtensionAPI` (not a shell hook, no `zx` dependency)
- Subscribes to `tool_call` event, narrows to `bash` tool via `isToolCallEventType`
- Calls `rtk rewrite` as a subprocess; mutates `event.input.command` in-place if rewrite differs
- Returns `{ block: true, reason }` on deny (exit code 2); all other error paths return `undefined`
- Version guard at load time: checks `rtk >= 0.23.0`; warns and registers no-op if too old or missing
- Installed to `.pi/extensions/rtk.ts` by `rtk init --pi` (project-local) or `~/.pi/agent/extensions/rtk.ts` by `rtk init --pi --global`
- `rtk-awareness.md` is embedded into `AGENTS.md` by `rtk init --pi`

## Testing

```bash
# Load the extension directly without installing
pi -e ./hooks/pi/rtk.ts

# Verify rewrites are active — ask the agent to run a command, then check history
rtk gain --history   # should show rtk-prefixed commands with savings %

# Test RTK_DISABLED passthrough
RTK_DISABLED=1 pi -e ./hooks/pi/rtk.ts
# → commands pass through unchanged; no rewrites in rtk gain --history

# Test version guard — temporarily shadow rtk with a stub that prints "rtk 0.22.0"
# → extension logs a warning at startup and registers a no-op; pi starts normally
```

## Design Notes

- All filtering logic lives in `rtk rewrite` (the Rust registry), not in this file
- Exit code 3 (ask) is treated as allow — Pi has no per-tool confirmation UI
- No `zx` library required; uses `node:child_process` `spawn` directly
- See [`docs/specs/pi-hook-integration.md`](../../docs/specs/pi-hook-integration.md) for the full design spec
