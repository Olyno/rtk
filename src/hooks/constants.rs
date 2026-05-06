1|pub const REWRITE_HOOK_FILE: &str = "rtk-rewrite.sh";
2|pub const GEMINI_HOOK_FILE: &str = "rtk-hook-gemini.sh";
3|pub const CLAUDE_DIR: &str = ".claude";
4|pub const HOOKS_SUBDIR: &str = "hooks";
5|pub const SETTINGS_JSON: &str = "settings.json";
6|pub const SETTINGS_LOCAL_JSON: &str = "settings.local.json";
7|pub const HOOKS_JSON: &str = "hooks.json";
8|pub const PRE_TOOL_USE_KEY: &str = "PreToolUse";
9|pub const BEFORE_TOOL_KEY: &str = "BeforeTool";
10|
11|/// Native Rust hook command for Claude Code (replaces rtk-rewrite.sh).
12|pub const CLAUDE_HOOK_COMMAND: &str = "rtk hook claude";
13|/// Native Rust hook command for Cursor (replaces rtk-rewrite.sh).
14|pub const CURSOR_HOOK_COMMAND: &str = "rtk hook cursor";
15|
16|pub const CONFIG_DIR: &str = ".config";
17|pub const OPENCODE_SUBDIR: &str = "opencode";
18|pub const PLUGIN_SUBDIR: &str = "plugins";
19|pub const OPENCODE_PLUGIN_FILE: &str = "rtk.ts";
20|
21|pub const CURSOR_DIR: &str = ".cursor";
22|pub const CODEX_DIR: &str = ".codex";
23|pub const GEMINI_DIR: &str = ".gemini";
24|25|pub const HERMES_DIR: &str = ".hermes";
26|pub const HERMES_PLUGINS_SUBDIR: &str = "plugins";
27|pub const HERMES_PLUGIN_NAME: &str = "rtk-rewrite";
28|pub const HERMES_PLUGIN_INIT_FILE: &str = "__init__.py";
29|pub const HERMES_PLUGIN_MANIFEST_FILE: &str = "plugin.yaml";
30|pub const KIMI_DIR: &str = ".kimi";
31|pub const KIMI_CONFIG_FILE: &str = "config.toml";
32|43|