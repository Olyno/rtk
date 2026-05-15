<<<<<<< HEAD
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
57

... [OUTPUT TRUNCATED - 130004 chars omitted out of 180004 total] ...

50 file.php' → cmd=head, args=["-50", "file.php"]
            // e.g. rtk proxy 'git log --format="%H %s"' → cmd=git, args=["log", "--format=%H %s"]
            let (cmd_name, cmd_args): (String, Vec<String>) = if args.len() == 1 {
                let full = args[0].to_string_lossy();
                let parts = shell_split(&full);
                if parts.len() > 1 {
                    (parts[0].clone(), parts[1..].to_vec())
                } else {
                    (full.into_owned(), vec![])
                }
            } else {
                (
                    args[0].to_string_lossy().into_owned(),
                    args[1..]
                        .iter()
                        .map(|s| s.to_string_lossy().into_owned())
                        .collect(),
                )
            };

            if cli.verbose > 0 {
                eprintln!("Proxy mode: {} {}", cmd_name, cmd_args.join(" "));
            }

            // ISSUE #897: Kill proxy child on SIGINT/SIGTERM to prevent orphan
            // processes. Drop-based ChildGuard doesn't run on signals with
            // panic=abort, so we register a signal handler that kills the child
            // PID stored in this atomic.
            static PROXY_CHILD_PID: AtomicU32 = AtomicU32::new(0);

            #[cfg(unix)]
            #[allow(unsafe_code)]
            {
                unsafe extern "C" fn handle_signal(sig: libc::c_int) {
                    let pid = PROXY_CHILD_PID.load(Ordering::SeqCst);
                    if pid != 0 {
                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
                        libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0);
                    }
                    libc::signal(sig, libc::SIG_DFL);
                    libc::raise(sig);
                }
                // nosemgrep: unsafe-block
                unsafe {
                    libc::signal(
                        libc::SIGINT,
                        handle_signal as *const () as libc::sighandler_t,
                    );
                    libc::signal(
                        libc::SIGTERM,
                        handle_signal as *const () as libc::sighandler_t,
                    );
                }
            }

            struct ChildGuard(Option<std::process::Child>);
            impl Drop for ChildGuard {
                fn drop(&mut self) {
                    if let Some(mut child) = self.0.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    PROXY_CHILD_PID.store(0, Ordering::SeqCst);
                }
            }

            let mut child = ChildGuard(Some(
                core::utils::resolved_command(cmd_name.as_ref())
                    .args(&cmd_args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .context(format!("Failed to execute command: {}", cmd_name))?,
            ));

            // Store child PID for signal handler before anything can fail
            if let Some(ref inner) = child.0 {
                PROXY_CHILD_PID.store(inner.id(), Ordering::SeqCst);
            }

            let inner = child.0.as_mut().context("Child process missing")?;
            let stdout_pipe = inner
                .stdout
                .take()
                .context("Failed to capture child stdout")?;
            let stderr_pipe = inner
                .stderr
                .take()
                .context("Failed to capture child stderr")?;

            const CAP: usize = 1_048_576;

            let stdout_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
                let mut reader = stdout_pipe;
                let mut captured = Vec::new();
                let mut buf = [0u8; 8192];

                loop {
                    let count = reader.read(&mut buf)?;
                    if count == 0 {
                        break;
                    }
                    if captured.len() < CAP {
                        let take = count.min(CAP - captured.len());
                        captured.extend_from_slice(&buf[..take]);
                    }
                    let mut out = std::io::stdout().lock();
                    out.write_all(&buf[..count])?;
                    out.flush()?;
                }

                Ok(captured)
            });

            let stderr_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
                let mut reader = stderr_pipe;
                let mut captured = Vec::new();
                let mut buf = [0u8; 8192];

                loop {
                    let count = reader.read(&mut buf)?;
                    if count == 0 {
                        break;
                    }
                    if captured.len() < CAP {
                        let take = count.min(CAP - captured.len());
                        captured.extend_from_slice(&buf[..take]);
                    }
                    let mut err = std::io::stderr().lock();
                    err.write_all(&buf[..count])?;
                    err.flush()?;
                }

                Ok(captured)
            });

            let status = child
                .0
                .take()
                .context("Child process missing")?
                .wait()
                .context(format!("Failed waiting for command: {}", cmd_name))?;

            let stdout_bytes = stdout_handle
                .join()
                .map_err(|_| anyhow::anyhow!("stdout streaming thread panicked"))??;
            let stderr_bytes = stderr_handle
                .join()
                .map_err(|_| anyhow::anyhow!("stderr streaming thread panicked"))??;

            let stdout = String::from_utf8_lossy(&stdout_bytes);
            let stderr = String::from_utf8_lossy(&stderr_bytes);
            let full_output = format!("{}{}", stdout, stderr);

            // Track usage (input = output since no filtering)
            timer.track(
                &format!("{} {}", cmd_name, cmd_args.join(" ")),
                &format!("rtk proxy {} {}", cmd_name, cmd_args.join(" ")),
                &full_output,
                &full_output,
            );

            core::utils::exit_code_from_status(&status, &cmd_name)
        }

        Commands::Trust { list } => {
            hooks::trust::run_trust(list)?;
            0
        }

        Commands::Untrust => {
            hooks::trust::run_untrust()?;
            0
        }

        Commands::Verify {
            filter,
            require_all,
        } => {
            if filter.is_some() {
                // Filter-specific mode: run only that filter's tests
                hooks::verify_cmd::run(filter, require_all)?;
            } else {
                // Default or --require-all: always run integrity check first
                hooks::integrity::run_verify(cli.verbose)?;
                hooks::verify_cmd::run(None, require_all)?;
            }
            0
        }
    };

    Ok(code)
}

/// Returns true for commands that are invoked via the hook pipeline
/// (i.e., commands that process rewritten shell commands).
/// Meta commands (init, gain, verify, etc.) are excluded because
/// they are run directly by the user, not through the hook.
/// Returns true for commands that go through the hook pipeline
/// and therefore require integrity verification.
///
/// SECURITY: whitelist pattern — new commands are NOT integrity-checked
/// until explicitly added here. A forgotten command fails open (no check)
/// rather than creating false confidence about what's protected.
fn is_operational_command(cmd: &Commands) -> bool {
    matches!(
        cmd,
        Commands::Ls { .. }
            | Commands::Tree { .. }
            | Commands::Read { .. }
            | Commands::Smart { .. }
            | Commands::Git { .. }
            | Commands::Gh { .. }
            | Commands::Glab { .. }
            | Commands::Pnpm { .. }
            | Commands::Err { .. }
            | Commands::Test { .. }
            | Commands::Json { .. }
            | Commands::Deps { .. }
            | Commands::Env { .. }
            | Commands::Find { .. }
            | Commands::Diff { .. }
            | Commands::Log { .. }
            | Commands::Dotnet { .. }
            | Commands::Docker { .. }
            | Commands::Kubectl { .. }
            | Commands::Summary { .. }
            | Commands::Grep { .. }
            | Commands::Wget { .. }
            | Commands::Vitest { .. }
            | Commands::Prisma { .. }
            | Commands::Tsc { .. }
            | Commands::Next { .. }
            | Commands::Lint { .. }
            | Commands::Prettier { .. }
            | Commands::Playwright { .. }
            | Commands::Cargo { .. }
            | Commands::Npm { .. }
            | Commands::Npx { .. }
            | Commands::Curl { .. }
            | Commands::Ruff { .. }
            | Commands::Pytest { .. }
            | Commands::Rake { .. }
            | Commands::Rubocop { .. }
            | Commands::Rspec { .. }
            | Commands::Pip { .. }
            | Commands::Go { .. }
            | Commands::GolangciLint { .. }
            | Commands::Gt { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::cell::Cell;

    #[test]
    fn test_git_commit_single_message() {
        let cli = Cli::try_parse_from(["rtk", "git", "commit", "-m", "fix: typo"]).unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(args, vec!["-m", "fix: typo"]);
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    #[test]
    fn test_git_commit_multiple_messages() {
        let cli = Cli::try_parse_from([
            "rtk",
            "git",
            "commit",
            "-m",
            "feat: add support",
            "-m",
            "Body paragraph here.",
        ])
        .unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(
                    args,
                    vec!["-m", "feat: add support", "-m", "Body paragraph here."]
                );
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    // #327: git commit -am "msg" was rejected by Clap
    #[test]
    fn test_git_commit_am_flag() {
        let cli = Cli::try_parse_from(["rtk", "git", "commit", "-am", "quick fix"]).unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(args, vec!["-am", "quick fix"]);
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    #[test]
    fn test_git_commit_amend() {
        let cli =
            Cli::try_parse_from(["rtk", "git", "commit", "--amend", "-m", "new msg"]).unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(args, vec!["--amend", "-m", "new msg"]);
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    #[test]
    fn test_git_global_options_parsing() {
        let cli =
            Cli::try_parse_from(["rtk", "git", "--no-pager", "--no-optional-locks", "status"])
                .unwrap();
        match cli.command {
            Commands::Git {
                no_pager,
                no_optional_locks,
                bare,
                literal_pathspecs,
                ..
            } => {
                assert!(no_pager);
                assert!(no_optional_locks);
                assert!(!bare);
                assert!(!literal_pathspecs);
            }
            _ => panic!("Expected Git command"),
        }
    }

    #[test]
    fn test_git_commit_long_flag_multiple() {
        let cli = Cli::try_parse_from([
            "rtk",
            "git",
            "commit",
            "--message",
            "title",
            "--message",
            "body",
            "--message",
            "footer",
        ])
        .unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(
                    args,
                    vec![
                        "--message",
                        "title",
                        "--message",
                        "body",
                        "--message",
                        "footer"
                    ]
                );
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    #[test]
    fn test_try_parse_valid_git_status() {
        let result = Cli::try_parse_from(["rtk", "git", "status"]);
        assert!(result.is_ok(), "git status should parse successfully");
    }

    #[test]
    fn test_try_parse_init_agent_hermes() {
        let cli = Cli::try_parse_from(["rtk", "init", "--agent", "hermes"]).unwrap();
        match cli.command {
            Commands::Init { agent, .. } => {
                assert_eq!(agent, Some(AgentTarget::Hermes));
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_try_parse_init_agent_hermes_uninstall() {
        let cli = Cli::try_parse_from(["rtk", "init", "--agent", "hermes", "--uninstall"]).unwrap();
        match cli.command {
            Commands::Init {
                agent, uninstall, ..
            } => {
                assert_eq!(agent, Some(AgentTarget::Hermes));
                assert!(uninstall);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_init_uninstall_dispatch_routes_hermes_to_hermes_cleanup() {
        let hermes_called = Cell::new(false);
        let standard_called = Cell::new(false);
        let ctx = hooks::init::InitContext {
            verbose: 2,
            dry_run: true,
        };

        let result = uninstall_init_dispatch(
            Some(AgentTarget::Hermes),
            true,
            false,
            false,
            ctx,
            |ctx| {
                hermes_called.set(true);
                assert_eq!(ctx.verbose, 2);
                assert!(ctx.dry_run);
                Ok(())
            },
            |_, _, _, _, _| {
                standard_called.set(true);
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert!(hermes_called.get());
        assert!(!standard_called.get());
    }

    #[test]
    fn test_try_parse_help_is_display_help() {
        match Cli::try_parse_from(["rtk", "--help"]) {
            Err(e) => assert_eq!(e.kind(), ErrorKind::DisplayHelp),
            Ok(_) => panic!("Expected DisplayHelp error"),
        }
    }

    #[test]
    fn test_try_parse_version_is_display_version() {
        match Cli::try_parse_from(["rtk", "--version"]) {
            Err(e) => assert_eq!(e.kind(), ErrorKind::DisplayVersion),
            Ok(_) => panic!("Expected DisplayVersion error"),
        }
    }

    #[test]
    fn test_try_parse_unknown_subcommand_is_error() {
        match Cli::try_parse_from(["rtk", "nonexistent-command"]) {
            Err(e) => assert!(!matches!(
                e.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            )),
            Ok(_) => panic!("Expected parse error for unknown subcommand"),
        }
    }

    #[test]
    fn test_try_parse_git_with_dash_c_succeeds() {
        let result = Cli::try_parse_from(["rtk", "git", "-C", "/path", "status"]);
        assert!(
            result.is_ok(),
            "git -C /path status should parse successfully"
        );
        if let Ok(cli) = result {
            match cli.command {
                Commands::Git { directory, .. } => {
                    assert_eq!(directory, vec!["/path"]);
                }
                _ => panic!("Expected Git command"),
            }
        }
    }

    #[test]
    fn test_gain_failures_flag_parses() {
        let result = Cli::try_parse_from(["rtk", "gain", "--failures"]);
        assert!(result.is_ok());
        if let Ok(cli) = result {
            match cli.command {
                Commands::Gain { failures, .. } => assert!(failures),
                _ => panic!("Expected Gain command"),
            }
        }
    }

    #[test]
    fn test_gain_failures_short_flag_parses() {
        let result = Cli::try_parse_from(["rtk", "gain", "-F"]);
        assert!(result.is_ok());
        if let Ok(cli) = result {
            match cli.command {
                Commands::Gain { failures, .. } => assert!(failures),
                _ => panic!("Expected Gain command"),
            }
        }
    }

    #[test]
    fn test_meta_commands_reject_bad_flags() {
        // RTK meta-commands should produce parse errors (not fall through to raw execution).
        // Skip "proxy" because it uses trailing_var_arg (accepts any args by design).
        for cmd in RTK_META_COMMANDS {
            if matches!(*cmd, "proxy" | "run" | "rewrite" | "session") {
                continue; // these use trailing_var_arg (accept any args by design)
            }
            let result = Cli::try_parse_from(["rtk", cmd, "--nonexistent-flag-xyz"]);
            assert!(
                result.is_err(),
                "Meta-command '{}' with bad flag should fail to parse",
                cmd
            );
        }
    }

    #[test]
    fn test_run_command_with_dash_c() {
        let cli = Cli::try_parse_from(["rtk", "run", "-c", "git status && echo done"]).unwrap();
        match cli.command {
            Commands::Run { command, args } => {
                assert_eq!(command, Some("git status && echo done".to_string()));
                assert!(args.is_empty());
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_positional_args() {
        let cli = Cli::try_parse_from(["rtk", "run", "echo", "hello"]).unwrap();
        match cli.command {
            Commands::Run { command, args } => {
                assert!(command.is_none());
                assert_eq!(args, vec!["echo", "hello"]);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_hook_claude_parses() {
        let cli = Cli::try_parse_from(["rtk", "hook", "claude"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Hook {
                command: HookCommands::Claude
            }
        ));
    }

    #[test]
    fn test_hook_check_parses() {
        let cli = Cli::try_parse_from(["rtk", "hook", "check", "git", "status"]).unwrap();
        match cli.command {
            Commands::Hook {
                command: HookCommands::Check { agent, command },
            } => {
                assert_eq!(agent, "claude");
                assert_eq!(command, vec!["git", "status"]);
            }
            _ => panic!("Expected Hook Check command"),
        }
    }

    #[test]
    fn test_hook_check_with_agent() {
        let cli =
            Cli::try_parse_from(["rtk", "hook", "check", "--agent", "gemini", "cargo", "test"])
                .unwrap();
        match cli.command {
            Commands::Hook {
                command: HookCommands::Check { agent, command },
            } => {
                assert_eq!(agent, "gemini");
                assert_eq!(command, vec!["cargo", "test"]);
            }
            _ => panic!("Expected Hook Check command"),
        }
    }

    #[test]
    fn test_hook_check_preserves_double_dash_in_command() {
        let cli = Cli::try_parse_from([
            "rtk",
            "hook",
            "check",
            "shadowenv",
            "exec",
            "--",
            "git",
            "status",
        ])
        .unwrap();
        match cli.command {
            Commands::Hook {
                command: HookCommands::Check { agent, command },
            } => {
                assert_eq!(agent, "claude");
                assert_eq!(command, vec!["shadowenv", "exec", "--", "git", "status"]);
            }
            _ => panic!("Expected Hook Check command"),
        }
    }

    #[test]
    fn test_meta_command_list_is_complete() {
        // Verify all meta-commands are in the guard list by checking they parse with valid syntax
        let meta_cmds_that_parse = [
            vec!["rtk", "gain"],
            vec!["rtk", "discover"],
            vec!["rtk", "learn"],
            vec!["rtk", "init"],
            vec!["rtk", "config"],
            vec!["rtk", "proxy", "echo", "hi"],
            vec!["rtk", "run", "-c", "echo hi"],
            vec!["rtk", "hook-audit"],
            vec!["rtk", "cc-economics"],
        ];
        for args in &meta_cmds_that_parse {
            let result = Cli::try_parse_from(args.iter());
            assert!(
                result.is_ok(),
                "Meta-command {:?} should parse successfully",
                args
            );
        }
    }

    #[test]
    fn test_shell_split_simple() {
        assert_eq!(
            shell_split("head -50 file.php"),
            vec!["head", "-50", "file.php"]
        );
    }

    #[test]
    fn test_shell_split_double_quotes() {
        assert_eq!(
            shell_split(r#"git log --format="%H %s""#),
            vec!["git", "log", "--format=%H %s"]
        );
    }

    #[test]
    fn test_shell_split_single_quotes() {
        assert_eq!(
            shell_split("grep -r 'hello world' ."),
            vec!["grep", "-r", "hello world", "."]
        );
    }

    #[test]
    fn test_shell_split_single_word() {
        assert_eq!(shell_split("ls"), vec!["ls"]);
    }

    #[test]
    fn test_shell_split_empty() {
        let result: Vec<String> = shell_split("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_rewrite_clap_multi_args() {
        // This is the bug KuSh reported: `rtk rewrite ls -al` failed because
        // Clap rejected `-al` as an unknown flag. With trailing_var_arg + allow_hyphen_values,
        // multiple args are accepted and joined into a single command string.
        let cases = vec![
            vec!["rtk", "rewrite", "ls", "-al"],
            vec!["rtk", "rewrite", "git", "status"],
            vec!["rtk", "rewrite", "npm", "exec"],
            vec!["rtk", "rewrite", "cargo", "test"],
            vec!["rtk", "rewrite", "du", "-sh", "."],
            vec!["rtk", "rewrite", "head", "-50", "file.txt"],
        ];
        for args in &cases {
            let result = Cli::try_parse_from(args.iter());
            assert!(
                result.is_ok(),
                "rtk rewrite {:?} should parse (was failing before trailing_var_arg fix)",
                &args[2..]
            );
            if let Ok(cli) = result {
                match cli.command {
                    Commands::Rewrite { ref args } => {
                        assert!(args.len() >= 2, "rewrite args should capture all tokens");
                    }
                    _ => panic!("expected Rewrite command"),
                }
            }
        }
    }

    #[test]
    fn test_rewrite_clap_quoted_single_arg() {
        // Quoted form: `rtk rewrite "git status"` — single arg containing spaces
        let result = Cli::try_parse_from(["rtk", "rewrite", "git status"]);
        assert!(result.is_ok());
        if let Ok(cli) = result {
            match cli.command {
                Commands::Rewrite { ref args } => {
                    assert_eq!(args.len(), 1);
                    assert_eq!(args[0], "git status");
                }
                _ => panic!("expected Rewrite command"),
            }
        }
    }

    #[test]
    fn test_merge_filters_with_no_args() {
        let filters = vec![];
        let args = vec!["--depth=0".to_string(), "--no-verbose".to_string()];
        let expected_args = vec!["--depth=0", "--no-verbose"];
        assert_eq!(merge_pnpm_args(&filters, &args), expected_args);
    }

    #[test]
    fn test_merge_filters_with_args() {
        let filters = vec!["@app1".to_string(), "@app2".to_string()];
        let args = vec![
            "--filter=@app3".to_string(),
            "--depth=0".to_string(),
            "--no-verbose".to_string(),
        ];
        let expected_args = vec![
            "--filter=@app1",
            "--filter=@app2",
            "--filter=@app3",
            "--depth=0",
            "--no-verbose",
        ];
        assert_eq!(merge_pnpm_args(&filters, &args), expected_args);
    }

    #[test]
    fn test_merge_filters_with_no_args_os() {
        let filters = vec![];
        let args = vec![OsString::from("--depth=0")];
        let expected_args = vec![OsString::from("--depth=0")];
        assert_eq!(merge_pnpm_args_os(&filters, &args), expected_args);
    }

    #[test]
    fn test_merge_filters_with_args_os() {
        let filters = vec!["@app1".to_string()];
        let args = vec![OsString::from("--depth=0")];
        let expected_args = vec![
            OsString::from("--filter=@app1"),
            OsString::from("--depth=0"),
        ];
        assert_eq!(merge_pnpm_args_os(&filters, &args), expected_args);
    }

    #[test]
    fn test_pnpm_subcommand_with_filter() {
        let cli = Cli::try_parse_from([
            "rtk", "pnpm", "--filter", "@app1", "--filter", "@app2", "list", "--filter", "@app3",
            "--filter", "@app4", "--prod",
        ])
        .unwrap();
        match cli.command {
            Commands::Pnpm {
                filter,
                command: PnpmCommands::List { depth, args },
            } => {
                assert_eq!(depth, 0);
                assert_eq!(filter, vec!["@app1", "@app2"]);
                assert_eq!(
                    args,
                    vec!["--filter", "@app3", "--filter", "@app4", "--prod"]
                );
            }
            _ => panic!("Expected Pnpm List command"),
        }
    }

    #[test]
    fn test_git_push_u_flag_passes_through() {
        let cli = Cli::try_parse_from(["rtk", "git", "push", "-u", "origin", "my-branch"]).unwrap();
        assert!(
            !cli.ultra_compact,
            "-u on git push must NOT be consumed as --ultra-compact"
        );
        match cli.command {
            Commands::Git {
                command: GitCommands::Push { args },
                ..
            } => {
                assert!(
                    args.contains(&"-u".to_string()),
                    "-u must be forwarded to git push, got: {:?}",
                    args
                );
            }
            _ => panic!("Expected Git Push command"),
        }
    }

    #[test]
    fn test_pnpm_subcommand_with_short_filter() {
        // -F is the short form of --filter in pnpm
        let cli =
            Cli::try_parse_from(["rtk", "pnpm", "-F", "@app1", "-F", "@app2", "list"]).unwrap();
        match cli.command {
            Commands::Pnpm { filter, .. } => {
                assert_eq!(filter, vec!["@app1", "@app2"]);
            }
            _ => panic!("Expected Pnpm command"),
        }
    }

    #[test]
    fn test_pnpm_typecheck_without_filters() {
        let cli = Cli::try_parse_from([
            "rtk",
            "pnpm",
            "typecheck",
            "--filter",
            "@app3",
            "--filter",
            "@app4",
        ])
        .unwrap();
        match cli.command {
            Commands::Pnpm { filter, command } => {
                let warning = validate_pnpm_filters(&filter, &command);

                assert!(filter.is_empty());
                assert!(warning.is_none())
            }
            _ => panic!("Expected Pnpm Build command"),
        }
    }

    #[test]
    fn test_pnpm_typecheck_with_filters() {
        let cli = Cli::try_parse_from([
            "rtk",
            "pnpm",
            "--filter",
            "@app1",
            "--filter",
            "@app2",
            "typecheck",
            "--filter",
            "@app3",
            "--filter",
            "@app4",
        ])
        .unwrap();
        match cli.command {
            Commands::Pnpm { filter, command } => {
                let warning = validate_pnpm_filters(&filter, &command).unwrap();

                assert_eq!(filter, vec!["@app1", "@app2"]);
                assert_eq!(warning, "[rtk] warning: --filter is not yet supported for pnpm tsc, filters preceding the subcommand will be ignored")
            }
            _ => panic!("Expected Pnpm Build command"),
        }
    }

    #[test]
    fn test_ultra_compact_long_form_still_works() {
        let cli = Cli::try_parse_from(["rtk", "--ultra-compact", "git", "status"]).unwrap();
        assert!(
            cli.ultra_compact,
            "--ultra-compact long form must still enable ultra-compact mode"
        );
    }

    #[test]
    fn test_npx_unknown_tool_passthrough() {
        // The bug (rtk-ai/rtk#815) was that unknown tools under `rtk npx`
        // were dispatched to `npm` instead of `npx`. At the parse level, the
        // Npx variant must carry all args through unchanged so the dispatch
        // arm can forward them to npx.
        let cli = Cli::try_parse_from(["rtk", "npx", "cowsay", "hello"]).unwrap();
        match cli.command {
            Commands::Npx { args } => {
                assert_eq!(args, vec!["cowsay", "hello"]);
            }
            _ => panic!("Expected Commands::Npx for unknown tool"),
        }
    }
}
>>>>>>> 16803a6 (chore(filters): remove filter-level annotations and restore compose logs tail arg)
