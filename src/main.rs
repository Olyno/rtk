1|1|<<<<<<< HEAD
2|2|<<<<<<< HEAD
3|3|<<<<<<< HEAD
4|4|1|mod analytics;
5|5|2|mod cmds;
6|6|3|mod core;
7|7|4|mod discover;
8|8|5|mod hooks;
9|9|6|mod index;
10|10|7|mod learn;
11|11|8|mod parser;
12|12|9|
13|13|10|// Re-export command modules for routing
14|14|11|use cmds::cloud::{aws_cmd, container, curl_cmd, psql_cmd, wget_cmd};
15|15|12|use cmds::dart::{dart_cmd, flutter_cmd};
16|16|13|use cmds::dotnet::{binlog, dotnet_cmd, dotnet_format_report, dotnet_trx};
17|17|14|use cmds::git::{diff_cmd, gh_cmd, git, glab_cmd, gt_cmd};
18|18|15|use cmds::go::{go_cmd, golangci_cmd};
19|19|16|use cmds::js::{
20|20|17|    lint_cmd, next_cmd, npm_cmd, playwright_cmd, pnpm_cmd, prettier_cmd, prisma_cmd, tsc_cmd,
21|21|18|    vitest_cmd,
22|22|19|};
23|23|20|use cmds::jvm::gradlew_cmd;
24|24|21|use cmds::python::{mypy_cmd, pip_cmd, pytest_cmd, ruff_cmd};
25|25|22|use cmds::ruby::{rake_cmd, rspec_cmd, rubocop_cmd};
26|26|23|use cmds::rust::{cargo_cmd, runner};
27|27|24|use cmds::system::{
28|28|25|    deps, env_cmd, find_cmd, format_cmd, grep_cmd, json_cmd, local_llm, log_cmd, ls, pipe_cmd,
29|29|26|    read, summary, tree, wc_cmd,
30|30|27|};
31|31|28|
32|32|29|use anyhow::{Context, Result};
33|33|30|use clap::error::ErrorKind;
34|34|31|use clap::{Parser, Subcommand, ValueEnum};
35|35|32|use std::ffi::OsString;
36|36|33|use std::path::{Path, PathBuf};
37|37|34|
38|38|35|/// Target agent for hook installation.
39|39|36|#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
40|40|37|pub enum AgentTarget {
41|41|38|    /// Claude Code (default)
42|42|39|    Claude,
43|43|40|    /// Cursor Agent (editor and CLI)
44|44|41|    Cursor,
45|45|42|    /// Windsurf IDE (Cascade)
46|46|43|    Windsurf,
47|47|44|    /// Cline / Roo Code (VS Code)
48|48|45|    Cline,
49|49|46|    /// Kilo Code
50|50|47|    Kilocode,
51|51|48|    /// Google Antigravity
52|52|49|    Antigravity,
53|53|50|    /// Hermes CLI
54|54|51|    Hermes,
55|55|52|    /// Kimi CLI
56|56|53|    Kimi,
57|57|54|}
58|58|55|
59|59|56|#[derive(Parser)]
60|60|57|#[command(
61|61|58|    name = "rtk",
62|62|59|    version,
63|63|60|    about = "Rust Token Killer - Minimize LLM token consumption",
64|64|61|    long_about = "A high-performance CLI proxy designed to filter and summarize system outputs before they reach your LLM context."
65|65|62|)]
66|66|63|struct Cli {
67|67|64|    #[command(subcommand)]
68|68|65|    command: Commands,
69|69|66|
70|70|67|    /// Verbosity level (-v, -vv, -vvv)
71|71|68|    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
72|72|69|    verbose: u8,
73|73|70|
74|74|71|    /// Ultra-compact mode: ASCII icons, inline format (Level 2 optimizations)
75|75|72|    #[arg(long, global = true)]
76|76|73|    ultra_compact: bool,
77|77|74|
78|78|75|    /// Set SKIP_ENV_VALIDATION=1 for child processes (Next.js, tsc, lint, prisma)
79|79|76|    #[arg(long = "skip-env", global = true)]
80|80|77|    skip_env: bool,
81|81|78|}
82|82|79|
83|83|80|#[derive(Debug, Subcommand)]
84|84|81|enum Commands {
85|85|82|    /// Install rtk hooks for AI assistants (Claude, Cursor, Hermes, Codex, Copilot)
86|86|83|    Init {
87|87|84|        /// Add to global assistant config directory instead of local project file
88|88|85|        #[arg(short, long)]
89|89|86|        global: bool,
90|90|87|
91|91|88|        /// Install OpenCode plugin (in addition to Claude Code)
92|92|89|        #[arg(long)]
93|93|90|        opencode: bool,
94|94|91|
95|95|92|        /// Initialize for Gemini CLI instead of Claude Code
96|96|93|        #[arg(long)]
97|97|94|        gemini: bool,
98|98|95|
99|99|96|        /// Target agent to install hooks for (default: claude)
100|100|97|        #[arg(long, value_enum)]
101|101|98|        agent: Option<AgentTarget>,
102|102|99|
103|103|100|        /// Show current configuration
104|104|101|        #[arg(long)]
105|105|102|        show: bool,
106|106|103|
107|107|104|        /// Inject full instructions into CLAUDE.md (legacy mode)
108|108|105|        #[arg(long = "claude-md", group = "mode")]
109|109|106|        claude_md: bool,
110|110|107|
111|111|108|        /// Hook only, no RTK.md
112|112|109|        #[arg(long = "hook-only", group = "mode")]
113|113|110|        hook_only: bool,
114|114|111|
115|115|112|        /// Auto-patch settings.json without prompting
116|116|113|        #[arg(long = "auto-patch", group = "patch")]
117|117|114|        auto_patch: bool,
118|118|115|
119|119|116|        /// Skip settings.json patching (print manual instructions)
120|120|117|        #[arg(long = "no-patch", group = "patch")]
121|121|118|        no_patch: bool,
122|122|119|
123|123|120|        /// Remove RTK artifacts for the selected assistant mode
124|124|121|        #[arg(long)]
125|125|122|        uninstall: bool,
126|126|123|
127|127|124|        /// Target Codex CLI (uses AGENTS.md + RTK.md, no Claude hook patching)
128|128|125|        #[arg(long)]
129|129|126|        codex: bool,
130|130|127|
131|131|128|        /// Install GitHub Copilot integration (VS Code + CLI)
132|132|129|        #[arg(long)]
133|133|130|        copilot: bool,
134|134|131|
135|135|132|        /// Preview changes without writing any files (combine with -v to show content)
136|136|133|        #[arg(long = "dry-run", conflicts_with = "show")]
137|137|134|        dry_run: bool,
138|138|135|    },
139|139|136|
140|140|137|    /// Show token savings: summary, history, daily/weekly/monthly charts
141|141|138|    Gain {
142|142|139|        /// Filter statistics to current project (current working directory) // added
143|143|140|        #[arg(short, long)]
144|144|141|        project: bool,
145|145|142|        /// Show ASCII graph of daily savings
146|146|143|        #[arg(short, long)]
147|147|144|        graph: bool,
148|148|145|        /// Show recent command history
149|149|146|        #[arg(short = 'H', long)]
150|150|147|        history: bool,
151|151|148|        /// Show monthly quota savings estimate
152|152|149|        #[arg(short, long)]
153|153|150|        quota: bool,
154|154|151|        /// Subscription tier for quota calculation: pro, 5x, 20x
155|155|152|        #[arg(short, long, default_value = "20x", requires = "quota")]
156|156|153|        tier: String,
157|157|154|        /// Show detailed daily breakdown (all days)
158|158|155|        #[arg(short, long)]
159|159|156|        daily: bool,
160|160|157|        /// Show weekly breakdown
161|161|158|        #[arg(short, long)]
162|162|159|        weekly: bool,
163|163|160|        /// Show monthly breakdown
164|164|161|        #[arg(short, long)]
165|165|162|        monthly: bool,
166|166|163|        /// Show all time breakdowns (daily + weekly + monthly)
167|167|164|        #[arg(short, long)]
168|168|165|        all: bool,
169|169|166|        /// Output format: text, json, csv
170|170|167|        #[arg(short, long, default_value = "text")]
171|171|168|        format: String,
172|172|169|        /// Show parse failure log (commands that fell back to raw execution)
173|173|170|        #[arg(short = 'F', long)]
174|174|171|        failures: bool,
175|175|172|        /// Show potential commands (unregistered, executed >5 times)
176|176|173|        #[arg(short = 'P', long)]
177|177|174|        potential: bool,
178|178|175|        /// Reset all token savings stats to zero
179|179|176|        #[arg(long)]
180|180|177|        reset: bool,
181|181|178|        /// Skip confirmation prompt when resetting
182|182|179|        #[arg(long, requires = "reset")]
183|183|180|        yes: bool,
184|184|181|    },
185|185|182|
186|186|183|    /// Show or create rtk configuration file
187|187|184|    Config {
188|188|185|        /// Create default config file
189|189|186|        #[arg(long)]
190|190|187|        create: bool,
191|191|188|    },
192|192|189|
193|193|190|    /// Find commands in Claude Code history that rtk could optimize
194|194|191|    Discover {
195|195|192|        /// Filter by project path (substring match)
196|196|193|        #[arg(short, long)]
197|197|194|        project: Option<String>,
198|198|195|        /// Max commands per section
199|199|196|        #[arg(short, long, default_value = "15")]
200|200|197|        limit: usize,
201|201|198|        /// Scan all projects (default: current project only)
202|202|199|        #[arg(short, long)]
203|203|200|        all: bool,
204|204|201|        /// Limit to sessions from last N days
205|205|202|        #[arg(short, long, default_value = "30")]
206|206|203|        since: u64,
207|207|204|        /// Output format: text, json
208|208|205|        #[arg(short, long, default_value = "text")]
209|209|206|        format: String,
210|210|207|    },
211|211|208|
212|212|209|    /// Audit Claude Code sessions: rtk adoption and missed opportunities
213|213|210|    Session {},
214|214|211|
215|215|212|    /// Manage anonymous usage data sharing (GDPR compliant)
216|216|213|    Telemetry {
217|217|214|        #[command(subcommand)]
218|218|215|        command: core::telemetry_cmd::TelemetrySubcommand,
219|219|216|    },
220|220|217|
221|221|218|    /// Build project search index for faster grep and find
222|222|219|    Index {
223|223|220|        /// Path to index (default: current directory)
224|224|221|        #[arg(default_value = ".")]
225|225|222|        path: PathBuf,
226|226|223|        /// Show index statistics after scanning
227|227|224|        #[arg(short, long)]
228|228|225|        stats: bool,
229|229|226|    },
230|230|227|
231|231|228|    /// AWS CLI with compact output (force JSON, compress)
232|232|229|    Aws {
233|233|230|        /// AWS service subcommand (e.g., sts, s3, ec2, ecs, rds, cloudformation)
234|234|231|        subcommand: String,
235|235|232|        /// Additional arguments
236|236|233|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
237|237|234|        args: Vec<String>,
238|238|235|    },
239|239|236|
240|240|237|    /// Cargo commands with compact output
241|241|238|    Cargo {
242|242|239|        #[command(subcommand)]
243|243|240|        command: CargoCommands,
244|244|241|    },
245|245|242|
246|246|243|    /// Compare Claude Code API costs vs tokens saved by rtk
247|247|244|    CcEconomics {
248|248|245|        /// Show detailed daily breakdown
249|249|246|        #[arg(short, long)]
250|250|247|        daily: bool,
251|251|248|        /// Show weekly breakdown
252|252|249|        #[arg(short, long)]
253|253|250|        weekly: bool,
254|254|251|        /// Show monthly breakdown
255|255|252|        #[arg(short, long)]
256|256|253|        monthly: bool,
257|257|254|        /// Show all time breakdowns (daily + weekly + monthly)
258|258|255|        #[arg(short, long)]
259|259|256|        all: bool,
260|260|257|        /// Output format: text, json, csv
261|261|258|        #[arg(short, long, default_value = "text")]
262|262|259|        format: String,
263|263|260|    },
264|264|261|
265|265|262|    /// Curl with auto-JSON detection and schema output
266|266|263|    Curl {
267|267|264|        /// Curl arguments (URL + options)
268|268|265|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
269|269|266|        args: Vec<String>,
270|270|267|    },
271|271|268|
272|272|269|    /// Dart commands with compact analyzer, formatter, and test output
273|273|270|    Dart {
274|274|271|        /// Dart arguments
275|275|272|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
276|276|273|        args: Vec<String>,
277|277|274|    },
278|278|275|
279|279|276|    /// Summarize project dependencies (Cargo.toml, package.json, etc.)
280|280|277|    Deps {
281|281|278|        /// Project path
282|282|279|        #[arg(default_value = ".")]
283|283|280|        path: PathBuf,
284|284|281|    },
285|285|282|
286|286|283|    /// Ultra-condensed diff: only changed lines, no context
287|287|284|    Diff {
288|288|285|        /// First file or - for stdin (unified diff)
289|289|286|        file1: PathBuf,
290|290|287|        /// Second file (optional if stdin)
291|291|288|        file2: Option<PathBuf>,
292|292|289|    },
293|293|290|
294|294|291|    /// Docker commands with compact output
295|295|292|    Docker {
296|296|293|        #[command(subcommand)]
297|297|294|        command: DockerCommands,
298|298|295|    },
299|299|296|
300|300|297|    /// .NET commands with compact output (build/test/restore/format)
301|301|298|    Dotnet {
302|302|299|        #[command(subcommand)]
303|303|300|        command: DotnetCommands,
304|304|301|    },
305|305|302|
306|306|303|    /// Show environment variables (sensitive values masked)
307|307|304|    Env {
308|308|305|        /// Filter by name (e.g. PATH, AWS)
309|309|306|        #[arg(short, long)]
310|310|307|        filter: Option<String>,
311|311|308|        /// Show all (include sensitive)
312|312|309|        #[arg(long)]
313|313|310|        show_all: bool,
314|314|311|    },
315|315|312|
316|316|313|    /// Run command, show only errors and warnings
317|317|314|    Err {
318|318|315|        /// Command to run
319|319|316|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
320|320|317|        command: Vec<String>,
321|321|318|    },
322|322|319|
323|323|320|    /// Find files with compact tree output (accepts native find flags)
324|324|321|    Find {
325|325|322|        /// All find arguments (supports both RTK and native find syntax)
326|326|323|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
327|327|324|        args: Vec<String>,
328|328|325|    },
329|329|326|
330|330|327|    /// Flutter commands with compact analyzer and test output
331|331|328|    Flutter {
332|332|329|        /// Flutter arguments
333|333|330|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
334|334|331|        args: Vec<String>,
335|335|332|    },
336|336|333|
337|337|334|    /// Universal format checker (auto-detects prettier, black, ruff)
338|338|335|    Format {
339|339|336|        /// Formatter arguments (auto-detects formatter from project files)
340|340|337|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
341|341|338|        args: Vec<String>,
342|342|339|    },
343|343|340|
344|344|341|    /// GitHub CLI (gh) commands with token-optimized output
345|345|342|    Gh {
346|346|343|        /// Subcommand: pr, issue, run, repo
347|347|344|        subcommand: String,
348|348|345|        /// Additional arguments
349|349|346|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
350|350|347|        args: Vec<String>,
351|351|348|    },
352|352|349|
353|353|350|    /// Git commands with compact output
354|354|351|    Git {
355|355|352|        /// Change to directory before executing (like git -C <path>, can be repeated)
356|356|353|        #[arg(short = 'C', action = clap::ArgAction::Append)]
357|357|354|        directory: Vec<String>,
358|358|355|
359|359|356|        /// Git configuration override (like git -c key=value, can be repeated)
360|360|357|        #[arg(short = 'c', action = clap::ArgAction::Append)]
361|361|358|        config_override: Vec<String>,
362|362|359|
363|363|360|        /// Set the path to the .git directory
364|364|361|        #[arg(long = "git-dir")]
365|365|362|        git_dir: Option<String>,
366|366|363|
367|367|364|        /// Set the path to the working tree
368|368|365|        #[arg(long = "work-tree")]
369|369|366|        work_tree: Option<String>,
370|370|367|
371|371|368|        /// Disable pager (like git --no-pager)
372|372|369|        #[arg(long = "no-pager")]
373|373|370|        no_pager: bool,
374|374|371|
375|375|372|        /// Skip optional locks (like git --no-optional-locks)
376|376|373|        #[arg(long = "no-optional-locks")]
377|377|374|        no_optional_locks: bool,
378|378|375|
379|379|376|        /// Treat repository as bare (like git --bare)
380|380|377|        #[arg(long)]
381|381|378|        bare: bool,
382|382|379|
383|383|380|        /// Treat pathspecs literally (like git --literal-pathspecs)
384|384|381|        #[arg(long = "literal-pathspecs")]
385|385|382|        literal_pathspecs: bool,
386|386|383|
387|387|384|        #[command(subcommand)]
388|388|385|        command: GitCommands,
389|389|386|    },
390|390|387|
391|391|388|    /// GitLab CLI (glab) commands with token-optimized output
392|392|389|    Glab {
393|393|390|        /// Target repository (owner/repo), passed as glab -R flag
394|394|391|        #[arg(short = 'R', long = "repo")]
395|395|392|        repo: Option<String>,
396|396|393|        /// Target group, passed as glab -g flag
397|397|394|        #[arg(short = 'g', long = "group")]
398|398|395|        group: Option<String>,
399|399|396|        /// Subcommand: mr, issue, ci, pipeline, api
400|400|397|        subcommand: String,
401|401|398|        /// Additional arguments
402|402|399|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
403|403|400|        args: Vec<String>,
404|404|401|    },
405|405|402|
406|406|403|    /// Go commands with compact output
407|407|404|    Go {
408|408|405|        #[command(subcommand)]
409|409|406|        command: GoCommands,
410|410|407|    },
411|411|408|
412|412|409|    /// golangci-lint wrapper with compact `run` support and passthrough for other invocations
413|413|410|    #[command(name = "golangci-lint")]
414|414|411|    GolangciLint {
415|415|412|        /// Additional golangci-lint arguments
416|416|413|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
417|417|414|        args: Vec<String>,
418|418|415|    },
419|419|416|
420|420|417|    /// Android Gradle wrapper with compact output (build, test, lint)
421|421|418|    #[command(name = "gradlew")]
422|422|419|    Gradlew {
423|423|420|        /// Gradle tasks and arguments (e.g., assembleDebug, testDebugUnitTest, lint, --info)
424|424|421|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
425|425|422|        args: Vec<String>,
426|426|423|    },
427|427|424|
428|428|425|    /// Compact grep - strips whitespace, truncates, groups by file
429|429|426|    Grep {
430|430|427|        /// Pattern to search
431|431|428|        pattern: String,
432|432|429|        /// Path to search in
433|433|430|        #[arg(default_value = ".")]
434|434|431|        path: String,
435|435|432|        /// Max line length
436|436|433|        #[arg(short = 'l', long, default_value = "80")]
437|437|434|        max_len: usize,
438|438|435|        /// Max results to show
439|439|436|        #[arg(short, long, default_value = "200")]
440|440|437|        max: usize,
441|441|438|        /// Show only match context (not full line)
442|442|439|        #[arg(long)]
443|443|440|        context_only: bool,
444|444|441|        /// Filter by file type (e.g., ts, py, rust)
445|445|442|        #[arg(short = 't', long)]
446|446|443|        file_type: Option<String>,
447|447|444|        /// Show line numbers (always on, accepted for grep/rg compatibility)
448|448|445|        #[arg(short = 'n', long)]
449|449|446|        line_numbers: bool,
450|450|447|        /// Extra ripgrep arguments (e.g., -i, -A 3, -w, --glob)
451|451|448|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
452|452|449|        extra_args: Vec<String>,
453|453|450|    },
454|454|451|
455|455|452|    /// Graphite (gt) stacked PR commands with compact output
456|456|453|    Gt {
457|457|454|        #[command(subcommand)]
458|458|455|        command: GtCommands,
459|459|456|    },
460|460|457|
461|461|458|    /// Jest commands with compact output
462|462|459|    Jest {
463|463|460|        /// Additional jest arguments
464|464|461|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
465|465|462|        args: Vec<String>,
466|466|463|    },
467|467|464|
468|468|465|    /// Show JSON with compact values or keys-only structure view
469|469|466|    Json {
470|470|467|        /// JSON file
471|471|468|        file: PathBuf,
472|472|469|        /// Max depth
473|473|470|        #[arg(short, long, default_value = "5")]
474|474|471|        depth: usize,
475|475|472|        /// Show keys only (strip all values, show structure)
476|476|473|        #[arg(long)]
477|477|474|        keys_only: bool,
478|478|475|    },
479|479|476|
480|480|477|    /// Kubectl commands with compact output
481|481|478|    Kubectl {
482|482|479|        #[command(subcommand)]
483|483|480|        command: KubectlCommands,
484|484|481|    },
485|485|482|
486|486|483|    /// Analyze Claude Code error history to suggest rtk corrections
487|487|484|    Learn {
488|488|485|        /// Filter by project path (substring match)
489|489|486|        #[arg(short, long)]
490|490|487|        project: Option<String>,
491|491|488|        /// Scan all projects (default: current project only)
492|492|489|        #[arg(short, long)]
493|493|490|        all: bool,
494|494|491|        /// Limit to sessions from last N days
495|495|492|        #[arg(short, long, default_value = "30")]
496|496|493|        since: u64,
497|497|494|        /// Output format: text, json
498|498|495|        #[arg(short, long, default_value = "text")]
499|499|496|        format: String,
500|500|497|        /// Generate .claude/rules/cli-corrections.md file
501|501|498|        #[arg(short, long)]
502|502|499|        write_rules: bool,
503|503|500|        /// Minimum confidence threshold (0.0-1.0)
504|504|501|        #[arg(long, default_value = "0.6")]
505|505|502|        min_confidence: f64,
506|506|503|        /// Minimum occurrences to include in report
507|507|504|        #[arg(long, default_value = "1")]
508|508|505|        min_occurrences: usize,
509|509|506|    },
510|510|507|
511|511|508|    /// ESLint with grouped rule violations
512|512|509|    Lint {
513|513|510|        /// Linter arguments
514|514|511|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
515|515|512|        args: Vec<String>,
516|516|513|    },
517|517|514|
518|518|515|    /// Filter and deduplicate log output (stdin or file)
519|519|516|    Log {
520|520|517|        /// Log file (omit for stdin)
521|521|518|        file: Option<PathBuf>,
522|522|519|    },
523|523|520|
524|524|521|522|    /// List directory with token-optimized output (proxy to native ls)
525|525|523|    Ls {
526|526|524|        /// Arguments passed to ls (supports all native ls flags like -l, -a, -h, -R)
527|527|525|526|    /// Read stdin, apply filter, print filtered output (Unix pipe mode)
528|528|527|    Pipe {
529|529|528|        /// Filter name as a positional argument (e.g. `rtk pipe log`)
530|530|529|        filter_name: Option<String>,
531|531|530|
532|532|531|        /// Filter name (cargo-test, pytest, grep, find, git-log, etc.)
533|533|532|        #[arg(short, long)]
534|534|533|        filter: Option<String>,
535|535|534|
536|536|535|        /// Pass stdin through without filtering
537|537|536|        #[arg(long)]
538|538|537|        passthrough: bool,
539|539|538|    },
540|540|539|
541|541|540|    /// Trust project-local TOML filters in current directory
542|542|541|    Trust {
543|543|542|        /// List all trusted projects
544|544|543|        #[arg(long)]
545|545|544|        list: bool,
546|546|545|    },
547|547|546|
548|548|547|    /// Revoke trust for project-local TOML filters
549|549|548|    Untrust,
550|550|549|
551|551|550|    /// Verify hook integrity and run TOML filter inline tests
552|552|551|    Verify {
553|553|552|        /// Run tests only for this filter name
554|554|553|        #[arg(long)]
555|555|554|        filter: Option<String>,
556|556|555|        /// Fail if any filter has no inline tests (CI mode)
557|557|556|        #[arg(long)]
558|558|557|        require_all: bool,
559|559|558|    },
560|560|559|
561|561|560|    /// Ruff linter/formatter with compact output
562|562|561|    Ruff {
563|563|562|        /// Ruff arguments (e.g., check, format --check)
564|564|563|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
565|565|564|        args: Vec<String>,
566|566|565|    },
567|567|566|
568|568|567|    /// Pytest test runner with compact output
569|569|568|    Pytest {
570|570|569|        /// Pytest arguments
571|571|570|571|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
572|572|572|        args: Vec<String>,
573|573|573|    },
574|574|574|
575|575|575|    /// Mypy type checker with grouped error output
576|576|576|    Mypy {
577|577|577|        /// Mypy arguments
578|578|578|        #[arg(trailing_var_arg = true, allow
579|579|
580|580|... [OUTPUT TRUNCATED - 76470 chars omitted out of 126470 total] ...
581|581|
582|582|violations at 50)
583|583|                } else {
584|584|                    (full.into_owned(), vec![])
585|585|                }
586|586|            } else {
587|587|                (
588|588|                    args[0].to_string_lossy().into_owned(),
589|589|                    args[1..]
590|590|                        .iter()
591|591|                        .map(|s| s.to_string_lossy().into_owned())
592|592|                        .collect(),
593|593|                )
594|594|            };
595|595|
596|596|            if cli.verbose > 0 {
597|597|                eprintln!("Proxy mode: {} {}", cmd_name, cmd_args.join(" "));
598|598|            }
599|599|
600|600|            // ISSUE #897: Kill proxy child on SIGINT/SIGTERM to prevent orphan
601|601|            // processes. Drop-based ChildGuard doesn't run on signals with
602|602|            // panic=abort, so we register a signal handler that kills the child
603|603|            // PID stored in this atomic.
604|604|            static PROXY_CHILD_PID: AtomicU32 = AtomicU32::new(0);
605|605|
606|606|            #[cfg(unix)]
607|607|            #[allow(unsafe_code)]
608|608|            {
609|609|                unsafe extern "C" fn handle_signal(sig: libc::c_int) {
610|610|                    let pid = PROXY_CHILD_PID.load(Ordering::SeqCst);
611|611|                    if pid != 0 {
612|612|                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
613|613|                        libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0);
614|614|                    }
615|615|                    libc::signal(sig, libc::SIG_DFL);
616|616|                    libc::raise(sig);
617|617|                }
618|618|                // nosemgrep: unsafe-block
619|619|                unsafe {
620|620|                    libc::signal(
621|621|                        libc::SIGINT,
622|622|                        handle_signal as *const () as libc::sighandler_t,
623|623|                    );
624|624|                    libc::signal(
625|625|                        libc::SIGTERM,
626|626|                        handle_signal as *const () as libc::sighandler_t,
627|627|                    );
628|628|                }
629|629|            }
630|630|
631|631|            struct ChildGuard(Option<std::process::Child>);
632|632|            impl Drop for ChildGuard {
633|633|                fn drop(&mut self) {
634|634|                    if let Some(mut child) = self.0.take() {
635|635|                        let _ = child.kill();
636|636|                        let _ = child.wait();
637|637|                    }
638|638|                    PROXY_CHILD_PID.store(0, Ordering::SeqCst);
639|639|                }
640|640|            }
641|641|
642|642|            let mut child = ChildGuard(Some(
643|643|                core::utils::resolved_command(cmd_name.as_ref())
644|644|                    .args(&cmd_args)
645|645|                    .stdout(Stdio::piped())
646|646|                    .stderr(Stdio::piped())
647|647|                    .spawn()
648|648|                    .context(format!("Failed to execute command: {}", cmd_name))?,
649|649|            ));
650|650|
651|651|            // Store child PID for signal handler before anything can fail
652|652|            if let Some(ref inner) = child.0 {
653|653|                PROXY_CHILD_PID.store(inner.id(), Ordering::SeqCst);
654|654|            }
655|655|
656|656|            let inner = child.0.as_mut().context("Child process missing")?;
657|657|            let stdout_pipe = inner
658|658|                .stdout
659|659|                .take()
660|660|                .context("Failed to capture child stdout")?;
661|661|            let stderr_pipe = inner
662|662|                .stderr
663|663|                .take()
664|664|                .context("Failed to capture child stderr")?;
665|665|
666|666|            const CAP: usize = 1_048_576;
667|667|
668|668|            let stdout_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
669|669|                let mut reader = stdout_pipe;
670|670|                let mut captured = Vec::new();
671|671|                let mut buf = [0u8; 8192];
672|672|
673|673|                loop {
674|674|                    let count = reader.read(&mut buf)?;
675|675|                    if count == 0 {
676|676|                        break;
677|677|                    }
678|678|                    if captured.len() < CAP {
679|679|                        let take = count.min(CAP - captured.len());
680|680|                        captured.extend_from_slice(&buf[..take]);
681|681|                    }
682|682|                    let mut out = std::io::stdout().lock();
683|683|                    out.write_all(&buf[..count])?;
684|684|                    out.flush()?;
685|685|                }
686|686|
687|687|                Ok(captured)
688|688|            });
689|689|
690|690|            let stderr_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
691|691|                let mut reader = stderr_pipe;
692|692|                let mut captured = Vec::new();
693|693|                let mut buf = [0u8; 8192];
694|694|
695|695|                loop {
696|696|                    let count = reader.read(&mut buf)?;
697|697|                    if count == 0 {
698|698|                        break;
699|699|                    }
700|700|                    if captured.len() < CAP {
701|701|                        let take = count.min(CAP - captured.len());
702|702|                        captured.extend_from_slice(&buf[..take]);
703|703|                    }
704|704|                    let mut err = std::io::stderr().lock();
705|705|                    err.write_all(&buf[..count])?;
706|706|                    err.flush()?;
707|707|                }
708|708|
709|709|                Ok(captured)
710|710|            });
711|711|
712|712|            let status = child
713|713|                .0
714|714|                .take()
715|715|                .context("Child process missing")?
716|716|                .wait()
717|717|                .context(format!("Failed waiting for command: {}", cmd_name))?;
718|718|
719|719|            let stdout_bytes = stdout_handle
720|720|                .join()
721|721|                .map_err(|_| anyhow::anyhow!("stdout streaming thread panicked"))??;
722|722|            let stderr_bytes = stderr_handle
723|723|                .join()
724|724|                .map_err(|_| anyhow::anyhow!("stderr streaming thread panicked"))??;
725|725|
726|726|            let stdout = String::from_utf8_lossy(&stdout_bytes);
727|727|            let stderr = String::from_utf8_lossy(&stderr_bytes);
728|728|            let full_output = format!("{}{}", stdout, stderr);
729|729|
730|730|            // Track usage (input = output since no filtering)
731|731|            timer.track(
732|732|                &format!("{} {}", cmd_name, cmd_args.join(" ")),
733|733|                &format!("rtk proxy {} {}", cmd_name, cmd_args.join(" ")),
734|734|                &full_output,
735|735|                &full_output,
736|736|            );
737|737|
738|738|            core::utils::exit_code_from_status(&status, &cmd_name)
739|739|        }
740|740|
741|741|        Commands::Trust { list } => {
742|742|            hooks::trust::run_trust(list)?;
743|743|            0
744|744|        }
745|745|
746|746|        Commands::Untrust => {
747|747|            hooks::trust::run_untrust()?;
748|748|            0
749|749|        }
750|750|
751|751|        Commands::Verify {
752|752|            filter,
753|753|            require_all,
754|754|        } => {
755|755|            if filter.is_some() {
756|756|                // Filter-specific mode: run only that filter's tests
757|757|                hooks::verify_cmd::run(filter, require_all)?;
758|758|            } else {
759|759|                // Default or --require-all: always run integrity check first
760|760|                hooks::integrity::run_verify(cli.verbose)?;
761|761|                hooks::verify_cmd::run(None, require_all)?;
762|762|            }
763|763|            0
764|764|        }
765|765|    };
766|766|
767|767|    Ok(code)
768|768|}
769|769|
770|770|/// Returns true for commands that are invoked via the hook pipeline
771|771|/// (i.e., commands that process rewritten shell commands).
772|772|/// Meta commands (init, gain, verify, etc.) are excluded because
773|773|/// they are run directly by the user, not through the hook.
774|774|/// Returns true for commands that go through the hook pipeline
775|775|/// and therefore require integrity verification.
776|776|///
777|777|/// SECURITY: whitelist pattern — new commands are NOT integrity-checked
778|778|/// until explicitly added here. A forgotten command fails open (no check)
779|779|/// rather than creating false confidence about what's protected.
780|780|fn is_operational_command(cmd: &Commands) -> bool {
781|781|    matches!(
782|782|        cmd,
783|783|        Commands::Ls { .. }
784|784|            | Commands::Tree { .. }
785|785|            | Commands::Read { .. }
786|786|            | Commands::Smart { .. }
787|787|            | Commands::Git { .. }
788|788|            | Commands::Gh { .. }
789|789|            | Commands::Glab { .. }
790|790|            | Commands::Pnpm { .. }
791|791|            | Commands::Err { .. }
792|792|            | Commands::Test { .. }
793|793|            | Commands::Json { .. }
794|794|            | Commands::Deps { .. }
795|795|            | Commands::Env { .. }
796|796|            | Commands::Find { .. }
797|797|            | Commands::Diff { .. }
798|798|            | Commands::Log { .. }
799|799|            | Commands::Dotnet { .. }
800|800|            | Commands::Docker { .. }
801|801|            | Commands::Kubectl { .. }
802|802|            | Commands::Summary { .. }
803|803|            | Commands::Grep { .. }
804|804|            | Commands::Wget { .. }
805|805|            | Commands::Vitest { .. }
806|806|            | Commands::Prisma { .. }
807|807|            | Commands::Tsc { .. }
808|808|            | Commands::Next { .. }
809|809|            | Commands::Lint { .. }
810|810|            | Commands::Prettier { .. }
811|811|            | Commands::Playwright { .. }
812|812|            | Commands::Cargo { .. }
813|813|            | Commands::Npm { .. }
814|814|            | Commands::Npx { .. }
815|815|            | Commands::Curl { .. }
816|816|            | Commands::Ruff { .. }
817|817|            | Commands::Pytest { .. }
818|818|            | Commands::Rake { .. }
819|819|            | Commands::Rubocop { .. }
820|820|            | Commands::Rspec { .. }
821|821|            | Commands::Pip { .. }
822|822|            | Commands::Go { .. }
823|823|            | Commands::GolangciLint { .. }
824|824|            | Commands::Gt { .. }
825|825|    )
826|826|}
827|827|
828|828|#[cfg(test)]
829|829|mod tests {
830|830|    use super::*;
831|831|    use clap::Parser;
832|832|    use std::cell::Cell;
833|833|
834|834|    #[test]
835|835|    fn test_git_commit_single_message() {
836|836|        let cli = Cli::try_parse_from(["rtk", "git", "commit", "-m", "fix: typo"]).unwrap();
837|837|        match cli.command {
838|838|            Commands::Git {
839|839|                command: GitCommands::Commit { args },
840|840|                ..
841|841|            } => {
842|842|                assert_eq!(args, vec!["-m", "fix: typo"]);
843|843|            }
844|844|            _ => panic!("Expected Git Commit command"),
845|845|        }
846|846|    }
847|847|
848|848|    #[test]
849|849|    fn test_git_commit_multiple_messages() {
850|850|        let cli = Cli::try_parse_from([
851|851|            "rtk",
852|852|            "git",
853|853|            "commit",
854|854|            "-m",
855|855|            "feat: add support",
856|856|            "-m",
857|857|            "Body paragraph here.",
858|858|        ])
859|859|        .unwrap();
860|860|        match cli.command {
861|861|            Commands::Git {
862|862|                command: GitCommands::Commit { args },
863|863|                ..
864|864|            } => {
865|865|                assert_eq!(
866|866|                    args,
867|867|                    vec!["-m", "feat: add support", "-m", "Body paragraph here."]
868|868|                );
869|869|            }
870|870|            _ => panic!("Expected Git Commit command"),
871|871|        }
872|872|    }
873|873|
874|874|    // #327: git commit -am "msg" was rejected by Clap
875|875|    #[test]
876|876|    fn test_git_commit_am_flag() {
877|877|        let cli = Cli::try_parse_from(["rtk", "git", "commit", "-am", "quick fix"]).unwrap();
878|878|        match cli.command {
879|879|            Commands::Git {
880|880|                command: GitCommands::Commit { args },
881|881|                ..
882|882|            } => {
883|883|                assert_eq!(args, vec!["-am", "quick fix"]);
884|884|            }
885|885|            _ => panic!("Expected Git Commit command"),
886|886|        }
887|887|    }
888|888|
889|889|    #[test]
890|890|    fn test_git_commit_amend() {
891|891|        let cli =
892|892|            Cli::try_parse_from(["rtk", "git", "commit", "--amend", "-m", "new msg"]).unwrap();
893|893|        match cli.command {
894|894|            Commands::Git {
895|895|                command: GitCommands::Commit { args },
896|896|                ..
897|897|            } => {
898|898|                assert_eq!(args, vec!["--amend", "-m", "new msg"]);
899|899|            }
900|900|            _ => panic!("Expected Git Commit command"),
901|901|        }
902|902|    }
903|903|
904|904|    #[test]
905|905|    fn test_git_global_options_parsing() {
906|906|        let cli =
907|907|            Cli::try_parse_from(["rtk", "git", "--no-pager", "--no-optional-locks", "status"])
908|908|                .unwrap();
909|909|        match cli.command {
910|910|            Commands::Git {
911|911|                no_pager,
912|912|                no_optional_locks,
913|913|                bare,
914|914|                literal_pathspecs,
915|915|                ..
916|916|            } => {
917|917|                assert!(no_pager);
918|918|                assert!(no_optional_locks);
919|919|                assert!(!bare);
920|920|                assert!(!literal_pathspecs);
921|921|            }
922|922|            _ => panic!("Expected Git command"),
923|923|        }
924|924|    }
925|925|
926|926|    #[test]
927|927|    fn test_git_commit_long_flag_multiple() {
928|928|        let cli = Cli::try_parse_from([
929|929|            "rtk",
930|930|            "git",
931|931|            "commit",
932|932|            "--message",
933|933|            "title",
934|934|            "--message",
935|935|            "body",
936|936|            "--message",
937|937|            "footer",
938|938|        ])
939|939|        .unwrap();
940|940|        match cli.command {
941|941|            Commands::Git {
942|942|                command: GitCommands::Commit { args },
943|943|                ..
944|944|            } => {
945|945|                assert_eq!(
946|946|                    args,
947|947|                    vec![
948|948|                        "--message",
949|949|                        "title",
950|950|                        "--message",
951|951|                        "body",
952|952|                        "--message",
953|953|                        "footer"
954|954|                    ]
955|955|                );
956|956|            }
957|957|            _ => panic!("Expected Git Commit command"),
958|958|        }
959|959|    }
960|960|
961|961|    #[test]
962|962|    fn test_try_parse_valid_git_status() {
963|963|        let result = Cli::try_parse_from(["rtk", "git", "status"]);
964|964|        assert!(result.is_ok(), "git status should parse successfully");
965|965|    }
966|966|
967|967|    #[test]
968|968|    fn test_try_parse_init_agent_hermes() {
969|969|        let cli = Cli::try_parse_from(["rtk", "init", "--agent", "hermes"]).unwrap();
970|970|        match cli.command {
971|971|            Commands::Init { agent, .. } => {
972|972|                assert_eq!(agent, Some(AgentTarget::Hermes));
973|973|            }
974|974|            _ => panic!("Expected Init command"),
975|975|        }
976|976|    }
977|977|
978|978|    #[test]
979|979|    fn test_try_parse_kubectl_get_alias() {
980|980|        let cli = Cli::try_parse_from(["rtk", "kubectl", "get", "pods", "-n", "default"]).unwrap();
981|981|
982|982|        match cli.command {
983|983|            Commands::Kubectl {
984|984|                command: KubectlCommands::Get { args },
985|985|            } => assert_eq!(args, vec!["pods", "-n", "default"]),
986|986|            _ => panic!("Expected Kubectl Get command"),
987|987|        }
988|988|    }
989|989|
990|990|    #[test]
991|991|    fn test_try_parse_init_agent_hermes_uninstall() {
992|992|        let cli = Cli::try_parse_from(["rtk", "init", "--agent", "hermes", "--uninstall"]).unwrap();
993|993|        match cli.command {
994|994|            Commands::Init {
995|995|                agent, uninstall, ..
996|996|            } => {
997|997|                assert_eq!(agent, Some(AgentTarget::Hermes));
998|998|                assert!(uninstall);
999|999|            }
1000|1000|            _ => panic!("Expected Init command"),
1001|1001|        }
1002|1002|    }
1003|1003|
1004|1004|    #[test]
1005|1005|    fn test_init_uninstall_dispatch_routes_hermes_to_hermes_cleanup() {
1006|1006|        let hermes_called = Cell::new(false);
1007|1007|        let standard_called = Cell::new(false);
1008|1008|        let ctx = hooks::init::InitContext {
1009|1009|            verbose: 2,
1010|1010|            dry_run: true,
1011|1011|        };
1012|1012|
1013|1013|        let result = uninstall_init_dispatch(
1014|1014|            Some(AgentTarget::Hermes),
1015|1015|            true,
1016|1016|            false,
1017|1017|            false,
1018|1018|            ctx,
1019|1019|            |ctx| {
1020|1020|                hermes_called.set(true);
1021|1021|                assert_eq!(ctx.verbose, 2);
1022|1022|                assert!(ctx.dry_run);
1023|1023|                Ok(())
1024|1024|            },
1025|1025|            |_, _, _, _, _| {
1026|1026|                standard_called.set(true);
1027|1027|                Ok(())
1028|1028|            },
1029|1029|        );
1030|1030|
1031|1031|        assert!(result.is_ok());
1032|1032|        assert!(hermes_called.get());
1033|1033|        assert!(!standard_called.get());
1034|1034|    }
1035|1035|
1036|1036|    #[test]
1037|1037|    fn test_try_parse_help_is_display_help() {
1038|1038|        match Cli::try_parse_from(["rtk", "--help"]) {
1039|1039|            Err(e) => assert_eq!(e.kind(), ErrorKind::DisplayHelp),
1040|1040|            Ok(_) => panic!("Expected DisplayHelp error"),
1041|1041|        }
1042|1042|    }
1043|1043|
1044|1044|    #[test]
1045|1045|    fn test_try_parse_version_is_display_version() {
1046|1046|        match Cli::try_parse_from(["rtk", "--version"]) {
1047|1047|            Err(e) => assert_eq!(e.kind(), ErrorKind::DisplayVersion),
1048|1048|            Ok(_) => panic!("Expected DisplayVersion error"),
1049|1049|        }
1050|1050|    }
1051|1051|
1052|1052|    #[test]
1053|1053|    fn test_try_parse_unknown_subcommand_is_error() {
1054|1054|        match Cli::try_parse_from(["rtk", "nonexistent-command"]) {
1055|1055|            Err(e) => assert!(!matches!(
1056|1056|                e.kind(),
1057|1057|                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
1058|1058|            )),
1059|1059|            Ok(_) => panic!("Expected parse error for unknown subcommand"),
1060|1060|        }
1061|1061|    }
1062|1062|
1063|1063|    #[test]
1064|1064|    fn test_try_parse_git_with_dash_c_succeeds() {
1065|1065|        let result = Cli::try_parse_from(["rtk", "git", "-C", "/path", "status"]);
1066|1066|        assert!(
1067|1067|            result.is_ok(),
1068|1068|            "git -C /path status should parse successfully"
1069|1069|        );
1070|1070|        if let Ok(cli) = result {
1071|1071|            match cli.command {
1072|1072|                Commands::Git { directory, .. } => {
1073|1073|                    assert_eq!(directory, vec!["/path"]);
1074|1074|                }
1075|1075|                _ => panic!("Expected Git command"),
1076|1076|            }
1077|1077|        }
1078|1078|    }
1079|1079|
1080|1080|    #[test]
1081|1081|    fn test_gain_failures_flag_parses() {
1082|1082|        let result = Cli::try_parse_from(["rtk", "gain", "--failures"]);
1083|1083|        assert!(result.is_ok());
1084|1084|        if let Ok(cli) = result {
1085|1085|            match cli.command {
1086|1086|                Commands::Gain { failures, .. } => assert!(failures),
1087|1087|                _ => panic!("Expected Gain command"),
1088|1088|            }
1089|1089|        }
1090|1090|    }
1091|1091|
1092|1092|    #[test]
1093|1093|    fn test_gain_failures_short_flag_parses() {
1094|1094|        let result = Cli::try_parse_from(["rtk", "gain", "-F"]);
1095|1095|        assert!(result.is_ok());
1096|1096|        if let Ok(cli) = result {
1097|1097|            match cli.command {
1098|1098|                Commands::Gain { failures, .. } => assert!(failures),
1099|1099|                _ => panic!("Expected Gain command"),
1100|1100|            }
1101|1101|        }
1102|1102|    }
1103|1103|
1104|1104|    #[test]
1105|1105|    fn test_meta_commands_reject_bad_flags() {
1106|1106|        // RTK meta-commands should produce parse errors (not fall through to raw execution).
1107|1107|        // Skip "proxy" because it uses trailing_var_arg (accepts any args by design).
1108|1108|        for cmd in RTK_META_COMMANDS {
1109|1109|            if matches!(*cmd, "proxy" | "run" | "rewrite" | "session") {
1110|1110|                continue; // these use trailing_var_arg (accept any args by design)
1111|1111|            }
1112|1112|            let result = Cli::try_parse_from(["rtk", cmd, "--nonexistent-flag-xyz"]);
1113|1113|            assert!(
1114|1114|                result.is_err(),
1115|1115|                "Meta-command '{}' with bad flag should fail to parse",
1116|1116|                cmd
1117|1117|            );
1118|1118|        }
1119|1119|    }
1120|1120|
1121|1121|    #[test]
1122|1122|    fn test_run_command_with_dash_c() {
1123|1123|        let cli = Cli::try_parse_from(["rtk", "run", "-c", "git status && echo done"]).unwrap();
1124|1124|        match cli.command {
1125|1125|            Commands::Run { command, args } => {
1126|1126|                assert_eq!(command, Some("git status && echo done".to_string()));
1127|1127|                assert!(args.is_empty());
1128|1128|            }
1129|1129|            _ => panic!("Expected Run command"),
1130|1130|        }
1131|1131|    }
1132|1132|
1133|1133|    #[test]
1134|1134|    fn test_run_command_positional_args() {
1135|1135|        let cli = Cli::try_parse_from(["rtk", "run", "echo", "hello"]).unwrap();
1136|1136|        match cli.command {
1137|1137|            Commands::Run { command, args } => {
1138|1138|                assert!(command.is_none());
1139|1139|                assert_eq!(args, vec!["echo", "hello"]);
1140|1140|            }
1141|1141|            _ => panic!("Expected Run command"),
1142|1142|        }
1143|1143|    }
1144|1144|
1145|1145|    #[test]
1146|1146|    fn test_hook_claude_parses() {
1147|1147|        let cli = Cli::try_parse_from(["rtk", "hook", "claude"]).unwrap();
1148|1148|        assert!(matches!(
1149|1149|            cli.command,
1150|1150|            Commands::Hook {
1151|1151|                command: HookCommands::Claude
1152|1152|            }
1153|1153|        ));
1154|1154|    }
1155|1155|
1156|1156|    #[test]
1157|1157|    fn test_hook_check_parses() {
1158|1158|        let cli = Cli::try_parse_from(["rtk", "hook", "check", "git", "status"]).unwrap();
1159|1159|        match cli.command {
1160|1160|            Commands::Hook {
1161|1161|                command: HookCommands::Check { agent, command },
1162|1162|            } => {
1163|1163|                assert_eq!(agent, "claude");
1164|1164|                assert_eq!(command, vec!["git", "status"]);
1165|1165|            }
1166|1166|            _ => panic!("Expected Hook Check command"),
1167|1167|        }
1168|1168|    }
1169|1169|
1170|1170|    #[test]
1171|1171|    fn test_hook_check_with_agent() {
1172|1172|        let cli =
1173|1173|            Cli::try_parse_from(["rtk", "hook", "check", "--agent", "gemini", "cargo", "test"])
1174|1174|                .unwrap();
1175|1175|        match cli.command {
1176|1176|            Commands::Hook {
1177|1177|                command: HookCommands::Check { agent, command },
1178|1178|            } => {
1179|1179|                assert_eq!(agent, "gemini");
1180|1180|                assert_eq!(command, vec!["cargo", "test"]);
1181|1181|            }
1182|1182|            _ => panic!("Expected Hook Check command"),
1183|1183|        }
1184|1184|    }
1185|1185|
1186|1186|    #[test]
1187|1187|    fn test_hook_check_preserves_double_dash_in_command() {
1188|1188|        let cli = Cli::try_parse_from([
1189|1189|            "rtk",
1190|1190|            "hook",
1191|1191|            "check",
1192|1192|            "shadowenv",
1193|1193|            "exec",
1194|1194|            "--",
1195|1195|            "git",
1196|1196|            "status",
1197|1197|        ])
1198|1198|        .unwrap();
1199|1199|        match cli.command {
1200|1200|            Commands::Hook {
1201|1201|                command: HookCommands::Check { agent, command },
1202|1202|            } => {
1203|1203|                assert_eq!(agent, "claude");
1204|1204|                assert_eq!(command, vec!["shadowenv", "exec", "--", "git", "status"]);
1205|1205|            }
1206|1206|            _ => panic!("Expected Hook Check command"),
1207|1207|        }
1208|1208|    }
1209|1209|
1210|1210|    #[test]
1211|1211|    fn test_meta_command_list_is_complete() {
1212|1212|        // Verify all meta-commands are in the guard list by checking they parse with valid syntax
1213|1213|        let meta_cmds_that_parse = [
1214|1214|            vec!["rtk", "gain"],
1215|1215|            vec!["rtk", "discover"],
1216|1216|            vec!["rtk", "learn"],
1217|1217|            vec!["rtk", "init"],
1218|1218|            vec!["rtk", "config"],
1219|1219|            vec!["rtk", "proxy", "echo", "hi"],
1220|1220|            vec!["rtk", "run", "-c", "echo hi"],
1221|1221|            vec!["rtk", "hook-audit"],
1222|1222|            vec!["rtk", "cc-economics"],
1223|1223|        ];
1224|1224|        for args in &meta_cmds_that_parse {
1225|1225|            let result = Cli::try_parse_from(args.iter());
1226|1226|            assert!(
1227|1227|                result.is_ok(),
1228|1228|                "Meta-command {:?} should parse successfully",
1229|1229|                args
1230|1230|            );
1231|1231|        }
1232|1232|    }
1233|1233|
1234|1234|    #[test]
1235|1235|    fn test_shell_split_simple() {
1236|1236|        assert_eq!(
1237|1237|            shell_split("head -50 file.php"),
1238|1238|            vec!["head", "-50", "file.php"]
1239|1239|        );
1240|1240|    }
1241|1241|
1242|1242|    #[test]
1243|1243|    fn test_shell_split_double_quotes() {
1244|1244|        assert_eq!(
1245|1245|            shell_split(r#"git log --format="%H %s""#),
1246|1246|            vec!["git", "log", "--format=%H %s"]
1247|1247|        );
1248|1248|    }
1249|1249|
1250|1250|    #[test]
1251|1251|    fn test_shell_split_single_quotes() {
1252|1252|        assert_eq!(
1253|1253|            shell_split("grep -r 'hello world' ."),
1254|1254|            vec!["grep", "-r", "hello world", "."]
1255|1255|        );
1256|1256|    }
1257|1257|
1258|1258|    #[test]
1259|1259|    fn test_shell_split_single_word() {
1260|1260|        assert_eq!(shell_split("ls"), vec!["ls"]);
1261|1261|    }
1262|1262|
1263|1263|    #[test]
1264|1264|    fn test_shell_split_empty() {
1265|1265|        let result: Vec<String> = shell_split("");
1266|1266|        assert!(result.is_empty());
1267|1267|    }
1268|1268|
1269|1269|    #[test]
1270|1270|    fn test_rewrite_clap_multi_args() {
1271|1271|        // This is the bug KuSh reported: `rtk rewrite ls -al` failed because
1272|1272|        // Clap rejected `-al` as an unknown flag. With trailing_var_arg + allow_hyphen_values,
1273|1273|        // multiple args are accepted and joined into a single command string.
1274|1274|        let cases = vec![
1275|1275|            vec!["rtk", "rewrite", "ls", "-al"],
1276|1276|            vec!["rtk", "rewrite", "git", "status"],
1277|1277|            vec!["rtk", "rewrite", "npm", "exec"],
1278|1278|            vec!["rtk", "rewrite", "cargo", "test"],
1279|1279|            vec!["rtk", "rewrite", "du", "-sh", "."],
1280|1280|            vec!["rtk", "rewrite", "head", "-50", "file.txt"],
1281|1281|        ];
1282|1282|        for args in &cases {
1283|1283|            let result = Cli::try_parse_from(args.iter());
1284|1284|            assert!(
1285|1285|                result.is_ok(),
1286|1286|                "rtk rewrite {:?} should parse (was failing before trailing_var_arg fix)",
1287|1287|                &args[2..]
1288|1288|            );
1289|1289|            if let Ok(cli) = result {
1290|1290|                match cli.command {
1291|1291|                    Commands::Rewrite { ref args } => {
1292|1292|                        assert!(args.len() >= 2, "rewrite args should capture all tokens");
1293|1293|                    }
1294|1294|                    _ => panic!("expected Rewrite command"),
1295|1295|                }
1296|1296|            }
1297|1297|        }
1298|1298|    }
1299|1299|
1300|1300|    #[test]
1301|1301|    fn test_rewrite_clap_quoted_single_arg() {
1302|1302|        // Quoted form: `rtk rewrite "git status"` — single arg containing spaces
1303|1303|        let result = Cli::try_parse_from(["rtk", "rewrite", "git status"]);
1304|1304|        assert!(result.is_ok());
1305|1305|        if let Ok(cli) = result {
1306|1306|            match cli.command {
1307|1307|                Commands::Rewrite { ref args } => {
1308|1308|                    assert_eq!(args.len(), 1);
1309|1309|                    assert_eq!(args[0], "git status");
1310|1310|                }
1311|1311|                _ => panic!("expected Rewrite command"),
1312|1312|            }
1313|1313|        }
1314|1314|    }
1315|1315|
1316|1316|    #[test]
1317|1317|    fn test_merge_filters_with_no_args() {
1318|1318|        let filters = vec![];
1319|1319|        let args = vec!["--depth=0".to_string(), "--no-verbose".to_string()];
1320|1320|        let expected_args = vec!["--depth=0", "--no-verbose"];
1321|1321|        assert_eq!(merge_pnpm_args(&filters, &args), expected_args);
1322|1322|    }
1323|1323|
1324|1324|    #[test]
1325|1325|    fn test_merge_filters_with_args() {
1326|1326|        let filters = vec!["@app1".to_string(), "@app2".to_string()];
1327|1327|        let args = vec![
1328|1328|            "--filter=@app3".to_string(),
1329|1329|            "--depth=0".to_string(),
1330|1330|            "--no-verbose".to_string(),
1331|1331|        ];
1332|1332|        let expected_args = vec![
1333|1333|            "--filter=@app1",
1334|1334|            "--filter=@app2",
1335|1335|            "--filter=@app3",
1336|1336|            "--depth=0",
1337|1337|            "--no-verbose",
1338|1338|        ];
1339|1339|        assert_eq!(merge_pnpm_args(&filters, &args), expected_args);
1340|1340|    }
1341|1341|
1342|1342|    #[test]
1343|1343|    fn test_merge_filters_with_no_args_os() {
1344|1344|        let filters = vec![];
1345|1345|        let args = vec![OsString::from("--depth=0")];
1346|1346|        let expected_args = vec![OsString::from("--depth=0")];
1347|1347|        assert_eq!(merge_pnpm_args_os(&filters, &args), expected_args);
1348|1348|    }
1349|1349|
1350|1350|    #[test]
1351|1351|    fn test_merge_filters_with_args_os() {
1352|1352|        let filters = vec!["@app1".to_string()];
1353|1353|        let args = vec![OsString::from("--depth=0")];
1354|1354|        let expected_args = vec![
1355|1355|            OsString::from("--filter=@app1"),
1356|1356|            OsString::from("--depth=0"),
1357|1357|        ];
1358|1358|        assert_eq!(merge_pnpm_args_os(&filters, &args), expected_args);
1359|1359|    }
1360|1360|
1361|1361|    #[test]
1362|1362|    fn test_pnpm_subcommand_with_filter() {
1363|1363|        let cli = Cli::try_parse_from([
1364|1364|            "rtk", "pnpm", "--filter", "@app1", "--filter", "@app2", "list", "--filter", "@app3",
1365|1365|            "--filter", "@app4", "--prod",
1366|1366|        ])
1367|1367|        .unwrap();
1368|1368|        match cli.command {
1369|1369|            Commands::Pnpm {
1370|1370|                filter,
1371|1371|                command: PnpmCommands::List { depth, args },
1372|1372|            } => {
1373|1373|                assert_eq!(depth, 0);
1374|1374|                assert_eq!(filter, vec!["@app1", "@app2"]);
1375|1375|                assert_eq!(
1376|1376|                    args,
1377|1377|                    vec!["--filter", "@app3", "--filter", "@app4", "--prod"]
1378|1378|                );
1379|1379|            }
1380|1380|            _ => panic!("Expected Pnpm List command"),
1381|1381|        }
1382|1382|    }
1383|1383|
1384|1384|    #[test]
1385|1385|    fn test_git_push_u_flag_passes_through() {
1386|1386|        let cli = Cli::try_parse_from(["rtk", "git", "push", "-u", "origin", "my-branch"]).unwrap();
1387|1387|        assert!(
1388|1388|            !cli.ultra_compact,
1389|1389|            "-u on git push must NOT be consumed as --ultra-compact"
1390|1390|        );
1391|1391|        match cli.command {
1392|1392|            Commands::Git {
1393|1393|                command: GitCommands::Push { args },
1394|1394|                ..
1395|1395|            } => {
1396|1396|                assert!(
1397|1397|                    args.contains(&"-u".to_string()),
1398|1398|                    "-u must be forwarded to git push, got: {:?}",
1399|1399|                    args
1400|1400|                );
1401|1401|            }
1402|1402|            _ => panic!("Expected Git Push command"),
1403|1403|        }
1404|1404|    }
1405|1405|
1406|1406|    #[test]
1407|1407|    fn test_pnpm_subcommand_with_short_filter() {
1408|1408|        // -F is the short form of --filter in pnpm
1409|1409|        let cli =
1410|1410|            Cli::try_parse_from(["rtk", "pnpm", "-F", "@app1", "-F", "@app2", "list"]).unwrap();
1411|1411|        match cli.command {
1412|1412|            Commands::Pnpm { filter, .. } => {
1413|1413|                assert_eq!(filter, vec!["@app1", "@app2"]);
1414|1414|            }
1415|1415|            _ => panic!("Expected Pnpm command"),
1416|1416|        }
1417|1417|    }
1418|1418|
1419|1419|    #[test]
1420|1420|    fn test_pnpm_typecheck_without_filters() {
1421|1421|        let cli = Cli::try_parse_from([
1422|1422|            "rtk",
1423|1423|            "pnpm",
1424|1424|            "typecheck",
1425|1425|            "--filter",
1426|1426|            "@app3",
1427|1427|            "--filter",
1428|1428|            "@app4",
1429|1429|        ])
1430|1430|        .unwrap();
1431|1431|        match cli.command {
1432|1432|            Commands::Pnpm { filter, command } => {
1433|1433|                let warning = validate_pnpm_filters(&filter, &command);
1434|1434|
1435|1435|                assert!(filter.is_empty());
1436|1436|                assert!(warning.is_none())
1437|1437|            }
1438|1438|            _ => panic!("Expected Pnpm Build command"),
1439|1439|        }
1440|1440|    }
1441|1441|
1442|1442|    #[test]
1443|1443|    fn test_pnpm_typecheck_with_filters() {
1444|1444|        let cli = Cli::try_parse_from([
1445|1445|            "rtk",
1446|1446|            "pnpm",
1447|1447|            "--filter",
1448|1448|            "@app1",
1449|1449|            "--filter",
1450|1450|            "@app2",
1451|1451|            "typecheck",
1452|1452|            "--filter",
1453|1453|            "@app3",
1454|1454|            "--filter",
1455|1455|            "@app4",
1456|1456|        ])
1457|1457|        .unwrap();
1458|1458|        match cli.command {
1459|1459|            Commands::Pnpm { filter, command } => {
1460|1460|                let warning = validate_pnpm_filters(&filter, &command).unwrap();
1461|1461|
1462|1462|                assert_eq!(filter, vec!["@app1", "@app2"]);
1463|1463|                assert_eq!(warning, "[rtk] warning: --filter is not yet supported for pnpm tsc, filters preceding the subcommand will be ignored")
1464|1464|            }
1465|1465|            _ => panic!("Expected Pnpm Build command"),
1466|1466|        }
1467|1467|    }
1468|1468|
1469|1469|    #[test]
1470|1470|    fn test_ultra_compact_long_form_still_works() {
1471|1471|        let cli = Cli::try_parse_from(["rtk", "--ultra-compact", "git", "status"]).unwrap();
1472|1472|        assert!(
1473|1473|            cli.ultra_compact,
1474|1474|            "--ultra-compact long form must still enable ultra-compact mode"
1475|1475|        );
1476|1476|    }
1477|1477|
1478|1478|    #[test]
1479|1479|    fn test_npx_unknown_tool_passthrough() {
1480|1480|        // The bug (rtk-ai/rtk#815) was that unknown tools under `rtk npx`
1481|1481|        // were dispatched to `npm` instead of `npx`. At the parse level, the
1482|1482|        // Npx variant must carry all args through unchanged so the dispatch
1483|1483|        // arm can forward them to npx.
1484|1484|        let cli = Cli::try_parse_from(["rtk", "npx", "cowsay", "hello"]).unwrap();
1485|1485|        match cli.command {
1486|1486|            Commands::Npx { args } => {
1487|1487|                assert_eq!(args, vec!["cowsay", "hello"]);
1488|1488|            }
1489|1489|            _ => panic!("Expected Commands::Npx for unknown tool"),
1490|1490|        }
1491|1491|    }
1492|1492|}
1493|1493|>>>>>>> 16803a6 (chore(filters): remove filter-level annotations and restore compose logs tail arg)
1494|1494|