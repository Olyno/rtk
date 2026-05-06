1|<<<<<<< HEAD
2|1|<<<<<<< HEAD
3|2|=======
4|3|mod analytics;
5|4|mod cmds;
6|5|mod core;
7|6|mod discover;
8|7|mod hooks;
9|8|mod learn;
10|9|mod parser;
11|10|
12|11|// Re-export command modules for routing
13|12|use cmds::cloud::{aws_cmd, container, curl_cmd, psql_cmd, wget_cmd};
14|13|use cmds::dotnet::{binlog, dotnet_cmd, dotnet_format_report, dotnet_trx};
15|14|use cmds::git::{diff_cmd, gh_cmd, git, glab_cmd, gt_cmd};
16|15|use cmds::go::{go_cmd, golangci_cmd};
17|16|use cmds::js::{
18|17|    lint_cmd, next_cmd, npm_cmd, playwright_cmd, pnpm_cmd, prettier_cmd, prisma_cmd, tsc_cmd,
19|18|    vitest_cmd,
20|19|};
21|20|use cmds::python::{mypy_cmd, pip_cmd, pytest_cmd, ruff_cmd};
22|21|use cmds::ruby::{rake_cmd, rspec_cmd, rubocop_cmd};
23|22|use cmds::rust::{cargo_cmd, runner};
24|23|use cmds::system::{
25|24|    deps, env_cmd, find_cmd, format_cmd, grep_cmd, json_cmd, local_llm, log_cmd, ls, pipe_cmd,
26|25|    read, summary, tree, wc_cmd,
27|26|};
28|27|
29|28|use anyhow::{Context, Result};
30|29|use clap::error::ErrorKind;
31|30|use clap::{Parser, Subcommand, ValueEnum};
32|31|use std::ffi::OsString;
33|32|use std::path::{Path, PathBuf};
34|33|
35|34|/// Target agent for hook installation.
36|35|#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
37|36|pub enum AgentTarget {
38|37|    /// Claude Code (default)
39|38|    Claude,
40|39|    /// Cursor Agent (editor and CLI)
41|40|    Cursor,
42|41|    /// Windsurf IDE (Cascade)
43|42|    Windsurf,
44|43|    /// Cline / Roo Code (VS Code)
45|44|    Cline,
46|45|    /// Kilo Code
47|46|    Kilocode,
48|47|    /// Google Antigravity
49|48|    Antigravity,
50|49|    /// Pi coding agent
51|50|    Pi,
52|51|}
53|52|
54|53|#[derive(Parser)]
55|54|#[command(
56|55|    name = "rtk",
57|56|    version,
58|57|    about = "Rust Token Killer - Minimize LLM token consumption",
59|58|    long_about = "A high-performance CLI proxy designed to filter and summarize system outputs before they reach your LLM context."
60|59|)]
61|60|struct Cli {
62|61|    #[command(subcommand)]
63|62|    command: Commands,
64|63|
65|64|    /// Verbosity level (-v, -vv, -vvv)
66|65|    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
67|66|    verbose: u8,
68|67|
69|68|    /// Ultra-compact mode: ASCII icons, inline format (Level 2 optimizations)
70|69|    #[arg(long, global = true)]
71|70|    ultra_compact: bool,
72|71|
73|72|    /// Set SKIP_ENV_VALIDATION=1 for child processes (Next.js, tsc, lint, prisma)
74|73|    #[arg(long = "skip-env", global = true)]
75|74|    skip_env: bool,
76|75|}
77|76|
78|77|#[derive(Debug, Subcommand)]
79|78|enum Commands {
80|79|    /// List directory contents with token-optimized output (proxy to native ls)
81|80|    Ls {
82|81|        /// Arguments passed to ls (supports all native ls flags like -l, -a, -h, -R)
83|82|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
84|83|        args: Vec<String>,
85|84|    },
86|85|
87|86|    /// Directory tree with token-optimized output (proxy to native tree)
88|87|    Tree {
89|88|        /// Arguments passed to tree (supports all native tree flags like -L, -d, -a)
90|89|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
91|90|        args: Vec<String>,
92|91|    },
93|92|
94|93|    /// Read file with intelligent filtering
95|94|    Read {
96|95|        /// Files to read (supports multiple, like cat)
97|96|        #[arg(required = true, num_args = 1..)]
98|97|        files: Vec<PathBuf>,
99|98|        /// Filter: none (default, full content), minimal, aggressive
100|99|        #[arg(short, long, default_value = "none")]
101|100|        level: core::filter::FilterLevel,
102|101|        /// Max lines
103|102|        #[arg(short, long, conflicts_with = "tail_lines")]
104|103|        max_lines: Option<usize>,
105|104|        /// Keep only last N lines
106|105|        #[arg(long, conflicts_with = "max_lines")]
107|106|        tail_lines: Option<usize>,
108|107|        /// Show line numbers
109|108|        #[arg(short = 'n', long)]
110|109|        line_numbers: bool,
111|110|    },
112|111|
113|112|    /// Generate 2-line technical summary (heuristic-based)
114|113|    Smart {
115|114|        /// File to analyze
116|115|        file: PathBuf,
117|116|        /// Model: heuristic
118|117|        #[arg(short, long, default_value = "heuristic")]
119|118|        model: String,
120|119|        /// Force model download
121|120|        #[arg(long)]
122|121|        force_download: bool,
123|122|    },
124|123|
125|124|    /// Git commands with compact output
126|125|    Git {
127|126|        /// Change to directory before executing (like git -C <path>, can be repeated)
128|127|        #[arg(short = 'C', action = clap::ArgAction::Append)]
129|128|        directory: Vec<String>,
130|129|
131|130|        /// Git configuration override (like git -c key=value, can be repeated)
132|131|        #[arg(short = 'c', action = clap::ArgAction::Append)]
133|132|        config_override: Vec<String>,
134|133|
135|134|        /// Set the path to the .git directory
136|135|        #[arg(long = "git-dir")]
137|136|        git_dir: Option<String>,
138|137|
139|138|        /// Set the path to the working tree
140|139|        #[arg(long = "work-tree")]
141|140|        work_tree: Option<String>,
142|141|
143|142|        /// Disable pager (like git --no-pager)
144|143|        #[arg(long = "no-pager")]
145|144|        no_pager: bool,
146|145|
147|146|        /// Skip optional locks (like git --no-optional-locks)
148|147|        #[arg(long = "no-optional-locks")]
149|148|        no_optional_locks: bool,
150|149|
151|150|        /// Treat repository as bare (like git --bare)
152|151|        #[arg(long)]
153|152|        bare: bool,
154|153|
155|154|        /// Treat pathspecs literally (like git --literal-pathspecs)
156|155|        #[arg(long = "literal-pathspecs")]
157|156|        literal_pathspecs: bool,
158|157|
159|158|        #[command(subcommand)]
160|159|        command: GitCommands,
161|160|    },
162|161|
163|162|    /// GitHub CLI (gh) commands with token-optimized output
164|163|    Gh {
165|164|        /// Subcommand: pr, issue, run, repo
166|165|        subcommand: String,
167|166|        /// Additional arguments
168|167|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
169|168|        args: Vec<String>,
170|169|    },
171|170|
172|171|    /// GitLab CLI (glab) commands with token-optimized output
173|172|    Glab {
174|173|        /// Target repository (owner/repo), passed as glab -R flag
175|174|        #[arg(short = 'R', long = "repo")]
176|175|        repo: Option<String>,
177|176|        /// Target group, passed as glab -g flag
178|177|        #[arg(short = 'g', long = "group")]
179|178|        group: Option<String>,
180|179|        /// Subcommand: mr, issue, ci, pipeline, api
181|180|        subcommand: String,
182|181|        /// Additional arguments
183|182|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
184|183|        args: Vec<String>,
185|184|    },
186|185|
187|186|    /// AWS CLI with compact output (force JSON, compress)
188|187|    Aws {
189|188|        /// AWS service subcommand (e.g., sts, s3, ec2, ecs, rds, cloudformation)
190|189|        subcommand: String,
191|190|        /// Additional arguments
192|191|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
193|192|        args: Vec<String>,
194|193|    },
195|194|
196|195|    /// PostgreSQL client with compact output (strip borders, compress tables)
197|196|    #[command(disable_help_flag = true)]
198|197|    Psql {
199|198|        /// psql arguments
200|199|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
201|200|        args: Vec<String>,
202|201|    },
203|202|
204|203|    /// pnpm commands with ultra-compact output
205|204|    Pnpm {
206|205|        /// pnpm filter arguments (can be repeated: --filter @app1 --filter @app2)
207|206|        #[arg(long, short = 'F')]
208|207|        filter: Vec<String>,
209|208|
210|209|        #[command(subcommand)]
211|210|        command: PnpmCommands,
212|211|    },
213|212|
214|213|    /// Run command and show only errors/warnings
215|214|    Err {
216|215|        /// Command to run
217|216|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
218|217|        command: Vec<String>,
219|218|    },
220|219|
221|220|    /// Run tests and show only failures
222|221|    Test {
223|222|        /// Test command (e.g. cargo test)
224|223|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
225|224|        command: Vec<String>,
226|225|    },
227|226|
228|227|    /// Show JSON (compact values by default, or keys-only with --keys-only)
229|228|    Json {
230|229|        /// JSON file
231|230|        file: PathBuf,
232|231|        /// Max depth
233|232|        #[arg(short, long, default_value = "5")]
234|233|        depth: usize,
235|234|        /// Show keys only (strip all values, show structure)
236|235|        #[arg(long)]
237|236|        keys_only: bool,
238|237|    },
239|238|
240|239|    /// Summarize project dependencies
241|240|    Deps {
242|241|        /// Project path
243|242|        #[arg(default_value = ".")]
244|243|        path: PathBuf,
245|244|    },
246|245|
247|246|    /// Show environment variables (filtered, sensitive masked)
248|247|    Env {
249|248|        /// Filter by name (e.g. PATH, AWS)
250|249|        #[arg(short, long)]
251|250|        filter: Option<String>,
252|251|        /// Show all (include sensitive)
253|252|        #[arg(long)]
254|253|        show_all: bool,
255|254|    },
256|255|
257|256|    /// Find files with compact tree output (accepts native find flags like -name, -type)
258|257|    Find {
259|258|        /// All find arguments (supports both RTK and native find syntax)
260|259|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
261|260|        args: Vec<String>,
262|261|    },
263|262|
264|263|    /// Ultra-condensed diff (only changed lines)
265|264|    Diff {
266|265|        /// First file or - for stdin (unified diff)
267|266|        file1: PathBuf,
268|267|        /// Second file (optional if stdin)
269|268|        file2: Option<PathBuf>,
270|269|    },
271|270|
272|271|    /// Filter and deduplicate log output
273|272|    Log {
274|273|        /// Log file (omit for stdin)
275|274|        file: Option<PathBuf>,
276|275|    },
277|276|
278|277|    /// .NET commands with compact output (build/test/restore/format)
279|278|    Dotnet {
280|279|        #[command(subcommand)]
281|280|        command: DotnetCommands,
282|281|    },
283|282|
284|283|    /// Docker commands with compact output
285|284|    Docker {
286|285|        #[command(subcommand)]
287|286|        command: DockerCommands,
288|287|    },
289|288|
290|289|    /// Kubectl commands with compact output
291|290|    Kubectl {
292|291|        #[command(subcommand)]
293|292|        command: KubectlCommands,
294|293|    },
295|294|
296|295|    /// Run command and show heuristic summary
297|296|    Summary {
298|297|        /// Command to run and summarize
299|298|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
300|299|        command: Vec<String>,
301|300|    },
302|301|
303|302|    /// Compact grep - strips whitespace, truncates, groups by file
304|303|    Grep {
305|304|        /// Pattern to search
306|305|        pattern: String,
307|306|        /// Path to search in
308|307|        #[arg(default_value = ".")]
309|308|        path: String,
310|309|        /// Max line length
311|310|        #[arg(short = 'l', long, default_value = "80")]
312|311|        max_len: usize,
313|312|        /// Max results to show
314|313|        #[arg(short, long, default_value = "200")]
315|314|        max: usize,
316|315|        /// Show only match context (not full line)
317|316|        #[arg(long)]
318|317|        context_only: bool,
319|318|        /// Filter by file type (e.g., ts, py, rust)
320|319|        #[arg(short = 't', long)]
321|320|        file_type: Option<String>,
322|321|        /// Show line numbers (always on, accepted for grep/rg compatibility)
323|322|        #[arg(short = 'n', long)]
324|323|        line_numbers: bool,
325|324|        /// Extra ripgrep arguments (e.g., -i, -A 3, -w, --glob)
326|325|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
327|326|        extra_args: Vec<String>,
328|327|    },
329|328|
330|329|    /// Initialize rtk instructions for assistant CLI usage
331|330|    Init {
332|331|        /// Add to global assistant config directory instead of local project file
333|332|        #[arg(short, long)]
334|333|        global: bool,
335|334|
336|335|        /// Install OpenCode plugin (in addition to Claude Code)
337|336|        #[arg(long)]
338|337|        opencode: bool,
339|338|
340|339|        /// Initialize for Gemini CLI instead of Claude Code
341|340|        #[arg(long)]
342|341|        gemini: bool,
343|342|
344|343|        /// Target agent to install hooks for (default: claude)
345|344|        #[arg(long, value_enum)]
346|345|        agent: Option<AgentTarget>,
347|346|
348|347|        /// Show current configuration
349|348|        #[arg(long)]
350|349|        show: bool,
351|350|
352|351|        /// Inject full instructions into CLAUDE.md (legacy mode)
353|352|        #[arg(long = "claude-md", group = "mode")]
354|353|        claude_md: bool,
355|354|
356|355|        /// Hook only, no RTK.md
357|356|        #[arg(long = "hook-only", group = "mode")]
358|357|        hook_only: bool,
359|358|
360|359|        /// Auto-patch settings.json without prompting
361|360|        #[arg(long = "auto-patch", group = "patch")]
362|361|        auto_patch: bool,
363|362|
364|363|        /// Skip settings.json patching (print manual instructions)
365|364|        #[arg(long = "no-patch", group = "patch")]
366|365|        no_patch: bool,
367|366|
368|367|        /// Remove RTK artifacts for the selected assistant mode
369|368|        #[arg(long)]
370|369|        uninstall: bool,
371|370|
372|371|        /// Target Codex CLI (uses AGENTS.md + RTK.md, no Claude hook patching)
373|372|        #[arg(long)]
374|373|        codex: bool,
375|374|
376|375|        /// Install GitHub Copilot integration (VS Code + CLI)
377|376|        #[arg(long)]
378|377|        copilot: bool,
379|378|
380|379|        /// Install Pi coding agent extension
381|380|        #[arg(long)]
382|381|        pi: bool,
383|382|    },
384|383|
385|384|    /// Download with compact output (strips progress bars)
386|385|    Wget {
387|386|        /// URL to download
388|387|        url: String,
389|388|        /// Output file (-O - for stdout)
390|389|        #[arg(short = 'O', long = "output-document", allow_hyphen_values = true)]
391|390|        output: Option<String>,
392|391|        /// Additional wget arguments
393|392|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
394|393|        args: Vec<String>,
395|394|    },
396|395|
397|396|    /// Word/line/byte count with compact output (strips paths and padding)
398|397|    Wc {
399|398|        /// Arguments passed to wc (files, flags like -l, -w, -c)
400|399|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
401|400|        args: Vec<String>,
402|401|    },
403|402|
404|403|    /// Show token savings summary and history
405|404|    Gain {
406|405|        /// Filter statistics to current project (current working directory) // added
407|406|        #[arg(short, long)]
408|407|        project: bool,
409|408|        /// Show ASCII graph of daily savings
410|409|        #[arg(short, long)]
411|410|        graph: bool,
412|411|        /// Show recent command history
413|412|        #[arg(short = 'H', long)]
414|413|        history: bool,
415|414|        /// Show monthly quota savings estimate
416|415|        #[arg(short, long)]
417|416|        quota: bool,
418|417|        /// Subscription tier for quota calculation: pro, 5x, 20x
419|418|        #[arg(short, long, default_value = "20x", requires = "quota")]
420|419|        tier: String,
421|420|        /// Show detailed daily breakdown (all days)
422|421|        #[arg(short, long)]
423|422|        daily: bool,
424|423|        /// Show weekly breakdown
425|424|        #[arg(short, long)]
426|425|        weekly: bool,
427|426|        /// Show monthly breakdown
428|427|        #[arg(short, long)]
429|428|        monthly: bool,
430|429|        /// Show all time breakdowns (daily + weekly + monthly)
431|430|        #[arg(short, long)]
432|431|        all: bool,
433|432|        /// Output format: text, json, csv
434|433|        #[arg(short, long, default_value = "text")]
435|434|        format: String,
436|435|        /// Show parse failure log (commands that fell back to raw execution)
437|436|        #[arg(short = 'F', long)]
438|437|        failures: bool,
439|438|        /// Reset all token savings stats to zero
440|439|        #[arg(long)]
441|440|        reset: bool,
442|441|        /// Skip confirmation prompt when resetting
443|442|        #[arg(long, requires = "reset")]
444|443|        yes: bool,
445|444|    },
446|445|
447|446|    /// Claude Code economics: spending (ccusage) vs savings (rtk) analysis
448|447|    CcEconomics {
449|448|        /// Show detailed daily breakdown
450|449|        #[arg(short, long)]
451|450|        daily: bool,
452|451|        /// Show weekly breakdown
453|452|        #[arg(short, long)]
454|453|        weekly: bool,
455|454|        /// Show monthly breakdown
456|455|        #[arg(short, long)]
457|456|        monthly: bool,
458|457|        /// Show all time breakdowns (daily + weekly + monthly)
459|458|        #[arg(short, long)]
460|459|        all: bool,
461|460|        /// Output format: text, json, csv
462|461|        #[arg(short, long, default_value = "text")]
463|462|        format: String,
464|463|    },
465|464|
466|465|    /// Show or create configuration file
467|466|    Config {
468|467|        /// Create default config file
469|468|        #[arg(long)]
470|469|        create: bool,
471|470|    },
472|471|
473|472|    /// Jest commands with compact output
474|473|    Jest {
475|474|        /// Additional jest arguments
476|475|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
477|476|        args: Vec<String>,
478|477|    },
479|478|
480|479|    /// Vitest commands with compact output
481|480|    Vitest {
482|481|        /// Additional vitest arguments
483|482|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
484|483|        args: Vec<String>,
485|484|    },
486|485|
487|486|    /// Prisma commands with compact output (no ASCII art)
488|487|    Prisma {
489|488|        #[command(subcommand)]
490|489|        command: PrismaCommands,
491|490|    },
492|491|
493|492|    /// TypeScript compiler with grouped error output
494|493|    Tsc {
495|494|        /// TypeScript compiler arguments
496|495|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
497|496|        args: Vec<String>,
498|497|    },
499|498|
500|499|    /// Next.js build with compact output
501|500|    Next {
502|501|        /// Next.js build arguments
503|502|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
504|503|        args: Vec<String>,
505|504|    },
506|505|
507|506|    /// ESLint with grouped rule violations
508|507|    Lint {
509|508|        /// Linter arguments
510|509|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
511|510|        args: Vec<String>,
512|511|    },
513|512|
514|513|    /// Prettier format checker with compact output
515|514|    Prettier {
516|515|        /// Prettier arguments (e.g., --check, --write)
517|516|        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
518|517|        args: Vec<String>,
519|518|    },
520|519|
521|520|    /// Universal format checker (prettier, black, ruff format)
522|521|    Format {
523|522|        /// Formatter arguments (auto-detects formatter from project files)
524|523|    

... [OUTPUT TRUNCATED - 33838 chars omitted out of 83838 total] ...

       _ => None,
1333|1332|    }
1334|1333|}
1335|1334|
1336|1335|fn main() {
1337|1336|    let code = match run_cli() {
1338|1337|        Ok(code) => code,
1339|1338|        Err(e) => {
1340|1339|            eprintln!("rtk: {:#}", e);
1341|1340|            1
1342|1341|        }
1343|1342|    };
1344|1343|    std::process::exit(code);
1345|1344|}
1346|1345|
1347|1346|fn run_cli() -> Result<i32> {
1348|1347|    // Fire-and-forget telemetry ping (1/day, non-blocking)
1349|1348|    core::telemetry::maybe_ping();
1350|1349|
1351|1350|    let cli = match Cli::try_parse() {
1352|1351|        Ok(cli) => cli,
1353|1352|        Err(e) => {
1354|1353|            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
1355|1354|                e.exit();
1356|1355|            }
1357|1356|            return run_fallback(e);
1358|1357|        }
1359|1358|    };
1360|1359|
1361|1360|    // Warn if installed hook is outdated/missing (1/day, non-blocking).
1362|1361|    // Skip for Gain — it shows its own inline hook warning.
1363|1362|    if !matches!(cli.command, Commands::Gain { .. }) {
1364|1363|        hooks::hook_check::maybe_warn();
1365|1364|    }
1366|1365|
1367|1366|    // Runtime integrity check for operational commands.
1368|1367|    // Meta commands (init, gain, verify, config, etc.) skip the check
1369|1368|    // because they don't go through the hook pipeline.
1370|1369|    if is_operational_command(&cli.command) {
1371|1370|        hooks::integrity::runtime_check()?;
1372|1371|    }
1373|1372|
1374|1373|    let code = match cli.command {
1375|1374|        Commands::Ls { args } => ls::run(&args, cli.verbose)?,
1376|1375|
1377|1376|        Commands::Tree { args } => tree::run(&args, cli.verbose)?,
1378|1377|
1379|1378|        // ISSUE #989: support multiple files (cat file1 file2 → rtk read file1 file2)
1380|1379|        Commands::Read {
1381|1380|            files,
1382|1381|            level,
1383|1382|            max_lines,
1384|1383|            tail_lines,
1385|1384|            line_numbers,
1386|1385|        } => {
1387|1386|            let mut had_error = false;
1388|1387|            let mut stdin_seen = false;
1389|1388|            for file in &files {
1390|1389|                let result = if file == Path::new("-") {
1391|1390|                    if stdin_seen {
1392|1391|                        eprintln!("rtk: warning: stdin specified more than once");
1393|1392|                        continue;
1394|1393|                    }
1395|1394|                    stdin_seen = true;
1396|1395|                    read::run_stdin(level, max_lines, tail_lines, line_numbers, cli.verbose)
1397|1396|                } else {
1398|1397|                    read::run(
1399|1398|                        file,
1400|1399|                        level,
1401|1400|                        max_lines,
1402|1401|                        tail_lines,
1403|1402|                        line_numbers,
1404|1403|                        cli.verbose,
1405|1404|                    )
1406|1405|                };
1407|1406|                if let Err(e) = result {
1408|1407|                    eprintln!("cat: {}: {}", file.display(), e.root_cause());
1409|1408|                    had_error = true;
1410|1409|                }
1411|1410|            }
1412|1411|            if had_error {
1413|1412|                1
1414|1413|            } else {
1415|1414|                0
1416|1415|            }
1417|1416|        }
1418|1417|
1419|1418|        Commands::Smart {
1420|1419|            file,
1421|1420|            model,
1422|1421|            force_download,
1423|1422|        } => {
1424|1423|            local_llm::run(&file, &model, force_download, cli.verbose)?;
1425|1424|            0
1426|1425|        }
1427|1426|
1428|1427|        Commands::Git {
1429|1428|            directory,
1430|1429|            config_override,
1431|1430|            git_dir,
1432|1431|            work_tree,
1433|1432|            no_pager,
1434|1433|            no_optional_locks,
1435|1434|            bare,
1436|1435|            literal_pathspecs,
1437|1436|            command,
1438|1437|        } => {
1439|1438|            // Build global git args (inserted between "git" and subcommand)
1440|1439|            let mut global_args: Vec<String> = Vec::new();
1441|1440|            for dir in &directory {
1442|1441|                global_args.push("-C".to_string());
1443|1442|                global_args.push(dir.clone());
1444|1443|            }
1445|1444|            for cfg in &config_override {
1446|1445|                global_args.push("-c".to_string());
1447|1446|                global_args.push(cfg.clone());
1448|1447|            }
1449|1448|            if let Some(ref dir) = git_dir {
1450|1449|                global_args.push("--git-dir".to_string());
1451|1450|                global_args.push(dir.clone());
1452|1451|            }
1453|1452|            if let Some(ref tree) = work_tree {
1454|1453|                global_args.push("--work-tree".to_string());
1455|1454|                global_args.push(tree.clone());
1456|1455|            }
1457|1456|            if no_pager {
1458|1457|                global_args.push("--no-pager".to_string());
1459|1458|            }
1460|1459|            if no_optional_locks {
1461|1460|                global_args.push("--no-optional-locks".to_string());
1462|1461|            }
1463|1462|            if bare {
1464|1463|                global_args.push("--bare".to_string());
1465|1464|            }
1466|1465|            if literal_pathspecs {
1467|1466|                global_args.push("--literal-pathspecs".to_string());
1468|1467|            }
1469|1468|
1470|1469|            match command {
1471|1470|                GitCommands::Diff { args } => git::run(
1472|1471|                    git::GitCommand::Diff,
1473|1472|                    &args,
1474|1473|                    None,
1475|1474|                    cli.verbose,
1476|1475|                    &global_args,
1477|1476|                )?,
1478|1477|                GitCommands::Log { args } => {
1479|1478|                    git::run(git::GitCommand::Log, &args, None, cli.verbose, &global_args)?
1480|1479|                }
1481|1480|                GitCommands::Status { args } => git::run(
1482|1481|                    git::GitCommand::Status,
1483|1482|                    &args,
1484|1483|                    None,
1485|1484|                    cli.verbose,
1486|1485|                    &global_args,
1487|1486|                )?,
1488|1487|                GitCommands::Show { args } => git::run(
1489|1488|                    git::GitCommand::Show,
1490|1489|                    &args,
1491|1490|                    None,
1492|1491|                    cli.verbose,
1493|1492|                    &global_args,
1494|1493|                )?,
1495|1494|                GitCommands::Add { args } => {
1496|1495|                    git::run(git::GitCommand::Add, &args, None, cli.verbose, &global_args)?
1497|1496|                }
1498|1497|                GitCommands::Commit { args } => git::run(
1499|1498|                    git::GitCommand::Commit,
1500|1499|                    &args,
1501|1500|                    None,
1502|1501|                    cli.verbose,
1503|1502|                    &global_args,
1504|1503|                )?,
1505|1504|                GitCommands::Push { args } => git::run(
1506|1505|                    git::GitCommand::Push,
1507|1506|                    &args,
1508|1507|                    None,
1509|1508|                    cli.verbose,
1510|1509|                    &global_args,
1511|1510|                )?,
1512|1511|                GitCommands::Pull { args } => git::run(
1513|1512|                    git::GitCommand::Pull,
1514|1513|                    &args,
1515|1514|                    None,
1516|1515|                    cli.verbose,
1517|1516|                    &global_args,
1518|1517|                )?,
1519|1518|                GitCommands::Branch { args } => git::run(
1520|1519|                    git::GitCommand::Branch,
1521|1520|                    &args,
1522|1521|                    None,
1523|1522|                    cli.verbose,
1524|1523|                    &global_args,
1525|1524|                )?,
1526|1525|                GitCommands::Fetch { args } => git::run(
1527|1526|                    git::GitCommand::Fetch,
1528|1527|                    &args,
1529|1528|                    None,
1530|1529|                    cli.verbose,
1531|1530|                    &global_args,
1532|1531|                )?,
1533|1532|                GitCommands::Stash { subcommand, args } => git::run(
1534|1533|                    git::GitCommand::Stash { subcommand },
1535|1534|                    &args,
1536|1535|                    None,
1537|1536|                    cli.verbose,
1538|1537|                    &global_args,
1539|1538|                )?,
1540|1539|                GitCommands::Worktree { args } => git::run(
1541|1540|                    git::GitCommand::Worktree,
1542|1541|                    &args,
1543|1542|                    None,
1544|1543|                    cli.verbose,
1545|1544|                    &global_args,
1546|1545|                )?,
1547|1546|                GitCommands::Other(args) => git::run_passthrough(&args, &global_args, cli.verbose)?,
1548|1547|            }
1549|1548|        }
1550|1549|
1551|1550|        Commands::Gh { subcommand, args } => {
1552|1551|            gh_cmd::run(&subcommand, &args, cli.verbose, cli.ultra_compact)?
1553|1552|        }
1554|1553|
1555|1554|        Commands::Glab {
1556|1555|            repo,
1557|1556|            group,
1558|1557|            subcommand,
1559|1558|            mut args,
1560|1559|        } => {
1561|1560|            // Append -R / -g flags at end so they don't interfere with
1562|1561|            // subcommand dispatch (args[0] must be the sub-subcommand like "list")
1563|1562|            if let Some(r) = repo {
1564|1563|                args.push("-R".to_string());
1565|1564|                args.push(r);
1566|1565|            }
1567|1566|            if let Some(g) = group {
1568|1567|                args.push("-g".to_string());
1569|1568|                args.push(g);
1570|1569|            }
1571|1570|            glab_cmd::run(&subcommand, &args, cli.verbose, cli.ultra_compact)?
1572|1571|        }
1573|1572|
1574|1573|        Commands::Aws { subcommand, args } => aws_cmd::run(&subcommand, &args, cli.verbose)?,
1575|1574|
1576|1575|        Commands::Psql { args } => psql_cmd::run(&args, cli.verbose)?,
1577|1576|
1578|1577|        Commands::Pnpm { filter, command } => {
1579|1578|            // Warns user if filters are used with unsupported subcommands like typecheck
1580|1579|            if let Some(warning) = validate_pnpm_filters(&filter, &command) {
1581|1580|                eprintln!("{}", warning);
1582|1581|            }
1583|1582|
1584|1583|            match command {
1585|1584|                PnpmCommands::List { depth, args } => pnpm_cmd::run(
1586|1585|                    pnpm_cmd::PnpmCommand::List { depth },
1587|1586|                    &merge_pnpm_args(&filter, &args),
1588|1587|                    cli.verbose,
1589|1588|                )?,
1590|1589|                PnpmCommands::Outdated { args } => pnpm_cmd::run(
1591|1590|                    pnpm_cmd::PnpmCommand::Outdated,
1592|1591|                    &merge_pnpm_args(&filter, &args),
1593|1592|                    cli.verbose,
1594|1593|                )?,
1595|1594|                PnpmCommands::Install { args } => pnpm_cmd::run(
1596|1595|                    pnpm_cmd::PnpmCommand::Install,
1597|1596|                    &merge_pnpm_args(&filter, &args),
1598|1597|                    cli.verbose,
1599|1598|                )?,
1600|1599|                PnpmCommands::Typecheck { args } => tsc_cmd::run(&args, cli.verbose)?,
1601|1600|                PnpmCommands::Other(args) => {
1602|1601|                    pnpm_cmd::run_passthrough(&merge_pnpm_args_os(&filter, &args), cli.verbose)?
1603|1602|                }
1604|1603|            }
1605|1604|        }
1606|1605|
1607|1606|        Commands::Err { command } => {
1608|1607|            let cmd = command.join(" ");
1609|1608|            runner::run_err(&cmd, cli.verbose)?
1610|1609|        }
1611|1610|
1612|1611|        Commands::Test { command } => {
1613|1612|            let cmd = command.join(" ");
1614|1613|            runner::run_test(&cmd, cli.verbose)?
1615|1614|        }
1616|1615|
1617|1616|        Commands::Json {
1618|1617|            file,
1619|1618|            depth,
1620|1619|            keys_only,
1621|1620|        } => {
1622|1621|            if file == Path::new("-") {
1623|1622|                json_cmd::run_stdin(depth, keys_only, cli.verbose)?;
1624|1623|            } else {
1625|1624|                json_cmd::run(&file, depth, keys_only, cli.verbose)?;
1626|1625|            }
1627|1626|            0
1628|1627|        }
1629|1628|
1630|1629|        Commands::Deps { path } => {
1631|1630|            deps::run(&path, cli.verbose)?;
1632|1631|            0
1633|1632|        }
1634|1633|
1635|1634|        Commands::Env { filter, show_all } => {
1636|1635|            env_cmd::run(filter.as_deref(), show_all, cli.verbose)?;
1637|1636|            0
1638|1637|        }
1639|1638|
1640|1639|        Commands::Find { args } => {
1641|1640|            find_cmd::run_from_args(&args, cli.verbose)?;
1642|1641|            0
1643|1642|        }
1644|1643|
1645|1644|        Commands::Diff { file1, file2 } => {
1646|1645|            if let Some(f2) = file2 {
1647|1646|                diff_cmd::run(&file1, &f2, cli.verbose)?;
1648|1647|            } else {
1649|1648|                diff_cmd::run_stdin(cli.verbose)?;
1650|1649|            }
1651|1650|            0
1652|1651|        }
1653|1652|
1654|1653|        Commands::Log { file } => {
1655|1654|            if let Some(f) = file {
1656|1655|                log_cmd::run_file(&f, cli.verbose)?;
1657|1656|            } else {
1658|1657|                log_cmd::run_stdin(cli.verbose)?;
1659|1658|            }
1660|1659|            0
1661|1660|        }
1662|1661|
1663|1662|        Commands::Dotnet { command } => match command {
1664|1663|            DotnetCommands::Build { args } => dotnet_cmd::run_build(&args, cli.verbose)?,
1665|1664|            DotnetCommands::Test { args } => dotnet_cmd::run_test(&args, cli.verbose)?,
1666|1665|            DotnetCommands::Restore { args } => dotnet_cmd::run_restore(&args, cli.verbose)?,
1667|1666|            DotnetCommands::Format { args } => dotnet_cmd::run_format(&args, cli.verbose)?,
1668|1667|            DotnetCommands::Other(args) => dotnet_cmd::run_passthrough(&args, cli.verbose)?,
1669|1668|        },
1670|1669|
1671|1670|        Commands::Docker { command } => match command {
1672|1671|            DockerCommands::Ps => {
1673|1672|                container::run(container::ContainerCmd::DockerPs, &[], cli.verbose)?
1674|1673|            }
1675|1674|            DockerCommands::Images => {
1676|1675|                container::run(container::ContainerCmd::DockerImages, &[], cli.verbose)?
1677|1676|            }
1678|1677|            DockerCommands::Logs { container: c } => {
1679|1678|                container::run(container::ContainerCmd::DockerLogs, &[c], cli.verbose)?
1680|1679|            }
1681|1680|            DockerCommands::Compose { command: compose } => match compose {
1682|1681|                ComposeCommands::Ps => container::run_compose_ps(cli.verbose)?,
1683|1682|                ComposeCommands::Logs { service } => {
1684|1683|                    container::run_compose_logs(service.as_deref(), cli.verbose)?
1685|1684|                }
1686|1685|                ComposeCommands::Build { service } => {
1687|1686|                    container::run_compose_build(service.as_deref(), cli.verbose)?
1688|1687|                }
1689|1688|                ComposeCommands::Other(args) => {
1690|1689|                    container::run_compose_passthrough(&args, cli.verbose)?
1691|1690|                }
1692|1691|            },
1693|1692|            DockerCommands::Other(args) => container::run_docker_passthrough(&args, cli.verbose)?,
1694|1693|        },
1695|1694|
1696|1695|        Commands::Kubectl { command } => match command {
1697|1696|            KubectlCommands::Pods { namespace, all } => {
1698|1697|                let mut args: Vec<String> = Vec::new();
1699|1698|                if all {
1700|1699|                    args.push("-A".to_string());
1701|1700|                } else if let Some(n) = namespace {
1702|1701|                    args.push("-n".to_string());
1703|1702|                    args.push(n);
1704|1703|                }
1705|1704|                container::run(container::ContainerCmd::KubectlPods, &args, cli.verbose)?
1706|1705|            }
1707|1706|            KubectlCommands::Services { namespace, all } => {
1708|1707|                let mut args: Vec<String> = Vec::new();
1709|1708|                if all {
1710|1709|                    args.push("-A".to_string());
1711|1710|                } else if let Some(n) = namespace {
1712|1711|                    args.push("-n".to_string());
1713|1712|                    args.push(n);
1714|1713|                }
1715|1714|                container::run(container::ContainerCmd::KubectlServices, &args, cli.verbose)?
1716|1715|            }
1717|1716|            KubectlCommands::Logs { pod, container: c } => {
1718|1717|                let mut args = vec![pod];
1719|1718|                if let Some(cont) = c {
1720|1719|                    args.push("-c".to_string());
1721|1720|                    args.push(cont);
1722|1721|                }
1723|1722|                container::run(container::ContainerCmd::KubectlLogs, &args, cli.verbose)?
1724|1723|            }
1725|1724|            KubectlCommands::Other(args) => container::run_kubectl_passthrough(&args, cli.verbose)?,
1726|1725|        },
1727|1726|
1728|1727|        Commands::Summary { command } => {
1729|1728|            let cmd = command.join(" ");
1730|1729|            summary::run(&cmd, cli.verbose)?
1731|1730|        }
1732|1731|
1733|1732|        Commands::Grep {
1734|1733|            pattern,
1735|1734|            path,
1736|1735|            max_len,
1737|1736|            max,
1738|1737|            context_only,
1739|1738|            file_type,
1740|1739|            line_numbers: _, // no-op: line numbers always enabled in grep_cmd::run
1741|1740|            extra_args,
1742|1741|        } => grep_cmd::run(
1743|1742|            &pattern,
1744|1743|            &path,
1745|1744|            max_len,
1746|1745|            max,
1747|1746|            context_only,
1748|1747|            file_type.as_deref(),
1749|1748|            &extra_args,
1750|1749|            cli.verbose,
1751|1750|        )?,
1752|1751|
1753|1752|        Commands::Init {
1754|1753|            global,
1755|1754|            opencode,
1756|1755|            gemini,
1757|1756|            agent,
1758|1757|            show,
1759|1758|            claude_md,
1760|1759|            hook_only,
1761|1760|            auto_patch,
1762|1761|            no_patch,
1763|1762|            uninstall,
1764|1763|            codex,
1765|1764|            copilot,
1766|1765|            pi,
1767|1766|        } => {
1768|1767|            if show {
1769|1768|                hooks::init::show_config(codex)?
1770|1769|            } else if uninstall {
1771|1770|                let cursor = agent == Some(AgentTarget::Cursor);
1772|1771|                let pi = pi || agent == Some(AgentTarget::Pi);
1773|1772|                hooks::init::uninstall(global, gemini, codex, cursor, pi, cli.verbose)?;
1774|1773|            } else if gemini {
1775|1774|                let patch_mode = if auto_patch {
1776|1775|                    hooks::init::PatchMode::Auto
1777|1776|                } else if no_patch {
1778|1777|                    hooks::init::PatchMode::Skip
1779|1778|                } else {
1780|1779|                    hooks::init::PatchMode::Ask
1781|1780|                };
1782|1781|                hooks::init::run_gemini(global, hook_only, patch_mode, cli.verbose)?;
1783|1782|            } else if copilot {
1784|1783|                hooks::init::run_copilot(cli.verbose)?;
1785|1784|            } else if agent == Some(AgentTarget::Pi) {
1786|1785|                hooks::init::run_pi_mode(global, cli.verbose)?;
1787|1786|            } else if agent == Some(AgentTarget::Kilocode) {
1788|1787|                if global {
1789|1788|                    anyhow::bail!("Kilo Code is project-scoped. Use: rtk init --agent kilocode");
1790|1789|                }
1791|1790|                hooks::init::run_kilocode_mode(cli.verbose)?;
1792|1791|            } else if agent == Some(AgentTarget::Antigravity) {
1793|1792|                if global {
1794|1793|                    anyhow::bail!(
1795|1794|                        "Antigravity is project-scoped. Use: rtk init --agent antigravity"
1796|1795|                    );
1797|1796|                }
1798|1797|                hooks::init::run_antigravity_mode(cli.verbose)?;
1799|1798|            } else {
1800|1799|                let install_opencode = opencode;
1801|1800|                let install_claude = !opencode;
1802|1801|                let install_cursor = agent == Some(AgentTarget::Cursor);
1803|1802|                let install_windsurf = agent == Some(AgentTarget::Windsurf);
1804|1803|                let install_cline = agent == Some(AgentTarget::Cline);
1805|1804|
1806|1805|                let patch_mode = if auto_patch {
1807|1806|                    hooks::init::PatchMode::Auto
1808|1807|                } else if no_patch {
1809|1808|                    hooks::init::PatchMode::Skip
1810|1809|                } else {
1811|1810|                    hooks::init::PatchMode::Ask
1812|1811|                };
1813|1812|                hooks::init::run(
1814|1813|                    global,
1815|1814|                    install_claude,
1816|1815|                    install_opencode,
1817|1816|                    install_cursor,
1818|1817|                    install_windsurf,
1819|1818|                    install_cline,
1820|1819|                    claude_md,
1821|1820|                    hook_only,
1822|1821|                    codex,
1823|1822|                    patch_mode,
1824|1823|                    cli.verbose,
1825|1824|                )?;
1826|1825|                if pi {
1827|1826|                    hooks::init::run_pi_mode(global, cli.verbose)?;
1828|1827|                }
1829|1828|            }
1830|1829|            0
1831|1830|        }
1832|1831|
1833|1832|        Commands::Wget { url, output, args } => {
1834|1833|            if output.as_deref() == Some("-") {
1835|1834|                wget_cmd::run_stdout(&url, &args, cli.verbose)?
1836|1835|            } else {
1837|1836|                // Pass -O <file> through to wget via args
1838|1837|                let mut all_args = Vec::new();
1839|1838|                if let Some(out_file) = &output {
1840|1839|                    all_args.push("-O".to_string());
1841|1840|                    all_args.push(out_file.clone());
1842|1841|                }
1843|1842|                all_args.extend(args);
1844|1843|                wget_cmd::run(&url, &all_args, cli.verbose)?
1845|1844|            }
1846|1845|        }
1847|1846|
1848|1847|        Commands::Wc { args } => wc_cmd::run(&args, cli.verbose)?,
1849|1848|
1850|1849|        Commands::Gain {
1851|1850|            project, // added
1852|1851|            graph,
1853|1852|            history,
1854|1853|            quota,
1855|1854|            tier,
1856|1855|            daily,
1857|1856|            weekly,
1858|1857|            monthly,
1859|1858|            all,
1860|1859|            format,
1861|1860|            failures,
1862|1861|            reset,
1863|1862|            yes,
1864|1863|        } => {
1865|1864|            analytics::gain::run(
1866|1865|                project, // added: pass project flag
1867|1866|                graph,
1868|1867|                history,
1869|1868|                quota,
1870|1869|                &tier,
1871|1870|                daily,
1872|1871|                weekly,
1873|1872|                monthly,
1874|1873|                all,
1875|1874|                &format,
1876|1875|                failures,
1877|1876|                reset,
1878|1877|                yes,
1879|1878|                cli.verbose,
1880|1879|            )?;
1881|1880|            0
1882|1881|        }
1883|1882|
1884|1883|        Commands::CcEconomics {
1885|1884|            daily,
1886|1885|            weekly,
1887|1886|            monthly,
1888|1887|            all,
1889|1888|            format,
1890|1889|        } => {
1891|1890|            analytics::cc_economics::run(daily, weekly, monthly, all, &format, cli.verbose)?;
1892|1891|            0
1893|1892|        }
1894|1893|
1895|1894|        Commands::Config { create } => {
1896|1895|            if create {
1897|1896|                let path = core::config::Config::create_default()?;
1898|1897|                println!("Created: {}", path.display());
1899|1898|            } else {
1900|1899|                core::config::show_config()?;
1901|1900|            }
1902|1901|            0
1903|1902|        }
1904|1903|
1905|1904|        Commands::Jest { ref args } | Commands::Vitest { ref args } => {
1906|1905|            vitest_cmd::run_test(&cli.command, args, cli.verbose)?
1907|1906|        }
1908|1907|
1909|1908|        Commands::Prisma { command } => match command {
1910|1909|            PrismaCommands::Generate { args } => {
1911|1910|                prisma_cmd::run(prisma_cmd::PrismaCommand::Generate, &args, cli.verbose)?
1912|1911|            }
1913|1912|            PrismaCommands::Migrate { command } => match command {
1914|1913|                PrismaMigrateCommands::Dev { name, args } => prisma_cmd::run(
1915|1914|                    prisma_cmd::PrismaCommand::Migrate {
1916|1915|                        subcommand: prisma_cmd::MigrateSubcommand::Dev { name },
1917|1916|                    },
1918|1917|                    &args,
1919|1918|                    cli.verbose,
1920|1919|                )?,
1921|1920|                PrismaMigrateCommands::Status { args } => prisma_cmd::run(
1922|1921|                    prisma_cmd::PrismaCommand::Migrate {
1923|1922|                        subcommand: prisma_cmd::MigrateSubcommand::Status,
1924|1923|                    },
1925|1924|                    &args,
1926|1925|                    cli.verbose,
1927|1926|                )?,
1928|1927|                PrismaMigrateCommands::Deploy { args } => prisma_cmd::run(
1929|1928|                    prisma_cmd::PrismaCommand::Migrate {
1930|1929|                        subcommand: prisma_cmd::MigrateSubcommand::Deploy,
1931|1930|                    },
1932|1931|                    &args,
1933|1932|                    cli.verbose,
1934|1933|                )?,
1935|1934|            },
1936|1935|            PrismaCommands::DbPush { args } => {
1937|1936|                prisma_cmd::run(prisma_cmd::PrismaCommand::DbPush, &args, cli.verbose)?
1938|1937|            }
1939|1938|        },
1940|1939|
1941|1940|        Commands::Tsc { args } => tsc_cmd::run(&args, cli.verbose)?,
1942|1941|
1943|1942|        Commands::Next { args } => next_cmd::run(&args, cli.verbose)?,
1944|1943|
1945|1944|        Commands::Lint { args } => lint_cmd::run(&args, cli.verbose)?,
1946|1945|
1947|1946|        Commands::Prettier { args } => prettier_cmd::run(&args, cli.verbose)?,
1948|1947|
1949|1948|        Commands::Format { args } => format_cmd::run(&args, cli.verbose)?,
1950|1949|
1951|1950|        Commands::Playwright { args } => playwright_cmd::run(&args, cli.verbose)?,
1952|1951|
1953|1952|        Commands::Cargo { command } => match command {
1954|1953|            CargoCommands::Build { args } => {
1955|1954|                cargo_cmd::run(cargo_cmd::CargoCommand::Build, &args, cli.verbose)?
1956|1955|            }
1957|1956|            CargoCommands::Test { args } => {
1958|1957|                cargo_cmd::run(cargo_cmd::CargoCommand::Test, &args, cli.verbose)?
1959|1958|            }
1960|1959|            CargoCommands::Clippy { args } => {
1961|1960|                cargo_cmd::run(cargo_cmd::CargoCommand::Clippy, &args, cli.verbose)?
1962|1961|            }
1963|1962|            CargoCommands::Check { args } => {
1964|1963|                cargo_cmd::run(cargo_cmd::CargoCommand::Check, &args, cli.verbose)?
1965|1964|            }
1966|1965|            CargoCommands::Install { args } => {
1967|1966|                cargo_cmd::run(cargo_cmd::CargoCommand::Install, &args, cli.verbose)?
1968|1967|            }
1969|1968|            CargoCommands::Nextest { args } => {
1970|1969|                cargo_cmd::run(cargo_cmd::CargoCommand::Nextest, &args, cli.verbose)?
1971|1970|            }
1972|1971|            CargoCommands::Other(args) => cargo_cmd::run_passthrough(&args, cli.verbose)?,
1973|1972|        },
1974|1973|
1975|1974|        Commands::Npm { args } => npm_cmd::run(&args, cli.verbose, cli.skip_env)?,
1976|1975|
1977|1976|        Commands::Curl { args } => curl_cmd::run(&args, cli.verbose)?,
1978|1977|
1979|1978|        Commands::Discover {
1980|1979|            project,
1981|1980|            limit,
1982|1981|            all,
1983|1982|            since,
1984|1983|            format,
1985|1984|        } => {
1986|1985|            discover::run(project.as_deref(), all, since, limit, &format, cli.verbose)?;
1987|1986|            0
1988|1987|        }
1989|1988|
1990|1989|        Commands::Session {} => {
1991|1990|            analytics::session_cmd::run(cli.verbose)?;
1992|1991|            0
1993|1992|        }
1994|1993|
1995|1994|        Commands::Telemetry { command } => {
1996|1995|            core::telemetry_cmd::run(&command)?;
1997|1996|            0
1998|1997|        }
1999|1998|
2000|1999|        Commands::Learn {
2001|