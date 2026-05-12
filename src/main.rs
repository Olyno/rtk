1|mod analytics;
2|mod cmds;
3|mod core;
4|mod discover;
5|mod hooks;
6|mod index;
7|mod learn;
8|mod parser;
9|
10|// Re-export command modules for routing
11|use cmds::cloud::{aws_cmd, container, curl_cmd, psql_cmd, wget_cmd};
12|use cmds::dart::{dart_cmd, flutter_cmd};
13|use cmds::dotnet::{binlog, dotnet_cmd, dotnet_format_report, dotnet_trx};
14|use cmds::git::{diff_cmd, gh_cmd, git, glab_cmd, gt_cmd};
15|use cmds::go::{go_cmd, golangci_cmd};
16|use cmds::js::{
17|    lint_cmd, next_cmd, npm_cmd, playwright_cmd, pnpm_cmd, prettier_cmd, prisma_cmd, tsc_cmd,
18|    vitest_cmd,
19|};
20|use cmds::jvm::gradlew_cmd;
21|use cmds::python::{mypy_cmd, pip_cmd, pytest_cmd, ruff_cmd};
22|use cmds::ruby::{rake_cmd, rspec_cmd, rubocop_cmd};
23|use cmds::rust::{cargo_cmd, runner};
24|use cmds::system::{
25|    deps, env_cmd, find_cmd, format_cmd, grep_cmd, json_cmd, local_llm, log_cmd, ls, pipe_cmd,
26|    read, summary, tree, wc_cmd,
27|};
28|
29|use anyhow::{Context, Result};
30|use clap::error::ErrorKind;
31|use clap::{Parser, Subcommand, ValueEnum};
32|use std::ffi::OsString;
33|use std::path::{Path, PathBuf};
34|
35|/// Target agent for hook installation.
36|#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
37|pub enum AgentTarget {
38|    /// Claude Code (default)
39|    Claude,
40|    /// Cursor Agent (editor and CLI)
41|    Cursor,
42|    /// Windsurf IDE (Cascade)
43|    Windsurf,
44|    /// Cline / Roo Code (VS Code)
45|    Cline,
46|    /// Kilo Code
47|    Kilocode,
48|    /// Google Antigravity
49|    Antigravity,
50|    /// Hermes CLI
51|    Hermes,
52|    /// Kimi CLI
53|    Kimi,
54|}
55|
56|#[derive(Parser)]
57|#[command(
58|    name = "rtk",
59|    version,
60|    about = "Rust Token Killer - Minimize LLM token consumption",
61|    long_about = "A high-performance CLI proxy designed to filter and summarize system outputs before they reach your LLM context."
62|)]
63|struct Cli {
64|    #[command(subcommand)]
65|    command: Commands,
66|
67|    /// Verbosity level (-v, -vv, -vvv)
68|    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
69|    verbose: u8,
70|
71|    /// Ultra-compact mode: ASCII icons, inline format (Level 2 optimizations)
72|    #[arg(long, global = true)]
73|    ultra_compact: bool,
74|
75|    /// Set SKIP_ENV_VALIDATION=1 for child processes (Next.js, tsc, lint, prisma)
76|    #[arg(long = "skip-env", global = true)]
77|    skip_env: bool,
78|}
79|
80|#[derive(Debug, Subcommand)]
81|enum Commands {
82|    /// Install rtk hooks for AI assistants (Claude, Cursor, Hermes, Codex, Copilot)
83|    Init {
84|        /// Add to global assistant config directory instead of local project file
85|        #[arg(short, long)]
86|        global: bool,
87|
88|        /// Install OpenCode plugin (in addition to Claude Code)
89|        #[arg(long)]
90|        opencode: bool,
91|
92|        /// Initialize for Gemini CLI instead of Claude Code
93|        #[arg(long)]
94|        gemini: bool,
95|
96|        /// Target agent to install hooks for (default: claude)
97|        #[arg(long, value_enum)]
98|        agent: Option<AgentTarget>,
99|
100|        /// Show current configuration
101|        #[arg(long)]
102|        show: bool,
103|
104|        /// Inject full instructions into CLAUDE.md (legacy mode)
105|        #[arg(long = "claude-md", group = "mode")]
106|        claude_md: bool,
107|
108|        /// Hook only, no RTK.md
109|        #[arg(long = "hook-only", group = "mode")]
110|        hook_only: bool,
111|
112|        /// Auto-patch settings.json without prompting
113|        #[arg(long = "auto-patch", group = "patch")]
114|        auto_patch: bool,
115|
116|        /// Skip settings.json patching (print manual instructions)
117|        #[arg(long = "no-patch", group = "patch")]
118|        no_patch: bool,
119|
120|        /// Remove RTK artifacts for the selected assistant mode
121|        #[arg(long)]
122|        uninstall: bool,
123|
124|        /// Target Codex CLI (uses AGENTS.md + RTK.md, no Claude hook patching)
125|        #[arg(long)]
126|        codex: bool,
127|
128|        /// Install GitHub Copilot integration (VS Code + CLI)
129|        #[arg(long)]
130|        copilot: bool,
131|
132|        /// Preview changes without writing any files (combine with -v to show content)
133|        #[arg(long = "dry-run", conflicts_with = "show")]
134|        dry_run: bool,
135|    },
136|
137|    /// Show token savings: summary, history, daily/weekly/monthly charts
138|    Gain {
139|        /// Filter statistics to current project (current working directory) // added
140|        #[arg(short, long)]
141|        project: bool,
142|        /// Show ASCII graph of daily savings
143|        #[arg(short, long)]
144|        graph: bool,
145|        /// Show recent command history
146|        #[arg(short = 'H', long)]
147|        history: bool,
148|        /// Show monthly quota savings estimate
149|        #[arg(short, long)]
150|        quota: bool,
151|        /// Subscription tier for quota calculation: pro, 5x, 20x
152|        #[arg(short, long, default_value = "20x", requires = "quota")]
153|        tier: String,
154|        /// Show detailed daily breakdown (all days)
155|        #[arg(short, long)]
156|        daily: bool,
157|        /// Show weekly breakdown
158|        #[arg(short, long)]
159|        weekly: bool,
160|        /// Show monthly breakdown
161|        #[arg(short, long)]
162|        monthly: bool,
163|        /// Show all time breakdowns (daily + weekly + monthly)
164|        #[arg(short, long)]
165|        all: bool,
166|        /// Output format: text, json, csv
167|        #[arg(short, long, default_value = "text")]
168|        format: String,
169|        /// Show parse failure log (commands that fell back to raw execution)
170|        #[arg(short = 'F', long)]
171|        failures: bool,
172|        /// Show potential commands (unregistered, executed >5 times)
173|        #[arg(short = 'P', long)]
174|        potential: bool,
175|        /// Reset all token savings stats to zero
176|        #[arg(long)]
177|        reset: bool,
178|        /// Skip confirmation prompt when resetting
179|        #[arg(long, requires = "reset")]
180|        yes: bool,
181|    },
182|
183|    /// Show or create rtk configuration file
184|    Config {
185|        /// Create default config file
186|        #[arg(long)]
187|        create: bool,
188|    },
189|
190|    /// Find commands in Claude Code history that rtk could optimize
191|    Discover {
192|        /// Filter by project path (substring match)
193|        #[arg(short, long)]
194|        project: Option<String>,
195|        /// Max commands per section
196|        #[arg(short, long, default_value = "15")]
197|        limit: usize,
198|        /// Scan all projects (default: current project only)
199|        #[arg(short, long)]
200|        all: bool,
201|        /// Limit to sessions from last N days
202|        #[arg(short, long, default_value = "30")]
203|        since: u64,
204|        /// Output format: text, json
205|        #[arg(short, long, default_value = "text")]
206|        format: String,
207|    },
208|
209|    /// Audit Claude Code sessions: rtk adoption and missed opportunities
210|    Session {},
211|
212|    /// Manage anonymous usage data sharing (GDPR compliant)
213|    Telemetry {
214|        #[command(subcommand)]
215|        command: core::telemetry_cmd::TelemetrySubcommand,
216|    },
217|
218|    /// Build project search index for faster grep and find
219|    Index {
220|        /// Path to index (default: current directory)
221|        #[arg(default_value = ".")]
222|        path: PathBuf,
223|        /// Show index statistics after scanning
224|        #[arg(short, long)]
225|        stats: bool,
226|    },
227|
228|    /// AWS CLI with compact output (force JSON, compress)
229|    Aws {
230|        /// AWS service subcommand (e.g., sts, s3, ec2, ecs, rds, cloudformation)
231|        subcommand: String,
232|        /// Additional arguments
233|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
234|        args: Vec<String>,
235|    },
236|
237|    /// Cargo commands with compact output
238|    Cargo {
239|        #[command(subcommand)]
240|        command: CargoCommands,
241|    },
242|
243|    /// Compare Claude Code API costs vs tokens saved by rtk
244|    CcEconomics {
245|        /// Show detailed daily breakdown
246|        #[arg(short, long)]
247|        daily: bool,
248|        /// Show weekly breakdown
249|        #[arg(short, long)]
250|        weekly: bool,
251|        /// Show monthly breakdown
252|        #[arg(short, long)]
253|        monthly: bool,
254|        /// Show all time breakdowns (daily + weekly + monthly)
255|        #[arg(short, long)]
256|        all: bool,
257|        /// Output format: text, json, csv
258|        #[arg(short, long, default_value = "text")]
259|        format: String,
260|    },
261|
262|    /// Curl with auto-JSON detection and schema output
263|    Curl {
264|        /// Curl arguments (URL + options)
265|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
266|        args: Vec<String>,
267|    },
268|
269|    /// Dart commands with compact analyzer, formatter, and test output
270|    Dart {
271|        /// Dart arguments
272|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
273|        args: Vec<String>,
274|    },
275|
276|    /// Summarize project dependencies (Cargo.toml, package.json, etc.)
277|    Deps {
278|        /// Project path
279|        #[arg(default_value = ".")]
280|        path: PathBuf,
281|    },
282|
283|    /// Ultra-condensed diff: only changed lines, no context
284|    Diff {
285|        /// First file or - for stdin (unified diff)
286|        file1: PathBuf,
287|        /// Second file (optional if stdin)
288|        file2: Option<PathBuf>,
289|    },
290|
291|    /// Docker commands with compact output
292|    Docker {
293|        #[command(subcommand)]
294|        command: DockerCommands,
295|    },
296|
297|    /// .NET commands with compact output (build/test/restore/format)
298|    Dotnet {
299|        #[command(subcommand)]
300|        command: DotnetCommands,
301|    },
302|
303|    /// Show environment variables (sensitive values masked)
304|    Env {
305|        /// Filter by name (e.g. PATH, AWS)
306|        #[arg(short, long)]
307|        filter: Option<String>,
308|        /// Show all (include sensitive)
309|        #[arg(long)]
310|        show_all: bool,
311|    },
312|
313|    /// Run command, show only errors and warnings
314|    Err {
315|        /// Command to run
316|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
317|        command: Vec<String>,
318|    },
319|
320|    /// Find files with compact tree output (accepts native find flags)
321|    Find {
322|        /// All find arguments (supports both RTK and native find syntax)
323|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
324|        args: Vec<String>,
325|    },
326|
327|    /// Flutter commands with compact analyzer and test output
328|    Flutter {
329|        /// Flutter arguments
330|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
331|        args: Vec<String>,
332|    },
333|
334|    /// Universal format checker (auto-detects prettier, black, ruff)
335|    Format {
336|        /// Formatter arguments (auto-detects formatter from project files)
337|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
338|        args: Vec<String>,
339|    },
340|
341|    /// GitHub CLI (gh) commands with token-optimized output
342|    Gh {
343|        /// Subcommand: pr, issue, run, repo
344|        subcommand: String,
345|        /// Additional arguments
346|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
347|        args: Vec<String>,
348|    },
349|
350|    /// Git commands with compact output
351|    Git {
352|        /// Change to directory before executing (like git -C <path>, can be repeated)
353|        #[arg(short = 'C', action = clap::ArgAction::Append)]
354|        directory: Vec<String>,
355|
356|        /// Git configuration override (like git -c key=value, can be repeated)
357|        #[arg(short = 'c', action = clap::ArgAction::Append)]
358|        config_override: Vec<String>,
359|
360|        /// Set the path to the .git directory
361|        #[arg(long = "git-dir")]
362|        git_dir: Option<String>,
363|
364|        /// Set the path to the working tree
365|        #[arg(long = "work-tree")]
366|        work_tree: Option<String>,
367|
368|        /// Disable pager (like git --no-pager)
369|        #[arg(long = "no-pager")]
370|        no_pager: bool,
371|
372|        /// Skip optional locks (like git --no-optional-locks)
373|        #[arg(long = "no-optional-locks")]
374|        no_optional_locks: bool,
375|
376|        /// Treat repository as bare (like git --bare)
377|        #[arg(long)]
378|        bare: bool,
379|
380|        /// Treat pathspecs literally (like git --literal-pathspecs)
381|        #[arg(long = "literal-pathspecs")]
382|        literal_pathspecs: bool,
383|
384|        #[command(subcommand)]
385|        command: GitCommands,
386|    },
387|
388|    /// GitLab CLI (glab) commands with token-optimized output
389|    Glab {
390|        /// Target repository (owner/repo), passed as glab -R flag
391|        #[arg(short = 'R', long = "repo")]
392|        repo: Option<String>,
393|        /// Target group, passed as glab -g flag
394|        #[arg(short = 'g', long = "group")]
395|        group: Option<String>,
396|        /// Subcommand: mr, issue, ci, pipeline, api
397|        subcommand: String,
398|        /// Additional arguments
399|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
400|        args: Vec<String>,
401|    },
402|
403|    /// Go commands with compact output
404|    Go {
405|        #[command(subcommand)]
406|        command: GoCommands,
407|    },
408|
409|    /// golangci-lint wrapper with compact `run` support and passthrough for other invocations
410|    #[command(name = "golangci-lint")]
411|    GolangciLint {
412|        /// Additional golangci-lint arguments
413|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
414|        args: Vec<String>,
415|    },
416|
417|    /// Android Gradle wrapper with compact output (build, test, lint)
418|    #[command(name = "gradlew")]
419|    Gradlew {
420|        /// Gradle tasks and arguments (e.g., assembleDebug, testDebugUnitTest, lint, --info)
421|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
422|        args: Vec<String>,
423|    },
424|
425|    /// Compact grep - strips whitespace, truncates, groups by file
426|    Grep {
427|        /// Pattern to search
428|        pattern: String,
429|        /// Path to search in
430|        #[arg(default_value = ".")]
431|        path: String,
432|        /// Max line length
433|        #[arg(short = 'l', long, default_value = "80")]
434|        max_len: usize,
435|        /// Max results to show
436|        #[arg(short, long, default_value = "200")]
437|        max: usize,
438|        /// Show only match context (not full line)
439|        #[arg(long)]
440|        context_only: bool,
441|        /// Filter by file type (e.g., ts, py, rust)
442|        #[arg(short = 't', long)]
443|        file_type: Option<String>,
444|        /// Show line numbers (always on, accepted for grep/rg compatibility)
445|        #[arg(short = 'n', long)]
446|        line_numbers: bool,
447|        /// Extra ripgrep arguments (e.g., -i, -A 3, -w, --glob)
448|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
449|        extra_args: Vec<String>,
450|    },
451|
452|    /// Graphite (gt) stacked PR commands with compact output
453|    Gt {
454|        #[command(subcommand)]
455|        command: GtCommands,
456|    },
457|
458|    /// Jest commands with compact output
459|    Jest {
460|        /// Additional jest arguments
461|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
462|        args: Vec<String>,
463|    },
464|
465|    /// Show JSON with compact values or keys-only structure view
466|    Json {
467|        /// JSON file
468|        file: PathBuf,
469|        /// Max depth
470|        #[arg(short, long, default_value = "5")]
471|        depth: usize,
472|        /// Show keys only (strip all values, show structure)
473|        #[arg(long)]
474|        keys_only: bool,
475|    },
476|
477|    /// Kubectl commands with compact output
478|    Kubectl {
479|        #[command(subcommand)]
480|        command: KubectlCommands,
481|    },
482|
483|    /// Analyze Claude Code error history to suggest rtk corrections
484|    Learn {
485|        /// Filter by project path (substring match)
486|        #[arg(short, long)]
487|        project: Option<String>,
488|        /// Scan all projects (default: current project only)
489|        #[arg(short, long)]
490|        all: bool,
491|        /// Limit to sessions from last N days
492|        #[arg(short, long, default_value = "30")]
493|        since: u64,
494|        /// Output format: text, json
495|        #[arg(short, long, default_value = "text")]
496|        format: String,
497|        /// Generate .claude/rules/cli-corrections.md file
498|        #[arg(short, long)]
499|        write_rules: bool,
500|        /// Minimum confidence threshold (0.0-1.0)
501|        #[arg(long, default_value = "0.6")]
502|        min_confidence: f64,
503|        /// Minimum occurrences to include in report
504|        #[arg(long, default_value = "1")]
505|        min_occurrences: usize,
506|    },
507|
508|    /// ESLint with grouped rule violations
509|    Lint {
510|        /// Linter arguments
511|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
512|        args: Vec<String>,
513|    },
514|
515|    /// Filter and deduplicate log output (stdin or file)
516|    Log {
517|        /// Log file (omit for stdin)
518|        file: Option<PathBuf>,
519|    },
520|
521|522|    /// List directory with token-optimized output (proxy to native ls)
523|    Ls {
524|        /// Arguments passed to ls (supports all native ls flags like -l, -a, -h, -R)
525|526|    /// Read stdin, apply filter, print filtered output (Unix pipe mode)
527|    Pipe {
528|        /// Filter name as a positional argument (e.g. `rtk pipe log`)
529|        filter_name: Option<String>,
530|
531|        /// Filter name (cargo-test, pytest, grep, find, git-log, etc.)
532|        #[arg(short, long)]
533|        filter: Option<String>,
534|
535|        /// Pass stdin through without filtering
536|        #[arg(long)]
537|        passthrough: bool,
538|    },
539|
540|    /// Trust project-local TOML filters in current directory
541|    Trust {
542|        /// List all trusted projects
543|        #[arg(long)]
544|        list: bool,
545|    },
546|
547|    /// Revoke trust for project-local TOML filters
548|    Untrust,
549|
550|    /// Verify hook integrity and run TOML filter inline tests
551|    Verify {
552|        /// Run tests only for this filter name
553|        #[arg(long)]
554|        filter: Option<String>,
555|        /// Fail if any filter has no inline tests (CI mode)
556|        #[arg(long)]
557|        require_all: bool,
558|    },
559|
560|    /// Ruff linter/formatter with compact output
561|    Ruff {
562|        /// Ruff arguments (e.g., check, format --check)
563|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
564|        args: Vec<String>,
565|    },
566|
567|    /// Pytest test runner with compact output
568|    Pytest {
569|        /// Pytest arguments
570|571|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
572|        args: Vec<String>,
573|    },
574|
575|    /// Mypy type checker with grouped error output
576|    Mypy {
577|        /// Mypy arguments
578|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
579|        args: Vec<String>,
580|    },
581|
582|    /// Next.js build with compact output
583|    Next {
584|        /// Next.js build arguments
585|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
586|        args: Vec<String>,
587|    },
588|
589|    /// npm run with filtered output (strip boilerplate)
590|    #[command(next_help_heading = "NodeJS")]
591|    Npm {
592|        /// npm run arguments (script name + options)
593|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
594|        args: Vec<String>,
595|    },
596|
597|    /// npx with intelligent routing (tsc, eslint, prisma -> specialized filters)
598|    Npx {
599|        /// npx arguments (command + options)
600|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
601|        args: Vec<String>,
602|    },
603|
604|    /// Pip package manager with compact output (auto-detects uv)
605|    Pip {
606|        /// Pip arguments (e.g., list, outdated, install)
607|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
608|        args: Vec<String>,
609|    },
610|
611|    /// Filter piped stdin through rtk (e.g., cmd | rtk pipe -f cargo-test)
612|    Pipe {
613|        /// Filter name (cargo-test, pytest, grep, find, git-log, etc.)
614|        #[arg(short, long)]
615|        filter: Option<String>,
616|
617|        /// Pass stdin through without filtering
618|        #[arg(long)]
619|        passthrough: bool,
620|    },
621|
622|    /// Playwright E2E tests with compact output
623|    Playwright {
624|        /// Playwright arguments
625|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
626|        args: Vec<String>,
627|    },
628|
629|    /// pnpm commands with ultra-compact output
630|    Pnpm {
631|        /// pnpm filter arguments (can be repeated: --filter @app1 --filter @app2)
632|        #[arg(long, short = 'F')]
633|        filter: Vec<String>,
634|
635|        #[command(subcommand)]
636|        command: PnpmCommands,
637|    },
638|
639|    /// Prettier format checker with compact output
640|    Prettier {
641|        /// Prettier arguments (e.g., --check, --write)
642|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
643|        args: Vec<String>,
644|    },
645|
646|    /// Prisma commands with compact output (no ASCII art)
647|    Prisma {
648|        #[command(subcommand)]
649|        command: PrismaCommands,
650|    },
651|
652|    /// Run command unchanged but measure its token count (benchmark)
653|    Proxy {
654|        /// Command and arguments to execute
655|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
656|        args: Vec<OsString>,
657|    },
658|
659|    /// PostgreSQL client with compact output (strip borders, compress tables)
660|    #[command(disable_help_flag = true)]
661|    Psql {
662|        /// psql arguments
663|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
664|        args: Vec<String>,
665|    },
666|
667|    /// Pytest test runner with compact output
668|    Pytest {
669|        /// Pytest arguments
670|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
671|        args: Vec<String>,
672|    },
673|
674|    /// Rake/Rails test with compact Minitest output (Ruby)
675|    Rake {
676|        /// Rake arguments (e.g., test, test TEST=path/to/test.rb)
677|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
678|        args: Vec<String>,
679|    },
680|
681|    /// Read file with intelligent filtering and line numbers
682|    Read {
683|        /// Files to read (supports multiple, like cat)
684|        #[arg(required = true, num_args = 1..)]
685|        files: Vec<PathBuf>,
686|        /// Filter: none (default, full content), minimal, aggressive
687|        #[arg(short, long, default_value = "none")]
688|        level: core::filter::FilterLevel,
689|        /// Max lines
690|        #[arg(short, long, conflicts_with = "tail_lines")]
691|        max_lines: Option<usize>,
692|        /// Keep only last N lines
693|        #[arg(long, conflicts_with = "max_lines")]
694|        tail_lines: Option<usize>,
695|        /// Show line numbers
696|        #[arg(short = 'n', long)]
697|        line_numbers: bool,
698|    },
699|
700|    /// RSpec test runner with compact output (Rails/Ruby)
701|    Rspec {
702|        /// RSpec arguments (e.g., spec/models, --tag focus)
703|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
704|        args: Vec<String>,
705|    },
706|
707|    /// RuboCop linter with compact output (Ruby)
708|    Rubocop {
709|        /// RuboCop arguments (e.g., --auto-correct, -A)
710|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
711|        args: Vec<String>,
712|    },
713|
714|    /// Ruff linter/formatter with compact output
715|    Ruff {
716|        /// Ruff arguments (e.g., check, format --check)
717|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
718|        args: Vec<String>,
719|    },
720|
721|    /// Execute raw shell command, no filtering (escape hatch)
722|    Run {
723|        /// Command string to execute (use -c for shell-like invocation)
724|        #[arg(short = 'c', long = "command")]
725|        command: Option<String>,
726|        /// Positional command arguments (alternative to -c)
727|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
728|        args: Vec<String>,
729|    },
730|
731|    /// Generate 2-line technical summary of a file (heuristic-based)
732|    Smart {
733|        /// File to analyze
734|        file: PathBuf,
735|        /// Model: heuristic
736|        #[arg(short, long, default_value = "heuristic")]
737|        model: String,
738|        /// Force model download
739|        #[arg(long)]
740|        force_download: bool,
741|    },
742|
743|    /// Run command and show heuristic summary of its output
744|    Summary {
745|        /// Command to run and summarize
746|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
747|        command: Vec<String>,
748|    },
749|
750|    /// Run tests, show only failures (generic runner)
751|    Test {
752|        /// Test command (e.g. cargo test)
753|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
754|        command: Vec<String>,
755|    },
756|
757|    /// Directory tree with token-optimized output (proxy to native tree)
758|    Tree {
759|        /// Arguments passed to tree (supports all native tree flags like -L, -d, -a)
760|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
761|        args: Vec<String>,
762|    },
763|
764|    /// Authorize current project TOML filters
765|    Trust {
766|        /// List all trusted projects
767|        #[arg(long)]
768|        list: bool,
769|    },
770|
771|    /// TypeScript compiler with grouped error output
772|    Tsc {
773|        /// TypeScript compiler arguments
774|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
775|        args: Vec<String>,
776|    },
777|
778|    /// Revoke trust for project TOML filters
779|    Untrust,
780|
781|    /// Validate TOML filter files: syntax check + inline tests
782|    Verify {
783|        /// Run tests only for this filter name
784|        #[arg(long)]
785|        filter: Option<String>,
786|        /// Fail if any filter has no inline tests (CI mode)
787|        #[arg(long)]
788|        require_all: bool,
789|    },
790|
791|    /// Vitest commands with compact output
792|    Vitest {
793|        /// Additional vitest arguments
794|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
795|        args: Vec<String>,
796|    },
797|
798|    /// Word/line/byte count with compact output
799|    Wc {
800|        /// Arguments passed to wc (files, flags like -l, -w, -c)
801|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
802|        args: Vec<String>,
803|    },
804|
805|    /// Download with compact output (strips progress bars)
806|    Wget {
807|        /// URL to download
808|        url: String,
809|        /// Output file (-O - for stdout)
810|        #[arg(short = 'O', long = "output-document", allow_hyphen_values = true)]
811|        output: Option<String>,
812|        /// Additional wget arguments
813|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
814|        args: Vec<String>,
815|    },
816|
817|    /// Debug: inspect what rtk rewrote via hooks (needs RTK_HOOK_AUDIT=1)
818|    #[command(name = "hook-audit")]
819|    HookAudit {
820|        /// Show entries from last N days (0 = all time)
821|        #[arg(short, long, default_value = "7")]
822|        since: u64,
823|    },
824|
825|    /// Show what rtk would transform a command into (used by hooks)
826|    ///   REWRITTEN=$(rtk rewrite "$CMD") || exit 0
827|    Rewrite {
828|        /// Raw command to rewrite (e.g. "git status", "cargo test && git push")
829|        /// Accepts multiple args: `rtk rewrite ls -al` is equivalent to `rtk rewrite "ls -al"`
830|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
831|        args: Vec<String>,
832|    },
833|
834|    /// Internal hook processors for LLM CLI tools (Claude, Gemini, Copilot)
835|    Hook {
836|        #[command(subcommand)]
837|        command: HookCommands,
838|    },
839|}
840|
841|#[derive(Debug, Subcommand)]
842|enum HookCommands {
843|    /// Process Claude Code PreToolUse hook (reads JSON from stdin)
844|    Claude,
845|    /// Process Cursor Agent hook (reads JSON from stdin)
846|    Cursor,
847|    /// Process Gemini CLI BeforeTool hook (reads JSON from stdin)
848|    Gemini,
849|    /// Process Copilot preToolUse hook (VS Code + Copilot CLI, reads JSON from stdin)
850|    Copilot,
851|    /// Check how a command would be rewritten by the hook engine (dry-run)
852|    Check {
853|        /// Target agent
854|        #[arg(long, default_value = "claude")]
855|        agent: String,
856|        /// Command to check
857|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
858|        command: Vec<String>,
859|    },
860|}
861|
862|#[derive(Debug, Subcommand)]
863|enum GitCommands {
864|    /// Condensed diff output
865|    Diff {
866|        /// Git arguments (supports all git diff flags like --stat, --cached, etc)
867|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
868|        args: Vec<String>,
869|    },
870|    /// One-line commit history
871|    Log {
872|        /// Git arguments (supports all git log flags like --oneline, --graph, --all)
873|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
874|        args: Vec<String>,
875|    },
876|    /// Compact status (supports all git status flags)
877|    Status {
878|        /// Git arguments (supports all git status flags like --porcelain, --short, -s)
879|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
880|        args: Vec<String>,
881|    },
882|    /// Compact show (commit summary + stat + compacted diff)
883|    Show {
884|        /// Git arguments (supports all git show flags)
885|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
886|        args: Vec<String>,
887|    },
888|    /// Add files → "ok"
889|    Add {
890|        /// Files and flags to add (supports all git add flags like -A, -p, --all, etc)
891|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
892|        args: Vec<String>,
893|    },
894|    /// Commit → "ok \<hash\>"
895|    Commit {
896|        /// Git commit arguments (supports -a, -m, --amend, --allow-empty, etc)
897|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
898|        args: Vec<String>,
899|    },
900|    /// Push → "ok \<branch\>"
901|    Push {
902|        /// Git push arguments (supports -u, remote, branch, etc.)
903|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
904|        args: Vec<String>,
905|    },
906|    /// Pull → "ok \<stats\>"
907|    Pull {
908|        /// Git pull arguments (supports --rebase, remote, branch, etc.)
909|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
910|        args: Vec<String>,
911|    },
912|    /// Compact branch listing (current/local/remote)
913|    Branch {
914|        /// Git branch arguments (supports -d, -D, -m, etc.)
915|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
916|        args: Vec<String>,
917|    },
918|    /// Fetch → "ok fetched (N new refs)"
919|    Fetch {
920|        /// Git fetch arguments
921|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
922|        args: Vec<String>,
923|    },
924|    /// Stash management (list, show, pop, apply, drop)
925|    Stash {
926|        /// Subcommand: list, show, pop, apply, drop, push
927|        subcommand: Option<String>,
928|        /// Additional arguments
929|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
930|        args: Vec<String>,
931|    },
932|    /// Compact worktree listing
933|    Worktree {
934|        /// Git worktree arguments (add, remove, prune, or empty for list)
935|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
936|        args: Vec<String>,
937|    },
938|    /// Passthrough: runs any unsupported git subcommand directly
939|    #[command(external_subcommand)]
940|    Other(Vec<OsString>),
941|}
942|
943|#[derive(Debug, Subcommand)]
944|enum PnpmCommands {
945|    /// List installed packages (ultra-dense)
946|    List {
947|        /// Depth level (default: 0)
948|        #[arg(short, long, default_value = "0")]
949|        depth: usize,
950|        /// Additional pnpm arguments
951|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
952|        args: Vec<String>,
953|    },
954|    /// Show outdated packages (condensed: "pkg: old → new")
955|    Outdated {
956|        /// Additional pnpm arguments
957|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
958|        args: Vec<String>,
959|    },
960|    /// Install packages (filter progress bars)
961|    Install {
962|        /// Additional pnpm arguments
963|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
964|        args: Vec<String>,
965|    },
966|    /// Typecheck (delegates to tsc filter)
967|    Typecheck {
968|        /// Additional typecheck arguments
969|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
970|        args: Vec<String>,
971|    },
972|    /// Passthrough: runs any unsupported pnpm subcommand directly
973|    #[command(external_subcommand)]
974|    Other(Vec<OsString>),
975|}
976|
977|#[derive(Debug, Subcommand)]
978|enum DockerCommands {
979|    /// List running containers
980|    Ps {
981|        #[arg(short = 'a', long)]
982|        all: bool,
983|    },
984|    /// List images
985|    Images,
986|    /// Show container logs (deduplicated)
987|    Logs { container: String },
988|    /// Docker Compose commands with compact output
989|    Compose {
990|        #[command(subcommand)]
991|        command: ComposeCommands,
992|    },
993|    /// Passthrough: runs any unsupported docker subcommand directly
994|    #[command(external_subcommand)]
995|    Other(Vec<OsString>),
996|}
997|
998|#[derive(Debug, Subcommand)]
999|enum ComposeCommands {
1000|    /// List compose services (compact)
1001|    Ps {
1002|        #[arg(short = 'a', long)]
1003|        all: bool,
1004|    },
1005|    /// Show compose logs (deduplicated)
1006|    Logs {
1007|        /// Optional service name
1008|        service: Option<String>,
1009|        /// Number of log lines to fetch
1010|        #[arg(long, default_value_t = 100)]
1011|        tail: u32,
1012|    },
1013|    /// Build compose services (summary)
1014|    Build {
1015|        /// Optional service name
1016|        service: Option<String>,
1017|    },
1018|    /// Passthrough: runs any unsupported compose subcommand directly
1019|    #[command(external_subcommand)]
1020|    Other(Vec<OsString>),
1021|}
1022|
1023|#[derive(Debug, Subcommand)]
1024|enum KubectlCommands {
1025|    /// Get Kubernetes resources (compact for pods/services)
1026|    Get {
1027|        /// kubectl get arguments
1028|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1029|        args: Vec<String>,
1030|    },
1031|    /// List pods
1032|    Pods {
1033|        #[arg(short, long)]
1034|        namespace: Option<String>,
1035|        /// All namespaces
1036|        #[arg(short = 'A', long)]
1037|        all: bool,
1038|    },
1039|    /// List services
1040|    Services {
1041|        #[arg(short, long)]
1042|        namespace: Option<String>,
1043|        /// All namespaces
1044|        #[arg(short = 'A', long)]
1045|        all: bool,
1046|    },
1047|    /// Show pod logs (deduplicated)
1048|    Logs {
1049|        pod: String,
1050|        #[arg(short, long)]
1051|        container: Option<String>,
1052|    },
1053|    /// Passthrough: runs any unsupported kubectl subcommand directly
1054|    #[command(external_subcommand)]
1055|    Other(Vec<OsString>),
1056|}
1057|
1058|#[derive(Debug, Subcommand)]
1059|enum PrismaCommands {
1060|    /// Generate Prisma Client (strip ASCII art)
1061|    Generate {
1062|        /// Additional prisma arguments
1063|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1064|        args: Vec<String>,
1065|    },
1066|    /// Manage migrations
1067|    Migrate {
1068|        #[command(subcommand)]
1069|        command: PrismaMigrateCommands,
1070|    },
1071|    /// Push schema to database
1072|    DbPush {
1073|        /// Additional prisma arguments
1074|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1075|        args: Vec<String>,
1076|    },
1077|}
1078|
1079|#[derive(Debug, Subcommand)]
1080|enum PrismaMigrateCommands {
1081|    /// Create and apply migration
1082|    Dev {
1083|        /// Migration name
1084|        #[arg(short, long)]
1085|        name: Option<String>,
1086|        /// Additional arguments
1087|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1088|        args: Vec<String>,
1089|    },
1090|    /// Check migration status
1091|    Status {
1092|        /// Additional arguments
1093|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1094|        args: Vec<String>,
1095|    },
1096|    /// Deploy migrations to production
1097|    Deploy {
1098|        /// Additional arguments
1099|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1100|        args: Vec<String>,
1101|    },
1102|}
1103|
1104|#[derive(Debug, Subcommand)]
1105|enum CargoCommands {
1106|    /// Build with compact output (strip Compiling lines, keep errors)
1107|    Build {
1108|        /// Additional cargo build arguments
1109|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1110|        args: Vec<String>,
1111|    },
1112|    /// Test with failures-only output
1113|    Test {
1114|        /// Additional cargo test arguments
1115|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1116|        args: Vec<String>,
1117|    },
1118|    /// Clippy with warnings grouped by lint rule
1119|    Clippy {
1120|        /// Additional cargo clippy arguments
1121|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1122|        args: Vec<String>,
1123|    },
1124|    /// Check with compact output (strip Checking lines, keep errors)
1125|    Check {
1126|        /// Additional cargo check arguments
1127|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1128|        args: Vec<String>,
1129|    },
1130|    /// Install with compact output (strip dep compilation, keep installed/errors)
1131|    Install {
1132|        /// Additional cargo install arguments
1133|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1134|        args: Vec<String>,
1135|    },
1136|    /// Nextest with failures-only output
1137|    Nextest {
1138|        /// Additional cargo nextest arguments (e.g., run, list, --lib)
1139|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1140|        args: Vec<String>,
1141|    },
1142|    /// Passthrough: runs any unsupported cargo subcommand directly
1143|    #[command(external_subcommand)]
1144|    Other(Vec<OsString>),
1145|}
1146|
1147|#[derive(Debug, Subcommand)]
1148|enum DotnetCommands {
1149|    /// Build with compact output
1150|    Build {
1151|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1152|        args: Vec<String>,
1153|    },
1154|    /// Test with compact output
1155|    Test {
1156|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1157|        args: Vec<String>,
1158|    },
1159|    /// Restore with compact output
1160|    Restore {
1161|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1162|        args: Vec<String>,
1163|    },
1164|    /// Format with compact output
1165|    Format {
1166|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1167|        args: Vec<String>,
1168|    },
1169|    /// Passthrough: runs any unsupported dotnet subcommand directly
1170|    #[command(external_subcommand)]
1171|    Other(Vec<OsString>),
1172|}
1173|
1174|#[derive(Debug, Subcommand)]
1175|enum GoCommands {
1176|    /// Run tests with compact output (90% token reduction via JSON streaming)
1177|    Test {
1178|        /// Additional go test arguments
1179|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1180|        args: Vec<String>,
1181|    },
1182|    /// Build with compact output (errors only)
1183|    Build {
1184|        /// Additional go build arguments
1185|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1186|        args: Vec<String>,
1187|    },
1188|    /// Vet with compact output
1189|    Vet {
1190|        /// Additional go vet arguments
1191|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1192|        args: Vec<String>,
1193|    },
1194|    /// Passthrough: runs any unsupported go subcommand directly
1195|    #[command(external_subcommand)]
1196|    Other(Vec<OsString>),
1197|}
1198|
1199|/// RTK-only subcommands that should never fall back to raw execution.
1200|/// If Clap fails to parse these, show the Clap error directly.
1201|const RTK_META_COMMANDS: &[&str] = &[
1202|    "gain",
1203|    "discover",
1204|    "learn",
1205|    "init",
1206|    "config",
1207|    "proxy",
1208|    "run",
1209|    "hook",
1210|    "hook-audit",
1211|    "pipe",
1212|    "cc-economics",
1213|    "verify",
1214|    "trust",
1215|    "untrust",
1216|    "session",
1217|    "rewrite",
1218|    "index",
1219|];
1220|
1221|fn print_custom_help() {
1222|    const VERSION: &str = env!("CARGO_PKG_VERSION");
1223|    const W: usize = 74; // content width (80 - 6 for border/padding)
1224|
1225|    // ANSI helpers
1226|    let bold = "\x1b[1m";
1227|    let dim = "\x1b[2m";
1228|    let reset = "\x1b[0m";
1229|
1230|    // ── Header ──
1231|    let title = format!("rtk v{}", VERSION);
1232|    let subtitle = "Reduce LLM token consumption by 60–90%";
1233|    let hr = "\u{2500}".repeat(W + 4); // horizontal rule
1234|    println!("  {bold}{title}{reset}", bold = bold, reset = reset);
1235|    println!("  {dim}{subtitle}{reset}", dim = dim, reset = reset);
1236|    println!("  {dim}{hr}{reset}", dim = dim, reset = reset);
1237|
1238|    // ── Usage ──
1239|    println!(
1240|        "\n  {bold}Usage:{reset}  rtk <command> [args...]",
1241|        bold = bold,
1242|        reset = reset
1243|    );
1244|
1245|    // ── Setup section ──
1246|    println!("\n  {bold}Setup{reset}", bold = bold, reset = reset);
1247|    println!("  {dim}●{reset} init       Install rtk hooks for AI assistants (Claude, Cursor, Hermes, Codex…)", dim = dim, reset = reset);
1248|    println!(
1249|        "  {dim}●{reset} gain       Show token savings: summary, history, daily/weekly charts",
1250|        dim = dim,
1251|        reset = reset
1252|    );
1253|    println!(
1254|        "  {dim}●{reset} config     Show or create rtk configuration file",
1255|        dim = dim,
1256|        reset = reset
1257|    );
1258|
1259|    // ── Commands by ecosystem ──
1260|    println!(
1261|        "\n  {bold}Commands by ecosystem{reset}",
1262|        bold = bold,
1263|        reset = reset
1264|    );
1265|
1266|    // Two-column layout: label (18 chars) | commands (wrapped)
1267|    let label_width = 18;
1268|    let cmd_width = W.saturating_sub(label_width + 2); // +2 for spacing
1269|
1270|    let groups = vec![
1271|        ("Version Control", vec!["git", "gh", "glab", "diff", "gt"]),
1272|        ("Rust/Cargo", vec!["cargo"]),
1273|        (
1274|            "JavaScript/TS",
1275|            vec![
1276|                "npm",
1277|                "npx",
1278|                "pnpm",
1279|                "vitest",
1280|                "jest",
1281|                "playwright",
1282|                "tsc",
1283|                "next",
1284|                "lint",
1285|                "prettier",
1286|                "prisma",
1287|                "format",
1288|            ],
1289|        ),
1290|        ("Python", vec!["pytest", "ruff", "mypy", "pip"]),
1291|        ("Ruby", vec!["rspec", "rubocop", "rake"]),
1292|        ("Go", vec!["go", "golangci-lint"]),
1293|        ("Dart/Flutter", vec!["flutter", "dart"]),
1294|        (".NET/Java", vec!["dotnet", "gradlew"]),
1295|        (
1296|            "Cloud/Infra",
1297|            vec!["docker", "kubectl", "aws", "psql", "curl", "wget"],
1298|        ),
1299|        (
1300|            "System",
1301|            vec![
1302|                "ls", "tree", "read", "find", "grep", "wc", "env", "deps", "log", "json", "pipe",
1303|                "err", "test", "summary", "smart",
1304|            ],
1305|        ),
1306|        (
1307|            "Other",
1308|            vec![
1309|                "discover",
1310|                "session",
1311|                "telemetry",
1312|                "index",
1313|                "learn",
1314|                "cc-economics",
1315|                "run",
1316|                "proxy",
1317|            ],
1318|        ),
1319|        (
1320|            "Hooks",
1321|            vec![
1322|                "hook",
1323|                "hook-audit",
1324|                "rewrite",
1325|                "verify",
1326|                "trust",
1327|                "untrust",
1328|            ],
1329|        ),
1330|    ];
1331|
1332|    for (label, cmds) in &groups {
1333|        let cmd_list = cmds.join(", ");
1334|        // Word-wrap the command list into multiple lines
1335|        let mut remaining = cmd_list.as_str();
1336|        let mut first = true;
1337|        loop {
1338|            if remaining.is_empty() {
1339|                break;
1340|            }
1341|            let prefix = if first {
1342|                format!("  {:<lw$}  ", label, lw = label_width)
1343|            } else {
1344|                format!("  {:<lw$}  ", "", lw = label_width)
1345|            };
1346|            let avail = cmd_width.saturating_sub(2); // account for prefix spacing tweak
1347|
1348|            if remaining.len() <= avail {
1349|                println!("{}{}", prefix, remaining);
1350|                break;
1351|            }
1352|            // Find last comma within available width
1353|            let slice = &remaining[..std::cmp::min(avail, remaining.len())];
1354|            let break_at = slice.rfind(", ").map(|p| p + 2).unwrap_or(avail);
1355|            println!("{}{}", prefix, &remaining[..break_at]);
1356|            remaining = remaining[break_at..].trim();
1357|            first = false;
1358|        }
1359|    }
1360|
1361|    // ── Options ──
1362|    println!("\n  {bold}Options{reset}", bold = bold, reset = reset);
1363|    println!("  -v, --verbose...      Verbosity level (-v, -vv, -vvv)");
1364|    println!("      --ultra-compact   Ultra-compact mode (Level 2 optimizations)");
1365|    println!("      --skip-env        Set SKIP_ENV_VALIDATION=1 for child processes");
1366|    println!("  -h, --help            Show this help");
1367|    println!("  -V, --version         Show version");
1368|
1369|    println!(
1370|        "\n  {dim}Type 'rtk <command> --help' for detailed options.{reset}",
1371|        dim = dim,
1372|        reset = reset
1373|    );
1374|}
1375|
1376|fn run_fallback(parse_error: clap::Error) -> Result<i32> {
1377|    let args: Vec<String> = std::env::args().skip(1).collect();
1378|
1379|    // No args → show Clap's error (user ran just "rtk" with bad syntax)
1380|    if args.is_empty() {
1381|        parse_error.exit();
1382|    }
1383|
1384|    // RTK meta-commands should never fall back to raw execution.
1385|    // e.g. `rtk gain --badtypo` should show Clap's error, not try to run `gain` from $PATH.
1386|    if RTK_META_COMMANDS.contains(&args[0].as_str()) {
1387|        parse_error.exit();
1388|    }
1389|
1390|    let raw_command = args.join(" ");
1391|    let error_message = core::utils::strip_ansi(&parse_error.to_string());
1392|
1393|    // Start timer before execution to capture actual command runtime
1394|    let timer = core::tracking::TimedExecution::start();
1395|
1396|    // TOML filter lookup — bypass with RTK_NO_TOML=1
1397|    // Use basename of args[0] so absolute paths (/usr/bin/make) still match "^make\b".
1398|    let lookup_cmd = {
1399|        let base = std::path::Path::new(&args[0])
1400|            .file_name()
1401|            .map(|n| n.to_string_lossy().into_owned())
1402|            .unwrap_or_else(|| args[0].clone());
1403|        std::iter::once(base.as_str())
1404|            .chain(args[1..].iter().map(|s| s.as_str()))
1405|            .collect::<Vec<_>>()
1406|            .join(" ")
1407|    };
1408|    let toml_match = if std::env::var("RTK_NO_TOML").ok().as_deref() == Some("1") {
1409|        None
1410|    } else {
1411|        core::toml_filter::find_matching_filter(&lookup_cmd)
1412|    };
1413|
1414|    if let Some(filter) = toml_match {
1415|        // TOML match: capture stdout for filtering
1416|        let result = if filter.filter_stderr {
1417|            // Merge stderr into stdout so the filter can strip banners emitted by tools like liquibase
1418|            core::utils::resolved_command(&args[0])
1419|                .args(&args[1..])
1420|                .stdin(std::process::Stdio::inherit())
1421|                .stdout(std::process::Stdio::piped())
1422|                .stderr(std::process::Stdio::piped()) // captured for merging
1423|                .output()
1424|        } else {
1425|            core::utils::resolved_command(&args[0])
1426|                .args(&args[1..])
1427|                .stdin(std::process::Stdio::inherit())
1428|                .stdout(std::process::Stdio::piped()) // capture
1429|                .stderr(std::process::Stdio::inherit()) // stderr always direct
1430|                .output()
1431|        };
1432|
1433|        match result {
1434|            Ok(output) => {
1435|                let exit_code = core::utils::exit_code_from_output(&output, &raw_command);
1436|                let stdout_raw = String::from_utf8_lossy(&output.stdout);
1437|                let stderr_raw = String::from_utf8_lossy(&output.stderr);
1438|
1439|                // Merge stderr into the text to filter when filter_stderr is enabled;
1440|                // otherwise emit stderr directly so it is always visible.
1441|                let combined_raw = if filter.filter_stderr {
1442|                    format!("{}{}", stdout_raw, stderr_raw)
1443|                } else {
1444|                    stdout_raw.to_string()
1445|                };
1446|                // Tee raw output BEFORE filtering on failure — lets LLM re-read if needed
1447|                let tee_hint = if !output.status.success() {
1448|                    core::tee::tee_and_hint(&combined_raw, &raw_command, exit_code)
1449|                } else {
1450|                    None
1451|                };
1452|
1453|                let filtered = core::toml_filter::apply_filter(filter, &combined_raw);
1454|                println!("{}", filtered);
1455|                if let Some(hint) = tee_hint {
1456|                    println!("{}", hint);
1457|                }
1458|
1459|                timer.track(
1460|                    &raw_command,
1461|                    &format!("rtk:toml {}", raw_command),
1462|                    &combined_raw,
1463|                    &filtered,
1464|                );
1465|                core::tracking::record_parse_failure_silent(&raw_command, &error_message, true);
1466|
1467|                Ok(exit_code)
1468|            }
1469|            Err(e) => {
1470|                // Command not found — same behaviour as no-TOML path
1471|                core::tracking::record_parse_failure_silent(&raw_command, &error_message, false);
1472|                eprintln!("[rtk: {}]", e);
1473|                Ok(127)
1474|            }
1475|        }
1476|    } else {
1477|        // No TOML match: capture output for potential tracking, then print
1478|        let result = core::utils::resolved_command(&args[0])
1479|            .args(&args[1..])
1480|            .stdin(std::process::Stdio::inherit())
1481|            .stdout(std::process::Stdio::piped())
1482|            .stderr(std::process::Stdio::piped())
1483|            .output();
1484|
1485|        match result {
1486|            Ok(output) => {
1487|                let stdout_str = String::from_utf8_lossy(&output.stdout);
1488|                let stderr_str = String::from_utf8_lossy(&output.stderr);
1489|
1490|                // Print captured output (preserve original visibility)
1491|                if !stdout_str.is_empty() {
1492|                    print!("{}", stdout_str);
1493|                }
1494|                if !stderr_str.is_empty() {
1495|                    eprint!("{}", stderr_str);
1496|                }
1497|
1498|                let combined = format!("{}{}", stdout_str, stderr_str);
1499|                let input_tokens = core::tracking::estimate_tokens(&combined);
1500|                let command_name = args.first().map(|s| s.as_str()).unwrap_or(&raw_command);
1501|                let elapsed_ms = timer.elapsed_ms();
1502|                core::tracking::record_potential_silent(command_name, input_tokens, elapsed_ms);
1503|
1504|                timer.track_passthrough(&raw_command, &format!("rtk fallback: {}", raw_command));
1505|                core::tracking::record_parse_failure_silent(&raw_command, &error_message, true);
1506|
1507|                Ok(core::utils::exit_code_from_output(&output, &raw_command))
1508|            }
1509|            Err(e) => {
1510|                let command_name2 = args.first().map(|s| s.as_str()).unwrap_or(&raw_command);
1511|                core::tracking::record_potential_silent(command_name2, 0, 0);
1512|                core::tracking::record_parse_failure_silent(&raw_command, &error_message, false);
1513|                eprintln!("[rtk: {}]", e);
1514|                Ok(127)
1515|            }
1516|        }
1517|    }
1518|}
1519|
1520|#[derive(Debug, Subcommand)]
1521|enum GtCommands {
1522|    /// Compact stack log output
1523|    Log {
1524|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1525|        args: Vec<String>,
1526|    },
1527|    /// Compact submit output
1528|    Submit {
1529|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1530|        args: Vec<String>,
1531|    },
1532|    /// Compact sync output
1533|    Sync {
1534|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1535|        args: Vec<String>,
1536|    },
1537|    /// Compact restack output
1538|    Restack {
1539|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1540|        args: Vec<String>,
1541|    },
1542|    /// Compact create output
1543|    Create {
1544|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1545|        args: Vec<String>,
1546|    },
1547|    /// Branch info and management
1548|    Branch {
1549|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1550|        args: Vec<String>,
1551|    },
1552|    /// Passthrough: git-passthrough detection or direct gt execution
1553|    #[command(external_subcommand)]
1554|    Other(Vec<OsString>),
1555|}
1556|
1557|/// Split a string into shell-like tokens, respecting single and double quotes.
1558|/// e.g. `git log --format="%H %s"` → ["git", "log", "--format=%H %s"]
1559|fn shell_split(input: &str) -> Vec<String> {
1560|    discover::lexer::shell_split(input)
1561|}
1562|
1563|/// Merge pnpm global filters args with other ones for standard String-based commands
1564|fn merge_pnpm_args(filters: &[String], args: &[String]) -> Vec<String> {
1565|    filters
1566|        .iter()
1567|        .map(|filter| format!("--filter={}", filter))
1568|        .chain(args.iter().cloned())
1569|        .collect()
1570|}
1571|
1572|/// Merge pnpm global filters args with other ones, using OsString for passthrough compatibility
1573|fn merge_pnpm_args_os(filters: &[String], args: &[OsString]) -> Vec<OsString> {
1574|    filters
1575|        .iter()
1576|        .map(|filter| OsString::from(format!("--filter={}", filter)))
1577|        .chain(args.iter().cloned())
1578|        .collect()
1579|}
1580|
1581|/// Validate that pnpm filters are only used in the global context, not before subcommands like tsc.
1582|fn validate_pnpm_filters(filters: &[String], command: &PnpmCommands) -> Option<String> {
1583|    // Check if this is a Build or Typecheck command with filters
1584|    match command {
1585|        PnpmCommands::Typecheck { .. } => {
1586|            // FIXME: if filters are present, we should find out which workspaces are selected before running rtk dedicated commands
1587|            if !filters.is_empty() {
1588|                let cmd_name = match command {
1589|                    PnpmCommands::Typecheck { .. } => "tsc",
1590|                    _ => unreachable!(),
1591|                };
1592|                let msg = format!(
1593|                    "[rtk] warning: --filter is not yet supported for pnpm {}, filters preceding the subcommand will be ignored",
1594|                    cmd_name
1595|                );
1596|                return Some(msg);
1597|            }
1598|            None
1599|        }
1600|        _ => None,
1601|    }
1602|}
1603|
1604|fn main() {
1605|    let code = match run_cli() {
1606|        Ok(code) => code,
1607|        Err(e) => {
1608|            eprintln!("rtk: {:#}", e);
1609|            1
1610|        }
1611|    };
1612|    std::process::exit(code);
1613|}
1614|
1615|fn uninstall_init_dispatch<UninstallHermes, UninstallKimi, UninstallStandard>(
1616|    agent: Option<AgentTarget>,
1617|    global: bool,
1618|    gemini: bool,
1619|    codex: bool,
1620|    ctx: hooks::init::InitContext,
1621|    uninstall_hermes: UninstallHermes,
1622|    uninstall_kimi: UninstallKimi,
1623|    uninstall_standard: UninstallStandard,
1624|) -> Result<()>
1625|where
1626|    UninstallHermes: FnOnce(hooks::init::InitContext) -> Result<()>,
1627|    UninstallKimi: FnOnce(hooks::init::InitContext) -> Result<()>,
1628|    UninstallStandard: FnOnce(bool, bool, bool, bool, bool, hooks::init::InitContext) -> Result<()>,
1629|{
1630|    if agent == Some(AgentTarget::Hermes) {
1631|        uninstall_hermes(ctx)
1632|    } else if agent == Some(AgentTarget::Kimi) {
1633|        uninstall_kimi(ctx)
1634|    } else {
1635|        let cursor = agent == Some(AgentTarget::Cursor);
1636|        let kimi = false;
1637|        uninstall_standard(global, gemini, codex, cursor, kimi, ctx)
1638|    }
1639|}
1640|
1641|fn run_cli() -> Result<i32> {
1642|    // Fire-and-forget telemetry ping (1/day, non-blocking)
1643|    core::telemetry::maybe_ping();
1644|
1645|    // Intercept bare --help/-h to show organized help by language/ecosystem
1646|    let args: Vec<String> = std::env::args().collect();
1647|    if args.len() == 2 && (args[1] == "--help" || args[1] == "-h") {
1648|        print_custom_help();
1649|        return Ok(0);
1650|    }
1651|
1652|    let cli = match Cli::try_parse() {
1653|        Ok(cli) => cli,
1654|        Err(e) => {
1655|            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
1656|                e.exit();
1657|            }
1658|            return run_fallback(e);
1659|        }
1660|    };
1661|
1662|    // Warn if installed hook is outdated/missing (1/day, non-blocking).
1663|    // Skip for Gain — it shows its own inline hook warning.
1664|    if !matches!(cli.command, Commands::Gain { .. }) {
1665|        hooks::hook_check::maybe_warn();
1666|    }
1667|
1668|    // Runtime integrity check for operational commands.
1669|    // Meta commands (init, gain, verify, config, etc.) skip the check
1670|    // because they don't go through the hook pipeline.
1671|    if is_operational_command(&cli.command) {
1672|        hooks::integrity::runtime_check()?;
1673|    }
1674|
1675|    let code = match cli.command {
1676|        Commands::Ls { args } => ls::run(&args, cli.verbose)?,
1677|
1678|        Commands::Tree { args } => tree::run(&args, cli.verbose)?,
1679|
1680|        // ISSUE #989: support multiple files (cat file1 file2 → rtk read file1 file2)
1681|        Commands::Read {
1682|            files,
1683|            level,
1684|            max_lines,
1685|            tail_lines,
1686|            line_numbers,
1687|        } => {
1688|            let mut had_error = false;
1689|            let mut stdin_seen = false;
1690|            for file in &files {
1691|                let result = if file == Path::new("-") {
1692|                    if stdin_seen {
1693|                        eprintln!("rtk: warning: stdin specified more than once");
1694|                        continue;
1695|                    }
1696|                    stdin_seen = true;
1697|                    read::run_stdin(level, max_lines, tail_lines, line_numbers, cli.verbose)
1698|                } else {
1699|                    read::run(
1700|                        file,
1701|                        level,
1702|                        max_lines,
1703|                        tail_lines,
1704|                        line_numbers,
1705|                        cli.verbose,
1706|                    )
1707|                };
1708|                if let Err(e) = result {
1709|                    eprintln!("cat: {}: {}", file.display(), e.root_cause());
1710|                    had_error = true;
1711|                }
1712|            }
1713|            if had_error {
1714|                1
1715|            } else {
1716|                0
1717|            }
1718|        }
1719|
1720|        Commands::Smart {
1721|            file,
1722|            model,
1723|            force_download,
1724|        } => {
1725|            local_llm::run(&file, &model, force_download, cli.verbose)?;
1726|            0
1727|        }
1728|
1729|        Commands::Git {
1730|            directory,
1731|            config_override,
1732|            git_dir,
1733|            work_tree,
1734|            no_pager,
1735|            no_optional_locks,
1736|            bare,
1737|            literal_pathspecs,
1738|            command,
1739|        } => {
1740|            // Build global git args (inserted between "git" and subcommand)
1741|            let mut global_args: Vec<String> = Vec::new();
1742|            for dir in &directory {
1743|                global_args.push("-C".to_string());
1744|                global_args.push(dir.clone());
1745|            }
1746|            for cfg in &config_override {
1747|                global_args.push("-c".to_string());
1748|                global_args.push(cfg.clone());
1749|            }
1750|            if let Some(ref dir) = git_dir {
1751|                global_args.push("--git-dir".to_string());
1752|                global_args.push(dir.clone());
1753|            }
1754|            if let Some(ref tree) = work_tree {
1755|                global_args.push("--work-tree".to_string());
1756|                global_args.push(tree.clone());
1757|            }
1758|            if no_pager {
1759|                global_args.push("--no-pager".to_string());
1760|            }
1761|            if no_optional_locks {
1762|                global_args.push("--no-optional-locks".to_string());
1763|            }
1764|            if bare {
1765|                global_args.push("--bare".to_string());
1766|            }
1767|            if literal_pathspecs {
1768|                global_args.push("--literal-pathspecs".to_string());
1769|            }
1770|
1771|            match command {
1772|                GitCommands::Diff { args } => git::run(
1773|                    git::GitCommand::Diff,
1774|                    &args,
1775|                    None,
1776|                    cli.verbose,
1777|                    &global_args,
1778|                )?,
1779|                GitCommands::Log { args } => {
1780|                    git::run(git::GitCommand::Log, &args, None, cli.verbose, &global_args)?
1781|                }
1782|                GitCommands::Status { args } => git::run(
1783|                    git::GitCommand::Status,
1784|                    &args,
1785|                    None,
1786|                    cli.verbose,
1787|                    &global_args,
1788|                )?,
1789|                GitCommands::Show { args } => git::run(
1790|                    git::GitCommand::Show,
1791|                    &args,
1792|                    None,
1793|                    cli.verbose,
1794|                    &global_args,
1795|                )?,
1796|                GitCommands::Add { args } => {
1797|                    git::run(git::GitCommand::Add, &args, None, cli.verbose, &global_args)?
1798|                }
1799|                GitCommands::Commit { args } => git::run(
1800|                    git::GitCommand::Commit,
1801|                    &args,
1802|                    None,
1803|                    cli.verbose,
1804|                    &global_args,
1805|                )?,
1806|                GitCommands::Push { args } => git::run(
1807|                    git::GitCommand::Push,
1808|                    &args,
1809|                    None,
1810|                    cli.verbose,
1811|                    &global_args,
1812|                )?,
1813|                GitCommands::Pull { args } => git::run(
1814|                    git::GitCommand::Pull,
1815|                    &args,
1816|                    None,
1817|                    cli.verbose,
1818|                    &global_args,
1819|                )?,
1820|                GitCommands::Branch { args } => git::run(
1821|                    git::GitCommand::Branch,
1822|                    &args,
1823|                    None,
1824|                    cli.verbose,
1825|                    &global_args,
1826|                )?,
1827|                GitCommands::Fetch { args } => git::run(
1828|                    git::GitCommand::Fetch,
1829|                    &args,
1830|                    None,
1831|                    cli.verbose,
1832|                    &global_args,
1833|                )?,
1834|                GitCommands::Stash { subcommand, args } => git::run(
1835|                    git::GitCommand::Stash { subcommand },
1836|                    &args,
1837|                    None,
1838|                    cli.verbose,
1839|                    &global_args,
1840|                )?,
1841|                GitCommands::Worktree { args } => git::run(
1842|                    git::GitCommand::Worktree,
1843|                    &args,
1844|                    None,
1845|                    cli.verbose,
1846|                    &global_args,
1847|                )?,
1848|                GitCommands::Other(args) => git::run_passthrough(&args, &global_args, cli.verbose)?,
1849|            }
1850|        }
1851|
1852|        Commands::Gh { subcommand, args } => {
1853|            gh_cmd::run(&subcommand, &args, cli.verbose, cli.ultra_compact)?
1854|        }
1855|
1856|        Commands::Glab {
1857|            repo,
1858|            group,
1859|            subcommand,
1860|            mut args,
1861|        } => {
1862|            // Append -R / -g flags at end so they don't interfere with
1863|            // subcommand dispatch (args[0] must be the sub-subcommand like "list")
1864|            if let Some(r) = repo {
1865|                args.push("-R".to_string());
1866|                args.push(r);
1867|            }
1868|            if let Some(g) = group {
1869|                args.push("-g".to_string());
1870|                args.push(g);
1871|            }
1872|            glab_cmd::run(&subcommand, &args, cli.verbose, cli.ultra_compact)?
1873|        }
1874|
1875|        Commands::Aws { subcommand, args } => aws_cmd::run(&subcommand, &args, cli.verbose)?,
1876|
1877|        Commands::Psql { args } => psql_cmd::run(&args, cli.verbose)?,
1878|
1879|        Commands::Flutter { args } => flutter_cmd::run(&args, cli.verbose)?,
1880|
1881|        Commands::Dart { args } => dart_cmd::run(&args, cli.verbose)?,
1882|
1883|        Commands::Pnpm { filter, command } => {
1884|            // Warns user if filters are used with unsupported subcommands like typecheck
1885|            if let Some(warning) = validate_pnpm_filters(&filter, &command) {
1886|                eprintln!("{}", warning);
1887|            }
1888|
1889|            match command {
1890|                PnpmCommands::List { depth, args } => pnpm_cmd::run(
1891|                    pnpm_cmd::PnpmCommand::List { depth },
1892|                    &merge_pnpm_args(&filter, &args),
1893|                    cli.verbose,
1894|                )?,
1895|                PnpmCommands::Outdated { args } => pnpm_cmd::run(
1896|                    pnpm_cmd::PnpmCommand::Outdated,
1897|                    &merge_pnpm_args(&filter, &args),
1898|                    cli.verbose,
1899|                )?,
1900|                PnpmCommands::Install { args } => pnpm_cmd::run(
1901|                    pnpm_cmd::PnpmCommand::Install,
1902|                    &merge_pnpm_args(&filter, &args),
1903|                    cli.verbose,
1904|                )?,
1905|                PnpmCommands::Typecheck { args } => tsc_cmd::run(&args, cli.verbose)?,
1906|                PnpmCommands::Other(args) => {
1907|                    pnpm_cmd::run_passthrough(&merge_pnpm_args_os(&filter, &args), cli.verbose)?
1908|                }
1909|            }
1910|        }
1911|
1912|        Commands::Err { command } => {
1913|            let cmd = command.join(" ");
1914|            runner::run_err(&cmd, cli.verbose)?
1915|        }
1916|
1917|        Commands::Test { command } => {
1918|            let cmd = command.join(" ");
1919|            runner::run_test(&cmd, cli.verbose)?
1920|        }
1921|
1922|        Commands::Json {
1923|            file,
1924|            depth,
1925|            keys_only,
1926|        } => {
1927|            if file == Path::new("-") {
1928|                json_cmd::run_stdin(depth, keys_only, cli.verbose)?;
1929|            } else {
1930|                json_cmd::run(&file, depth, keys_only, cli.verbose)?;
1931|            }
1932|            0
1933|        }
1934|
1935|        Commands::Deps { path } => {
1936|            deps::run(&path, cli.verbose)?;
1937|            0
1938|        }
1939|
1940|        Commands::Env { filter, show_all } => {
1941|            env_cmd::run(filter.as_deref(), show_all, cli.verbose)?;
1942|            0
1943|        }
1944|
1945|        Commands::Find { args } => {
1946|            find_cmd::run_from_args(&args, cli.verbose)?;
1947|            0
1948|        }
1949|
1950|        Commands::Diff { file1, file2 } => {
1951|            if let Some(f2) = file2 {
1952|                diff_cmd::run(&file1, &f2, cli.verbose)?;
1953|            } else {
1954|                diff_cmd::run_stdin(cli.verbose)?;
1955|            }
1956|            0
1957|        }
1958|
1959|        Commands::Log { file } => {
1960|            if let Some(f) = file {
1961|                log_cmd::run_file(&f, cli.verbose)?;
1962|            } else {
1963|                log_cmd::run_stdin(cli.verbose)?;
1964|            }
1965|            0
1966|        }
1967|
1968|        Commands::Dotnet { command } => match command {
1969|            DotnetCommands::Build { args } => dotnet_cmd::run_build(&args, cli.verbose)?,
1970|            DotnetCommands::Test { args } => dotnet_cmd::run_test(&args, cli.verbose)?,
1971|            DotnetCommands::Restore { args } => dotnet_cmd::run_restore(&args, cli.verbose)?,
1972|            DotnetCommands::Format { args } => dotnet_cmd::run_format(&args, cli.verbose)?,
1973|            DotnetCommands::Other(args) => dotnet_cmd::run_passthrough(&args, cli.verbose)?,
1974|        },
1975|
1976|        Commands::Docker { command } => match command {
1977|            DockerCommands::Ps { all } => {
1978|                let cmd = if all {
1979|                    container::ContainerCmd::DockerPsAll
1980|                } else {
1981|                    container::ContainerCmd::DockerPs
1982|                };
1983|                container::run(cmd, &[], cli.verbose)?
1984|            }
1985|            DockerCommands::Images => {
1986|                container::run(container::ContainerCmd::DockerImages, &[], cli.verbose)?
1987|            }
1988|            DockerCommands::Logs { container: c } => {
1989|                container::run(container::ContainerCmd::DockerLogs, &[c], cli.verbose)?
1990|            }
1991|            DockerCommands::Compose { command: compose } => match compose {
1992|                ComposeCommands::Ps { all } => container::run_compose_ps(all, cli.verbose)?,
1993|                ComposeCommands::Logs { service, tail } => {
1994|                    container::run_compose_logs(service.as_deref(), tail, cli.verbose)?
1995|                }
1996|                ComposeCommands::Build { service } => {
1997|                    container::run_compose_build(service.as_deref(), cli.verbose)?
1998|                }
1999|                ComposeCommands::Other(args) => {
2000|                    container::run_compose_passthrough(&args, cli.verbose)?
2001|