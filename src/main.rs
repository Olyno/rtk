1|<<<<<<< HEAD
2|=======
3|mod analytics;
4|mod cmds;
5|mod core;
6|mod discover;
7|mod hooks;
8|mod learn;
9|mod parser;
10|
11|// Re-export command modules for routing
12|use cmds::cloud::{aws_cmd, container, curl_cmd, psql_cmd, wget_cmd};
13|use cmds::dotnet::{binlog, dotnet_cmd, dotnet_format_report, dotnet_trx};
14|use cmds::git::{diff_cmd, gh_cmd, git, glab_cmd, gt_cmd};
15|use cmds::go::{go_cmd, golangci_cmd};
16|use cmds::js::{
17|    lint_cmd, next_cmd, npm_cmd, playwright_cmd, pnpm_cmd, prettier_cmd, prisma_cmd, tsc_cmd,
18|    vitest_cmd,
19|};
20|use cmds::python::{mypy_cmd, pip_cmd, pytest_cmd, ruff_cmd};
21|use cmds::ruby::{rake_cmd, rspec_cmd, rubocop_cmd};
22|use cmds::rust::{cargo_cmd, runner};
23|use cmds::system::{
24|    deps, env_cmd, find_cmd, format_cmd, grep_cmd, json_cmd, local_llm, log_cmd, ls, pipe_cmd,
25|    read, summary, tree, wc_cmd,
26|};
27|
28|use anyhow::{Context, Result};
29|use clap::error::ErrorKind;
30|use clap::{Parser, Subcommand, ValueEnum};
31|use std::ffi::OsString;
32|use std::path::{Path, PathBuf};
33|
34|/// Target agent for hook installation.
35|#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
36|pub enum AgentTarget {
37|    /// Claude Code (default)
38|    Claude,
39|    /// Cursor Agent (editor and CLI)
40|    Cursor,
41|    /// Windsurf IDE (Cascade)
42|    Windsurf,
43|    /// Cline / Roo Code (VS Code)
44|    Cline,
45|    /// Kilo Code
46|    Kilocode,
47|    /// Google Antigravity
48|    Antigravity,
49|    /// Pi coding agent
50|    Pi,
51|}
52|
53|#[derive(Parser)]
54|#[command(
55|    name = "rtk",
56|    version,
57|    about = "Rust Token Killer - Minimize LLM token consumption",
58|    long_about = "A high-performance CLI proxy designed to filter and summarize system outputs before they reach your LLM context."
59|)]
60|struct Cli {
61|    #[command(subcommand)]
62|    command: Commands,
63|
64|    /// Verbosity level (-v, -vv, -vvv)
65|    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
66|    verbose: u8,
67|
68|    /// Ultra-compact mode: ASCII icons, inline format (Level 2 optimizations)
69|    #[arg(long, global = true)]
70|    ultra_compact: bool,
71|
72|    /// Set SKIP_ENV_VALIDATION=1 for child processes (Next.js, tsc, lint, prisma)
73|    #[arg(long = "skip-env", global = true)]
74|    skip_env: bool,
75|}
76|
77|#[derive(Debug, Subcommand)]
78|enum Commands {
79|    /// List directory contents with token-optimized output (proxy to native ls)
80|    Ls {
81|        /// Arguments passed to ls (supports all native ls flags like -l, -a, -h, -R)
82|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
83|        args: Vec<String>,
84|    },
85|
86|    /// Directory tree with token-optimized output (proxy to native tree)
87|    Tree {
88|        /// Arguments passed to tree (supports all native tree flags like -L, -d, -a)
89|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
90|        args: Vec<String>,
91|    },
92|
93|    /// Read file with intelligent filtering
94|    Read {
95|        /// Files to read (supports multiple, like cat)
96|        #[arg(required = true, num_args = 1..)]
97|        files: Vec<PathBuf>,
98|        /// Filter: none (default, full content), minimal, aggressive
99|        #[arg(short, long, default_value = "none")]
100|        level: core::filter::FilterLevel,
101|        /// Max lines
102|        #[arg(short, long, conflicts_with = "tail_lines")]
103|        max_lines: Option<usize>,
104|        /// Keep only last N lines
105|        #[arg(long, conflicts_with = "max_lines")]
106|        tail_lines: Option<usize>,
107|        /// Show line numbers
108|        #[arg(short = 'n', long)]
109|        line_numbers: bool,
110|    },
111|
112|    /// Generate 2-line technical summary (heuristic-based)
113|    Smart {
114|        /// File to analyze
115|        file: PathBuf,
116|        /// Model: heuristic
117|        #[arg(short, long, default_value = "heuristic")]
118|        model: String,
119|        /// Force model download
120|        #[arg(long)]
121|        force_download: bool,
122|    },
123|
124|    /// Git commands with compact output
125|    Git {
126|        /// Change to directory before executing (like git -C <path>, can be repeated)
127|        #[arg(short = 'C', action = clap::ArgAction::Append)]
128|        directory: Vec<String>,
129|
130|        /// Git configuration override (like git -c key=value, can be repeated)
131|        #[arg(short = 'c', action = clap::ArgAction::Append)]
132|        config_override: Vec<String>,
133|
134|        /// Set the path to the .git directory
135|        #[arg(long = "git-dir")]
136|        git_dir: Option<String>,
137|
138|        /// Set the path to the working tree
139|        #[arg(long = "work-tree")]
140|        work_tree: Option<String>,
141|
142|        /// Disable pager (like git --no-pager)
143|        #[arg(long = "no-pager")]
144|        no_pager: bool,
145|
146|        /// Skip optional locks (like git --no-optional-locks)
147|        #[arg(long = "no-optional-locks")]
148|        no_optional_locks: bool,
149|
150|        /// Treat repository as bare (like git --bare)
151|        #[arg(long)]
152|        bare: bool,
153|
154|        /// Treat pathspecs literally (like git --literal-pathspecs)
155|        #[arg(long = "literal-pathspecs")]
156|        literal_pathspecs: bool,
157|
158|        #[command(subcommand)]
159|        command: GitCommands,
160|    },
161|
162|    /// GitHub CLI (gh) commands with token-optimized output
163|    Gh {
164|        /// Subcommand: pr, issue, run, repo
165|        subcommand: String,
166|        /// Additional arguments
167|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
168|        args: Vec<String>,
169|    },
170|
171|    /// GitLab CLI (glab) commands with token-optimized output
172|    Glab {
173|        /// Target repository (owner/repo), passed as glab -R flag
174|        #[arg(short = 'R', long = "repo")]
175|        repo: Option<String>,
176|        /// Target group, passed as glab -g flag
177|        #[arg(short = 'g', long = "group")]
178|        group: Option<String>,
179|        /// Subcommand: mr, issue, ci, pipeline, api
180|        subcommand: String,
181|        /// Additional arguments
182|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
183|        args: Vec<String>,
184|    },
185|
186|    /// AWS CLI with compact output (force JSON, compress)
187|    Aws {
188|        /// AWS service subcommand (e.g., sts, s3, ec2, ecs, rds, cloudformation)
189|        subcommand: String,
190|        /// Additional arguments
191|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
192|        args: Vec<String>,
193|    },
194|
195|    /// PostgreSQL client with compact output (strip borders, compress tables)
196|    #[command(disable_help_flag = true)]
197|    Psql {
198|        /// psql arguments
199|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
200|        args: Vec<String>,
201|    },
202|
203|    /// pnpm commands with ultra-compact output
204|    Pnpm {
205|        /// pnpm filter arguments (can be repeated: --filter @app1 --filter @app2)
206|        #[arg(long, short = 'F')]
207|        filter: Vec<String>,
208|
209|        #[command(subcommand)]
210|        command: PnpmCommands,
211|    },
212|
213|    /// Run command and show only errors/warnings
214|    Err {
215|        /// Command to run
216|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
217|        command: Vec<String>,
218|    },
219|
220|    /// Run tests and show only failures
221|    Test {
222|        /// Test command (e.g. cargo test)
223|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
224|        command: Vec<String>,
225|    },
226|
227|    /// Show JSON (compact values by default, or keys-only with --keys-only)
228|    Json {
229|        /// JSON file
230|        file: PathBuf,
231|        /// Max depth
232|        #[arg(short, long, default_value = "5")]
233|        depth: usize,
234|        /// Show keys only (strip all values, show structure)
235|        #[arg(long)]
236|        keys_only: bool,
237|    },
238|
239|    /// Summarize project dependencies
240|    Deps {
241|        /// Project path
242|        #[arg(default_value = ".")]
243|        path: PathBuf,
244|    },
245|
246|    /// Show environment variables (filtered, sensitive masked)
247|    Env {
248|        /// Filter by name (e.g. PATH, AWS)
249|        #[arg(short, long)]
250|        filter: Option<String>,
251|        /// Show all (include sensitive)
252|        #[arg(long)]
253|        show_all: bool,
254|    },
255|
256|    /// Find files with compact tree output (accepts native find flags like -name, -type)
257|    Find {
258|        /// All find arguments (supports both RTK and native find syntax)
259|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
260|        args: Vec<String>,
261|    },
262|
263|    /// Ultra-condensed diff (only changed lines)
264|    Diff {
265|        /// First file or - for stdin (unified diff)
266|        file1: PathBuf,
267|        /// Second file (optional if stdin)
268|        file2: Option<PathBuf>,
269|    },
270|
271|    /// Filter and deduplicate log output
272|    Log {
273|        /// Log file (omit for stdin)
274|        file: Option<PathBuf>,
275|    },
276|
277|    /// .NET commands with compact output (build/test/restore/format)
278|    Dotnet {
279|        #[command(subcommand)]
280|        command: DotnetCommands,
281|    },
282|
283|    /// Docker commands with compact output
284|    Docker {
285|        #[command(subcommand)]
286|        command: DockerCommands,
287|    },
288|
289|    /// Kubectl commands with compact output
290|    Kubectl {
291|        #[command(subcommand)]
292|        command: KubectlCommands,
293|    },
294|
295|    /// Run command and show heuristic summary
296|    Summary {
297|        /// Command to run and summarize
298|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
299|        command: Vec<String>,
300|    },
301|
302|    /// Compact grep - strips whitespace, truncates, groups by file
303|    Grep {
304|        /// Pattern to search
305|        pattern: String,
306|        /// Path to search in
307|        #[arg(default_value = ".")]
308|        path: String,
309|        /// Max line length
310|        #[arg(short = 'l', long, default_value = "80")]
311|        max_len: usize,
312|        /// Max results to show
313|        #[arg(short, long, default_value = "200")]
314|        max: usize,
315|        /// Show only match context (not full line)
316|        #[arg(long)]
317|        context_only: bool,
318|        /// Filter by file type (e.g., ts, py, rust)
319|        #[arg(short = 't', long)]
320|        file_type: Option<String>,
321|        /// Show line numbers (always on, accepted for grep/rg compatibility)
322|        #[arg(short = 'n', long)]
323|        line_numbers: bool,
324|        /// Extra ripgrep arguments (e.g., -i, -A 3, -w, --glob)
325|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
326|        extra_args: Vec<String>,
327|    },
328|
329|    /// Initialize rtk instructions for assistant CLI usage
330|    Init {
331|        /// Add to global assistant config directory instead of local project file
332|        #[arg(short, long)]
333|        global: bool,
334|
335|        /// Install OpenCode plugin (in addition to Claude Code)
336|        #[arg(long)]
337|        opencode: bool,
338|
339|        /// Initialize for Gemini CLI instead of Claude Code
340|        #[arg(long)]
341|        gemini: bool,
342|
343|        /// Target agent to install hooks for (default: claude)
344|        #[arg(long, value_enum)]
345|        agent: Option<AgentTarget>,
346|
347|        /// Show current configuration
348|        #[arg(long)]
349|        show: bool,
350|
351|        /// Inject full instructions into CLAUDE.md (legacy mode)
352|        #[arg(long = "claude-md", group = "mode")]
353|        claude_md: bool,
354|
355|        /// Hook only, no RTK.md
356|        #[arg(long = "hook-only", group = "mode")]
357|        hook_only: bool,
358|
359|        /// Auto-patch settings.json without prompting
360|        #[arg(long = "auto-patch", group = "patch")]
361|        auto_patch: bool,
362|
363|        /// Skip settings.json patching (print manual instructions)
364|        #[arg(long = "no-patch", group = "patch")]
365|        no_patch: bool,
366|
367|        /// Remove RTK artifacts for the selected assistant mode
368|        #[arg(long)]
369|        uninstall: bool,
370|
371|        /// Target Codex CLI (uses AGENTS.md + RTK.md, no Claude hook patching)
372|        #[arg(long)]
373|        codex: bool,
374|
375|        /// Install GitHub Copilot integration (VS Code + CLI)
376|        #[arg(long)]
377|        copilot: bool,
378|
379|        /// Install Pi coding agent extension
380|        #[arg(long)]
381|        pi: bool,
382|    },
383|
384|    /// Download with compact output (strips progress bars)
385|    Wget {
386|        /// URL to download
387|        url: String,
388|        /// Output file (-O - for stdout)
389|        #[arg(short = 'O', long = "output-document", allow_hyphen_values = true)]
390|        output: Option<String>,
391|        /// Additional wget arguments
392|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
393|        args: Vec<String>,
394|    },
395|
396|    /// Word/line/byte count with compact output (strips paths and padding)
397|    Wc {
398|        /// Arguments passed to wc (files, flags like -l, -w, -c)
399|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
400|        args: Vec<String>,
401|    },
402|
403|    /// Show token savings summary and history
404|    Gain {
405|        /// Filter statistics to current project (current working directory) // added
406|        #[arg(short, long)]
407|        project: bool,
408|        /// Show ASCII graph of daily savings
409|        #[arg(short, long)]
410|        graph: bool,
411|        /// Show recent command history
412|        #[arg(short = 'H', long)]
413|        history: bool,
414|        /// Show monthly quota savings estimate
415|        #[arg(short, long)]
416|        quota: bool,
417|        /// Subscription tier for quota calculation: pro, 5x, 20x
418|        #[arg(short, long, default_value = "20x", requires = "quota")]
419|        tier: String,
420|        /// Show detailed daily breakdown (all days)
421|        #[arg(short, long)]
422|        daily: bool,
423|        /// Show weekly breakdown
424|        #[arg(short, long)]
425|        weekly: bool,
426|        /// Show monthly breakdown
427|        #[arg(short, long)]
428|        monthly: bool,
429|        /// Show all time breakdowns (daily + weekly + monthly)
430|        #[arg(short, long)]
431|        all: bool,
432|        /// Output format: text, json, csv
433|        #[arg(short, long, default_value = "text")]
434|        format: String,
435|        /// Show parse failure log (commands that fell back to raw execution)
436|        #[arg(short = 'F', long)]
437|        failures: bool,
438|        /// Reset all token savings stats to zero
439|        #[arg(long)]
440|        reset: bool,
441|        /// Skip confirmation prompt when resetting
442|        #[arg(long, requires = "reset")]
443|        yes: bool,
444|    },
445|
446|    /// Claude Code economics: spending (ccusage) vs savings (rtk) analysis
447|    CcEconomics {
448|        /// Show detailed daily breakdown
449|        #[arg(short, long)]
450|        daily: bool,
451|        /// Show weekly breakdown
452|        #[arg(short, long)]
453|        weekly: bool,
454|        /// Show monthly breakdown
455|        #[arg(short, long)]
456|        monthly: bool,
457|        /// Show all time breakdowns (daily + weekly + monthly)
458|        #[arg(short, long)]
459|        all: bool,
460|        /// Output format: text, json, csv
461|        #[arg(short, long, default_value = "text")]
462|        format: String,
463|    },
464|
465|    /// Show or create configuration file
466|    Config {
467|        /// Create default config file
468|        #[arg(long)]
469|        create: bool,
470|    },
471|
472|    /// Jest commands with compact output
473|    Jest {
474|        /// Additional jest arguments
475|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
476|        args: Vec<String>,
477|    },
478|
479|    /// Vitest commands with compact output
480|    Vitest {
481|        /// Additional vitest arguments
482|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
483|        args: Vec<String>,
484|    },
485|
486|    /// Prisma commands with compact output (no ASCII art)
487|    Prisma {
488|        #[command(subcommand)]
489|        command: PrismaCommands,
490|    },
491|
492|    /// TypeScript compiler with grouped error output
493|    Tsc {
494|        /// TypeScript compiler arguments
495|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
496|        args: Vec<String>,
497|    },
498|
499|    /// Next.js build with compact output
500|    Next {
501|        /// Next.js build arguments
502|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
503|        args: Vec<String>,
504|    },
505|
506|    /// ESLint with grouped rule violations
507|    Lint {
508|        /// Linter arguments
509|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
510|        args: Vec<String>,
511|    },
512|
513|    /// Prettier format checker with compact output
514|    Prettier {
515|        /// Prettier arguments (e.g., --check, --write)
516|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
517|        args: Vec<String>,
518|    },
519|
520|    /// Universal format checker (prettier, black, ruff format)
521|    Format {
522|        /// Formatter arguments (auto-detects formatter from project files)
523|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
524|        args: Vec<String>,
525|    },
526|
527|    /// Playwright E2E tests with compact output
528|    Playwright {
529|        /// Playwright arguments
530|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
531|        args: Vec<String>,
532|    },
533|
534|    /// Cargo commands with compact output
535|    Cargo {
536|        #[command(subcommand)]
537|        command: CargoCommands,
538|    },
539|
540|    /// npm run with filtered output (strip boilerplate)
541|    Npm {
542|        /// npm run arguments (script name + options)
543|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
544|        args: Vec<String>,
545|    },
546|
547|    /// npx with intelligent routing (tsc, eslint, prisma -> specialized filters)
548|    Npx {
549|        /// npx arguments (command + options)
550|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
551|        args: Vec<String>,
552|    },
553|
554|    /// Curl with auto-JSON detection and schema output
555|    Curl {
556|        /// Curl arguments (URL + options)
557|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
558|        args: Vec<String>,
559|    },
560|
561|    /// Discover missed RTK savings from Claude Code history
562|    Discover {
563|        /// Filter by project path (substring match)
564|        #[arg(short, long)]
565|        project: Option<String>,
566|        /// Max commands per section
567|        #[arg(short, long, default_value = "15")]
568|        limit: usize,
569|        /// Scan all projects (default: current project only)
570|        #[arg(short, long)]
571|        all: bool,
572|        /// Limit to sessions from last N days
573|        #[arg(short, long, default_value = "30")]
574|        since: u64,
575|        /// Output format: text, json
576|        #[arg(short, long, default_value = "text")]
577|        format: String,
578|    },
579|
580|    /// Show RTK adoption across Claude Code sessions
581|    Session {},
582|
583|    /// Manage telemetry consent and data (RGPD/GDPR)
584|    Telemetry {
585|        #[command(subcommand)]
586|        command: core::telemetry_cmd::TelemetrySubcommand,
587|    },
588|
589|    /// Learn CLI corrections from Claude Code error history
590|    Learn {
591|        /// Filter by project path (substring match)
592|        #[arg(short, long)]
593|        project: Option<String>,
594|        /// Scan all projects (default: current project only)
595|        #[arg(short, long)]
596|        all: bool,
597|        /// Limit to sessions from last N days
598|        #[arg(short, long, default_value = "30")]
599|        since: u64,
600|        /// Output format: text, json
601|        #[arg(short, long, default_value = "text")]
602|        format: String,
603|        /// Generate .claude/rules/cli-corrections.md file
604|        #[arg(short, long)]
605|        write_rules: bool,
606|        /// Minimum confidence threshold (0.0-1.0)
607|        #[arg(long, default_value = "0.6")]
608|        min_confidence: f64,
609|        /// Minimum occurrences to include in report
610|        #[arg(long, default_value = "1")]
611|        min_occurrences: usize,
612|    },
613|
614|    /// Execute a shell command via sh -c (raw, no filtering or tracking)
615|    Run {
616|        /// Command string to execute (use -c for shell-like invocation)
617|        #[arg(short = 'c', long = "command")]
618|        command: Option<String>,
619|        /// Positional command arguments (alternative to -c)
620|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
621|        args: Vec<String>,
622|    },
623|
624|    /// Execute command without filtering but track usage
625|    Proxy {
626|        /// Command and arguments to execute
627|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
628|        args: Vec<OsString>,
629|    },
630|
631|    /// Read stdin, apply filter, print filtered output (Unix pipe mode)
632|    Pipe {
633|        /// Filter name (cargo-test, pytest, grep, find, git-log, etc.)
634|        #[arg(short, long)]
635|        filter: Option<String>,
636|
637|        /// Pass stdin through without filtering
638|        #[arg(long)]
639|        passthrough: bool,
640|    },
641|
642|    /// Trust project-local TOML filters in current directory
643|    Trust {
644|        /// List all trusted projects
645|        #[arg(long)]
646|        list: bool,
647|    },
648|
649|    /// Revoke trust for project-local TOML filters
650|    Untrust,
651|
652|    /// Verify hook integrity and run TOML filter inline tests
653|    Verify {
654|        /// Run tests only for this filter name
655|        #[arg(long)]
656|        filter: Option<String>,
657|        /// Fail if any filter has no inline tests (CI mode)
658|        #[arg(long)]
659|        require_all: bool,
660|    },
661|
662|    /// Ruff linter/formatter with compact output
663|    Ruff {
664|        /// Ruff arguments (e.g., check, format --check)
665|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
666|        args: Vec<String>,
667|    },
668|
669|    /// Pytest test runner with compact output
670|    Pytest {
671|        /// Pytest arguments
672|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
673|        args: Vec<String>,
674|    },
675|
676|    /// Mypy type checker with grouped error output
677|    Mypy {
678|        /// Mypy arguments
679|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
680|        args: Vec<String>,
681|    },
682|
683|    /// Rake/Rails test with compact Minitest output (Ruby)
684|    Rake {
685|        /// Rake arguments (e.g., test, test TEST=path/to/test.rb)
686|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
687|        args: Vec<String>,
688|    },
689|
690|    /// RuboCop linter with compact output (Ruby)
691|    Rubocop {
692|        /// RuboCop arguments (e.g., --auto-correct, -A)
693|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
694|        args: Vec<String>,
695|    },
696|
697|    /// RSpec test runner with compact output (Rails/Ruby)
698|    Rspec {
699|        /// RSpec arguments (e.g., spec/models, --tag focus)
700|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
701|        args: Vec<String>,
702|    },
703|
704|    /// Pip package manager with compact output (auto-detects uv)
705|    Pip {
706|        /// Pip arguments (e.g., list, outdated, install)
707|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
708|        args: Vec<String>,
709|    },
710|
711|    /// Go commands with compact output
712|    Go {
713|        #[command(subcommand)]
714|        command: GoCommands,
715|    },
716|
717|    /// Graphite (gt) stacked PR commands with compact output
718|    Gt {
719|        #[command(subcommand)]
720|        command: GtCommands,
721|    },
722|
723|    /// golangci-lint wrapper with compact `run` support and passthrough for other invocations
724|    #[command(name = "golangci-lint")]
725|    GolangciLint {
726|        /// Additional golangci-lint arguments
727|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
728|        args: Vec<String>,
729|    },
730|
731|    /// Show hook rewrite audit metrics (requires RTK_HOOK_AUDIT=1)
732|    #[command(name = "hook-audit")]
733|    HookAudit {
734|        /// Show entries from last N days (0 = all time)
735|        #[arg(short, long, default_value = "7")]
736|        since: u64,
737|    },
738|
739|    /// Rewrite a raw command to its RTK equivalent (single source of truth for hooks)
740|    ///
741|    /// Exits 0 and prints the rewritten command if supported.
742|    /// Exits 1 with no output if the command has no RTK equivalent.
743|    ///
744|    /// Used by Claude Code, Gemini CLI, and other LLM hooks:
745|    ///   REWRITTEN=$(rtk rewrite "$CMD") || exit 0
746|    Rewrite {
747|        /// Raw command to rewrite (e.g. "git status", "cargo test && git push")
748|        /// Accepts multiple args: `rtk rewrite ls -al` is equivalent to `rtk rewrite "ls -al"`
749|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
750|        args: Vec<String>,
751|    },
752|
753|    /// Hook processors for LLM CLI tools (Gemini CLI, Copilot, etc.)
754|    Hook {
755|        #[command(subcommand)]
756|        command: HookCommands,
757|    },
758|}
759|
760|#[derive(Debug, Subcommand)]
761|enum HookCommands {
762|    /// Process Claude Code PreToolUse hook (reads JSON from stdin)
763|    Claude,
764|    /// Process Cursor Agent hook (reads JSON from stdin)
765|    Cursor,
766|    /// Process Gemini CLI BeforeTool hook (reads JSON from stdin)
767|    Gemini,
768|    /// Process Copilot preToolUse hook (VS Code + Copilot CLI, reads JSON from stdin)
769|    Copilot,
770|    /// Check how a command would be rewritten by the hook engine (dry-run)
771|    Check {
772|        /// Target agent
773|        #[arg(long, default_value = "claude")]
774|        agent: String,
775|        /// Command to check
776|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
777|        command: Vec<String>,
778|    },
779|}
780|
781|#[derive(Debug, Subcommand)]
782|enum GitCommands {
783|    /// Condensed diff output
784|    Diff {
785|        /// Git arguments (supports all git diff flags like --stat, --cached, etc)
786|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
787|        args: Vec<String>,
788|    },
789|    /// One-line commit history
790|    Log {
791|        /// Git arguments (supports all git log flags like --oneline, --graph, --all)
792|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
793|        args: Vec<String>,
794|    },
795|    /// Compact status (supports all git status flags)
796|    Status {
797|        /// Git arguments (supports all git status flags like --porcelain, --short, -s)
798|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
799|        args: Vec<String>,
800|    },
801|    /// Compact show (commit summary + stat + compacted diff)
802|    Show {
803|        /// Git arguments (supports all git show flags)
804|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
805|        args: Vec<String>,
806|    },
807|    /// Add files → "ok"
808|    Add {
809|        /// Files and flags to add (supports all git add flags like -A, -p, --all, etc)
810|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
811|        args: Vec<String>,
812|    },
813|    /// Commit → "ok \<hash\>"
814|    Commit {
815|        /// Git commit arguments (supports -a, -m, --amend, --allow-empty, etc)
816|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
817|        args: Vec<String>,
818|    },
819|    /// Push → "ok \<branch\>"
820|    Push {
821|        /// Git push arguments (supports -u, remote, branch, etc.)
822|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
823|        args: Vec<String>,
824|    },
825|    /// Pull → "ok \<stats\>"
826|    Pull {
827|        /// Git pull arguments (supports --rebase, remote, branch, etc.)
828|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
829|        args: Vec<String>,
830|    },
831|    /// Compact branch listing (current/local/remote)
832|    Branch {
833|        /// Git branch arguments (supports -d, -D, -m, etc.)
834|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
835|        args: Vec<String>,
836|    },
837|    /// Fetch → "ok fetched (N new refs)"
838|    Fetch {
839|        /// Git fetch arguments
840|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
841|        args: Vec<String>,
842|    },
843|    /// Stash management (list, show, pop, apply, drop)
844|    Stash {
845|        /// Subcommand: list, show, pop, apply, drop, push
846|        subcommand: Option<String>,
847|        /// Additional arguments
848|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
849|        args: Vec<String>,
850|    },
851|    /// Compact worktree listing
852|    Worktree {
853|        /// Git worktree arguments (add, remove, prune, or empty for list)
854|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
855|        args: Vec<String>,
856|    },
857|    /// Passthrough: runs any unsupported git subcommand directly
858|    #[command(external_subcommand)]
859|    Other(Vec<OsString>),
860|}
861|
862|#[derive(Debug, Subcommand)]
863|enum PnpmCommands {
864|    /// List installed packages (ultra-dense)
865|    List {
866|        /// Depth level (default: 0)
867|        #[arg(short, long, default_value = "0")]
868|        depth: usize,
869|        /// Additional pnpm arguments
870|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
871|        args: Vec<String>,
872|    },
873|    /// Show outdated packages (condensed: "pkg: old → new")
874|    Outdated {
875|        /// Additional pnpm arguments
876|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
877|        args: Vec<String>,
878|    },
879|    /// Install packages (filter progress bars)
880|    Install {
881|        /// Additional pnpm arguments
882|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
883|        args: Vec<String>,
884|    },
885|    /// Typecheck (delegates to tsc filter)
886|    Typecheck {
887|        /// Additional typecheck arguments
888|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
889|        args: Vec<String>,
890|    },
891|    /// Passthrough: runs any unsupported pnpm subcommand directly
892|    #[command(external_subcommand)]
893|    Other(Vec<OsString>),
894|}
895|
896|#[derive(Debug, Subcommand)]
897|enum DockerCommands {
898|    /// List running containers
899|    Ps,
900|    /// List images
901|    Images,
902|    /// Show container logs (deduplicated)
903|    Logs { container: String },
904|    /// Docker Compose commands with compact output
905|    Compose {
906|        #[command(subcommand)]
907|        command: ComposeCommands,
908|    },
909|    /// Passthrough: runs any unsupported docker subcommand directly
910|    #[command(external_subcommand)]
911|    Other(Vec<OsString>),
912|}
913|
914|#[derive(Debug, Subcommand)]
915|enum ComposeCommands {
916|    /// List compose services (compact)
917|    Ps,
918|    /// Show compose logs (deduplicated)
919|    Logs {
920|        /// Optional service name
921|        service: Option<String>,
922|    },
923|    /// Build compose services (summary)
924|    Build {
925|        /// Optional service name
926|        service: Option<String>,
927|    },
928|    /// Passthrough: runs any unsupported compose subcommand directly
929|    #[command(external_subcommand)]
930|    Other(Vec<OsString>),
931|}
932|
933|#[derive(Debug, Subcommand)]
934|enum KubectlCommands {
935|    /// List pods
936|    Pods {
937|        #[arg(short, long)]
938|        namespace: Option<String>,
939|        /// All namespaces
940|        #[arg(short = 'A', long)]
941|        all: bool,
942|    },
943|    /// List services
944|    Services {
945|        #[arg(short, long)]
946|        namespace: Option<String>,
947|        /// All namespaces
948|        #[arg(short = 'A', long)]
949|        all: bool,
950|    },
951|    /// Show pod logs (deduplicated)
952|    Logs {
953|        pod: String,
954|        #[arg(short, long)]
955|        container: Option<String>,
956|    },
957|    /// Passthrough: runs any unsupported kubectl subcommand directly
958|    #[command(external_subcommand)]
959|    Other(Vec<OsString>),
960|}
961|
962|#[derive(Debug, Subcommand)]
963|enum PrismaCommands {
964|    /// Generate Prisma Client (strip ASCII art)
965|    Generate {
966|        /// Additional prisma arguments
967|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
968|        args: Vec<String>,
969|    },
970|    /// Manage migrations
971|    Migrate {
972|        #[command(subcommand)]
973|        command: PrismaMigrateCommands,
974|    },
975|    /// Push schema to database
976|    DbPush {
977|        /// Additional prisma arguments
978|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
979|        args: Vec<String>,
980|    },
981|}
982|
983|#[derive(Debug, Subcommand)]
984|enum PrismaMigrateCommands {
985|    /// Create and apply migration
986|    Dev {
987|        /// Migration name
988|        #[arg(short, long)]
989|        name: Option<String>,
990|        /// Additional arguments
991|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
992|        args: Vec<String>,
993|    },
994|    /// Check migration status
995|    Status {
996|        /// Additional arguments
997|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
998|        args: Vec<String>,
999|    },
1000|    /// Deploy migrations to production
1001|    Deploy {
1002|        /// Additional arguments
1003|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1004|        args: Vec<String>,
1005|    },
1006|}
1007|
1008|#[derive(Debug, Subcommand)]
1009|enum CargoCommands {
1010|    /// Build with compact output (strip Compiling lines, keep errors)
1011|    Build {
1012|        /// Additional cargo build arguments
1013|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1014|        args: Vec<String>,
1015|    },
1016|    /// Test with failures-only output
1017|    Test {
1018|        /// Additional cargo test arguments
1019|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1020|        args: Vec<String>,
1021|    },
1022|    /// Clippy with warnings grouped by lint rule
1023|    Clippy {
1024|        /// Additional cargo clippy arguments
1025|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1026|        args: Vec<String>,
1027|    },
1028|    /// Check with compact output (strip Checking lines, keep errors)
1029|    Check {
1030|        /// Additional cargo check arguments
1031|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1032|        args: Vec<String>,
1033|    },
1034|    /// Install with compact output (strip dep compilation, keep installed/errors)
1035|    Install {
1036|        /// Additional cargo install arguments
1037|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1038|        args: Vec<String>,
1039|    },
1040|    /// Nextest with failures-only output
1041|    Nextest {
1042|        /// Additional cargo nextest arguments (e.g., run, list, --lib)
1043|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1044|        args: Vec<String>,
1045|    },
1046|    /// Passthrough: runs any unsupported cargo subcommand directly
1047|    #[command(external_subcommand)]
1048|    Other(Vec<OsString>),
1049|}
1050|
1051|#[derive(Debug, Subcommand)]
1052|enum DotnetCommands {
1053|    /// Build with compact output
1054|    Build {
1055|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1056|        args: Vec<String>,
1057|    },
1058|    /// Test with compact output
1059|    Test {
1060|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1061|        args: Vec<String>,
1062|    },
1063|    /// Restore with compact output
1064|    Restore {
1065|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1066|        args: Vec<String>,
1067|    },
1068|    /// Format with compact output
1069|    Format {
1070|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1071|        args: Vec<String>,
1072|    },
1073|    /// Passthrough: runs any unsupported dotnet subcommand directly
1074|    #[command(external_subcommand)]
1075|    Other(Vec<OsString>),
1076|}
1077|
1078|#[derive(Debug, Subcommand)]
1079|enum GoCommands {
1080|    /// Run tests with compact output (90% token reduction via JSON streaming)
1081|    Test {
1082|        /// Additional go test arguments
1083|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1084|        args: Vec<String>,
1085|    },
1086|    /// Build with compact output (errors only)
1087|    Build {
1088|        /// Additional go build arguments
1089|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1090|        args: Vec<String>,
1091|    },
1092|    /// Vet with compact output
1093|    Vet {
1094|        /// Additional go vet arguments
1095|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1096|        args: Vec<String>,
1097|    },
1098|    /// Passthrough: runs any unsupported go subcommand directly
1099|    #[command(external_subcommand)]
1100|    Other(Vec<OsString>),
1101|}
1102|
1103|/// RTK-only subcommands that should never fall back to raw execution.
1104|/// If Clap fails to parse these, show the Clap error directly.
1105|const RTK_META_COMMANDS: &[&str] = &[
1106|    "gain",
1107|    "discover",
1108|    "learn",
1109|    "init",
1110|    "config",
1111|    "proxy",
1112|    "run",
1113|    "hook",
1114|    "hook-audit",
1115|    "pipe",
1116|    "cc-economics",
1117|    "verify",
1118|    "trust",
1119|    "untrust",
1120|    "session",
1121|    "rewrite",
1122|];
1123|
1124|fn run_fallback(parse_error: clap::Error) -> Result<i32> {
1125|    let args: Vec<String> = std::env::args().skip(1).collect();
1126|
1127|    // No args → show Clap's error (user ran just "rtk" with bad syntax)
1128|    if args.is_empty() {
1129|        parse_error.exit();
1130|    }
1131|
1132|    // RTK meta-commands should never fall back to raw execution.
1133|    // e.g. `rtk gain --badtypo` should show Clap's error, not try to run `gain` from $PATH.
1134|    if RTK_META_COMMANDS.contains(&args[0].as_str()) {
1135|        parse_error.exit();
1136|    }
1137|
1138|    let raw_command = args.join(" ");
1139|    let error_message = core::utils::strip_ansi(&parse_error.to_string());
1140|
1141|    // Start timer before execution to capture actual command runtime
1142|    let timer = core::tracking::TimedExecution::start();
1143|
1144|    // TOML filter lookup — bypass with RTK_NO_TOML=1
1145|    // Use basename of args[0] so absolute paths (/usr/bin/make) still match "^make\b".
1146|    let lookup_cmd = {
1147|        let base = std::path::Path::new(&args[0])
1148|            .file_name()
1149|            .map(|n| n.to_string_lossy().into_owned())
1150|            .unwrap_or_else(|| args[0].clone());
1151|        std::iter::once(base.as_str())
1152|            .chain(args[1..].iter().map(|s| s.as_str()))
1153|            .collect::<Vec<_>>()
1154|            .join(" ")
1155|    };
1156|    let toml_match = if std::env::var("RTK_NO_TOML").ok().as_deref() == Some("1") {
1157|        None
1158|    } else {
1159|        core::toml_filter::find_matching_filter(&lookup_cmd)
1160|    };
1161|
1162|    if let Some(filter) = toml_match {
1163|        // TOML match: capture stdout for filtering
1164|        let result = if filter.filter_stderr {
1165|            // Merge stderr into stdout so the filter can strip banners emitted by tools like liquibase
1166|            core::utils::resolved_command(&args[0])
1167|                .args(&args[1..])
1168|                .stdin(std::process::Stdio::inherit())
1169|                .stdout(std::process::Stdio::piped())
1170|                .stderr(std::process::Stdio::piped()) // captured for merging
1171|                .output()
1172|        } else {
1173|            core::utils::resolved_command(&args[0])
1174|                .args(&args[1..])
1175|                .stdin(std::process::Stdio::inherit())
1176|                .stdout(std::process::Stdio::piped()) // capture
1177|                .stderr(std::process::Stdio::inherit()) // stderr always direct
1178|                .output()
1179|        };
1180|
1181|        match result {
1182|            Ok(output) => {
1183|                let exit_code = core::utils::exit_code_from_output(&output, &raw_command);
1184|                let stdout_raw = String::from_utf8_lossy(&output.stdout);
1185|                let stderr_raw = String::from_utf8_lossy(&output.stderr);
1186|
1187|                // Merge stderr into the text to filter when filter_stderr is enabled;
1188|                // otherwise emit stderr directly so it is always visible.
1189|                let combined_raw = if filter.filter_stderr {
1190|                    format!("{}{}", stdout_raw, stderr_raw)
1191|                } else {
1192|                    stdout_raw.to_string()
1193|                };
1194|                // Tee raw output BEFORE filtering on failure — lets LLM re-read if needed
1195|                let tee_hint = if !output.status.success() {
1196|                    core::tee::tee_and_hint(&combined_raw, &raw_command, exit_code)
1197|                } else {
1198|                    None
1199|                };
1200|
1201|                let filtered = core::toml_filter::apply_filter(filter, &combined_raw);
1202|                println!("{}", filtered);
1203|                if let Some(hint) = tee_hint {
1204|                    println!("{}", hint);
1205|                }
1206|
1207|                timer.track(
1208|                    &raw_command,
1209|                    &format!("rtk:toml {}", raw_command),
1210|                    &combined_raw,
1211|                    &filtered,
1212|                );
1213|                core::tracking::record_parse_failure_silent(&raw_command, &error_message, true);
1214|
1215|                Ok(exit_code)
1216|            }
1217|            Err(e) => {
1218|                // Command not found — same behaviour as no-TOML path
1219|                core::tracking::record_parse_failure_silent(&raw_command, &error_message, false);
1220|                eprintln!("[rtk: {}]", e);
1221|                Ok(127)
1222|            }
1223|        }
1224|    } else {
1225|        // No TOML match: original passthrough behaviour (Stdio::inherit, streaming)
1226|        let status = core::utils::resolved_command(&args[0])
1227|            .args(&args[1..])
1228|            .stdin(std::process::Stdio::inherit())
1229|            .stdout(std::process::Stdio::inherit())
1230|            .stderr(std::process::Stdio::inherit())
1231|            .status();
1232|
1233|        match status {
1234|            Ok(s) => {
1235|                timer.track_passthrough(&raw_command, &format!("rtk fallback: {}", raw_command));
1236|
1237|                core::tracking::record_parse_failure_silent(&raw_command, &error_message, true);
1238|
1239|                Ok(core::utils::exit_code_from_status(&s, &raw_command))
1240|            }
1241|            Err(e) => {
1242|                core::tracking::record_parse_failure_silent(&raw_command, &error_message, false);
1243|                // Command not found or other OS error — single message, no duplicate Clap error
1244|                eprintln!("[rtk: {}]", e);
1245|                Ok(127)
1246|            }
1247|        }
1248|    }
1249|}
1250|
1251|#[derive(Debug, Subcommand)]
1252|enum GtCommands {
1253|    /// Compact stack log output
1254|    Log {
1255|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1256|        args: Vec<String>,
1257|    },
1258|    /// Compact submit output
1259|    Submit {
1260|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1261|        args: Vec<String>,
1262|    },
1263|    /// Compact sync output
1264|    Sync {
1265|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1266|        args: Vec<String>,
1267|    },
1268|    /// Compact restack output
1269|    Restack {
1270|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1271|        args: Vec<String>,
1272|    },
1273|    /// Compact create output
1274|    Create {
1275|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1276|        args: Vec<String>,
1277|    },
1278|    /// Branch info and management
1279|    Branch {
1280|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
1281|        args: Vec<String>,
1282|    },
1283|    /// Passthrough: git-passthrough detection or direct gt execution
1284|    #[command(external_subcommand)]
1285|    Other(Vec<OsString>),
1286|}
1287|
1288|/// Split a string into shell-like tokens, respecting single and double quotes.
1289|/// e.g. `git log --format="%H %s"` → ["git", "log", "--format=%H %s"]
1290|fn shell_split(input: &str) -> Vec<String> {
1291|    discover::lexer::shell_split(input)
1292|}
1293|
1294|/// Merge pnpm global filters args with other ones for standard String-based commands
1295|fn merge_pnpm_args(filters: &[String], args: &[String]) -> Vec<String> {
1296|    filters
1297|        .iter()
1298|        .map(|filter| format!("--filter={}", filter))
1299|        .chain(args.iter().cloned())
1300|        .collect()
1301|}
1302|
1303|/// Merge pnpm global filters args with other ones, using OsString for passthrough compatibility
1304|fn merge_pnpm_args_os(filters: &[String], args: &[OsString]) -> Vec<OsString> {
1305|    filters
1306|        .iter()
1307|        .map(|filter| OsString::from(format!("--filter={}", filter)))
1308|        .chain(args.iter().cloned())
1309|        .collect()
1310|}
1311|
1312|/// Validate that pnpm filters are only used in the global context, not before subcommands like tsc.
1313|fn validate_pnpm_filters(filters: &[String], command: &PnpmCommands) -> Option<String> {
1314|    // Check if this is a Build or Typecheck command with filters
1315|    match command {
1316|        PnpmCommands::Typecheck { .. } => {
1317|            // FIXME: if filters are present, we should find out which workspaces are selected before running rtk dedicated commands
1318|            if !filters.is_empty() {
1319|                let cmd_name = match command {
1320|                    PnpmCommands::Typecheck { .. } => "tsc",
1321|                    _ => unreachable!(),
1322|                };
1323|                let msg = format!(
1324|                    "[rtk] warning: --filter is not yet supported for pnpm {}, filters preceding the subcommand will be ignored",
1325|                    cmd_name
1326|                );
1327|                return Some(msg);
1328|            }
1329|            None
1330|        }
1331|        _ => None,
1332|    }
1333|}
1334|
1335|fn main() {
1336|    let code = match run_cli() {
1337|        Ok(code) => code,
1338|        Err(e) => {
1339|            eprintln!("rtk: {:#}", e);
1340|            1
1341|        }
1342|    };
1343|    std::process::exit(code);
1344|}
1345|
1346|fn run_cli() -> Result<i32> {
1347|    // Fire-and-forget telemetry ping (1/day, non-blocking)
1348|    core::telemetry::maybe_ping();
1349|
1350|    let cli = match Cli::try_parse() {
1351|        Ok(cli) => cli,
1352|        Err(e) => {
1353|            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
1354|                e.exit();
1355|            }
1356|            return run_fallback(e);
1357|        }
1358|    };
1359|
1360|    // Warn if installed hook is outdated/missing (1/day, non-blocking).
1361|    // Skip for Gain — it shows its own inline hook warning.
1362|    if !matches!(cli.command, Commands::Gain { .. }) {
1363|        hooks::hook_check::maybe_warn();
1364|    }
1365|
1366|    // Runtime integrity check for operational commands.
1367|    // Meta commands (init, gain, verify, config, etc.) skip the check
1368|    // because they don't go through the hook pipeline.
1369|    if is_operational_command(&cli.command) {
1370|        hooks::integrity::runtime_check()?;
1371|    }
1372|
1373|    let code = match cli.command {
1374|        Commands::Ls { args } => ls::run(&args, cli.verbose)?,
1375|
1376|        Commands::Tree { args } => tree::run(&args, cli.verbose)?,
1377|
1378|        // ISSUE #989: support multiple files (cat file1 file2 → rtk read file1 file2)
1379|        Commands::Read {
1380|            files,
1381|            level,
1382|            max_lines,
1383|            tail_lines,
1384|            line_numbers,
1385|        } => {
1386|            let mut had_error = false;
1387|            let mut stdin_seen = false;
1388|            for file in &files {
1389|                let result = if file == Path::new("-") {
1390|                    if stdin_seen {
1391|                        eprintln!("rtk: warning: stdin specified more than once");
1392|                        continue;
1393|                    }
1394|                    stdin_seen = true;
1395|                    read::run_stdin(level, max_lines, tail_lines, line_numbers, cli.verbose)
1396|                } else {
1397|                    read::run(
1398|                        file,
1399|                        level,
1400|                        max_lines,
1401|                        tail_lines,
1402|                        line_numbers,
1403|                        cli.verbose,
1404|                    )
1405|                };
1406|                if let Err(e) = result {
1407|                    eprintln!("cat: {}: {}", file.display(), e.root_cause());
1408|                    had_error = true;
1409|                }
1410|            }
1411|            if had_error {
1412|                1
1413|            } else {
1414|                0
1415|            }
1416|        }
1417|
1418|        Commands::Smart {
1419|            file,
1420|            model,
1421|            force_download,
1422|        } => {
1423|            local_llm::run(&file, &model, force_download, cli.verbose)?;
1424|            0
1425|        }
1426|
1427|        Commands::Git {
1428|            directory,
1429|            config_override,
1430|            git_dir,
1431|            work_tree,
1432|            no_pager,
1433|            no_optional_locks,
1434|            bare,
1435|            literal_pathspecs,
1436|            command,
1437|        } => {
1438|            // Build global git args (inserted between "git" and subcommand)
1439|            let mut global_args: Vec<String> = Vec::new();
1440|            for dir in &directory {
1441|                global_args.push("-C".to_string());
1442|                global_args.push(dir.clone());
1443|            }
1444|            for cfg in &config_override {
1445|                global_args.push("-c".to_string());
1446|                global_args.push(cfg.clone());
1447|            }
1448|            if let Some(ref dir) = git_dir {
1449|                global_args.push("--git-dir".to_string());
1450|                global_args.push(dir.clone());
1451|            }
1452|            if let Some(ref tree) = work_tree {
1453|                global_args.push("--work-tree".to_string());
1454|                global_args.push(tree.clone());
1455|            }
1456|            if no_pager {
1457|                global_args.push("--no-pager".to_string());
1458|            }
1459|            if no_optional_locks {
1460|                global_args.push("--no-optional-locks".to_string());
1461|            }
1462|            if bare {
1463|                global_args.push("--bare".to_string());
1464|            }
1465|            if literal_pathspecs {
1466|                global_args.push("--literal-pathspecs".to_string());
1467|            }
1468|
1469|            match command {
1470|                GitCommands::Diff { args } => git::run(
1471|                    git::GitCommand::Diff,
1472|                    &args,
1473|                    None,
1474|                    cli.verbose,
1475|                    &global_args,
1476|                )?,
1477|                GitCommands::Log { args } => {
1478|                    git::run(git::GitCommand::Log, &args, None, cli.verbose, &global_args)?
1479|                }
1480|                GitCommands::Status { args } => git::run(
1481|                    git::GitCommand::Status,
1482|                    &args,
1483|                    None,
1484|                    cli.verbose,
1485|                    &global_args,
1486|                )?,
1487|                GitCommands::Show { args } => git::run(
1488|                    git::GitCommand::Show,
1489|                    &args,
1490|                    None,
1491|                    cli.verbose,
1492|                    &global_args,
1493|                )?,
1494|                GitCommands::Add { args } => {
1495|                    git::run(git::GitCommand::Add, &args, None, cli.verbose, &global_args)?
1496|                }
1497|                GitCommands::Commit { args } => git::run(
1498|                    git::GitCommand::Commit,
1499|                    &args,
1500|                    None,
1501|                    cli.verbose,
1502|                    &global_args,
1503|                )?,
1504|                GitCommands::Push { args } => git::run(
1505|                    git::GitCommand::Push,
1506|                    &args,
1507|                    None,
1508|                    cli.verbose,
1509|                    &global_args,
1510|                )?,
1511|                GitCommands::Pull { args } => git::run(
1512|                    git::GitCommand::Pull,
1513|                    &args,
1514|                    None,
1515|                    cli.verbose,
1516|                    &global_args,
1517|                )?,
1518|                GitCommands::Branch { args } => git::run(
1519|                    git::GitCommand::Branch,
1520|                    &args,
1521|                    None,
1522|                    cli.verbose,
1523|                    &global_args,
1524|                )?,
1525|                GitCommands::Fetch { args } => git::run(
1526|                    git::GitCommand::Fetch,
1527|                    &args,
1528|                    None,
1529|                    cli.verbose,
1530|                    &global_args,
1531|                )?,
1532|                GitCommands::Stash { subcommand, args } => git::run(
1533|                    git::GitCommand::Stash { subcommand },
1534|                    &args,
1535|                    None,
1536|                    cli.verbose,
1537|                    &global_args,
1538|                )?,
1539|                GitCommands::Worktree { args } => git::run(
1540|                    git::GitCommand::Worktree,
1541|                    &args,
1542|                    None,
1543|                    cli.verbose,
1544|                    &global_args,
1545|                )?,
1546|                GitCommands::Other(args) => git::run_passthrough(&args, &global_args, cli.verbose)?,
1547|            }
1548|        }
1549|
1550|        Commands::Gh { subcommand, args } => {
1551|            gh_cmd::run(&subcommand, &args, cli.verbose, cli.ultra_compact)?
1552|        }
1553|
1554|        Commands::Glab {
1555|            repo,
1556|            group,
1557|            subcommand,
1558|            mut args,
1559|        } => {
1560|            // Append -R / -g flags at end so they don't interfere with
1561|            // subcommand dispatch (args[0] must be the sub-subcommand like "list")
1562|            if let Some(r) = repo {
1563|                args.push("-R".to_string());
1564|                args.push(r);
1565|            }
1566|            if let Some(g) = group {
1567|                args.push("-g".to_string());
1568|                args.push(g);
1569|            }
1570|            glab_cmd::run(&subcommand, &args, cli.verbose, cli.ultra_compact)?
1571|        }
1572|
1573|        Commands::Aws { subcommand, args } => aws_cmd::run(&subcommand, &args, cli.verbose)?,
1574|
1575|        Commands::Psql { args } => psql_cmd::run(&args, cli.verbose)?,
1576|
1577|        Commands::Pnpm { filter, command } => {
1578|            // Warns user if filters are used with unsupported subcommands like typecheck
1579|            if let Some(warning) = validate_pnpm_filters(&filter, &command) {
1580|                eprintln!("{}", warning);
1581|            }
1582|
1583|            match command {
1584|                PnpmCommands::List { depth, args } => pnpm_cmd::run(
1585|                    pnpm_cmd::PnpmCommand::List { depth },
1586|                    &merge_pnpm_args(&filter, &args),
1587|                    cli.verbose,
1588|                )?,
1589|                PnpmCommands::Outdated { args } => pnpm_cmd::run(
1590|                    pnpm_cmd::PnpmCommand::Outdated,
1591|                    &merge_pnpm_args(&filter, &args),
1592|                    cli.verbose,
1593|                )?,
1594|                PnpmCommands::Install { args } => pnpm_cmd::run(
1595|                    pnpm_cmd::PnpmCommand::Install,
1596|                    &merge_pnpm_args(&filter, &args),
1597|                    cli.verbose,
1598|                )?,
1599|                PnpmCommands::Typecheck { args } => tsc_cmd::run(&args, cli.verbose)?,
1600|                PnpmCommands::Other(args) => {
1601|                    pnpm_cmd::run_passthrough(&merge_pnpm_args_os(&filter, &args), cli.verbose)?
1602|                }
1603|            }
1604|        }
1605|
1606|        Commands::Err { command } => {
1607|            let cmd = command.join(" ");
1608|            runner::run_err(&cmd, cli.verbose)?
1609|        }
1610|
1611|        Commands::Test { command } => {
1612|            let cmd = command.join(" ");
1613|            runner::run_test(&cmd, cli.verbose)?
1614|        }
1615|
1616|        Commands::Json {
1617|            file,
1618|            depth,
1619|            keys_only,
1620|        } => {
1621|            if file == Path::new("-") {
1622|                json_cmd::run_stdin(depth, keys_only, cli.verbose)?;
1623|            } else {
1624|                json_cmd::run(&file, depth, keys_only, cli.verbose)?;
1625|            }
1626|            0
1627|        }
1628|
1629|        Commands::Deps { path } => {
1630|            deps::run(&path, cli.verbose)?;
1631|            0
1632|        }
1633|
1634|        Commands::Env { filter, show_all } => {
1635|            env_cmd::run(filter.as_deref(), show_all, cli.verbose)?;
1636|            0
1637|        }
1638|
1639|        Commands::Find { args } => {
1640|            find_cmd::run_from_args(&args, cli.verbose)?;
1641|            0
1642|        }
1643|
1644|        Commands::Diff { file1, file2 } => {
1645|            if let Some(f2) = file2 {
1646|                diff_cmd::run(&file1, &f2, cli.verbose)?;
1647|            } else {
1648|                diff_cmd::run_stdin(cli.verbose)?;
1649|            }
1650|            0
1651|        }
1652|
1653|        Commands::Log { file } => {
1654|            if let Some(f) = file {
1655|                log_cmd::run_file(&f, cli.verbose)?;
1656|            } else {
1657|                log_cmd::run_stdin(cli.verbose)?;
1658|            }
1659|            0
1660|        }
1661|
1662|        Commands::Dotnet { command } => match command {
1663|            DotnetCommands::Build { args } => dotnet_cmd::run_build(&args, cli.verbose)?,
1664|            DotnetCommands::Test { args } => dotnet_cmd::run_test(&args, cli.verbose)?,
1665|            DotnetCommands::Restore { args } => dotnet_cmd::run_restore(&args, cli.verbose)?,
1666|            DotnetCommands::Format { args } => dotnet_cmd::run_format(&args, cli.verbose)?,
1667|            DotnetCommands::Other(args) => dotnet_cmd::run_passthrough(&args, cli.verbose)?,
1668|        },
1669|
1670|        Commands::Docker { command } => match command {
1671|            DockerCommands::Ps => {
1672|                container::run(container::ContainerCmd::DockerPs, &[], cli.verbose)?
1673|            }
1674|            DockerCommands::Images => {
1675|                container::run(container::ContainerCmd::DockerImages, &[], cli.verbose)?
1676|            }
1677|            DockerCommands::Logs { container: c } => {
1678|                container::run(container::ContainerCmd::DockerLogs, &[c], cli.verbose)?
1679|            }
1680|            DockerCommands::Compose { command: compose } => match compose {
1681|                ComposeCommands::Ps => container::run_compose_ps(cli.verbose)?,
1682|                ComposeCommands::Logs { service } => {
1683|                    container::run_compose_logs(service.as_deref(), cli.verbose)?
1684|                }
1685|                ComposeCommands::Build { service } => {
1686|                    container::run_compose_build(service.as_deref(), cli.verbose)?
1687|                }
1688|                ComposeCommands::Other(args) => {
1689|                    container::run_compose_passthrough(&args, cli.verbose)?
1690|                }
1691|            },
1692|            DockerCommands::Other(args) => container::run_docker_passthrough(&args, cli.verbose)?,
1693|        },
1694|
1695|        Commands::Kubectl { command } => match command {
1696|            KubectlCommands::Pods { namespace, all } => {
1697|                let mut args: Vec<String> = Vec::new();
1698|                if all {
1699|                    args.push("-A".to_string());
1700|                } else if let Some(n) = namespace {
1701|                    args.push("-n".to_string());
1702|                    args.push(n);
1703|                }
1704|                container::run(container::ContainerCmd::KubectlPods, &args, cli.verbose)?
1705|            }
1706|            KubectlCommands::Services { namespace, all } => {
1707|                let mut args: Vec<String> = Vec::new();
1708|                if all {
1709|                    args.push("-A".to_string());
1710|                } else if let Some(n) = namespace {
1711|                    args.push("-n".to_string());
1712|                    args.push(n);
1713|                }
1714|                container::run(container::ContainerCmd::KubectlServices, &args, cli.verbose)?
1715|            }
1716|            KubectlCommands::Logs { pod, container: c } => {
1717|                let mut args = vec![pod];
1718|                if let Some(cont) = c {
1719|                    args.push("-c".to_string());
1720|                    args.push(cont);
1721|                }
1722|                container::run(container::ContainerCmd::KubectlLogs, &args, cli.verbose)?
1723|            }
1724|            KubectlCommands::Other(args) => container::run_kubectl_passthrough(&args, cli.verbose)?,
1725|        },
1726|
1727|        Commands::Summary { command } => {
1728|            let cmd = command.join(" ");
1729|            summary::run(&cmd, cli.verbose)?
1730|        }
1731|
1732|        Commands::Grep {
1733|            pattern,
1734|            path,
1735|            max_len,
1736|            max,
1737|            context_only,
1738|            file_type,
1739|            line_numbers: _, // no-op: line numbers always enabled in grep_cmd::run
1740|            extra_args,
1741|        } => grep_cmd::run(
1742|            &pattern,
1743|            &path,
1744|            max_len,
1745|            max,
1746|            context_only,
1747|            file_type.as_deref(),
1748|            &extra_args,
1749|            cli.verbose,
1750|        )?,
1751|
1752|        Commands::Init {
1753|            global,
1754|            opencode,
1755|            gemini,
1756|            agent,
1757|            show,
1758|            claude_md,
1759|            hook_only,
1760|            auto_patch,
1761|            no_patch,
1762|            uninstall,
1763|            codex,
1764|            copilot,
1765|            pi,
1766|        } => {
1767|            if show {
1768|                hooks::init::show_config(codex)?
1769|            } else if uninstall {
1770|                let cursor = agent == Some(AgentTarget::Cursor);
1771|                let pi = pi || agent == Some(AgentTarget::Pi);
1772|                hooks::init::uninstall(global, gemini, codex, cursor, pi, cli.verbose)?;
1773|            } else if gemini {
1774|                let patch_mode = if auto_patch {
1775|                    hooks::init::PatchMode::Auto
1776|                } else if no_patch {
1777|                    hooks::init::PatchMode::Skip
1778|                } else {
1779|                    hooks::init::PatchMode::Ask
1780|                };
1781|                hooks::init::run_gemini(global, hook_only, patch_mode, cli.verbose)?;
1782|            } else if copilot {
1783|                hooks::init::run_copilot(cli.verbose)?;
1784|            } else if agent == Some(AgentTarget::Pi) {
1785|                hooks::init::run_pi_mode(global, cli.verbose)?;
1786|            } else if agent == Some(AgentTarget::Kilocode) {
1787|                if global {
1788|                    anyhow::bail!("Kilo Code is project-scoped. Use: rtk init --agent kilocode");
1789|                }
1790|                hooks::init::run_kilocode_mode(cli.verbose)?;
1791|            } else if agent == Some(AgentTarget::Antigravity) {
1792|                if global {
1793|                    anyhow::bail!(
1794|                        "Antigravity is project-scoped. Use: rtk init --agent antigravity"
1795|                    );
1796|                }
1797|                hooks::init::run_antigravity_mode(cli.verbose)?;
1798|            } else {
1799|                let install_opencode = opencode;
1800|                let install_claude = !opencode;
1801|                let install_cursor = agent == Some(AgentTarget::Cursor);
1802|                let install_windsurf = agent == Some(AgentTarget::Windsurf);
1803|                let install_cline = agent == Some(AgentTarget::Cline);
1804|
1805|                let patch_mode = if auto_patch {
1806|                    hooks::init::PatchMode::Auto
1807|                } else if no_patch {
1808|                    hooks::init::PatchMode::Skip
1809|                } else {
1810|                    hooks::init::PatchMode::Ask
1811|                };
1812|                hooks::init::run(
1813|                    global,
1814|                    install_claude,
1815|                    install_opencode,
1816|                    install_cursor,
1817|                    install_windsurf,
1818|                    install_cline,
1819|                    claude_md,
1820|                    hook_only,
1821|                    codex,
1822|                    patch_mode,
1823|                    cli.verbose,
1824|                )?;
1825|                if pi {
1826|                    hooks::init::run_pi_mode(global, cli.verbose)?;
1827|                }
1828|            }
1829|            0
1830|        }
1831|
1832|        Commands::Wget { url, output, args } => {
1833|            if output.as_deref() == Some("-") {
1834|                wget_cmd::run_stdout(&url, &args, cli.verbose)?
1835|            } else {
1836|                // Pass -O <file> through to wget via args
1837|                let mut all_args = Vec::new();
1838|                if let Some(out_file) = &output {
1839|                    all_args.push("-O".to_string());
1840|                    all_args.push(out_file.clone());
1841|                }
1842|                all_args.extend(args);
1843|                wget_cmd::run(&url, &all_args, cli.verbose)?
1844|            }
1845|        }
1846|
1847|        Commands::Wc { args } => wc_cmd::run(&args, cli.verbose)?,
1848|
1849|        Commands::Gain {
1850|            project, // added
1851|            graph,
1852|            history,
1853|            quota,
1854|            tier,
1855|            daily,
1856|            weekly,
1857|            monthly,
1858|            all,
1859|            format,
1860|            failures,
1861|            reset,
1862|            yes,
1863|        } => {
1864|            analytics::gain::run(
1865|                project, // added: pass project flag
1866|                graph,
1867|                history,
1868|                quota,
1869|                &tier,
1870|                daily,
1871|                weekly,
1872|                monthly,
1873|                all,
1874|                &format,
1875|                failures,
1876|                reset,
1877|                yes,
1878|                cli.verbose,
1879|            )?;
1880|            0
1881|        }
1882|
1883|        Commands::CcEconomics {
1884|            daily,
1885|            weekly,
1886|            monthly,
1887|            all,
1888|            format,
1889|        } => {
1890|            analytics::cc_economics::run(daily, weekly, monthly, all, &format, cli.verbose)?;
1891|            0
1892|        }
1893|
1894|        Commands::Config { create } => {
1895|            if create {
1896|                let path = core::config::Config::create_default()?;
1897|                println!("Created: {}", path.display());
1898|            } else {
1899|                core::config::show_config()?;
1900|            }
1901|            0
1902|        }
1903|
1904|        Commands::Jest { ref args } | Commands::Vitest { ref args } => {
1905|            vitest_cmd::run_test(&cli.command, args, cli.verbose)?
1906|        }
1907|
1908|        Commands::Prisma { command } => match command {
1909|            PrismaCommands::Generate { args } => {
1910|                prisma_cmd::run(prisma_cmd::PrismaCommand::Generate, &args, cli.verbose)?
1911|            }
1912|            PrismaCommands::Migrate { command } => match command {
1913|                PrismaMigrateCommands::Dev { name, args } => prisma_cmd::run(
1914|                    prisma_cmd::PrismaCommand::Migrate {
1915|                        subcommand: prisma_cmd::MigrateSubcommand::Dev { name },
1916|                    },
1917|                    &args,
1918|                    cli.verbose,
1919|                )?,
1920|                PrismaMigrateCommands::Status { args } => prisma_cmd::run(
1921|                    prisma_cmd::PrismaCommand::Migrate {
1922|                        subcommand: prisma_cmd::MigrateSubcommand::Status,
1923|                    },
1924|                    &args,
1925|                    cli.verbose,
1926|                )?,
1927|                PrismaMigrateCommands::Deploy { args } => prisma_cmd::run(
1928|                    prisma_cmd::PrismaCommand::Migrate {
1929|                        subcommand: prisma_cmd::MigrateSubcommand::Deploy,
1930|                    },
1931|                    &args,
1932|                    cli.verbose,
1933|                )?,
1934|            },
1935|            PrismaCommands::DbPush { args } => {
1936|                prisma_cmd::run(prisma_cmd::PrismaCommand::DbPush, &args, cli.verbose)?
1937|            }
1938|        },
1939|
1940|        Commands::Tsc { args } => tsc_cmd::run(&args, cli.verbose)?,
1941|
1942|        Commands::Next { args } => next_cmd::run(&args, cli.verbose)?,
1943|
1944|        Commands::Lint { args } => lint_cmd::run(&args, cli.verbose)?,
1945|
1946|        Commands::Prettier { args } => prettier_cmd::run(&args, cli.verbose)?,
1947|
1948|        Commands::Format { args } => format_cmd::run(&args, cli.verbose)?,
1949|
1950|        Commands::Playwright { args } => playwright_cmd::run(&args, cli.verbose)?,
1951|
1952|        Commands::Cargo { command } => match command {
1953|            CargoCommands::Build { args } => {
1954|                cargo_cmd::run(cargo_cmd::CargoCommand::Build, &args, cli.verbose)?
1955|            }
1956|            CargoCommands::Test { args } => {
1957|                cargo_cmd::run(cargo_cmd::CargoCommand::Test, &args, cli.verbose)?
1958|            }
1959|            CargoCommands::Clippy { args } => {
1960|                cargo_cmd::run(cargo_cmd::CargoCommand::Clippy, &args, cli.verbose)?
1961|            }
1962|            CargoCommands::Check { args } => {
1963|                cargo_cmd::run(cargo_cmd::CargoCommand::Check, &args, cli.verbose)?
1964|            }
1965|            CargoCommands::Install { args } => {
1966|                cargo_cmd::run(cargo_cmd::CargoCommand::Install, &args, cli.verbose)?
1967|            }
1968|            CargoCommands::Nextest { args } => {
1969|                cargo_cmd::run(cargo_cmd::CargoCommand::Nextest, &args, cli.verbose)?
1970|            }
1971|            CargoCommands::Other(args) => cargo_cmd::run_passthrough(&args, cli.verbose)?,
1972|        },
1973|
1974|        Commands::Npm { args } => npm_cmd::run(&args, cli.verbose, cli.skip_env)?,
1975|
1976|        Commands::Curl { args } => curl_cmd::run(&args, cli.verbose)?,
1977|
1978|        Commands::Discover {
1979|            project,
1980|            limit,
1981|            all,
1982|            since,
1983|            format,
1984|        } => {
1985|            discover::run(project.as_deref(), all, since, limit, &format, cli.verbose)?;
1986|            0
1987|        }
1988|
1989|        Commands::Session {} => {
1990|            analytics::session_cmd::run(cli.verbose)?;
1991|            0
1992|        }
1993|
1994|        Commands::Telemetry { command } => {
1995|            core::telemetry_cmd::run(&command)?;
1996|            0
1997|        }
1998|
1999|        Commands::Learn {
2000|            project,
2001|