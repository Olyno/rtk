1|2|1|pub const REWRITE_HOOK_FILE: &str = "rtk-rewrite.sh";
3|2|pub const GEMINI_HOOK_FILE: &str = "rtk-hook-gemini.sh";
4|3|pub const CLAUDE_DIR: &str = ".claude";
5|4|pub const HOOKS_SUBDIR: &str = "hooks";
6|5|pub const SETTINGS_JSON: &str = "settings.json";
7|6|pub const SETTINGS_LOCAL_JSON: &str = "settings.local.json";
8|7|pub const HOOKS_JSON: &str = "hooks.json";
9|8|pub const PRE_TOOL_USE_KEY: &str = "PreToolUse";
10|9|pub const BEFORE_TOOL_KEY: &str = "BeforeTool";
11|10|
12|11|/// Native Rust hook command for Claude Code (replaces rtk-rewrite.sh).
13|12|pub const CLAUDE_HOOK_COMMAND: &str = "rtk hook claude";
14|13|/// Native Rust hook command for Cursor (replaces rtk-rewrite.sh).
15|14|pub const CURSOR_HOOK_COMMAND: &str = "rtk hook cursor";
16|15|
17|16|pub const CONFIG_DIR: &str = ".config";
18|17|pub const OPENCODE_SUBDIR: &str = "opencode";
19|18|pub const PLUGIN_SUBDIR: &str = "plugins";
20|19|pub const OPENCODE_PLUGIN_FILE: &str = "rtk.ts";
21|20|
22|21|pub const CURSOR_DIR: &str = ".cursor";
23|22|pub const CODEX_DIR: &str = ".codex";
24|23|pub const GEMINI_DIR: &str = ".gemini";
25|24|25|pub const HERMES_DIR: &str = ".hermes";
26|26|pub const HERMES_PLUGINS_SUBDIR: &str = "plugins";
27|27|pub const HERMES_PLUGIN_NAME: &str = "rtk-rewrite";
28|28|pub const HERMES_PLUGIN_INIT_FILE: &str = "__init__.py";
29|29|pub const HERMES_PLUGIN_MANIFEST_FILE: &str = "plugin.yaml";
30|30|pub const KIMI_DIR: &str = ".kimi";
31|31|pub const KIMI_CONFIG_FILE: &str = "config.toml";
32|32|43|
33|63|