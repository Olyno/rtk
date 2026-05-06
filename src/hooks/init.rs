1|//! Sets up RTK hooks so AI coding agents automatically route commands through RTK.
2|
3|use anyhow::{Context, Result};
4|5|use std::ffi::OsString;
6|10|use std::fs;
11|use std::io::Write;
12|use std::path::{Path, PathBuf};
13|use tempfile::NamedTempFile;
14|
15|use crate::hooks::constants::{
16|    CONFIG_DIR, CURSOR_DIR, GEMINI_DIR, OPENCODE_PLUGIN_FILE, OPENCODE_SUBDIR, PLUGIN_SUBDIR,
17|};
18|
19|use super::constants::{
20|    BEFORE_TOOL_KEY, CLAUDE_DIR, CLAUDE_HOOK_COMMAND, CODEX_DIR, CURSOR_HOOK_COMMAND,
21|22|    GEMINI_HOOK_FILE, HERMES_DIR, HERMES_PLUGINS_SUBDIR, HERMES_PLUGIN_INIT_FILE,
23|    HERMES_PLUGIN_MANIFEST_FILE, HERMES_PLUGIN_NAME, HOOKS_JSON, HOOKS_SUBDIR, KIMI_CONFIG_FILE,
24|    KIMI_DIR, PRE_TOOL_USE_KEY, REWRITE_HOOK_FILE, SETTINGS_JSON,
25|29|};
30|use super::integrity;
31|
32|// Embedded OpenCode plugin (auto-rewrite)
33|const OPENCODE_PLUGIN: &str = include_str!("../../hooks/opencode/rtk.ts");
34|
35|// Embedded Pi extension (auto-rewrite)
36|const PI_PLUGIN: &str = include_str!("../../hooks/pi/rtk.ts");
37|
38|// Embedded slim RTK awareness instructions
39|const RTK_SLIM: &str = include_str!("../../hooks/claude/rtk-awareness.md");
40|const RTK_SLIM_CODEX: &str = include_str!("../../hooks/codex/rtk-awareness.md");
41|const RTK_SLIM_PI: &str = include_str!("../../hooks/pi/rtk-awareness.md");
42|
43|// Embedded Kimi CLI hook script
44|const KIMI_HOOK_SCRIPT: &str = include_str!("../../hooks/kimi/rtk-rewrite.sh");
45|
46|/// Template written by `rtk init` when no filters.toml exists yet.
47|const FILTERS_TEMPLATE: &str = r#"# Project-local RTK filters — commit this file with your repo.
48|# Filters here override user-global and built-in filters.
49|# Docs: https://github.com/rtk-ai/rtk#custom-filters
50|schema_version = 1
51|
52|# Example: suppress build noise from a custom tool
53|# [filters.my-tool]
54|# description = "Compact my-tool output"
55|# match_command = "^my-tool\\s+build"
56|# strip_ansi = true
57|# strip_lines_matching = ["^\\s*$", "^Downloading", "^Installing"]
58|# max_lines = 30
59|# on_empty = "my-tool: ok"
60|"#;
61|
62|/// Template for user-global filters (~/.config/rtk/filters.toml).
63|const FILTERS_GLOBAL_TEMPLATE: &str = r#"# User-global RTK filters — apply to all your projects.
64|# Project-local .rtk/filters.toml takes precedence over these.
65|# Docs: https://github.com/rtk-ai/rtk#custom-filters
66|schema_version = 1
67|
68|# Example: suppress noise from a tool you use everywhere
69|# [filters.my-global-tool]
70|# description = "Compact my-global-tool output"
71|# match_command = "^my-global-tool\\b"
72|# strip_ansi = true
73|# strip_lines_matching = ["^\\s*$"]
74|# max_lines = 40
75|"#;
76|
77|const RTK_MD: &str = "RTK.md";
78|const CLAUDE_MD: &str = "CLAUDE.md";
79|const AGENTS_MD: &str = "AGENTS.md";
80|const RTK_MD_REF: &str = "@RTK.md";
81|const GEMINI_MD: &str = "GEMINI.md";
82|
83|const RTK_BLOCK_START: &str = "<!-- rtk-instructions";
84|const RTK_BLOCK_END: &str = "<!-- /rtk-instructions -->";
85|
86|/// Control flow for settings.json patching
87|#[derive(Debug, Clone, Copy, PartialEq)]
88|pub enum PatchMode {
89|    Ask,  // Default: prompt user [y/N]
90|    Auto, // --auto-patch: no prompt
91|    Skip, // --no-patch: manual instructions
92|}
93|
94|/// Result of settings.json patching operation
95|#[derive(Debug, Clone, Copy, PartialEq)]
96|pub enum PatchResult {
97|    Patched,        // Hook was added successfully
98|    AlreadyPresent, // Hook was already in settings.json
99|    Declined,       // User declined when prompted
100|    Skipped,        // --no-patch flag used
101|    WouldPatch,     // Dry-run: hook would have been added
102|}
103|
104|/// Shared context threaded through every init/uninstall function.
105|///
106|/// Replaces ad-hoc `verbose: u8, dry_run: bool` parameter pairs to keep
107|/// signatures compact as more flags are added (mirrors `RunOptions` in
108|/// `src/core/runner.rs`).
109|#[derive(Clone, Copy, Default)]
110|pub struct InitContext {
111|    pub verbose: u8,
112|    pub dry_run: bool,
113|}
114|
115|/// Shared dry-run footer printed at the end of every init sub-mode.
116|fn print_dry_run_footer() {
117|    println!("\n[dry-run] Nothing written.");
118|}
119|
120|// Legacy full instructions for backward compatibility (--claude-md mode)
121|const RTK_INSTRUCTIONS: &str = r##"<!-- rtk-instructions v2 -->
122|# RTK (Rust Token Killer) - Token-Optimized Commands
123|
124|## Golden Rule
125|
126|**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.
127|
128|**Important**: Even in command chains with `&&`, use `rtk`:
129|```bash
130|# ❌ Wrong
131|git add . && git commit -m "msg" && git push
132|
133|# ✅ Correct
134|rtk git add . && rtk git commit -m "msg" && rtk git push
135|```
136|
137|## RTK Commands by Workflow
138|
139|### Build & Compile (80-90% savings)
140|```bash
141|rtk cargo build         # Cargo build output
142|rtk cargo check         # Cargo check output
143|rtk cargo clippy        # Clippy warnings grouped by file (80%)
144|rtk tsc                 # TypeScript errors grouped by file/code (83%)
145|rtk lint                # ESLint/Biome violations grouped (84%)
146|rtk prettier --check    # Files needing format only (70%)
147|rtk next build          # Next.js build with route metrics (87%)
148|```
149|
150|### Test (60-99% savings)
151|```bash
152|rtk cargo test          # Cargo test failures only (90%)
153|rtk go test             # Go test failures only (90%)
154|rtk jest                # Jest failures only (99.5%)
155|rtk vitest              # Vitest failures only (99.5%)
156|rtk playwright test     # Playwright failures only (94%)
157|rtk pytest              # Python test failures only (90%)
158|rtk rake test           # Ruby test failures only (90%)
159|rtk rspec               # RSpec test failures only (60%)
160|rtk test <cmd>          # Generic test wrapper - failures only
161|```
162|
163|### Git (59-80% savings)
164|```bash
165|rtk git status          # Compact status
166|rtk git log             # Compact log (works with all git flags)
167|rtk git diff            # Compact diff (80%)
168|rtk git show            # Compact show (80%)
169|rtk git add             # Ultra-compact confirmations (59%)
170|rtk git commit          # Ultra-compact confirmations (59%)
171|rtk git push            # Ultra-compact confirmations
172|rtk git pull            # Ultra-compact confirmations
173|rtk git branch          # Compact branch list
174|rtk git fetch           # Compact fetch
175|rtk git stash           # Compact stash
176|rtk git worktree        # Compact worktree
177|```
178|
179|Note: Git passthrough works for ALL subcommands, even those not explicitly listed.
180|
181|### GitHub (26-87% savings)
182|```bash
183|rtk gh pr view <num>    # Compact PR view (87%)
184|rtk gh pr checks        # Compact PR checks (79%)
185|rtk gh run list         # Compact workflow runs (82%)
186|rtk gh issue list       # Compact issue list (80%)
187|rtk gh api              # Compact API responses (26%)
188|```
189|
190|### JavaScript/TypeScript Tooling (70-90% savings)
191|```bash
192|rtk pnpm list           # Compact dependency tree (70%)
193|rtk pnpm outdated       # Compact outdated packages (80%)
194|rtk pnpm install        # Compact install output (90%)
195|rtk npm run <script>    # Compact npm script output
196|rtk npx <cmd>           # Compact npx command output
197|rtk prisma              # Prisma without ASCII art (88%)
198|```
199|
200|### Files & Search (60-75% savings)
201|```bash
202|rtk ls <path>           # Tree format, compact (65%)
203|rtk read <file>         # Code reading with filtering (60%)
204|rtk grep <pattern>      # Search grouped by file (75%). Format flags (-c, -l, -L, -o, -Z) run raw.
205|rtk find <pattern>      # Find grouped by directory (70%)
206|```
207|
208|### Analysis & Debug (70-90% savings)
209|```bash
210|rtk err <cmd>           # Filter errors only from any command
211|rtk log <file>          # Deduplicated logs with counts
212|rtk json <file>         # JSON structure without values
213|rtk deps                # Dependency overview
214|rtk env                 # Environment variables compact
215|rtk summary <cmd>       # Smart summary of command output
216|rtk diff                # Ultra-compact diffs
217|```
218|
219|### Infrastructure (85% savings)
220|```bash
221|rtk docker ps           # Compact container list
222|rtk docker images       # Compact image list
223|rtk docker logs <c>     # Deduplicated logs
224|rtk kubectl get         # Compact resource list
225|rtk kubectl logs        # Deduplicated pod logs
226|```
227|
228|### Network (65-70% savings)
229|```bash
230|rtk curl <url>          # Compact HTTP responses (70%)
231|rtk wget <url>          # Compact download output (65%)
232|```
233|
234|### Meta Commands
235|```bash
236|rtk gain                # View token savings statistics
237|rtk gain --history      # View command history with savings
238|rtk discover            # Analyze Claude Code sessions for missed RTK usage
239|rtk proxy <cmd>         # Run command without filtering (for debugging)
240|rtk init                # Add RTK instructions to CLAUDE.md
241|rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
242|```
243|
244|## Token Savings Overview
245|
246|| Category | Commands | Typical Savings |
247||----------|----------|-----------------|
248|| Tests | vitest, playwright, cargo test | 90-99% |
249|| Build | next, tsc, lint, prettier | 70-87% |
250|| Git | status, log, diff, add, commit | 59-80% |
251|| GitHub | gh pr, gh run, gh issue | 26-87% |
252|| Package Managers | pnpm, npm, npx | 70-90% |
253|| Files | ls, read, grep, find | 60-75% |
254|| Infrastructure | docker, kubectl | 85% |
255|| Network | curl, wget | 65-70% |
256|
257|Overall average: **60-90% token reduction** on common development operations.
258|<!-- /rtk-instructions -->
259|"##;
260|
261|/// Main entry point for `rtk init`
262|#[allow(clippy::too_many_arguments)]
263|pub fn run(
264|    global: bool,
265|    install_claude: bool,
266|    install_opencode: bool,
267|    install_cursor: bool,
268|    install_windsurf: bool,
269|    install_cline: bool,
270|    install_kimi: bool,
271|    claude_md: bool,
272|    hook_only: bool,
273|    codex: bool,
274|    patch_mode: PatchMode,
275|    ctx: InitContext,
276|) -> Result<()> {
277|    let InitContext { dry_run, .. } = ctx;
278|    // Validation: Codex mode conflicts
279|    if codex {
280|        if install_opencode {
281|            anyhow::bail!("--codex cannot be combined with --opencode");
282|        }
283|        if claude_md {
284|            anyhow::bail!("--codex cannot be combined with --claude-md");
285|        }
286|        if hook_only {
287|            anyhow::bail!("--codex cannot be combined with --hook-only");
288|        }
289|        if matches!(patch_mode, PatchMode::Auto) {
290|            anyhow::bail!("--codex cannot be combined with --auto-patch");
291|        }
292|        if matches!(patch_mode, PatchMode::Skip) {
293|            anyhow::bail!("--codex cannot be combined with --no-patch");
294|        }
295|        run_codex_mode(global, ctx)?;
296|    } else {
297|        // Validation: Global-only features
298|        if install_opencode && !global {
299|            anyhow::bail!("OpenCode plugin is global-only. Use: rtk init -g --opencode");
300|        }
301|
302|        if install_cursor && !global {
303|            anyhow::bail!("Cursor hooks are global-only. Use: rtk init -g --agent cursor");
304|        }
305|
306|        if install_windsurf && !global {
307|            anyhow::bail!("Windsurf support is global-only. Use: rtk init -g --agent windsurf");
308|        }
309|
310|        if install_windsurf {
311|            run_windsurf_mode(ctx)?;
312|        } else if install_cline {
313|            run_cline_mode(ctx)?;
314|        } else {
315|            // Mode selection (Claude Code / OpenCode)
316|            match (install_claude, install_opencode, claude_md, hook_only) {
317|                (false, true, _, _) => run_opencode_only_mode(ctx)?,
318|                (true, opencode, true, _) => run_claude_md_mode(global, opencode, ctx)?,
319|                (true, opencode, false, true) => {
320|                    run_hook_only_mode(global, patch_mode, opencode, ctx)?
321|                }
322|                (true, opencode, false, false) => {
323|                    run_default_mode(global, patch_mode, opencode, ctx)?
324|                }
325|                (false, false, _, _) => {
326|                    if !install_cursor {
327|                        anyhow::bail!(
328|                            "at least one of install_claude or install_opencode must be true"
329|                        )
330|                    }
331|                }
332|            }
333|
334|            // Cursor hooks (additive, installed alongside Claude Code)
335|            if install_cursor {
336|                install_cursor_hooks(ctx)?;
337|            }
338|
339|            // Kimi hooks (additive, installed alongside Claude Code)
340|            if install_kimi {
341|                install_kimi_hooks(ctx)?;
342|            }
343|        }
344|    }
345|
346|    if !dry_run {
347|        prompt_telemetry_consent()?;
348|    }
349|
350|    if dry_run {
351|        print_dry_run_footer();
352|    } else {
353|        println!();
354|    }
355|
356|    Ok(())
357|}
358|
359|/// Idempotent file write: create or update if content differs.
360|/// When `dry_run` is true, prints the intended action and does not touch the filesystem.
361|fn write_if_changed(path: &Path, content: &str, name: &str, ctx: InitContext) -> Result<bool> {
362|    let InitContext { verbose, dry_run } = ctx;
363|    if path.exists() {
364|        let existing = fs::read_to_string(path)
365|            .with_context(|| format!("Failed to read {}: {}", name, path.display()))?;
366|
367|        if existing == content {
368|            if verbose > 0 {
369|                eprintln!("{} already up to date: {}", name, path.display());
370|            }
371|            Ok(false)
372|        } else {
373|            if dry_run {
374|                println!("[dry-run] would update {}: {}", name, path.display());
375|                if verbose > 0 {
376|                    println!("[dry-run] content:\n{}", content);
377|                }
378|            } else {
379|                atomic_write(path, content)
380|                    .with_context(|| format!("Failed to write {}: {}", name, path.display()))?;
381|                if verbose > 0 {
382|                    eprintln!("Updated {}: {}", name, path.display());
383|                }
384|            }
385|            Ok(true)
386|        }
387|    } else {
388|        if dry_run {
389|            println!("[dry-run] would create {}: {}", name, path.display());
390|            if verbose > 0 {
391|                println!("[dry-run] content:\n{}", content);
392|            }
393|        } else {
394|            atomic_write(path, content)
395|                .with_context(|| format!("Failed to write {}: {}", name, path.display()))?;
396|            if verbose > 0 {
397|                eprintln!("Created {}: {}", name, path.display());
398|            }
399|        }
400|        Ok(true)
401|    }
402|}
403|
404|/// Atomic write using tempfile + rename
405|/// Prevents corruption on crash/interrupt
406|fn atomic_write(path: &Path, content: &str) -> Result<()> {
407|    let parent = path.parent().with_context(|| {
408|        format!(
409|            "Cannot write to {}: path has no parent directory",
410|            path.display()
411|        )
412|    })?;
413|
414|    // Create temp file in same directory (ensures same filesystem for atomic rename)
415|    let mut temp_file = NamedTempFile::new_in(parent)
416|        .with_context(|| format!("Failed to create temp file in {}", parent.display()))?;
417|
418|    // Write content
419|    temp_file
420|        .write_all(content.as_bytes())
421|        .with_context(|| format!("Failed to write {} bytes to temp file", content.len()))?;
422|
423|    // Atomic rename
424|    temp_file.persist(path).with_context(|| {
425|        format!(
426|            "Failed to atomically replace {} (disk full?)",
427|            path.display()
428|        )
429|    })?;
430|
431|    Ok(())
432|}
433|
434|/// Prompt user for consent to patch settings.json
435|/// Prints to stderr (stdout may be piped), reads from stdin
436|/// Default is No (capital N)
437|fn prompt_user_consent(settings_path: &Path) -> Result<bool> {
438|    use std::io::{self, BufRead, IsTerminal};
439|
440|    eprintln!("\nPatch existing {}? [y/N] ", settings_path.display());
441|
442|    // If stdin is not a terminal (piped), default to No
443|    if !io::stdin().is_terminal() {
444|        eprintln!("(non-interactive mode, defaulting to N)");
445|        return Ok(false);
446|    }
447|
448|    let stdin = io::stdin();
449|    let mut line = String::new();
450|    stdin
451|        .lock()
452|        .read_line(&mut line)
453|        .context("Failed to read user input")?;
454|
455|    let response = line.trim().to_lowercase();
456|    Ok(response == "y" || response == "yes")
457|}
458|
459|pub fn save_telemetry_consent(accepted: bool) -> Result<()> {
460|    let mut config = crate::core::config::Config::load().unwrap_or_default();
461|    config.telemetry.consent_given = Some(accepted);
462|    config.telemetry.enabled = accepted;
463|    config.telemetry.consent_date = Some(chrono::Utc::now().to_rfc3339());
464|    config
465|        .save()
466|        .context("Failed to save telemetry consent to config.toml")
467|}
468|
469|fn prompt_telemetry_consent() -> Result<()> {
470|    use std::io::{self, BufRead, IsTerminal};
471|
472|    let config = crate::core::config::Config::load().unwrap_or_default();
473|    match config.telemetry.consent_given {
474|        Some(true) => return Ok(()),
475|        Some(false) => return Ok(()),
476|        None => {}
477|    }
478|
479|    if !io::stdin().is_terminal() {
480|        return Ok(());
481|    }
482|
483|    eprintln!();
484|    eprintln!("--- Telemetry ---");
485|    eprintln!("RTK collects anonymous usage metrics once per day to improve filters.");
486|    eprintln!();
487|    eprintln!("  What:    command names (not arguments), token savings, OS, version");
488|    eprintln!("  Why:     prioritize filter development for the most-used commands");
489|    eprintln!("  Who:     RTK AI Labs, contact@rtk-ai.app");
490|    eprintln!("  Rights:  disable anytime with `rtk telemetry disable`,");
491|    eprintln!("           request erasure with `rtk telemetry forget`");
492|    eprintln!("  Details: https://github.com/rtk-ai/rtk/blob/master/docs/TELEMETRY.md");
493|    eprintln!();
494|    eprint!("Enable anonymous telemetry? [y/N] ");
495|
496|    let stdin = io::stdin();
497|    let mut line = String::new();
498|    stdin
499|        .lock()
500|        .read_line(&mut line)
501|        .context("Failed to read user input")?;
502|
503|    let accepted = {
504|        let response = line.trim().to_lowercase();
505|        response == "y" || response == "yes"
506|    };
507|
508|    save_telemetry_consent(accepted)?;
509|
510|    if accepted {
511|        eprintln!("  Telemetry enabled. Disable anytime: rtk telemetry disable");
512|    } else {
513|        eprintln!("  Telemetry disabled.");
514|    }
515|
516|    Ok(())
517|}
518|
519|fn print_manual_instructions(hook_command: &str, include_opencode: bool) {
520|    println!("\n  MANUAL STEP: Add this to ~/.claude/settings.json:");
521|    println!("  {{");
522|    println!("    \"hooks\": {{ \"PreToolUse\": [{{");
523|    println!("      \"matcher\": \"Bash\",");
524|    println!("      \"hooks\": [{{ \"type\": \"command\",");
525|    println!("        \"command\": \"{}\"", hook_command);
526|    println!("      }}]");
527|    println!("    }}]}}");
528|    println!("  }}");
529|    if include_opencode {
530|        println!("\n  Then restart Claude Code and OpenCode. Test with: git status\n");
531|    } else {
532|        println!("\n  Then restart Claude Code. Test with: git status\n");
533|    }
534|}
535|
536|fn remove_hook_from_json(root: &mut serde_json::Value) -> bool {
537|    let hooks = match root
538|        .get_mut("hooks")
539|        .and_then(|h| h.get_mut(PRE_TOOL_USE_KEY))
540|    {
541|        Some(pre_tool_use) => pre_tool_use,
542|        None => return false,
543|    };
544|
545|    let pre_tool_use_array = match hooks.as_array_mut() {
546|        Some(arr) => arr,
547|        None => return false,
548|    };
549|
550|    let original_len = pre_tool_use_array.len();
551|    pre_tool_use_array.retain(|entry| {
552|        if let Some(hooks_array) = entry.get("hooks").and_then(|h| h.as_array()) {
553|            for hook in hooks_array {
554|                if let Some(command) = hook.get("command").and_then(|c| c.as_str()) {
555|                    // Match both legacy script path and new binary command
556|                    if command.contains(REWRITE_HOOK_FILE) || command == CLAUDE_HOOK_COMMAND {
557|                        return false;
558|                    }
559|                }
560|            }
561|        }
562|        true
563|    });
564|
565|    pre_tool_use_array.len() < original_len
566|}
567|
568|/// Remove RTK hook from settings.json file
569|/// Backs up before modification, returns true if hook was found and removed
570|fn remove_hook_from_settings(ctx: InitContext) -> Result<bool> {
571|    let InitContext { verbose, dry_run } = ctx;
572|    let claude_dir = resolve_claude_dir()?;
573|    let settings_path = claude_dir.join(SETTINGS_JSON);
574|
575|    if !settings_path.exists() {
576|        if verbose > 0 {
577|            eprintln!("settings.json not found, nothing to remove");
578|        }
579|        return Ok(false);
580|    }
581|
582|    let content = fs::read_to_string(&settings_path)
583|        .with_context(|| format!("Failed to read {}", settings_path.display()))?;
584|
585|    if content.trim().is_empty() {
586|        return Ok(false);
587|    }
588|
589|    let mut root: serde_json::Value = serde_json::from_str(&content)
590|        .with_context(|| format!("Failed to parse {} as JSON", settings_path.display()))?;
591|
592|    let removed = remove_hook_from_json(&mut root);
593|
594|    if removed {
595|        if dry_run {
596|            println!(
597|                "[dry-run] would remove RTK hook entry from {}",
598|                settings_path.display()
599|            );
600|            if verbose > 0 {
601|                let serialized = serde_json::to_string_pretty(&root)
602|                    .context("Failed to serialize settings.json")?;
603|                println!("[dry-run] content:\n{}", serialized);
604|            }
605|            return Ok(true);
606|        }
607|
608|        // Backup original
609|        let backup_path = settings_path.with_extension("json.bak");
610|        fs::copy(&settings_path, &backup_path)
611|            .with_context(|| format!("Failed to backup to {}", backup_path.display()))?;
612|
613|        // Atomic write
614|        let serialized =
615|            serde_json::to_string_pretty(&root).context("Failed to serialize settings.json")?;
616|        atomic_write(&settings_path, &serialized)?;
617|
618|        if verbose > 0 {
619|            eprintln!("Removed RTK hook from settings.json");
620|        }
621|    }
622|
623|    Ok(removed)
624|}
625|
626|627|/// Full uninstall for Claude, Gemini, Codex, or Cursor artifacts.
628|631|pub fn uninstall(
632|    global: bool,
633|    gemini: bool,
634|    codex: bool,
635|    cursor: bool,
636|637|    kimi: bool,
638|    ctx: InitContext,
639|) -> Result<()> {
640|    let InitContext { verbose, dry_run } = ctx;
641|646|    if codex {
647|        uninstall_codex(global, ctx)?;
648|        if dry_run {
649|            print_dry_run_footer();
650|        }
651|        return Ok(());
652|    }
653|
654|    if kimi {
655|        if !global {
656|            anyhow::bail!("Kimi uninstall only works with --global flag");
657|        }
658|        uninstall_kimi(ctx)?;
659|        let header = if dry_run {
660|            "[dry-run] would uninstall RTK (Kimi)"
661|        } else {
662|            "RTK uninstalled (Kimi)"
663|        };
664|        println!("{}", header);
665|        if !dry_run {
666|            println!("\nRestart Kimi CLI to apply changes.");
667|        }
668|        if dry_run {
669|            print_dry_run_footer();
670|        }
671|        return Ok(());
672|    }
673|
674|    if cursor {
675|        if !global {
676|            anyhow::bail!("Cursor uninstall only works with --global flag");
677|        }
678|        let cursor_removed = remove_cursor_hooks(ctx).context("Failed to remove Cursor hooks")?;
679|        if !cursor_removed.is_empty() {
680|            let header = if dry_run {
681|                "[dry-run] would uninstall RTK (Cursor):"
682|            } else {
683|                "RTK uninstalled (Cursor):"
684|            };
685|            println!("{}", header);
686|            for item in &cursor_removed {
687|                println!("  - {}", item);
688|            }
689|            if !dry_run {
690|                println!("\nRestart Cursor to apply changes.");
691|            }
692|        } else {
693|            println!("RTK Cursor support was not installed (nothing to remove)");
694|        }
695|        if dry_run {
696|            print_dry_run_footer();
697|        }
698|        return Ok(());
699|    }
700|
701|    if pi {
702|        let plugin_path = pi_plugin_path_for_scope(global)?;
703|        let mut removed = Vec::new();
704|
705|        if plugin_path.exists() {
706|            fs::remove_file(&plugin_path).with_context(|| {
707|                format!("Failed to remove Pi extension: {}", plugin_path.display())
708|            })?;
709|            if verbose > 0 {
710|                eprintln!("Removed Pi extension: {}", plugin_path.display());
711|            }
712|            removed.push(plugin_path);
713|        }
714|
715|        let agents_md = pi_agents_md_path_for_scope(global)?;
716|        remove_pi_awareness(&agents_md, verbose).context("Failed to remove Pi awareness block")?;
717|
718|        if !removed.is_empty() {
719|            println!("RTK uninstalled (Pi):");
720|            for path in &removed {
721|                println!("  - {}", path.display());
722|            }
723|            println!("\nRestart pi to apply changes.");
724|        } else {
725|            println!("RTK Pi extension was not installed (nothing to remove)");
726|        }
727|        return Ok(());
728|    }
729|
730|    if !global {
731|        anyhow::bail!("Uninstall only works with --global flag. For local projects, manually remove RTK from CLAUDE.md");
732|    }
733|
734|    let claude_dir = resolve_claude_dir()?;
735|    let mut removed = Vec::new();
736|
737|    // Also uninstall Gemini artifacts if --gemini or always (clean everything)
738|    if gemini {
739|        let gemini_removed = uninstall_gemini(ctx)?;
740|        removed.extend(gemini_removed);
741|        if !removed.is_empty() {
742|            let header = if dry_run {
743|                "[dry-run] would uninstall RTK (Gemini):"
744|            } else {
745|                "RTK uninstalled (Gemini):"
746|            };
747|            println!("{}", header);
748|            for item in &removed {
749|                println!("  - {}", item);
750|            }
751|            if !dry_run {
752|                println!("\nRestart Gemini CLI to apply changes.");
753|            }
754|        } else {
755|            println!("RTK Gemini support was not installed (nothing to remove)");
756|        }
757|        if dry_run {
758|            print_dry_run_footer();
759|        }
760|        return Ok(());
761|    }
762|
763|    // 1. Remove legacy hook file (if exists from old installation)
764|    let hook_path = claude_dir.join(HOOKS_SUBDIR).join(REWRITE_HOOK_FILE);
765|    if hook_path.exists() {
766|        if dry_run {
767|            println!(
768|                "[dry-run] would remove hook script: {}",
769|                hook_path.display()
770|            );
771|        } else {
772|            fs::remove_file(&hook_path)
773|                .with_context(|| format!("Failed to remove hook: {}", hook_path.display()))?;
774|        }
775|        removed.push(format!("Hook script: {}", hook_path.display()));
776|    }
777|
778|    // 1b. Remove integrity hash file
779|    if dry_run {
780|        // integrity::remove_hash would delete the sidecar file; just report intent.
781|        if integrity::hash_path_for(&hook_path).exists() {
782|            println!("[dry-run] would remove integrity hash sidecar");
783|            removed.push("Integrity hash: removed".to_string());
784|        }
785|    } else if integrity::remove_hash(&hook_path)? {
786|        removed.push("Integrity hash: removed".to_string());
787|    }
788|
789|    // 2. Remove RTK.md
790|    let rtk_md_path = claude_dir.join(RTK_MD);
791|    if rtk_md_path.exists() {
792|        if dry_run {
793|            println!("[dry-run] would remove RTK.md: {}", rtk_md_path.display());
794|        } else {
795|            fs::remove_file(&rtk_md_path)
796|                .with_context(|| format!("Failed to remove RTK.md: {}", rtk_md_path.display()))?;
797|        }
798|        removed.push(format!("RTK.md: {}", rtk_md_path.display()));
799|    }
800|
801|    // 3. Remove @RTK.md reference from CLAUDE.md
802|    let claude_md_path = claude_dir.join(CLAUDE_MD);
803|    if claude_md_path.exists() {
804|        let content = fs::read_to_string(&claude_md_path)
805|            .with_context(|| format!("Failed to read CLAUDE.md: {}", claude_md_path.display()))?;
806|
807|        let mut claude_md_changed = false;
808|        let mut working_content = content.clone();
809|
810|        if working_content.contains(RTK_MD_REF) {
811|            let new_content = working_content
812|                .lines()
813|                .filter(|line| !line.trim().starts_with(RTK_MD_REF))
814|                .collect::<Vec<_>>()
815|                .join("\n");
816|
817|            working_content = clean_double_blanks(&new_content);
818|            claude_md_changed = true;
819|            removed.push("CLAUDE.md: removed @RTK.md reference".to_string());
820|        }
821|
822|        if working_content.contains(RTK_BLOCK_START) {
823|            let (cleaned, did_remove) = remove_rtk_block(&working_content);
824|            if did_remove {
825|                working_content = cleaned;
826|                claude_md_changed = true;
827|                removed.push("CLAUDE.md: removed rtk-instructions block".to_string());
828|            }
829|        }
830|
831|        if claude_md_changed {
832|            let trimmed = working_content.trim();
833|            if trimmed.is_empty() {
834|                if dry_run {
835|                    println!(
836|                        "[dry-run] would remove CLAUDE.md (empty after cleanup): {}",
837|                        claude_md_path.display()
838|                    );
839|                } else {
840|                    // nosemgrep: filesystem-deletion
841|                    fs::remove_file(&claude_md_path).with_context(|| {
842|                        format!(
843|                            "Failed to remove empty CLAUDE.md: {}",
844|                            claude_md_path.display()
845|                        )
846|                    })?;
847|                }
848|                removed.retain(|r| !r.starts_with("CLAUDE.md:"));
849|                removed.push("CLAUDE.md: removed (was empty after cleanup)".to_string());
850|            } else if dry_run {
851|                println!(
852|                    "[dry-run] would update CLAUDE.md: {}",
853|                    claude_md_path.display()
854|                );
855|                if verbose > 0 {
856|                    println!("[dry-run] content:\n{}", working_content);
857|                }
858|            } else {
859|                fs::write(&claude_md_path, &working_content).with_context(|| {
860|                    format!("Failed to write CLAUDE.md: {}", claude_md_path.display())
861|                })?;
862|            }
863|        }
864|    }
865|
866|    // 4. Remove hook entry from settings.json
867|    if remove_hook_from_settings(ctx)? {
868|        removed.push("settings.json: removed RTK hook entry".to_string());
869|    }
870|
871|    // 5. Remove OpenCode plugin
872|    let opencode_removed = remove_opencode_plugin(ctx)?;
873|    for path in opencode_removed {
874|        removed.push(format!("OpenCode plugin: {}", path.display()));
875|    }
876|
877|    // 6. Remove Cursor hooks
878|    let cursor_removed = remove_cursor_hooks(ctx)?;
879|    removed.extend(cursor_removed);
880|
881|    // Report results
882|    if removed.is_empty() {
883|        println!("RTK was not installed (nothing to remove)");
884|        println!("  Checked: {}", hook_path.display());
885|        println!("  Checked: {}", claude_dir.join(RTK_MD).display());
886|        println!("  Checked: {}", claude_md_path.display());
887|        println!("  Checked: {}", claude_dir.join(SETTINGS_JSON).display());
888|    } else {
889|        let header = if dry_run {
890|            "[dry-run] would uninstall RTK:"
891|        } else {
892|            "RTK uninstalled:"
893|        };
894|        println!("{}", header);
895|        for item in removed {
896|            println!("  - {}", item);
897|        }
898|        if !dry_run {
899|            println!("\nRestart Claude Code, OpenCode, and Cursor (if used) to apply changes.");
900|        }
901|    }
902|
903|    if dry_run {
904|        print_dry_run_footer();
905|    }
906|
907|    Ok(())
908|}
909|
910|fn uninstall_codex(global: bool, ctx: InitContext) -> Result<()> {
911|    let InitContext { dry_run, .. } = ctx;
912|    if !global {
913|        anyhow::bail!(
914|            "Uninstall only works with --global flag. For local projects, manually remove RTK from AGENTS.md"
915|        );
916|    }
917|
918|    let codex_dir = resolve_codex_dir()?;
919|    let removed = uninstall_codex_at(&codex_dir, ctx)?;
920|
921|    if removed.is_empty() {
922|        println!("RTK was not installed for Codex CLI (nothing to remove)");
923|    } else {
924|        let header = if dry_run {
925|            "[dry-run] would uninstall RTK for Codex CLI:"
926|        } else {
927|            "RTK uninstalled for Codex CLI:"
928|        };
929|        println!("{}", header);
930|        for item in removed {
931|            println!("  - {}", item);
932|        }
933|    }
934|
935|    Ok(())
936|}
937|
938|fn uninstall_codex_at(codex_dir: &Path, ctx: InitContext) -> Result<Vec<String>> {
939|    let InitContext { verbose, dry_run } = ctx;
940|    let mut removed = Vec::new();
941|    let absolute_rtk_md_ref = codex_rtk_md_ref(codex_dir);
942|
943|    let rtk_md_path = codex_dir.join(RTK_MD);
944|    if rtk_md_path.exists() {
945|        if dry_run {
946|            println!("[dry-run] would remove RTK.md: {}", rtk_md_path.display());
947|        } else {
948|            fs::remove_file(&rtk_md_path)
949|                .with_context(|| format!("Failed to remove RTK.md: {}", rtk_md_path.display()))?;
950|            if verbose > 0 {
951|                eprintln!("Removed RTK.md: {}", rtk_md_path.display());
952|            }
953|        }
954|        removed.push(format!("RTK.md: {}", rtk_md_path.display()));
955|    }
956|
957|    let agents_md_path = codex_dir.join(AGENTS_MD);
958|    if agents_md_path.exists() {
959|        let content = fs::read_to_string(&agents_md_path)
960|            .with_context(|| format!("Failed to read AGENTS.md: {}", agents_md_path.display()))?;
961|
962|        let mut working_content = content.clone();
963|        let mut agents_changed = false;
964|
965|        if working_content.contains(RTK_BLOCK_START) {
966|            let (cleaned, did_remove) = remove_rtk_block(&working_content);
967|            if did_remove {
968|                working_content = cleaned;
969|                agents_changed = true;
970|                removed.push("AGENTS.md: removed rtk-instructions block".to_string());
971|            }
972|        }
973|
974|        if agents_changed {
975|            atomic_write(&agents_md_path, &working_content).with_context(|| {
976|                format!("Failed to write AGENTS.md: {}", agents_md_path.display())
977|            })?;
978|        }
979|    }
980|
981|    if remove_rtk_reference_from_agents(
982|        &agents_md_path,
983|        &[RTK_MD_REF, absolute_rtk_md_ref.as_str()],
984|        ctx,
985|    )? {
986|        removed.push("AGENTS.md: removed @RTK.md reference".to_string());
987|    }
988|
989|    Ok(removed)
990|}
991|
992|/// Orchestrator: patch settings.json with RTK hook (binary command variant)
993|/// Handles reading, checking, prompting, merging, backing up, and atomic writing
994|fn patch_settings_json_command(
995|    hook_command: &str,
996|    mode: PatchMode,
997|    include_opencode: bool,
998|    ctx: InitContext,
999|) -> Result<PatchResult> {
1000|    let InitContext { verbose, dry_run } = ctx;
1001|    let claude_dir = resolve_claude_dir()?;
1002|    let settings_path = claude_dir.join(SETTINGS_JSON);
1003|
1004|    // Read or create settings.json
1005|    let mut root = if settings_path.exists() {
1006|        let content = fs::read_to_string(&settings_path)
1007|            .with_context(|| format!("Failed to read {}", settings_path.display()))?;
1008|
1009|        if content.trim().is_empty() {
1010|            serde_json::json!({})
1011|        } else {
1012|            serde_json::from_str(&content)
1013|                .with_context(|| format!("Failed to parse {} as JSON", settings_path.display()))?
1014|        }
1015|    } else {
1016|        serde_json::json!({})
1017|    };
1018|
1019|    // Check idempotency
1020|    if hook_already_present(&root, hook_command) {
1021|        if verbose > 0 {
1022|            eprintln!("settings.json: hook already present");
1023|        }
1024|        return Ok(PatchResult::AlreadyPresent);
1025|    }
1026|
1027|    // Handle mode
1028|    match mode {
1029|        PatchMode::Skip => {
1030|            print_manual_instructions(hook_command, include_opencode);
1031|            return Ok(PatchResult::Skipped);
1032|        }
1033|        PatchMode::Ask => {
1034|            // Skip the interactive prompt in dry-run: we must not mutate state or block on stdin.
1035|            if dry_run {
1036|                println!(
1037|                    "[dry-run] would prompt before patching {}",
1038|                    settings_path.display()
1039|                );
1040|            } else if !prompt_user_consent(&settings_path)? {
1041|                print_manual_instructions(hook_command, include_opencode);
1042|                return Ok(PatchResult::Declined);
1043|            }
1044|        }
1045|        PatchMode::Auto => {
1046|            // Proceed without prompting
1047|        }
1048|    }
1049|
1050|    insert_hook_entry(&mut root, hook_command)?;
1051|
1052|    let serialized =
1053|        serde_json::to_string_pretty(&root).context("Failed to serialize settings.json")?;
1054|
1055|    if dry_run {
1056|        println!(
1057|            "[dry-run] would patch settings.json: {}",
1058|            settings_path.display()
1059|        );
1060|        if verbose > 0 {
1061|            println!("[dry-run] content:\n{}", serialized);
1062|        }
1063|        return Ok(PatchResult::WouldPatch);
1064|    }
1065|
1066|    // Backup original
1067|    if settings_path.exists() {
1068|        let backup_path = settings_path.with_extension("json.bak");
1069|        fs::copy(&settings_path, &backup_path)
1070|            .with_context(|| format!("Failed to backup to {}", backup_path.display()))?;
1071|        if verbose > 0 {
1072|            eprintln!("Backup: {}", backup_path.display());
1073|        }
1074|    }
1075|
1076|    // Atomic write
1077|    atomic_write(&settings_path, &serialized)?;
1078|
1079|    println!("\n  settings.json: hook added");
1080|    if settings_path.with_extension("json.bak").exists() {
1081|        println!(
1082|            "  Backup: {}",
1083|            settings_path.with_extension("json.bak").display()
1084|        );
1085|    }
1086|    if include_opencode {
1087|        println!("  Restart Claude Code and OpenCode. Test with: git status");
1088|    } else {
1089|        println!("  Restart Claude Code. Test with: git status");
1090|    }
1091|
1092|    Ok(PatchResult::Patched)
1093|}
1094|
1095|/// Clean up consecutive blank lines (collapse 3+ to 2)
1096|/// Used when removing @RTK.md line from CLAUDE.md
1097|fn clean_double_blanks(content: &str) -> String {
1098|    let lines: Vec<&str> = content.lines().collect();
1099|    let mut result = Vec::new();
1100|    let mut i = 0;
1101|
1102|    while i < lines.len() {
1103|        let line = lines[i];
1104|
1105|        if line.trim().is_empty() {
1106|            // Count consecutive blank lines
1107|            let mut blank_count = 0;
1108|            while i < lines.len() && lines[i].trim().is_empty() {
1109|                blank_count += 1;
1110|                i += 1;
1111|            }
1112|
1113|            // Keep at most 2 blank lines
1114|            let keep = blank_count.min(2);
1115|            result.extend(std::iter::repeat_n("", keep));
1116|        } else {
1117|            result.push(line);
1118|            i += 1;
1119|        }
1120|    }
1121|
1122|    result.join("\n")
1123|}
1124|
1125|/// Deep-merge RTK hook entry into settings.json
1126|/// Creates hooks.PreToolUse structure if missing, preserves existing hooks
1127|fn insert_hook_entry(root: &mut serde_json::Value, hook_command: &str) -> Result<()> {
1128|    let root_obj = match root.as_object_mut() {
1129|        Some(obj) => obj,
1130|        None => {
1131|            *root = serde_json::json!({});
1132|            root.as_object_mut().expect("just-created json object")
1133|        }
1134|    };
1135|
1136|    let hooks = root_obj
1137|        .entry("hooks")
1138|        .or_insert_with(|| serde_json::json!({}))
1139|        .as_object_mut()
1140|        .context("hooks value is not an object")?;
1141|
1142|    let pre_tool_use = hooks
1143|        .entry(PRE_TOOL_USE_KEY)
1144|        .or_insert_with(|| serde_json::json!([]))
1145|        .as_array_mut()
1146|        .context("PreToolUse value is not an array")?;
1147|
1148|    pre_tool_use.push(serde_json::json!({
1149|        "matcher": "Bash",
1150|        "hooks": [{
1151|            "type": "command",
1152|            "command": hook_command
1153|        }]
1154|    }));
1155|    Ok(())
1156|}
1157|
1158|/// Check if RTK hook is already present in settings.json
1159|/// Matches on legacy rtk-rewrite.sh path OR new `rtk hook claude` command
1160|fn hook_already_present(root: &serde_json::Value, hook_command: &str) -> bool {
1161|    let pre_tool_use_array = match root
1162|        .get("hooks")
1163|        .and_then(|h| h.get(PRE_TOOL_USE_KEY))
1164|        .and_then(|p| p.as_array())
1165|    {
1166|        Some(arr) => arr,
1167|        None => return false,
1168|    };
1169|
1170|    pre_tool_use_array
1171|        .iter()
1172|        .filter_map(|entry| entry.get("hooks")?.as_array())
1173|        .flatten()
1174|        .filter_map(|hook| hook.get("command")?.as_str())
1175|        .any(|cmd| {
1176|            cmd == hook_command || cmd == CLAUDE_HOOK_COMMAND || cmd.contains(REWRITE_HOOK_FILE)
1177|        })
1178|}
1179|
1180|/// Default mode: hook + slim RTK.md + @RTK.md reference
1181|fn run_default_mode(
1182|    global: bool,
1183|    patch_mode: PatchMode,
1184|    install_opencode: bool,
1185|    ctx: InitContext,
1186|) -> Result<()> {
1187|    let InitContext { dry_run, .. } = ctx;
1188|    if !global {
1189|        // Local init: inject CLAUDE.md + generate project-local filters template
1190|        run_claude_md_mode(false, install_opencode, ctx)?;
1191|        generate_project_filters_template(ctx)?;
1192|        return Ok(());
1193|    }
1194|
1195|    let claude_dir = resolve_claude_dir()?;
1196|    let rtk_md_path = claude_dir.join(RTK_MD);
1197|    let claude_md_path = claude_dir.join(CLAUDE_MD);
1198|
1199|    // 1. Migrate old hook script if present
1200|    migrate_old_hook_script(ctx);
1201|
1202|    // 2. Write RTK.md
1203|    write_if_changed(&rtk_md_path, RTK_SLIM, RTK_MD, ctx)?;
1204|
1205|    let opencode_plugin_path = if install_opencode {
1206|        let path = prepare_opencode_plugin_path()?;
1207|        ensure_opencode_plugin_installed(&path, ctx)?;
1208|        Some(path)
1209|    } else {
1210|        None
1211|    };
1212|
1213|    // 3. Patch CLAUDE.md (add @RTK.md, migrate if needed)
1214|    let migrated = patch_claude_md(&claude_md_path, ctx)?;
1215|
1216|    // 4. Print success message (skip in dry-run)
1217|    if !dry_run {
1218|        println!("\nRTK hook registered (global).\n");
1219|        println!("  Command:   {}", CLAUDE_HOOK_COMMAND);
1220|        println!("  RTK.md:    {} (10 lines)", rtk_md_path.display());
1221|        if let Some(path) = &opencode_plugin_path {
1222|            println!("  OpenCode:  {}", path.display());
1223|        }
1224|        println!("  CLAUDE.md: @RTK.md reference added");
1225|
1226|        if migrated {
1227|            println!("\n  [ok] Migrated: removed 137-line RTK block from CLAUDE.md");
1228|            println!("              replaced with @RTK.md (10 lines)");
1229|        }
1230|    }
1231|
1232|    // 5. Patch settings.json with binary command
1233|    let patch_result =
1234|        patch_settings_json_command(CLAUDE_HOOK_COMMAND, patch_mode, install_opencode, ctx)?;
1235|
1236|    // Report result
1237|    if !dry_run {
1238|        match patch_result {
1239|            PatchResult::Patched => {
1240|                // Already printed by patch_settings_json_command
1241|            }
1242|            PatchResult::AlreadyPresent => {
1243|                println!("\n  settings.json: hook already present");
1244|                if install_opencode {
1245|                    println!("  Restart Claude Code and OpenCode. Test with: git status");
1246|                } else {
1247|                    println!("  Restart Claude Code. Test with: git status");
1248|                }
1249|            }
1250|            PatchResult::Declined | PatchResult::Skipped => {
1251|                // Manual instructions already printed
1252|            }
1253|            PatchResult::WouldPatch => {
1254|                // Cannot happen outside dry_run
1255|            }
1256|        }
1257|    }
1258|
1259|    // 6. Generate user-global filters template (~/.config/rtk/filters.toml)
1260|    generate_global_filters_template(ctx)?;
1261|
1262|    if !dry_run {
1263|        println!(); // Final newline
1264|    }
1265|
1266|    Ok(())
1267|}
1268|
1269|/// Migrate old hook script to new binary command.
1270|/// Deletes `~/.claude/hooks/rtk-rewrite.sh` and `.rtk-hook.sha256` if present,
1271|/// and removes the stale settings.json entry so the new `rtk hook claude` entry
1272|/// can be registered.
1273|fn migrate_old_hook_script(ctx: InitContext) {
1274|    let InitContext { verbose, dry_run } = ctx;
1275|    if let Some(home) = dirs::home_dir() {
1276|        let old_hook = home
1277|            .join(CLAUDE_DIR)
1278|            .join(HOOKS_SUBDIR)
1279|            .join(REWRITE_HOOK_FILE);
1280|        if old_hook.exists() {
1281|            if dry_run {
1282|                println!(
1283|                    "[dry-run] would migrate legacy hook script: {}",
1284|                    old_hook.display()
1285|                );
1286|            // nosemgrep: filesystem-deletion
1287|            } else if let Err(e) = std::fs::remove_file(&old_hook) {
1288|                if verbose > 0 {
1289|                    eprintln!("  [warn] Failed to remove old hook script: {e}");
1290|                }
1291|            } else {
1292|                if verbose > 0 {
1293|                    eprintln!("  [ok] Removed old hook script: {}", old_hook.display());
1294|                }
1295|                // Clean up the stale settings.json entry that pointed to the deleted script
1296|                if let Err(e) = remove_legacy_settings_entries(ctx) {
1297|                    if verbose > 0 {
1298|                        eprintln!("  [warn] Failed to clean legacy settings.json entry: {e}");
1299|                    }
1300|                }
1301|            }
1302|        }
1303|        // Remove legacy hash file
1304|        let hash_file = home
1305|            .join(CLAUDE_DIR)
1306|            .join(HOOKS_SUBDIR)
1307|            .join(".rtk-hook.sha256");
1308|        if hash_file.exists() {
1309|            if dry_run {
1310|                println!(
1311|                    "[dry-run] would remove legacy hash file: {}",
1312|                    hash_file.display()
1313|                );
1314|            } else {
1315|                let _ = std::fs::remove_file(&hash_file);
1316|            }
1317|        }
1318|        // Remove Cursor legacy hook
1319|        let cursor_hook = home.join(CURSOR_DIR).join("hooks").join(REWRITE_HOOK_FILE);
1320|        if cursor_hook.exists() {
1321|            if dry_run {
1322|                println!(
1323|                    "[dry-run] would remove legacy Cursor hook: {}",
1324|                    cursor_hook.display()
1325|                );
1326|            } else {
1327|                let _ = std::fs::remove_file(&cursor_hook);
1328|            }
1329|        }
1330|    }
1331|}
1332|
1333|/// Remove only legacy `rtk-rewrite.sh` entries from settings.json.
1334|/// Preserves any existing `rtk hook claude` entries (new format).
1335|fn remove_legacy_settings_entries(ctx: InitContext) -> Result<()> {
1336|    let InitContext { verbose, dry_run } = ctx;
1337|    let claude_dir = resolve_claude_dir()?;
1338|    let settings_path = claude_dir.join(SETTINGS_JSON);
1339|
1340|    if !settings_path.exists() {
1341|        return Ok(());
1342|    }
1343|
1344|    let content = fs::read_to_string(&settings_path)
1345|        .with_context(|| format!("Failed to read {}", settings_path.display()))?;
1346|    if content.trim().is_empty() {
1347|        return Ok(());
1348|    }
1349|
1350|    let mut root: serde_json::Value = serde_json::from_str(&content)
1351|        .with_context(|| format!("Failed to parse {}", settings_path.display()))?;
1352|
1353|    if !remove_legacy_hook_entries_from_json(&mut root) {
1354|        return Ok(());
1355|    }
1356|
1357|    if dry_run {
1358|        println!(
1359|            "[dry-run] would remove legacy rtk-rewrite.sh entry from {}",
1360|            settings_path.display()
1361|        );
1362|        return Ok(());
1363|    }
1364|
1365|    // Backup before modifying
1366|    let backup_path = settings_path.with_extension("json.bak");
1367|    fs::copy(&settings_path, &backup_path)
1368|        .with_context(|| format!("Failed to backup to {}", backup_path.display()))?;
1369|
1370|    let serialized =
1371|        serde_json::to_string_pretty(&root).context("Failed to serialize settings.json")?;
1372|    atomic_write(&settings_path, &serialized)?;
1373|
1374|    if verbose > 0 {
1375|        eprintln!("  [ok] Removed legacy rtk-rewrite.sh entry from settings.json");
1376|    }
1377|    Ok(())
1378|}
1379|
1380|/// Remove only legacy `rtk-rewrite.sh` hook entries from a parsed settings.json.
1381|/// Returns true if any entries were removed.
1382|/// Does NOT remove `rtk hook claude` entries — those are the new format.
1383|fn remove_legacy_hook_entries_from_json(root: &mut serde_json::Value) -> bool {
1384|    let pre_tool_use_array = match root
1385|        .get_mut("hooks")
1386|        .and_then(|h| h.get_mut(PRE_TOOL_USE_KEY))
1387|        .and_then(|p| p.as_array_mut())
1388|    {
1389|        Some(arr) => arr,
1390|        None => return false,
1391|    };
1392|
1393|    let original_len = pre_tool_use_array.len();
1394|    pre_tool_use_array.retain(|entry| {
1395|        let dominated_by_legacy = entry
1396|            .get("hooks")
1397|            .and_then(|h| h.as_array())
1398|            .map(|hooks| {
1399|                hooks.iter().all(|hook| {
1400|                    hook.get("command")
1401|                        .and_then(|c| c.as_str())
1402|                        .is_some_and(|cmd| cmd.contains(REWRITE_HOOK_FILE))
1403|                })
1404|            })
1405|            .unwrap_or(false);
1406|        !dominated_by_legacy
1407|    });
1408|
1409|    pre_tool_use_array.len() < original_len
1410|}
1411|
1412|/// Generate .rtk/filters.toml template in the current directory if not present.
1413|fn generate_project_filters_template(ctx: InitContext) -> Result<()> {
1414|    let InitContext { verbose, dry_run } = ctx;
1415|    let rtk_dir = std::path::Path::new(".rtk");
1416|    let path = rtk_dir.join("filters.toml");
1417|
1418|    if path.exists() {
1419|        if verbose > 0 {
1420|            eprintln!(".rtk/filters.toml already exists, skipping template");
1421|        }
1422|        return Ok(());
1423|    }
1424|
1425|    if dry_run {
1426|        println!(
1427|            "[dry-run] would create .rtk/filters.toml template: {}",
1428|            path.display()
1429|        );
1430|        return Ok(());
1431|    }
1432|
1433|    fs::create_dir_all(rtk_dir)
1434|        .with_context(|| format!("Failed to create directory: {}", rtk_dir.display()))?;
1435|    fs::write(&path, FILTERS_TEMPLATE)
1436|        .with_context(|| format!("Failed to write {}", path.display()))?;
1437|
1438|    println!(
1439|        "  filters:   {} (template, edit to add project filters)",
1440|        path.display()
1441|    );
1442|    Ok(())
1443|}
1444|
1445|/// Generate ~/.config/rtk/filters.toml template if not present.
1446|fn generate_global_filters_template(ctx: InitContext) -> Result<()> {
1447|    let InitContext { verbose, dry_run } = ctx;
1448|    let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".config"));
1449|    let rtk_dir = config_dir.join(crate::core::constants::RTK_DATA_DIR);
1450|    let path = rtk_dir.join("filters.toml");
1451|
1452|    if path.exists() {
1453|        if verbose > 0 {
1454|            eprintln!("{} already exists, skipping template", path.display());
1455|        }
1456|        return Ok(());
1457|    }
1458|
1459|    if dry_run {
1460|        println!(
1461|            "[dry-run] would create global filters template: {}",
1462|            path.display()
1463|        );
1464|        return Ok(());
1465|    }
1466|
1467|    fs::create_dir_all(&rtk_dir)
1468|        .with_context(|| format!("Failed to create directory: {}", rtk_dir.display()))?;
1469|    fs::write(&path, FILTERS_GLOBAL_TEMPLATE)
1470|        .with_context(|| format!("Failed to write {}", path.display()))?;
1471|
1472|    println!(
1473|        "  filters:   {} (template, edit to add user-global filters)",
1474|        path.display()
1475|    );
1476|    Ok(())
1477|}
1478|
1479|/// Hook-only mode: just the hook, no RTK.md
1480|fn run_hook_only_mode(
1481|    global: bool,
1482|    patch_mode: PatchMode,
1483|    install_opencode: bool,
1484|    ctx: InitContext,
1485|) -> Result<()> {
1486|    let InitContext { dry_run, .. } = ctx;
1487|    if !global {
1488|        eprintln!("[warn] Warning: --hook-only only makes sense with --global");
1489|        eprintln!("    For local projects, use default mode or --claude-md");
1490|        return Ok(());
1491|    }
1492|
1493|    // Migrate old hook script if present
1494|    migrate_old_hook_script(ctx);
1495|
1496|    let opencode_plugin_path = if install_opencode {
1497|        let path = prepare_opencode_plugin_path()?;
1498|        ensure_opencode_plugin_installed(&path, ctx)?;
1499|        Some(path)
1500|    } else {
1501|        None
1502|    };
1503|
1504|    if !dry_run {
1505|        println!("\nRTK hook registered (hook-only mode).\n");
1506|        println!("  Command: {}", CLAUDE_HOOK_COMMAND);
1507|        if let Some(path) = &opencode_plugin_path {
1508|            println!("  OpenCode: {}", path.display());
1509|        }
1510|        println!(
1511|            "  Note: No RTK.md created. Claude won't know about meta commands (gain, discover, proxy)."
1512|        );
1513|    }
1514|
1515|    // Patch settings.json with binary command
1516|    let patch_result =
1517|        patch_settings_json_command(CLAUDE_HOOK_COMMAND, patch_mode, install_opencode, ctx)?;
1518|
1519|    // Report result
1520|    if !dry_run {
1521|        match patch_result {
1522|            PatchResult::Patched => {
1523|                // Already printed by patch_settings_json_command
1524|            }
1525|            PatchResult::AlreadyPresent => {
1526|                println!("\n  settings.json: hook already present");
1527|                if install_opencode {
1528|                    println!("  Restart Claude Code and OpenCode. Test with: git status");
1529|                } else {
1530|                    println!("  Restart Claude Code. Test with: git status");
1531|                }
1532|            }
1533|            PatchResult::Declined | PatchResult::Skipped => {
1534|                // Manual instructions already printed
1535|            }
1536|            PatchResult::WouldPatch => {
1537|                // Cannot happen outside dry_run
1538|            }
1539|        }
1540|    }
1541|
1542|    if !dry_run {
1543|        println!(); // Final newline
1544|    }
1545|
1546|    Ok(())
1547|}
1548|
1549|/// Legacy mode: full 137-line injection into CLAUDE.md
1550|fn run_claude_md_mode(global: bool, install_opencode: bool, ctx: InitContext) -> Result<()> {
1551|    let InitContext { verbose, dry_run } = ctx;
1552|    let path = if global {
1553|        resolve_claude_dir()?.join(CLAUDE_MD)
1554|    } else {
1555|        PathBuf::from(CLAUDE_MD)
1556|    };
1557|
1558|    if global && !dry_run {
1559|        if let Some(parent) = path.parent() {
1560|            fs::create_dir_all(parent)?;
1561|        }
1562|    }
1563|
1564|    if verbose > 0 {
1565|        eprintln!("Writing rtk instructions to: {}", path.display());
1566|    }
1567|
1568|    let recovery_cmd = if global {
1569|        "rtk init -g --claude-md"
1570|    } else {
1571|        "rtk init --claude-md"
1572|    };
1573|
1574|    let action = write_rtk_block(
1575|        &path,
1576|        RTK_INSTRUCTIONS,
1577|        "rtk instructions",
1578|        recovery_cmd,
1579|        ctx,
1580|    )?;
1581|
1582|    if matches!(action, RtkBlockUpsert::Unchanged) {
1583|        return Ok(());
1584|    }
1585|
1586|    if global {
1587|        if install_opencode {
1588|            let opencode_plugin_path = prepare_opencode_plugin_path()?;
1589|            ensure_opencode_plugin_installed(&opencode_plugin_path, ctx)?;
1590|            if !dry_run {
1591|                println!(
1592|                    "[ok] OpenCode plugin installed: {}",
1593|                    opencode_plugin_path.display()
1594|                );
1595|            }
1596|        }
1597|        if !dry_run {
1598|            println!("   Claude Code will now use rtk in all sessions");
1599|        }
1600|    } else if !dry_run {
1601|        println!("   Claude Code will use rtk in this project");
1602|    }
1603|
1604|    Ok(())
1605|}
1606|
1607|// ─── Windsurf support ─────────────────────────────────────────
1608|
1609|/// Embedded Windsurf RTK rules
1610|const WINDSURF_RULES: &str = include_str!("../../hooks/windsurf/rules.md");
1611|
1612|/// Embedded Cline RTK rules
1613|const CLINE_RULES: &str = include_str!("../../hooks/cline/rules.md");
1614|
1615|// ─── Cline / Roo Code support ─────────────────────────────────
1616|
1617|fn run_cline_mode(ctx: InitContext) -> Result<()> {
1618|    let InitContext { verbose, dry_run } = ctx;
1619|    // Cline reads .clinerules from the project root (workspace-scoped)
1620|    let rules_path = PathBuf::from(".clinerules");
1621|
1622|    let existing = fs::read_to_string(&rules_path).unwrap_or_default();
1623|    if existing.contains("RTK") || existing.contains("rtk") {
1624|        if !dry_run {
1625|            println!("\nRTK already configured for Cline in this project.\n");
1626|            println!("  Rules: .clinerules (already present)");
1627|        }
1628|    } else {
1629|        let new_content = if existing.trim().is_empty() {
1630|            CLINE_RULES.to_string()
1631|        } else {
1632|            format!("{}\n\n{}", existing.trim(), CLINE_RULES)
1633|        };
1634|        if dry_run {
1635|            println!(
1636|                "[dry-run] would write .clinerules: {}",
1637|                rules_path.display()
1638|            );
1639|            if verbose > 0 {
1640|                println!("[dry-run] content:\n{}", new_content);
1641|            }
1642|        } else {
1643|            fs::write(&rules_path, &new_content).context("Failed to write .clinerules")?;
1644|
1645|            if verbose > 0 {
1646|                eprintln!("Wrote .clinerules");
1647|            }
1648|
1649|            println!("\nRTK configured for Cline.\n");
1650|            println!("  Rules: .clinerules (installed)");
1651|        }
1652|    }
1653|    if !dry_run {
1654|        println!("  Cline will now use rtk commands for token savings.");
1655|        println!("  Test with: git status\n");
1656|    }
1657|
1658|    Ok(())
1659|}
1660|
1661|fn run_windsurf_mode(ctx: InitContext) -> Result<()> {
1662|    let InitContext { verbose, dry_run } = ctx;
1663|    // Windsurf reads .windsurfrules from the project root (workspace-scoped).
1664|    // Global rules (~/.codeium/windsurf/memories/global_rules.md) are unreliable.
1665|    let rules_path = PathBuf::from(".windsurfrules");
1666|
1667|    let existing = fs::read_to_string(&rules_path).unwrap_or_default();
1668|    if existing.contains("RTK") || existing.contains("rtk") {
1669|        if !dry_run {
1670|            println!("\nRTK already configured for Windsurf in this project.\n");
1671|            println!("  Rules: .windsurfrules (already present)");
1672|        }
1673|    } else {
1674|        let new_content = if existing.trim().is_empty() {
1675|            WINDSURF_RULES.to_string()
1676|        } else {
1677|            format!("{}\n\n{}", existing.trim(), WINDSURF_RULES)
1678|        };
1679|        if dry_run {
1680|            println!(
1681|                "[dry-run] would write .windsurfrules: {}",
1682|                rules_path.display()
1683|            );
1684|            if verbose > 0 {
1685|                println!("[dry-run] content:\n{}", new_content);
1686|            }
1687|        } else {
1688|            fs::write(&rules_path, &new_content).context("Failed to write .windsurfrules")?;
1689|
1690|            if verbose > 0 {
1691|                eprintln!("Wrote .windsurfrules");
1692|            }
1693|
1694|            println!("\nRTK configured for Windsurf Cascade.\n");
1695|            println!("  Rules: .windsurfrules (installed)");
1696|        }
1697|    }
1698|    if !dry_run {
1699|        println!("  Cascade will now use rtk commands for token savings.");
1700|        println!("  Restart Windsurf. Test with: git status\n");
1701|    }
1702|
1703|    Ok(())
1704|}
1705|
1706|// ─── Kilo Code support ────────────────────────────────────────
1707|
1708|const KILOCODE_RULES: &str = include_str!("../../hooks/kilocode/rules.md");
1709|
1710|pub fn run_kilocode_mode(ctx: InitContext) -> Result<()> {
1711|    run_kilocode_mode_at(&std::env::current_dir()?, ctx)
1712|}
1713|
1714|fn run_kilocode_mode_at(base_dir: &Path, ctx: InitContext) -> Result<()> {
1715|    let InitContext { verbose, dry_run } = ctx;
1716|    // Kilo Code reads .kilocode/rules/ from the project root (workspace-scoped)
1717|    let target_dir = base_dir.join(".kilocode/rules");
1718|    let rules_path = target_dir.join("rtk-rules.md");
1719|
1720|    let existing = fs::read_to_string(&rules_path).unwrap_or_default();
1721|    if existing.contains("RTK") || existing.contains("rtk") {
1722|        if !dry_run {
1723|            println!("\nRTK already configured for Kilo Code in this project.\n");
1724|            println!("  Rules: .kilocode/rules/rtk-rules.md (already present)");
1725|        }
1726|    } else {
1727|        let new_content = if existing.trim().is_empty() {
1728|            KILOCODE_RULES.to_string()
1729|        } else {
1730|            format!("{}\n\n{}", existing.trim(), KILOCODE_RULES)
1731|        };
1732|        if dry_run {
1733|            println!(
1734|                "[dry-run] would write {}: (and create parent dir if missing)",
1735|                rules_path.display()
1736|            );
1737|            if verbose > 0 {
1738|                println!("[dry-run] content:\n{}", new_content);
1739|            }
1740|        } else {
1741|            fs::create_dir_all(&target_dir)
1742|                .context("Failed to create .kilocode/rules directory")?;
1743|            fs::write(&rules_path, &new_content)
1744|                .context("Failed to write .kilocode/rules/rtk-rules.md")?;
1745|
1746|            if verbose > 0 {
1747|                eprintln!("Wrote .kilocode/rules/rtk-rules.md");
1748|            }
1749|
1750|            println!("\nRTK configured for Kilo Code.\n");
1751|            println!("  Rules: .kilocode/rules/rtk-rules.md (installed)");
1752|        }
1753|    }
1754|    if dry_run {
1755|        print_dry_run_footer();
1756|    } else {
1757|        println!("  Kilo Code will now use rtk commands for token savings.");
1758|        println!("  Test with: git status\n");
1759|    }
1760|
1761|    Ok(())
1762|}
1763|
1764|// ─── Google Antigravity support ───────────────────────────────
1765|
1766|const ANTIGRAVITY_RULES: &str = include_str!("../../hooks/antigravity/rules.md");
1767|
1768|pub fn run_antigravity_mode(ctx: InitContext) -> Result<()> {
1769|    run_antigravity_mode_at(&std::env::current_dir()?, ctx)
1770|}
1771|
1772|fn run_antigravity_mode_at(base_dir: &Path, ctx: InitContext) -> Result<()> {
1773|    let InitContext { verbose, dry_run } = ctx;
1774|    // Antigravity reads .agents/rules/ from the project root (workspace-scoped)
1775|    let target_dir = base_dir.join(".agents/rules");
1776|    let rules_path = target_dir.join("antigravity-rtk-rules.md");
1777|
1778|    let existing = fs::read_to_string(&rules_path).unwrap_or_default();
1779|    if existing.contains("RTK") || existing.contains("rtk") {
1780|        if !dry_run {
1781|            println!("\nRTK already configured for Antigravity in this project.\n");
1782|            println!("  Rules: .agents/rules/antigravity-rtk-rules.md (already present)");
1783|        }
1784|    } else {
1785|        let new_content = if existing.trim().is_empty() {
1786|            ANTIGRAVITY_RULES.to_string()
1787|        } else {
1788|            format!("{}\n\n{}", existing.trim(), ANTIGRAVITY_RULES)
1789|        };
1790|        if dry_run {
1791|            println!(
1792|                "[dry-run] would write {}: (and create parent dir if missing)",
1793|                rules_path.display()
1794|            );
1795|            if verbose > 0 {
1796|                println!("[dry-run] content:\n{}", new_content);
1797|            }
1798|        } else {
1799|            fs::create_dir_all(&target_dir).context("Failed to create .agents/rules directory")?;
1800|            fs::write(&rules_path, &new_content)
1801|                .context("Failed to write .agents/rules/antigravity-rtk-rules.md")?;
1802|
1803|            if verbose > 0 {
1804|                eprintln!("Wrote .agents/rules/antigravity-rtk-rules.md");
1805|            }
1806|
1807|            println!("\nRTK configured for Google Antigravity.\n");
1808|            println!("  Rules: .agents/rules/antigravity-rtk-rules.md (installed)");
1809|        }
1810|    }
1811|    if dry_run {
1812|        print_dry_run_footer();
1813|    } else {
1814|        println!("  Antigravity will now use rtk commands for token savings.");
1815|        println!("  Test with: git status\n");
1816|    }
1817|
1818|    Ok(())
1819|}
1820|
1821|// ─── Hermes support ────────────────────────────────────────────
1822|
1823|const HERMES_PLUGIN_INIT: &str = include_str!("../../hooks/hermes/rtk-rewrite/__init__.py");
1824|const HERMES_PLUGIN_YAML: &str = include_str!("../../hooks/hermes/rtk-rewrite/plugin.yaml");
1825|
1826|pub fn run_hermes_mode(ctx: InitContext) -> Result<()> {
1827|    let hermes_home = resolve_hermes_home()?;
1828|    run_hermes_mode_at(&hermes_home, ctx)
1829|}
1830|
1831|fn hermes_plugin_dir(hermes_home: &Path) -> PathBuf {
1832|    hermes_home
1833|        .join(HERMES_PLUGINS_SUBDIR)
1834|        .join(HERMES_PLUGIN_NAME)
1835|}
1836|
1837|fn run_hermes_mode_at(hermes_home: &Path, ctx: InitContext) -> Result<()> {
1838|    let InitContext { dry_run, .. } = ctx;
1839|    let plugin_dir = hermes_plugin_dir(hermes_home);
1840|    if !dry_run {
1841|        fs::create_dir_all(&plugin_dir).with_context(|| {
1842|            format!(
1843|                "Failed to create Hermes plugin directory: {}",
1844|                plugin_dir.display()
1845|            )
1846|        })?;
1847|    }
1848|
1849|    let init_path = plugin_dir.join(HERMES_PLUGIN_INIT_FILE);
1850|    let manifest_path = plugin_dir.join(HERMES_PLUGIN_MANIFEST_FILE);
1851|    write_if_changed(&init_path, HERMES_PLUGIN_INIT, "Hermes plugin", ctx)?;
1852|    write_if_changed(
1853|        &manifest_path,
1854|        HERMES_PLUGIN_YAML,
1855|        "Hermes plugin manifest",
1856|        ctx,
1857|    )?;
1858|
1859|    let config_path = hermes_home.join("config.yaml");
1860|    let existing_config = if config_path.exists() {
1861|        fs::read_to_string(&config_path)
1862|            .with_context(|| format!("Failed to read Hermes config: {}", config_path.display()))?
1863|    } else {
1864|        String::new()
1865|    };
1866|    let patched_config = patch_hermes_config(&existing_config);
1867|    write_if_changed(&config_path, &patched_config, "Hermes config", ctx)?;
1868|
1869|    if dry_run {
1870|        print_dry_run_footer();
1871|    } else {
1872|        println!("\nRTK configured for Hermes.\n");
1873|        println!("  Plugin: {}", plugin_dir.display());
1874|        println!("  Config: {}", config_path.display());
1875|        println!("  Hermes will now rewrite terminal commands through rtk.");
1876|        println!("  Restart Hermes. Test with: git status\n");
1877|    }
1878|
1879|    Ok(())
1880|}
1881|
1882|pub fn uninstall_hermes(ctx: InitContext) -> Result<()> {
1883|    let InitContext { dry_run, .. } = ctx;
1884|    let hermes_home = resolve_hermes_home()?;
1885|    let removed = uninstall_hermes_at(&hermes_home, ctx)?;
1886|
1887|    if removed.is_empty() {
1888|        println!("RTK Hermes support was not installed (nothing to remove)");
1889|    } else {
1890|        let header = if dry_run {
1891|            "[dry-run] would uninstall RTK for Hermes CLI:"
1892|        } else {
1893|            "RTK uninstalled for Hermes CLI:"
1894|        };
1895|        println!("{}", header);
1896|        for item in removed {
1897|            println!("  - {}", item);
1898|        }
1899|    }
1900|
1901|    if dry_run {
1902|        print_dry_run_footer();
1903|    }
1904|
1905|    Ok(())
1906|}
1907|
1908|fn uninstall_hermes_at(hermes_home: &Path, ctx: InitContext) -> Result<Vec<String>> {
1909|    let InitContext { verbose, dry_run } = ctx;
1910|    let mut removed = Vec::new();
1911|
1912|    let plugin_dir = hermes_plugin_dir(hermes_home);
1913|    if plugin_dir.exists() {
1914|        if dry_run {
1915|            println!(
1916|                "[dry-run] would remove Hermes plugin directory: {}",
1917|                plugin_dir.display()
1918|            );
1919|        } else {
1920|            // nosemgrep: filesystem-deletion -- uninstall intentionally removes only RTK's Hermes plugin directory.
1921|            fs::remove_dir_all(&plugin_dir).with_context(|| {
1922|                format!(
1923|                    "Failed to remove Hermes plugin directory: {}",
1924|                    plugin_dir.display()
1925|                )
1926|            })?;
1927|            if verbose > 0 {
1928|                eprintln!("Removed Hermes plugin directory: {}", plugin_dir.display());
1929|            }
1930|        }
1931|        removed.push(format!("Hermes plugin: {}", plugin_dir.display()));
1932|    }
1933|
1934|    let config_path = hermes_home.join("config.yaml");
1935|    if config_path.exists() {
1936|        let existing_config = fs::read_to_string(&config_path)
1937|            .with_context(|| format!("Failed to read Hermes config: {}", config_path.display()))?;
1938|        let patched_config = unpatch_hermes_config(&existing_config);
1939|
1940|        if patched_config != existing_config {
1941|            if dry_run {
1942|                println!(
1943|                    "[dry-run] would update Hermes config: {}",
1944|                    config_path.display()
1945|                );
1946|                if verbose > 0 {
1947|                    println!("[dry-run] content:\n{}", patched_config);
1948|                }
1949|            } else {
1950|                atomic_write(&config_path, &patched_config).with_context(|| {
1951|                    format!("Failed to write Hermes config: {}", config_path.display())
1952|                })?;
1953|                if verbose > 0 {
1954|                    eprintln!("Updated Hermes config: {}", config_path.display());
1955|                }
1956|            }
1957|            removed.push("Hermes config: removed RTK plugin entry".to_string());
1958|        }
1959|    }
1960|
1961|    Ok(removed)
1962|}
1963|
1964|fn patch_hermes_config(existing: &str) -> String {
1965|    rewrite_hermes_config(existing, true)
1966|}
1967|
1968|fn unpatch_hermes_config(existing: &str) -> String {
1969|    rewrite_hermes_config(existing, false)
1970|}
1971|
1972|fn rewrite_hermes_config(existing: &str, add_rtk: bool) -> String {
1973|    if existing.trim().is_empty() {
1974|        return if add_rtk {
1975|            hermes_plugins_block()
1976|        } else {
1977|            String::new()
1978|        };
1979|    }
1980|
1981|    let mut lines = split_yaml_lines(existing);
1982|    let Some(plugins_idx) = find_yaml_key_line(&lines, "plugins", 0, None) else {
1983|        return if add_rtk {
1984|            append_hermes_plugins_block(existing)
1985|        } else {
1986|            existing.to_string()
1987|        };
1988|    };
1989|
1990|    let plugins_indent = yaml_indent(&lines[plugins_idx]);
1991|    let plugins_end = yaml_block_end(&lines, plugins_idx, plugins_indent);
1992|    let Some(enabled_idx) = find_yaml_key_line(
1993|        &lines,
1994|        "enabled",
1995|        plugins_idx + 1,
1996|        Some((plugins_end, plugins_indent)),
1997|    ) else {
1998|        if add_rtk {
1999|            let (enabled_indent, item_indent) =
2000|                hermes_missing_enabled_indents(&lines, plugins_idx, plugins_end, plugins_indent);
2001|