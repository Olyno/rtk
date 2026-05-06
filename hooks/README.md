1|# LLM Agent Hooks
2|
3|## Scope
4|
5|**Deployed hook artifacts** — the actual files installed on user machines by `rtk init`. These are shell scripts, TypeScript plugins, and rules files that run outside the Rust binary. They are **thin delegates**: parse agent-specific JSON, call `rtk rewrite` as a subprocess, format agent-specific response. Zero filtering logic lives here.
6|
7|8|Owns: per-agent hook scripts and configuration files for 10 supported agents (Claude Code, Copilot, Cursor, Cline, Windsurf, Codex, OpenCode, Hermes, Kimi CLI, Kilocode, Antigravity).
9|12|
13|Does **not** own: hook installation/uninstallation (that's `src/hooks/init.rs`), the rewrite pattern registry (that's `discover/registry`), or integrity verification (that's `src/hooks/integrity.rs`).
14|
15|Relationship to `src/hooks/`: that component **creates** these files; this directory **contains** them.
16|
17|## Purpose
18|
19|LLM agent integrations that intercept CLI commands and route them through RTK for token optimization. Each hook transparently rewrites raw commands (e.g., `git status`) to their RTK equivalents (e.g., `rtk git status`), delivering 60-90% token savings without requiring the agent or user to change their workflow.
20|
21|## How It Works
22|
23|```
24|Agent runs command (e.g., "cargo test --nocapture")
25|  -> Hook intercepts (PreToolUse / plugin event)
26|  -> Reads JSON input, extracts command string
27|  -> Calls `rtk rewrite "cargo test --nocapture"`
28|  -> Registry matches pattern, returns "rtk cargo test --nocapture"
29|  -> Hook sends response in agent-specific JSON format
30|  -> Agent executes "rtk cargo test --nocapture" instead
31|  -> Filtered output reaches LLM (~90% fewer tokens)
32|```
33|
34|All rewrite logic lives in the Rust binary (`src/discover/registry.rs`). Hook scripts are **thin delegates** that handle agent-specific JSON formats and call `rtk rewrite` for the actual decision. This ensures a single source of truth for all 70+ rewrite patterns.
35|
36|## Directory Structure
37|
38|Each agent subdirectory has its own README with hook-specific details:
39|
40|- **[`claude/`](claude/README.md)** — Shell hook, `PreToolUse` JSON format, `settings.json` patching, test script
41|- **[`copilot/`](copilot/README.md)** — Rust binary hook, dual format (VS Code Chat vs Copilot CLI), deny-with-suggestion fallback
42|- **[`cursor/`](cursor/README.md)** — Shell hook, Cursor JSON format, empty `{}` response requirement
43|- **[`cline/`](cline/README.md)** — Rules file (prompt-level), `.clinerules` project-local installation
44|- **[`windsurf/`](windsurf/README.md)** — Rules file (prompt-level), `.windsurfrules` workspace-scoped
45|- **[`codex/`](codex/README.md)** — Awareness document, `AGENTS.md` integration, `$CODEX_HOME` or `~/.codex/` location
46|- **[`opencode/`](opencode/README.md)** — TypeScript plugin, `zx` library, `tool.execute.before` event, in-place mutation
47|48|- **[`hermes/`](hermes/README.md)** — Python plugin, `pre_tool_call` hook, in-place terminal command mutation
49|- **[`kimi/`](kimi/README.md)** — Shell hook, `PreToolUse` JSON format, `~/.kimi/config.toml` integration
50|- **[`kilocode/`](kilocode/README.md)** — Rules file (prompt-level), `.kilocoderules` project-local installation
51|- **[`antigravity/`](antigravity/README.md)** — Rules file (prompt-level), `.antigravityrules` project-local installation
52|55|
56|## Supported Agents
57|
58|| Agent | Mechanism | Hook Type | Can Modify Command? |
59||-------|-----------|-----------|---------------------|
60|| Claude Code | Shell hook (`PreToolUse`) | Transparent rewrite | Yes (`updatedInput`) |
61|| VS Code Copilot Chat | Rust binary (`rtk hook copilot`) | Transparent rewrite | Yes (`updatedInput`) |
62|| GitHub Copilot CLI | Rust binary (`rtk hook copilot`) | Deny-with-suggestion | No (agent retries) |
63|| Cursor | Shell hook (`preToolUse`) | Transparent rewrite | Yes (`updated_input`) |
64|| Gemini CLI | Rust binary (`rtk hook gemini`) | Transparent rewrite | Yes (`hookSpecificOutput`) |
65|| Kimi CLI | Shell hook (`PreToolUse`) | Transparent rewrite | Yes (`hookSpecificOutput`) |
66|| Cline / Roo Code | Custom instructions (rules file) | Prompt-level guidance | N/A |
67|| Windsurf | Custom instructions (rules file) | Prompt-level guidance | N/A |
68|| Codex CLI | AGENTS.md / instructions | Prompt-level guidance | N/A |
69|| OpenCode | TypeScript plugin (`tool.execute.before`) | In-place mutation | Yes |
70|71|| Hermes | Python plugin (`pre_tool_call`) | In-place mutation | Yes |
72|| Kilocode | Custom instructions (rules file) | Prompt-level guidance | N/A |
73|| Antigravity | Custom instructions (rules file) | Prompt-level guidance | N/A |
74|77|
78|## JSON Formats by Agent
79|
80|### Claude Code (Shell Hook)
81|
82|**Input** (stdin):
83|```json
84|{
85|  "tool_name": "Bash",
86|  "tool_input": { "command": "git status" }
87|}
88|```
89|
90|**Output** (stdout, when rewritten):
91|```json
92|{
93|  "hookSpecificOutput": {
94|    "hookEventName": "PreToolUse",
95|    "permissionDecision": "allow",
96|    "permissionDecisionReason": "RTK auto-rewrite",
97|    "updatedInput": { "command": "rtk git status" }
98|  }
99|}
100|```
101|
102|### Cursor (Shell Hook)
103|
104|**Input**: Same as Claude Code.
105|
106|**Output** (stdout, when rewritten):
107|```json
108|{
109|  "permission": "allow",
110|  "updated_input": { "command": "rtk git status" }
111|}
112|```
113|
114|Returns `{}` when no rewrite (Cursor requires JSON for all paths).
115|
116|### Copilot CLI (Rust Binary)
117|
118|**Input** (stdin, camelCase, `toolArgs` is JSON-stringified):
119|```json
120|{
121|  "toolName": "bash",
122|  "toolArgs": "{\"command\": \"git status\"}"
123|}
124|```
125|
126|**Output** (no `updatedInput` support -- uses deny-with-suggestion):
127|```json
128|{
129|  "permissionDecision": "deny",
130|  "permissionDecisionReason": "Token savings: use `rtk git status` instead"
131|}
132|```
133|
134|### VS Code Copilot Chat (Rust Binary)
135|
136|**Input** (stdin, snake_case):
137|```json
138|{
139|  "tool_name": "Bash",
140|  "tool_input": { "command": "git status" }
141|}
142|```
143|
144|**Output**: Same as Claude Code format (with `updatedInput`).
145|
146|### Gemini CLI (Rust Binary)
147|
148|**Input** (stdin):
149|```json
150|{
151|  "tool_name": "run_shell_command",
152|  "tool_input": { "command": "git status" }
153|}
154|```
155|
156|**Output** (when rewritten):
157|```json
158|{
159|  "decision": "allow",
160|  "hookSpecificOutput": {
161|    "tool_input": { "command": "rtk git status" }
162|  }
163|}
164|```
165|
166|**No rewrite**: `{"decision": "allow"}`
167|
168|### Kimi CLI (Shell Hook)
169|
170|**Input** (stdin):
171|```json
172|{
173|  "session_id": "abc123",
174|  "cwd": "/path/to/project",
175|  "hook_event_name": "PreToolUse",
176|  "tool_name": "Shell",
177|  "tool_input": { "command": "git status" },
178|  "tool_call_id": "call_123"
179|}
180|```
181|
182|**Output** (stdout, when rewritten):
183|```json
184|{
185|  "hookSpecificOutput": {
186|    "hookEventName": "PreToolUse",
187|    "permissionDecision": "allow",
188|    "permissionDecisionReason": "RTK auto-rewrite",
189|    "updatedInput": { "command": "rtk git status" }
190|  }
191|}
192|```
193|
194|### OpenCode (TypeScript Plugin)
195|
196|Mutates `args.command` in-place via the zx library:
197|```typescript
198|const result = await $`rtk rewrite ${command}`.quiet().nothrow()
199|const rewritten = String(result.stdout).trim()
200|if (rewritten && rewritten !== command) {
201|  (args as Record<string, unknown>).command = rewritten
202|}
203|```
204|
205|### Hermes (Python Plugin)
206|
207|Mutates `args["command"]` in-place via the `pre_tool_call` hook:
208|
209|```python
210|result = subprocess.run(["rtk", "rewrite", command], capture_output=True, text=True, timeout=2)
211|rewritten = result.stdout.strip()
212|if result.returncode in {0, 3} and rewritten and rewritten != command:
213|    args["command"] = rewritten
214|```
215|
216|## Command Rewrite Registry
217|
218|The registry (`src/discover/registry.rs`) handles command patterns across these categories:
219|
220|| Category | Examples | Savings |
221||----------|----------|---------|
222|| Test Runners | vitest, pytest, cargo test, go test, playwright | 90-99% |
223|| Build Tools | cargo build, npm, pnpm, dotnet, make | 70-90% |
224|| VCS | git status/log/diff/show | 70-80% |
225|| Language Servers | tsc, mypy | 80-83% |
226|| Linters | eslint, ruff, golangci-lint, biome | 80-85% |
227|| Package Managers | pip, cargo install, pnpm list | 75-80% |
228|| File Operations | ls, find, grep, cat, head, tail | 60-75% |
229|| Infrastructure | docker, kubectl, aws, terraform | 75-85% |
230|
231|### Compound Command Handling
232|
233|The registry handles `&&`, `||`, `;`, `|`, and `&` operators:
234|
235|- **Pipe** (`|`): Only the left side is rewritten (right side consumes output format)
236|- **And/Or/Semicolon** (`&&`, `||`, `;`): Both sides rewritten independently
237|- **find/fd in pipes**: Never rewritten (output format incompatible with xargs/wc/grep)
238|
239|Example: `cargo fmt --all && cargo test` becomes `rtk cargo fmt --all && rtk cargo test`
240|
241|### Override Controls
242|
243|- **`RTK_DISABLED=1`**: Per-command override (`RTK_DISABLED=1 git status` runs raw)
244|- **`exclude_commands`**: In `~/.config/rtk/config.toml`, list commands to never rewrite. Matches against the full command after stripping env prefixes. Subcommand patterns work (`"git push"` excludes `git push origin main`). Patterns starting with `^` are treated as regex.
245|- **Already-RTK**: `rtk git status` passes through unchanged (no `rtk rtk git`)
246|
247|## Exit Code Contract
248|
249|Hooks must **never block command execution**. All error paths (missing binary, bad JSON, rewrite failure) must exit 0 so the agent's command runs unmodified. A hook that exits non-zero prevents the user's command from executing.
250|
251|When there is no rewrite to apply, the hook must produce no output (or `{}` for Cursor, which requires JSON on all paths).
252|
253|### Gaps (to be fixed)
254|
255|- `hook_cmd.rs::run_gemini()` — exits 1 on invalid JSON input instead of exit 0
256|
257|## Graceful Degradation
258|
259|Hooks are **non-blocking** -- they never prevent a command from executing:
260|
261|- jq not installed: warning to stderr, exit 0 (command runs raw)
262|- rtk binary not found: warning to stderr, exit 0
263|- rtk version too old (< 0.23.0): warning to stderr, exit 0
264|- Invalid JSON input: pass through unchanged
265|- `rtk rewrite` crashes: hook exits 0 (subprocess error ignored)
266|- Filter logic error: fallback to raw command output
267|
268|## Adding a New Agent Integration
269|
270|New integrations must follow the [Exit Code Contract](#exit-code-contract) and [Graceful Degradation](#graceful-degradation) above, as well as the project's [Design Philosophy](../CONTRIBUTING.md#design-philosophy).
271|
272|### Integration Tiers
273|
274|| Tier | Mechanism | Maintenance | Examples |
275||------|-----------|-------------|----------|
276|| **Full hook** | Shell script or Rust binary, intercepts commands via agent's hook API | High — must track agent API changes | Claude Code, Cursor, Copilot, Gemini |
277|278|| **Plugin** | TypeScript/JS/Python plugin in agent's plugin system | Medium — agent manages loading | OpenCode, Hermes |
279|282|| **Rules file** | Prompt-level instructions the agent reads | Low — no code to break | Cline, Windsurf, Codex |
283|
284|### Eligibility
285|
286|RTK supports AI coding assistants that developers actually use day-to-day. To add a new agent:
287|
288|- Agent has a **documented, stable hook/plugin API** (not experimental/alpha)
289|- Agent is **actively maintained** (commit activity in last 3 months)
290|- Integration follows the **exit code contract** (exit 0 on all error paths)
291|- Hook output matches the **agent's expected JSON format** exactly
292|
293|### Maintenance
294|
295|If an agent's API changes and the hook breaks, the integration should be updated promptly. If the agent becomes unmaintained or the hook can't be fixed, the integration may be deprecated with a release note.
296|297|304|