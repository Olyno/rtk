# Kimi CLI Hooks

> Part of [`hooks/`](../README.md) — see also [`src/hooks/`](../../src/hooks/README.md) for installation code

## Specifics

- Shell-based `PreToolUse` hook — requires `jq` for JSON parsing
- Returns `hookSpecificOutput.updatedInput` JSON for transparent command rewrite (agent doesn't know RTK is involved)
- Exits silently (exit 0) on any failure: jq missing, rtk missing, rtk too old (< 0.23.0), no match
- Version guard checks `rtk --version` against minimum 0.23.0
- Kimi CLI config location: `~/.kimi/config.toml`

## Manual Installation

Add to `~/.kimi/config.toml`:

```toml
[[hooks]]
event = "PreToolUse"
matcher = "Shell"
command = "/path/to/rtk-rewrite.sh"
timeout = 10
```

Or use the `rtk init` command (when available):

```bash
rtk init --global --agent kimi
```

## How It Works

Kimi CLI's `PreToolUse` hook receives JSON on stdin:

```json
{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "hook_event_name": "PreToolUse",
  "tool_name": "Shell",
  "tool_input": {"command": "git status"},
  "tool_call_id": "call_123"
}
```

The hook extracts `tool_input.command`, calls `rtk rewrite`, and returns:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "RTK auto-rewrite",
    "updatedInput": {"command": "rtk git status"}
  }
}
```

Kimi CLI then executes the rewritten command transparently.

## Testing

```bash
# Test the hook directly
echo '{"tool_name":"Shell","tool_input":{"command":"git status"}}' | bash hooks/kimi/rtk-rewrite.sh

# Test with a command that has an RTK equivalent
echo '{"tool_name":"Shell","tool_input":{"command":"cargo test"}}' | bash hooks/kimi/rtk-rewrite.sh

# Test with no match (should output nothing)
echo '{"tool_name":"Shell","tool_input":{"command":"echo hello"}}' | bash hooks/kimi/rtk-rewrite.sh
```
