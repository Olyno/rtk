1|<<<<<<< HEAD
2|<<<<<<< HEAD
3|<<<<<<< HEAD
4|1|//! Filters git output — log, status, diff, and more — keeping just the essential info.
5|2|
6|3|use crate::core::stream::{
7|4|    self, exec_capture, CaptureResult, FilterMode, LineHandler, LineStreamFilter, StdinMode,
8|5|};
9|6|use crate::core::tracking;
10|7|use crate::core::truncate::CAP_WARNINGS;
11|8|use crate::core::utils::{exit_code_from_output, exit_code_from_status, resolved_command};
12|9|use anyhow::{Context, Result};
13|10|use std::ffi::OsString;
14|11|use std::process::Command;
15|12|use std::process::Stdio;
16|13|
17|14|#[derive(Debug, Clone)]
18|15|pub enum GitCommand {
19|16|    Diff,
20|17|    Log,
21|18|    Status,
22|19|    Show,
23|20|    Add,
24|21|    Commit,
25|22|    Push,
26|23|    Pull,
27|24|    Branch,
28|25|    Fetch,
29|26|    Stash { subcommand: Option<String> },
30|27|    Worktree,
31|28|}
32|29|
33|30|/// Create a git Command with global options (e.g. -C, -c, --git-dir, --work-tree)
34|31|/// prepended before any subcommand arguments.
35|32|fn git_cmd(global_args: &[String]) -> Command {
36|33|    let mut cmd = resolved_command("git");
37|34|    for arg in global_args {
38|35|        cmd.arg(arg);
39|36|    }
40|37|    cmd
41|38|}
42|39|
43|40|/// Create a git Command for internal parsing that must be locale-stable.
44|41|///
45|42|/// We only use this for non-user-facing parses where RTK depends on git's
46|43|/// English status phrases. User-visible passthrough output keeps the user's
47|44|/// locale.
48|45|fn git_cmd_c_locale(global_args: &[String]) -> Command {
49|46|    let mut cmd = git_cmd(global_args);
50|47|    cmd.env("LC_ALL", "C");
51|48|    cmd
52|49|}
53|50|
54|51|fn uses_compact_status_path(args: &[String]) -> bool {
55|52|    if args.is_empty() {
56|53|        return true;
57|54|    }
58|55|
59|56|    let mut saw_branch = false;
60|57|    for arg in args {
61|58|        match arg.as_str() {
62|59|            "-b" | "--branch" => saw_branch = true,
63|60|            "-sb" | "-bs" => return true,
64|61|            "-s" | "--short" => {}
65|62|            _ => return false,
66|63|        }
67|64|    }
68|65|
69|66|    saw_branch
70|67|}
71|68|
72|69|fn build_status_command(args: &[String], global_args: &[String]) -> Command {
73|70|    let mut cmd = git_cmd(global_args);
74|71|    cmd.arg("status");
75|72|    if uses_compact_status_path(args) {
76|73|        cmd.args(["--porcelain", "-b"]);
77|74|    } else {
78|75|        cmd.args(args);
79|76|    }
80|77|    cmd
81|78|}
82|79|
83|80|pub fn run(
84|81|    cmd: GitCommand,
85|82|    args: &[String],
86|83|    max_lines: Option<usize>,
87|84|    verbose: u8,
88|85|    global_args: &[String],
89|86|) -> Result<i32> {
90|87|    match cmd {
91|88|        GitCommand::Diff => run_diff(args, max_lines, verbose, global_args),
92|89|        GitCommand::Log => run_log(args, max_lines, verbose, global_args),
93|90|        GitCommand::Status => run_status(args, verbose, global_args),
94|91|        GitCommand::Show => run_show(args, max_lines, verbose, global_args),
95|92|        GitCommand::Add => run_add(args, verbose, global_args),
96|93|        GitCommand::Commit => run_commit(args, verbose, global_args),
97|94|        GitCommand::Push => run_push(args, verbose, global_args),
98|95|        GitCommand::Pull => run_pull(args, verbose, global_args),
99|96|        GitCommand::Branch => run_branch(args, verbose, global_args),
100|97|        GitCommand::Fetch => run_fetch(args, verbose, global_args),
101|98|        GitCommand::Stash { subcommand } => {
102|99|            run_stash(subcommand.as_deref(), args, verbose, global_args)
103|100|        }
104|101|        GitCommand::Worktree => run_worktree(args, verbose, global_args),
105|102|    }
106|103|}
107|104|
108|105|/// Re-insert `--` before the first path-like argument when clap has consumed it.
109|106|///
110|107|/// clap's `trailing_var_arg = true` silently drops `--` when it appears as the
111|108|/// first positional argument (before any other positional).  This means:
112|109|///   `rtk git diff -- file` → args = ["file"]   (clap ate `--`)
113|110|///   `rtk git diff HEAD -- file` → args = ["HEAD", "--", "file"]  (preserved)
114|111|///
115|112|/// Without the `--` separator git may treat an unambiguous path as a revision and
116|113|/// emit "fatal: ambiguous argument".  We re-insert `--` before the first path-like
117|114|/// argument; see `normalize_diff_args_impl` for the detection rules.
118|115|fn normalize_diff_args(args: &[String]) -> Vec<String> {
119|116|    normalize_diff_args_impl(args, |p| std::path::Path::new(p).exists())
120|117|}
121|118|
122|119|/// Testable core of `normalize_diff_args` — accepts an injectable filesystem existence checker.
123|120|///
124|121|/// The path-detection logic is:
125|122|/// 1. Explicit path prefixes (`.`, `~`) → always a path, no filesystem check needed.
126|123|/// 2. Contains path separator (`/`, `\`) → use `path_exists` to distinguish branch names
127|124|///    (e.g. `feature/auth`) from real paths (e.g. `src/main.rs`).
128|125|/// 3. Bare word with no separator → never a path (avoids injecting `--` when a file
129|126|///    happens to share a name with a branch or ref, e.g. a file named `main`).
130|127|fn normalize_diff_args_impl<F>(args: &[String], path_exists: F) -> Vec<String>
131|128|where
132|129|    F: Fn(&str) -> bool,
133|130|{
134|131|    // Already has `--` — nothing to do
135|132|    if args.iter().any(|a| a == "--") {
136|133|        return args.to_vec();
137|134|    }
138|135|    let path_start = args.iter().position(|arg| {
139|136|        if arg.starts_with('-') {
140|137|            return false;
141|138|        }
142|139|        // Explicit path prefixes — always treat as path regardless of existence
143|140|        if arg.starts_with('.') || arg.starts_with('~') {
144|141|            return true;
145|142|        }
146|143|        // Contains path separator — use filesystem check to distinguish
147|144|        // branch names (feature/auth) from real paths (src/main.rs)
148|145|        if arg.contains('/') || arg.contains('\\') {
149|146|            return path_exists(arg);
150|147|        }
151|148|        // Bare word (no separator, no special prefix) — never inject `--`
152|149|        // This avoids misidentifying a ref/branch as a path even if a same-named
153|150|        // file happens to exist on disk.
154|151|        false
155|152|    });
156|153|    match path_start {
157|154|        Some(idx) => {
158|155|            let mut out = args[..idx].to_vec();
159|156|            out.push("--".to_string());
160|157|            out.extend_from_slice(&args[idx..]);
161|158|            out
162|159|        }
163|160|        None => args.to_vec(),
164|161|    }
165|162|}
166|163|
167|164|fn run_diff(
168|165|    args: &[String],
169|166|    max_lines: Option<usize>,
170|167|    verbose: u8,
171|168|    global_args: &[String],
172|169|) -> Result<i32> {
173|170|    let timer = tracking::TimedExecution::start();
174|171|
175|172|    // Re-insert `--` when clap's trailing_var_arg consumed it (issue #1215)
176|173|    let args = &normalize_diff_args(args);
177|174|
178|175|    // Check if user wants stat output
179|176|    let wants_stat = args
180|177|        .iter()
181|178|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
182|179|
183|180|    // Check if user wants compact diff (default RTK behavior)
184|181|    let wants_compact = !args.iter().any(|arg| arg == "--no-compact");
185|182|
186|183|    if wants_stat || !wants_compact {
187|184|        // User wants stat or explicitly no compacting - pass through directly
188|185|        let mut cmd = git_cmd(global_args);
189|186|        cmd.arg("diff");
190|187|        for arg in args {
191|188|            if arg == "--no-compact" {
192|189|                continue; // RTK flag, not a git flag
193|190|            }
194|191|            cmd.arg(arg);
195|192|        }
196|193|
197|194|        let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
198|195|
199|196|        if !result.success() {
200|197|            eprintln!("{}", result.stderr);
201|198|            return Ok(result.exit_code);
202|199|        }
203|200|
204|201|        println!("{}", result.stdout.trim());
205|202|
206|203|        timer.track(
207|204|            &format!("git diff {}", args.join(" ")),
208|205|            &format!("rtk git diff {} (passthrough)", args.join(" ")),
209|206|            &result.stdout,
210|207|            &result.stdout,
211|208|        );
212|209|
213|210|        return Ok(0);
214|211|    }
215|212|
216|213|    // Default RTK behavior: stat first, then compacted diff
217|214|    let mut cmd = git_cmd(global_args);
218|215|    cmd.arg("diff").arg("--stat");
219|216|
220|217|    for arg in args {
221|218|        cmd.arg(arg);
222|219|    }
223|220|
224|221|    let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
225|222|
226|223|    if !result.success() {
227|224|        if !result.stderr.trim().is_empty() {
228|225|            eprint!("{}", result.stderr);
229|226|        }
230|227|        timer.track(
231|228|            &format!("git diff {}", args.join(" ")),
232|229|            &format!("rtk git diff {}", args.join(" ")),
233|230|            &result.stdout,
234|231|            &result.stdout,
235|232|        );
236|233|        return Ok(result.exit_code);
237|234|    }
238|235|
239|236|    if verbose > 0 {
240|237|        eprintln!("Git diff summary:");
241|238|    }
242|239|
243|240|    // Print stat summary first
244|241|    println!("{}", result.stdout.trim());
245|242|
246|243|    // Now get actual diff but compact it
247|244|    let mut diff_cmd = git_cmd(global_args);
248|245|    diff_cmd.arg("diff");
249|246|    for arg in args {
250|247|        diff_cmd.arg(arg);
251|248|    }
252|249|
253|250|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git diff")?;
254|251|
255|252|    let mut final_output = result.stdout.clone();
256|253|    if !diff_result.stdout.is_empty() {
257|254|        println!("\n--- Changes ---");
258|255|        let compacted = compact_diff(&diff_result.stdout, max_lines.unwrap_or(500));
259|256|        println!("{}", compacted);
260|257|        final_output.push_str("\n--- Changes ---\n");
261|258|        final_output.push_str(&compacted);
262|259|    }
263|260|
264|261|    timer.track(
265|262|        &format!("git diff {}", args.join(" ")),
266|263|        &format!("rtk git diff {}", args.join(" ")),
267|264|        &format!("{}\n{}", result.stdout, diff_result.stdout),
268|265|        &final_output,
269|266|    );
270|267|
271|268|    Ok(0)
272|269|}
273|270|
274|271|fn run_show(
275|272|    args: &[String],
276|273|    max_lines: Option<usize>,
277|274|    verbose: u8,
278|275|    global_args: &[String],
279|276|) -> Result<i32> {
280|277|    let timer = tracking::TimedExecution::start();
281|278|
282|279|    // If user wants --stat or --format only, pass through
283|280|    let wants_stat_only = args
284|281|        .iter()
285|282|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
286|283|
287|284|    let wants_format = args
288|285|        .iter()
289|286|        .any(|arg| arg.starts_with("--pretty") || arg.starts_with("--format"));
290|287|
291|288|    // `git show rev:path` prints a blob, not a commit diff. In this mode we should
292|289|    // pass through directly to avoid duplicated output from compact-show steps.
293|290|    let wants_blob_show = args.iter().any(|arg| is_blob_show_arg(arg));
294|291|
295|292|    if wants_stat_only || wants_format || wants_blob_show {
296|293|        let mut cmd = git_cmd(global_args);
297|294|        cmd.arg("show");
298|295|        for arg in args {
299|296|            cmd.arg(arg);
300|297|        }
301|298|        let result = exec_capture(&mut cmd).context("Failed to run git show")?;
302|299|        if !result.success() {
303|300|            eprintln!("{}", result.stderr);
304|301|            return Ok(result.exit_code);
305|302|        }
306|303|        if wants_blob_show {
307|304|            print!("{}", result.stdout);
308|305|        } else {
309|306|            println!("{}", result.stdout.trim());
310|307|        }
311|308|
312|309|        timer.track(
313|310|            &format!("git show {}", args.join(" ")),
314|311|            &format!("rtk git show {} (passthrough)", args.join(" ")),
315|312|            &result.stdout,
316|313|            &result.stdout,
317|314|        );
318|315|
319|316|        return Ok(0);
320|317|    }
321|318|
322|319|    // Get raw output for tracking
323|320|    let mut raw_cmd = git_cmd(global_args);
324|321|    raw_cmd.arg("show");
325|322|    for arg in args {
326|323|        raw_cmd.arg(arg);
327|324|    }
328|325|    let raw_output = exec_capture(&mut raw_cmd)
329|326|        .map(|r| r.stdout)
330|327|        .unwrap_or_default();
331|328|
332|329|    // Step 1: one-line commit summary
333|330|    let mut summary_cmd = git_cmd(global_args);
334|331|    summary_cmd.args(["show", "--no-patch", "--pretty=format:%h %s (%ar) <%an>"]);
335|332|    for arg in args {
336|333|        summary_cmd.arg(arg);
337|334|    }
338|335|    let summary_result = exec_capture(&mut summary_cmd).context("Failed to run git show")?;
339|336|    if !summary_result.success() {
340|337|        eprintln!("{}", summary_result.stderr);
341|338|        return Ok(summary_result.exit_code);
342|339|    }
343|340|    println!("{}", summary_result.stdout.trim());
344|341|
345|342|    // Step 2: --stat summary
346|343|    let mut stat_cmd = git_cmd(global_args);
347|344|    stat_cmd.args(["show", "--stat", "--pretty=format:"]);
348|345|    for arg in args {
349|346|        stat_cmd.arg(arg);
350|347|    }
351|348|    let stat_result = exec_capture(&mut stat_cmd).context("Failed to run git show --stat")?;
352|349|    let stat_text = stat_result.stdout.trim();
353|350|    if !stat_text.is_empty() {
354|351|        println!("{}", stat_text);
355|352|    }
356|353|
357|354|    // Step 3: compacted diff
358|355|    let mut diff_cmd = git_cmd(global_args);
359|356|    diff_cmd.args(["show", "--pretty=format:"]);
360|357|    for arg in args {
361|358|        diff_cmd.arg(arg);
362|359|    }
363|360|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git show (diff)")?;
364|361|    let diff_text = diff_result.stdout.trim();
365|362|
366|363|    let mut final_output = summary_result.stdout.clone();
367|364|    if !diff_text.is_empty() {
368|365|        if verbose > 0 {
369|366|            println!("\n--- Changes ---");
370|367|        }
371|368|        let compacted = compact_diff(diff_text, max_lines.unwrap_or(500));
372|369|        println!("{}", compacted);
373|370|        final_output.push_str(&format!("\n{}", compacted));
374|371|    }
375|372|
376|373|    timer.track(
377|374|        &format!("git show {}", args.join(" ")),
378|375|        &format!("rtk git show {}", args.join(" ")),
379|376|        &raw_output,
380|377|        &final_output,
381|378|    );
382|379|
383|380|    Ok(0)
384|381|}
385|382|
386|383|fn is_blob_show_arg(arg: &str) -> bool {
387|384|    // Detect `rev:path` style arguments while ignoring flags like `--pretty=format:...`.
388|385|    !arg.starts_with('-') && arg.contains(':')
389|386|}
390|387|
391|388|pub(crate) fn compact_diff(diff: &str, max_lines: usize) -> String {
392|389|    let mut result = Vec::new();
393|390|    let mut current_file = String::new();
394|391|    let mut added = 0;
395|392|    let mut removed = 0;
396|393|    let mut in_hunk = false;
397|394|    let mut hunk_shown = 0;
398|395|    let mut hunk_skipped = 0usize;
399|396|    let max_hunk_lines = 100;
400|397|    let mut was_truncated = false;
401|398|
402|399|    for line in diff.lines() {
403|400|        if line.starts_with("diff --git") {
404|401|            // Flush hunk truncation before starting a new file
405|402|            if hunk_skipped > 0 {
406|403|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
407|404|                was_truncated = true;
408|405|                hunk_skipped = 0;
409|406|            }
410|407|            if !current_file.is_empty() && (added > 0 || removed > 0) {
411|408|                result.push(format!("  +{} -{}", added, removed));
412|409|            }
413|410|            current_file = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
414|411|            result.push(format!("\n{}", current_file));
415|412|            added = 0;
416|413|            removed = 0;
417|414|            in_hunk = false;
418|415|            hunk_shown = 0;
419|416|        } else if line.starts_with("@@") {
420|417|            // Flush hunk truncation before starting a new hunk
421|418|            if hunk_skipped > 0 {
422|419|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
423|420|                was_truncated = true;
424|421|                hunk_skipped = 0;
425|422|            }
426|423|            in_hunk = true;
427|424|            hunk_shown = 0;
428|425|            // Preserve the full unified diff hunk header, including trailing
429|426|            // function / symbol context after the second @@ marker.
430|427|            result.push(format!("  {}", line));
431|428|        } else if in_hunk {
432|429|            if line.starts_with('+') && !line.starts_with("+++") {
433|430|                added += 1;
434|431|                if hunk_shown < max_hunk_lines {
435|432|                    result.push(format!("  {}", line));
436|433|                    hunk_shown += 1;
437|434|                } else {
438|435|                    hunk_skipped += 1;
439|436|                }
440|437|            } else if line.starts_with('-') && !line.starts_with("---") {
441|438|                removed += 1;
442|439|                if hunk_shown < max_hunk_lines {
443|440|                    result.push(format!("  {}", line));
444|441|                    hunk_shown += 1;
445|442|                } else {
446|443|                    hunk_skipped += 1;
447|444|                }
448|445|            } else if hunk_shown < max_hunk_lines && !line.starts_with("\\") {
449|446|                // Context line
450|447|                if hunk_shown > 0 {
451|448|                    result.push(format!("  {}", line));
452|449|                    hunk_shown += 1;
453|450|                }
454|451|            }
455|452|        }
456|453|
457|454|        if result.len() >= max_lines {
458|455|            result.push("\n... (more changes truncated)".to_string());
459|456|            was_truncated = true;
460|457|            break;
461|458|        }
462|459|    }
463|460|
464|461|    // Flush last hunk
465|462|    if hunk_skipped > 0 {
466|463|        result.push(format!("  ... ({} lines truncated)", hunk_skipped));
467|464|        was_truncated = true;
468|465|    }
469|466|
470|467|    if !current_file.is_empty() && (added > 0 || removed > 0) {
471|468|        result.push(format!("  +{} -{}", added, removed));
472|469|    }
473|470|
474|471|    if was_truncated {
475|472|        result.push("[full diff: rtk git diff --no-compact]".to_string());
476|473|    }
477|474|
478|475|    result.join("\n")
479|476|}
480|477|
481|478|fn run_log(
482|479|    args: &[String],
483|480|    _max_lines: Option<usize>,
484|481|    verbose: u8,
485|482|    global_args: &[String],
486|483|) -> Result<i32> {
487|484|    let timer = tracking::TimedExecution::start();
488|485|
489|486|    let mut cmd = git_cmd(global_args);
490|487|    cmd.arg("log");
491|488|
492|489|    // Check if user provided format flags
493|490|    let has_format_flag = args.iter().any(|arg| {
494|491|        arg.starts_with("--oneline") || arg.starts_with("--pretty") || arg.starts_with("--format")
495|492|    });
496|493|
497|494|    // Check if user provided limit flag (-N, -n N, --max-count=N, --max-count N)
498|495|    let has_limit_flag = args.iter().any(|arg| {
499|496|        (arg.starts_with('-') && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
500|497|            || arg == "-n"
501|498|            || arg.starts_with("--max-count")
502|499|    });
503|500|
504|501|    // Apply RTK defaults only if user didn't specify them
505|502|    // Use %b (body) to preserve first line of commit body for agent context
506|503|    // (BREAKING CHANGE, Closes #xxx, design notes)
507|504|    if !has_format_flag {
508|505|        cmd.args(["--pretty=format:%h %s (%ar) <%an>%n%b%n---END---"]);
509|506|    }
510|507|
511|508|    // Determine limit: respect user's explicit -N flag, use sensible defaults otherwise
512|509|    let (limit, user_set_limit) = if has_limit_flag {
513|510|        // User explicitly passed -N / -n N / --max-count=N → respect their choice
514|511|        let n = parse_user_limit(args).unwrap_or(10);
515|512|        (n, true)
516|513|    } else if has_format_flag {
517|514|        // --oneline / --pretty without -N: user wants compact output, allow more
518|515|        cmd.arg("-50");
519|516|        (50, false)
520|517|    } else {
521|518|        // No flags at all: default to 10
522|519|        cmd.arg("-10");
523|520|        (10, false)
524|521|    };
525|522|
526|523|    // Only add --no-merges if user didn't explicitly request merge commits
527|524|    let wants_merges = args
528|525|        .iter()
529|526|        .any(|arg| arg == "--merges" || arg == "--min-parents=2");
530|527|    if !wants_merges {
531|528|        cmd.arg("--no-merges");
532|529|    }
533|530|
534|531|    // Pass all user arguments
535|532|    for arg in args {
536|533|        cmd.arg(arg);
537|534|    }
538|535|
539|536|    let result = exec_capture(&mut cmd).context("Failed to run git log")?;
540|537|
541|538|    if !result.success() {
542|539|        eprintln!("{}", result.stderr);
543|540|        return Ok(result.exit_code);
544|541|    }
545|542|
546|543|    if verbose > 0 {
547|544|        eprintln!("Git log output:");
548|545|    }
549|546|
550|547|    // Post-process: truncate long messages, cap lines only if RTK set the default
551|548|
552|
553|... [OUTPUT TRUNCATED - 72 chars omitted out of 50072 total] ...
554|
555|"experiment"),
556|=======
557|//! Filters git output — log, status, diff, and more — keeping just the essential info.
558|
559|use crate::core::stream::{
560|    self, exec_capture, CaptureResult, FilterMode, LineHandler, LineStreamFilter, StdinMode,
561|};
562|use crate::core::tracking;
563|use crate::core::utils::{exit_code_from_output, exit_code_from_status, resolved_command};
564|use anyhow::{Context, Result};
565|use std::ffi::OsString;
566|use std::process::Command;
567|use std::process::Stdio;
568|
569|#[derive(Debug, Clone)]
570|pub enum GitCommand {
571|    Diff,
572|    Log,
573|    Status,
574|    Show,
575|    Add,
576|    Commit,
577|    Push,
578|    Pull,
579|    Branch,
580|    Fetch,
581|    Stash { subcommand: Option<String> },
582|    Worktree,
583|}
584|
585|/// Create a git Command with global options (e.g. -C, -c, --git-dir, --work-tree)
586|/// prepended before any subcommand arguments.
587|fn git_cmd(global_args: &[String]) -> Command {
588|    let mut cmd = resolved_command("git");
589|    for arg in global_args {
590|        cmd.arg(arg);
591|    }
592|    cmd
593|}
594|
595|/// Create a git Command for internal parsing that must be locale-stable.
596|///
597|/// We only use this for non-user-facing parses where RTK depends on git's
598|/// English status phrases. User-visible passthrough output keeps the user's
599|/// locale.
600|fn git_cmd_c_locale(global_args: &[String]) -> Command {
601|    let mut cmd = git_cmd(global_args);
602|    cmd.env("LC_ALL", "C");
603|    cmd
604|}
605|
606|fn uses_compact_status_path(args: &[String]) -> bool {
607|    if args.is_empty() {
608|        return true;
609|    }
610|
611|    let mut saw_branch = false;
612|    for arg in args {
613|        match arg.as_str() {
614|            "-b" | "--branch" => saw_branch = true,
615|            "-sb" | "-bs" => return true,
616|            "-s" | "--short" => {}
617|            _ => return false,
618|        }
619|    }
620|
621|    saw_branch
622|}
623|
624|fn build_status_command(args: &[String], global_args: &[String]) -> Command {
625|    let mut cmd = git_cmd(global_args);
626|    cmd.arg("status");
627|    if uses_compact_status_path(args) {
628|        cmd.args(["--porcelain", "-b", "-uall"]);
629|    } else {
630|        cmd.args(args);
631|    }
632|    cmd
633|}
634|
635|pub fn run(
636|    cmd: GitCommand,
637|    args: &[String],
638|    max_lines: Option<usize>,
639|    verbose: u8,
640|    global_args: &[String],
641|) -> Result<i32> {
642|    match cmd {
643|        GitCommand::Diff => run_diff(args, max_lines, verbose, global_args),
644|        GitCommand::Log => run_log(args, max_lines, verbose, global_args),
645|        GitCommand::Status => run_status(args, verbose, global_args),
646|        GitCommand::Show => run_show(args, max_lines, verbose, global_args),
647|        GitCommand::Add => run_add(args, verbose, global_args),
648|        GitCommand::Commit => run_commit(args, verbose, global_args),
649|        GitCommand::Push => run_push(args, verbose, global_args),
650|        GitCommand::Pull => run_pull(args, verbose, global_args),
651|        GitCommand::Branch => run_branch(args, verbose, global_args),
652|        GitCommand::Fetch => run_fetch(args, verbose, global_args),
653|        GitCommand::Stash { subcommand } => {
654|            run_stash(subcommand.as_deref(), args, verbose, global_args)
655|        }
656|        GitCommand::Worktree => run_worktree(args, verbose, global_args),
657|    }
658|}
659|
660|/// Re-insert `--` before the first path-like argument when clap has consumed it.
661|///
662|/// clap's `trailing_var_arg = true` silently drops `--` when it appears as the
663|/// first positional argument (before any other positional).  This means:
664|///   `rtk git diff -- file` → args = ["file"]   (clap ate `--`)
665|///   `rtk git diff HEAD -- file` → args = ["HEAD", "--", "file"]  (preserved)
666|///
667|/// Without the `--` separator git may treat an unambiguous path as a revision and
668|/// emit "fatal: ambiguous argument".  We re-insert `--` before the first path-like
669|/// argument; see `normalize_diff_args_impl` for the detection rules.
670|fn normalize_diff_args(args: &[String]) -> Vec<String> {
671|    normalize_diff_args_impl(args, |p| std::path::Path::new(p).exists())
672|}
673|
674|/// Testable core of `normalize_diff_args` — accepts an injectable filesystem existence checker.
675|///
676|/// The path-detection logic is:
677|/// 1. Explicit path prefixes (`.`, `~`) → always a path, no filesystem check needed.
678|/// 2. Contains path separator (`/`, `\`) → use `path_exists` to distinguish branch names
679|///    (e.g. `feature/auth`) from real paths (e.g. `src/main.rs`).
680|/// 3. Bare word with no separator → never a path (avoids injecting `--` when a file
681|///    happens to share a name with a branch or ref, e.g. a file named `main`).
682|fn normalize_diff_args_impl<F>(args: &[String], path_exists: F) -> Vec<String>
683|where
684|    F: Fn(&str) -> bool,
685|{
686|    // Already has `--` — nothing to do
687|    if args.iter().any(|a| a == "--") {
688|        return args.to_vec();
689|    }
690|    let path_start = args.iter().position(|arg| {
691|        if arg.starts_with('-') {
692|            return false;
693|        }
694|        // Explicit path prefixes — always treat as path regardless of existence
695|        if arg.starts_with('.') || arg.starts_with('~') {
696|            return true;
697|        }
698|        // Contains path separator — use filesystem check to distinguish
699|        // branch names (feature/auth) from real paths (src/main.rs)
700|        if arg.contains('/') || arg.contains('\\') {
701|            return path_exists(arg);
702|        }
703|        // Bare word (no separator, no special prefix) — never inject `--`
704|        // This avoids misidentifying a ref/branch as a path even if a same-named
705|        // file happens to exist on disk.
706|        false
707|    });
708|    match path_start {
709|        Some(idx) => {
710|            let mut out = args[..idx].to_vec();
711|            out.push("--".to_string());
712|            out.extend_from_slice(&args[idx..]);
713|            out
714|        }
715|        None => args.to_vec(),
716|    }
717|}
718|
719|fn run_diff(
720|    args: &[String],
721|    max_lines: Option<usize>,
722|    verbose: u8,
723|    global_args: &[String],
724|) -> Result<i32> {
725|    let timer = tracking::TimedExecution::start();
726|
727|    // Re-insert `--` when clap's trailing_var_arg consumed it (issue #1215)
728|    let args = &normalize_diff_args(args);
729|
730|    // Check if user wants stat output
731|    let wants_stat = args
732|        .iter()
733|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
734|
735|    // Check if user wants compact diff (default RTK behavior)
736|    let wants_compact = !args.iter().any(|arg| arg == "--no-compact");
737|
738|    if wants_stat || !wants_compact {
739|        // User wants stat or explicitly no compacting - pass through directly
740|        let mut cmd = git_cmd(global_args);
741|        cmd.arg("diff");
742|        for arg in args {
743|            if arg == "--no-compact" {
744|                continue; // RTK flag, not a git flag
745|            }
746|            cmd.arg(arg);
747|        }
748|
749|        let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
750|
751|        if !result.success() {
752|            eprintln!("{}", result.stderr);
753|            return Ok(result.exit_code);
754|        }
755|
756|        println!("{}", result.stdout.trim());
757|
758|        timer.track(
759|            &format!("git diff {}", args.join(" ")),
760|            &format!("rtk git diff {} (passthrough)", args.join(" ")),
761|            &result.stdout,
762|            &result.stdout,
763|        );
764|
765|        return Ok(0);
766|    }
767|
768|    // Default RTK behavior: stat first, then compacted diff
769|    let mut cmd = git_cmd(global_args);
770|    cmd.arg("diff").arg("--stat");
771|
772|    for arg in args {
773|        cmd.arg(arg);
774|    }
775|
776|    let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
777|
778|    if !result.success() {
779|        if !result.stderr.trim().is_empty() {
780|            eprint!("{}", result.stderr);
781|        }
782|        timer.track(
783|            &format!("git diff {}", args.join(" ")),
784|            &format!("rtk git diff {}", args.join(" ")),
785|            &result.stdout,
786|            &result.stdout,
787|        );
788|        return Ok(result.exit_code);
789|    }
790|
791|    if verbose > 0 {
792|        eprintln!("Git diff summary:");
793|    }
794|
795|    // Print stat summary first
796|    println!("{}", result.stdout.trim());
797|
798|    // Now get actual diff but compact it
799|    let mut diff_cmd = git_cmd(global_args);
800|    diff_cmd.arg("diff");
801|    for arg in args {
802|        diff_cmd.arg(arg);
803|    }
804|
805|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git diff")?;
806|
807|    let mut final_output = result.stdout.clone();
808|    if !diff_result.stdout.is_empty() {
809|        println!("\n--- Changes ---");
810|        let compacted = compact_diff(&diff_result.stdout, max_lines.unwrap_or(500));
811|        println!("{}", compacted);
812|        final_output.push_str("\n--- Changes ---\n");
813|        final_output.push_str(&compacted);
814|    }
815|
816|    timer.track(
817|        &format!("git diff {}", args.join(" ")),
818|        &format!("rtk git diff {}", args.join(" ")),
819|        &format!("{}\n{}", result.stdout, diff_result.stdout),
820|        &final_output,
821|    );
822|
823|    Ok(0)
824|}
825|
826|fn run_show(
827|    args: &[String],
828|    max_lines: Option<usize>,
829|    verbose: u8,
830|    global_args: &[String],
831|) -> Result<i32> {
832|    let timer = tracking::TimedExecution::start();
833|
834|    // If user wants --stat or --format only, pass through
835|    let wants_stat_only = args
836|        .iter()
837|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
838|
839|    let wants_format = args
840|        .iter()
841|        .any(|arg| arg.starts_with("--pretty") || arg.starts_with("--format"));
842|
843|    // `git show rev:path` prints a blob, not a commit diff. In this mode we should
844|    // pass through directly to avoid duplicated output from compact-show steps.
845|    let wants_blob_show = args.iter().any(|arg| is_blob_show_arg(arg));
846|
847|    if wants_stat_only || wants_format || wants_blob_show {
848|        let mut cmd = git_cmd(global_args);
849|        cmd.arg("show");
850|        for arg in args {
851|            cmd.arg(arg);
852|        }
853|        let result = exec_capture(&mut cmd).context("Failed to run git show")?;
854|        if !result.success() {
855|            eprintln!("{}", result.stderr);
856|            return Ok(result.exit_code);
857|        }
858|        if wants_blob_show {
859|            print!("{}", result.stdout);
860|        } else {
861|            println!("{}", result.stdout.trim());
862|        }
863|
864|        timer.track(
865|            &format!("git show {}", args.join(" ")),
866|            &format!("rtk git show {} (passthrough)", args.join(" ")),
867|            &result.stdout,
868|            &result.stdout,
869|        );
870|
871|        return Ok(0);
872|    }
873|
874|    // Get raw output for tracking
875|    let mut raw_cmd = git_cmd(global_args);
876|    raw_cmd.arg("show");
877|    for arg in args {
878|        raw_cmd.arg(arg);
879|    }
880|    let raw_output = exec_capture(&mut raw_cmd)
881|        .map(|r| r.stdout)
882|        .unwrap_or_default();
883|
884|    // Step 1: one-line commit summary
885|    let mut summary_cmd = git_cmd(global_args);
886|    summary_cmd.args(["show", "--no-patch", "--pretty=format:%h %s (%ar) <%an>"]);
887|    for arg in args {
888|        summary_cmd.arg(arg);
889|    }
890|    let summary_result = exec_capture(&mut summary_cmd).context("Failed to run git show")?;
891|    if !summary_result.success() {
892|        eprintln!("{}", summary_result.stderr);
893|        return Ok(summary_result.exit_code);
894|    }
895|    println!("{}", summary_result.stdout.trim());
896|
897|    // Step 2: --stat summary
898|    let mut stat_cmd = git_cmd(global_args);
899|    stat_cmd.args(["show", "--stat", "--pretty=format:"]);
900|    for arg in args {
901|        stat_cmd.arg(arg);
902|    }
903|    let stat_result = exec_capture(&mut stat_cmd).context("Failed to run git show --stat")?;
904|    let stat_text = stat_result.stdout.trim();
905|    if !stat_text.is_empty() {
906|        println!("{}", stat_text);
907|    }
908|
909|    // Step 3: compacted diff
910|    let mut diff_cmd = git_cmd(global_args);
911|    diff_cmd.args(["show", "--pretty=format:"]);
912|    for arg in args {
913|        diff_cmd.arg(arg);
914|    }
915|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git show (diff)")?;
916|    let diff_text = diff_result.stdout.trim();
917|
918|    let mut final_output = summary_result.stdout.clone();
919|    if !diff_text.is_empty() {
920|        if verbose > 0 {
921|            println!("\n--- Changes ---");
922|        }
923|        let compacted = compact_diff(diff_text, max_lines.unwrap_or(500));
924|        println!("{}", compacted);
925|        final_output.push_str(&format!("\n{}", compacted));
926|    }
927|
928|    timer.track(
929|        &format!("git show {}", args.join(" ")),
930|        &format!("rtk git show {}", args.join(" ")),
931|        &raw_output,
932|        &final_output,
933|    );
934|
935|    Ok(0)
936|}
937|
938|fn is_blob_show_arg(arg: &str) -> bool {
939|    // Detect `rev:path` style arguments while ignoring flags like `--pretty=format:...`.
940|    !arg.starts_with('-') && arg.contains(':')
941|}
942|
943|pub(crate) fn compact_diff(diff: &str, max_lines: usize) -> String {
944|    let mut result = Vec::new();
945|    let mut current_file = String::new();
946|    let mut added = 0;
947|    let mut removed = 0;
948|    let mut in_hunk = false;
949|    let mut hunk_shown = 0;
950|    let mut hunk_skipped = 0usize;
951|    let max_hunk_lines = 100;
952|    let mut was_truncated = false;
953|
954|    for line in diff.lines() {
955|        if line.starts_with("diff --git") {
956|            // Flush hunk truncation before starting a new file
957|            if hunk_skipped > 0 {
958|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
959|                was_truncated = true;
960|                hunk_skipped = 0;
961|            }
962|            if !current_file.is_empty() && (added > 0 || removed > 0) {
963|                result.push(format!("  +{} -{}", added, removed));
964|            }
965|            current_file = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
966|            result.push(format!("\n{}", current_file));
967|            added = 0;
968|            removed = 0;
969|            in_hunk = false;
970|            hunk_shown = 0;
971|        } else if line.starts_with("@@") {
972|            // Flush hunk truncation before starting a new hunk
973|            if hunk_skipped > 0 {
974|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
975|                was_truncated = true;
976|                hunk_skipped = 0;
977|            }
978|            in_hunk = true;
979|            hunk_shown = 0;
980|            // Preserve the full unified diff hunk header, including trailing
981|            // function / symbol context after the second @@ marker.
982|            result.push(format!("  {}", line));
983|        } else if in_hunk {
984|            if line.starts_with('+') && !line.starts_with("+++") {
985|                added += 1;
986|                if hunk_shown < max_hunk_lines {
987|                    result.push(format!("  {}", line));
988|                    hunk_shown += 1;
989|                } else {
990|                    hunk_skipped += 1;
991|                }
992|            } else if line.starts_with('-') && !line.starts_with("---") {
993|                removed += 1;
994|                if hunk_shown < max_hunk_lines {
995|                    result.push(format!("  {}", line));
996|                    hunk_shown += 1;
997|                } else {
998|                    hunk_skipped += 1;
999|                }
1000|            } else if hunk_shown < max_hunk_lines && !line.starts_with("\\") {
1001|                // Context line
1002|                if hunk_shown > 0 {
1003|                    result.push(format!("  {}", line));
1004|                    hunk_shown += 1;
1005|                }
1006|            }
1007|        }
1008|
1009|        if result.len() >= max_lines {
1010|            result.push("\n... (more changes truncated)".to_string());
1011|            was_truncated = true;
1012|            break;
1013|        }
1014|    }
1015|
1016|    // Flush last hunk
1017|    if hunk_skipped > 0 {
1018|        result.push(format!("  ... ({} lines truncated)", hunk_skipped));
1019|        was_truncated = true;
1020|    }
1021|
1022|    if !current_file.is_empty() && (added > 0 || removed > 0) {
1023|        result.push(format!("  +{} -{}", added, removed));
1024|    }
1025|
1026|    if was_truncated {
1027|        result.push("[full diff: rtk git diff --no-compact]".to_string());
1028|    }
1029|
1030|    result.join("\n")
1031|}
1032|
1033|fn run_log(
1034|    args: &[String],
1035|    _max_lines: Option<usize>,
1036|    verbose: u8,
1037|    global_args: &[String],
1038|) -> Result<i32> {
1039|    let timer = tracking::TimedExecution::start();
1040|
1041|    let mut cmd = git_cmd(global_args);
1042|    cmd.arg("log");
1043|
1044|    // Check if user provided format flags
1045|    let has_format_flag = args.iter().any(|arg| {
1046|        arg.starts_with("--oneline") || arg.starts_with("--pretty") || arg.starts_with("--format")
1047|    });
1048|
1049|    // Check if user provided limit flag (-N, -n N, --max-count=N, --max-count N)
1050|    let has_limit_flag = args.iter().any(|arg| {
1051|        (arg.starts_with('-') && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
1052|            || arg == "-n"
1053|            || arg.starts_with("--max-count")
1054|    });
1055|
1056|    // Apply RTK defaults only if user didn't specify them
1057|    // Use %b (body) to preserve first line of commit body for agent context
1058|    // (BREAKING CHANGE, Closes #xxx, design notes)
1059|    if !has_format_flag {
1060|        cmd.args(["--pretty=format:%h %s (%ar) <%an>%n%b%n---END---"]);
1061|    }
1062|
1063|    // Determine limit: respect user's explicit -N flag, use sensible defaults otherwise
1064|    let (limit, user_set_limit) = if has_limit_flag {
1065|        // User explicitly passed -N / -n N / --max-count=N → respect their choice
1066|        let n = parse_user_limit(args).unwrap_or(10);
1067|        (n, true)
1068|    } else if has_format_flag {
1069|        // --oneline / --pretty without -N: user wants compact output, allow more
1070|        cmd.arg("-50");
1071|        (50, false)
1072|    } else {
1073|        // No flags at all: default to 10
1074|        cmd.arg("-10");
1075|        (10, false)
1076|    };
1077|
1078|    // Only add --no-merges if user didn't explicitly request merge commits
1079|    let wants_merges = args
1080|        .iter()
1081|        .any(|arg| arg == "--merges" || arg == "--min-parents=2" || arg == "--no-merges");
1082|    // Don't add --no-merges if user explicitly requested merges or an exact count (-n N / --max-count)
1083|    // When user passes -1 they want 1 commit regardless of whether it's a merge
1084|    if !wants_merges && !has_limit_flag {
1085|        cmd.arg("--no-merges");
1086|    }
1087|
1088|    // Pass all user arguments
1089|    for arg in args {
1090|        cmd.arg(arg);
1091|    }
1092|
1093|    let result = exec_capture(&mut cmd).context("Failed to run git log")?;
1094|
1095|    if !result.success() {
1096|        eprintln!("{}", result.stderr);
1097|        return Ok(result.exit_code);
1098|    }
1099|
1100|    if verbose > 0 {
1101|        eprintln!("Git log output:");
1102|    }
1103|
1104|    // Post-process: truncate long messages, cap lines only if RTK set the default
1105|    let filtered = filter_log_output(&result.stdout, limit, user_set_limit, has_format_flag);
1106|    println!("{}", filtered);
1107|
1108|    timer.track(
1109|        &format!("git log {}", args.join(" ")),
1110|        &format!("rtk git log {}", args.join(" ")),
1111|        &result.stdout,
1112|        &filtered,
1113|    );
1114|
1115|    Ok(0)
1116|}
1117|
1118|/// Filter git log output: truncate long messages, cap lines
1119|/// Parse the user-specified limit from git log args.
1120|/// Handles: -20, -n 20, --max-count=20, --max-count 20
1121|fn parse_user_limit(args: &[String]) -> Option<usize> {
1122|    let mut iter = args.iter();
1123|    while let Some(arg) = iter.next() {
1124|        // -20 (combined digit form)
1125|        if arg.starts_with('-')
1126|            && arg.len() > 1
1127|            && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
1128|        {
1129|            if let Ok(n) = arg[1..].parse::<usize>() {
1130|                return Some(n);
1131|            }
1132|        }
1133|        // -n 20 (two-token form)
1134|        if arg == "-n" {
1135|            if let Some(next) = iter.next() {
1136|                if let Ok(n) = next.parse::<usize>() {
1137|                    return Some(n);
1138|                }
1139|            }
1140|        }
1141|        // --max-count=20
1142|        if let Some(rest) = arg.strip_prefix("--max-count=") {
1143|            if let Ok(n) = rest.parse::<usize>() {
1144|                return Some(n);
1145|            }
1146|        }
1147|        // --max-count 20 (two-token form)
1148|        if arg == "--max-count" {
1149|            if let Some(next) = iter.next() {
1150|                if let Ok(n) = next.parse::<usize>() {
1151|                    return Some(n);
1152|                }
1153|            }
1154|        }
1155|    }
1156|    None
1157|}
1158|
1159|/// When `user_set_limit` is true, the user explicitly passed `-N` to git log,
1160|/// so we skip line capping (git already returns exactly N commits) and use a
1161|/// wider truncation threshold (120 chars) to preserve commit context that LLMs
1162|/// need for rebase/squash operations.
1163|pub(crate) fn filter_log_output(
1164|    output: &str,
1165|    limit: usize,
1166|    user_set_limit: bool,
1167|    user_format: bool,
1168|) -> String {
1169|    let truncate_width = if user_set_limit { 120 } else { 80 };
1170|
1171|    // When user specified their own format (--oneline, --pretty, --format),
1172|    // RTK did not inject ---END--- markers. Use simple line-based truncation.
1173|    if user_format {
1174|        let lines: Vec<&str> = output.lines().collect();
1175|        let max_lines = if user_set_limit { lines.len() } else { limit };
1176|        return lines
1177|            .iter()
1178|            .take(max_lines)
1179|            .map(|l| truncate_line(l, truncate_width))
1180|            .collect::<Vec<_>>()
1181|            .join("\n");
1182|    }
1183|
1184|    // RTK injected format: split output into commit blocks separated by ---END---
1185|    let commits: Vec<&str> = output.split("---END---").collect();
1186|    let max_commits = if user_set_limit { commits.len() } else { limit };
1187|
1188|    let mut result = Vec::new();
1189|    for block in commits.iter().take(max_commits) {
1190|        let block = block.trim();
1191|        if block.is_empty() {
1192|            continue;
1193|        }
1194|        let mut lines = block.lines();
1195|        // First line is the header: hash subject (date) <author>
1196|        let header = match lines.next() {
1197|            Some(h) => truncate_line(h.trim(), truncate_width),
1198|            None => continue,
1199|        };
1200|        // Remaining lines are the body — keep up to 3 non-empty, non-trailer lines
1201|        let all_body_lines: Vec<&str> = lines
1202|            .map(|l| l.trim())
1203|            .filter(|l| {
1204|                !l.is_empty()
1205|                    && !l.starts_with("Signed-off-by:")
1206|                    && !l.starts_with("Co-authored-by:")
1207|            })
1208|            .collect();
1209|        let body_omitted = all_body_lines.len().saturating_sub(3);
1210|        let body_lines = &all_body_lines[..all_body_lines.len().min(3)];
1211|
1212|        if body_lines.is_empty() {
1213|            result.push(header);
1214|        } else {
1215|            let mut entry = header;
1216|            for body in body_lines {
1217|                entry.push_str(&format!("\n  {}", truncate_line(body, truncate_width)));
1218|            }
1219|            if body_omitted > 0 {
1220|                entry.push_str(&format!("\n  [+{} lines omitted]", body_omitted));
1221|            }
1222|            result.push(entry);
1223|        }
1224|    }
1225|
1226|    result.join("\n").trim().to_string()
1227|}
1228|
1229|/// Truncate a single line to `width` characters, appending "..." if needed
1230|fn truncate_line(line: &str, width: usize) -> String {
1231|    if line.chars().count() > width {
1232|        let truncated: String = line.chars().take(width - 3).collect();
1233|        format!("{}...", truncated)
1234|    } else {
1235|        line.to_string()
1236|    }
1237|}
1238|
1239|pub(crate) fn format_status_output(porcelain: &str) -> String {
1240|    format_status_inner(porcelain, None)
1241|}
1242|
1243|pub(crate) fn format_status_output_detached(porcelain: &str, detached_ref: &str) -> String {
1244|    format_status_inner(porcelain, Some(detached_ref))
1245|}
1246|
1247|fn format_status_inner(porcelain: &str, detached: Option<&str>) -> String {
1248|    let lines: Vec<&str> = porcelain
1249|        .lines()
1250|        .filter(|line| !line.trim().is_empty())
1251|        .collect();
1252|
1253|    if lines.is_empty() {
1254|        return "Clean working tree".to_string();
1255|    }
1256|
1257|    let mut output = Vec::new();
1258|
1259|    if let Some(branch_line) = lines.first() {
1260|        if branch_line.starts_with("##") {
1261|            let branch = branch_line.trim_start_matches("## ");
1262|            let display = detached.unwrap_or(branch);
1263|            output.push(format!("* {}", display));
1264|        } else {
1265|            output.push((*branch_line).to_string());
1266|        }
1267|    }
1268|
1269|    for line in lines.iter().skip(1) {
1270|        output.push((*line).to_string());
1271|    }
1272|
1273|    if lines.len() == 1 && lines[0].starts_with("##") {
1274|        output.push("clean — nothing to commit".to_string());
1275|    }
1276|
1277|    output.join("\n")
1278|}
1279|
1280|#[derive(Debug, Clone, Copy, PartialEq, Eq)]
1281|enum GitStatusState {
1282|    Rebase,
1283|    MergeConflicts,
1284|    MergeReadyToCommit,
1285|    CherryPick,
1286|    Revert,
1287|    Bisect,
1288|    Am,
1289|    SparseCheckout,
1290|}
1291|
1292|impl GitStatusState {
1293|    fn summary(self) -> &'static str {
1294|        match self {
1295|            Self::Rebase => "rebase in progress",
1296|            Self::MergeConflicts => "merge in progress. unresolved conflicts",
1297|            Self::MergeReadyToCommit => "merge in progress. no conflicts",
1298|            Self::CherryPick => "cherry-pick in progress",
1299|            Self::Revert => "revert in progress",
1300|            Self::Bisect => "bisect in progress",
1301|            Self::Am => "am session in progress",
1302|            Self::SparseCheckout => "sparse checkout enabled",
1303|        }
1304|    }
1305|}
1306|
1307|const REBASE_INDICATORS: &[&str] = &[
1308|    "rebase in progress",
1309|    "You are currently rebasing",
1310|    "You are currently editing",
1311|    "You are currently splitting",
1312|    "Last command done",
1313|    "Next command to do",
1314|    "No commands remaining",
1315|];
1316|
1317|fn detect_status_state(line: &str) -> Option<GitStatusState> {
1318|    if line.contains("All conflicts fixed but you are still merging") {
1319|        Some(GitStatusState::MergeReadyToCommit)
1320|    } else if line.contains("You have unmerged paths") {
1321|        Some(GitStatusState::MergeConflicts)
1322|    } else if line.contains("You are currently cherry-picking") {
1323|        Some(GitStatusState::CherryPick)
1324|    } else if line.contains("You are currently reverting") {
1325|        Some(GitStatusState::Revert)
1326|    } else if line.contains("You are currently bisecting") {
1327|        Some(GitStatusState::Bisect)
1328|    } else if line.contains("You are in the middle of an am session") {
1329|        Some(GitStatusState::Am)
1330|    } else if line.contains("You are in a sparse checkout") {
1331|        Some(GitStatusState::SparseCheckout)
1332|    } else if REBASE_INDICATORS.iter().any(|i| line.contains(i)) {
1333|        Some(GitStatusState::Rebase)
1334|    } else {
1335|        None
1336|    }
1337|}
1338|
1339|/// Extract a compact in-progress state summary from plain `git status` output.
1340|///
1341|/// Compact mode runs `git status --porcelain -b`, which omits the state header
1342|/// git prints for rebase / merge / cherry-pick / revert / bisect / am / sparse
1343|/// checkout. Hiding that block is a correctness bug — e.g. during an interactive
1344|/// rebase edit, the user sees a "clean" status and misses "You are currently
1345|/// editing a commit while rebasing ...".
1346|///
1347|/// This helper walks the plain-status output we already capture for tracking
1348|/// and emits a compact, RTK-style summary rather than dumping git's full prose.
1349|/// Returns `None` when no state is in progress.
1350|fn extract_state_header(raw: &str) -> Option<String> {
1351|    // Headers of the file-change blocks — everything relevant to state appears
1352|    // above these in git's output, so they double as a terminator.
1353|    const STOPPERS: &[&str] = &[
1354|        "Changes to be committed:",
1355|        "Changes not staged for commit:",
1356|        "Untracked files:",
1357|        "Unmerged paths:",
1358|        "no changes added to commit",
1359|        "nothing to commit",
1360|        "nothing added to commit",
1361|    ];
1362|
1363|    for line in raw.lines() {
1364|        let stripped = line.trim();
1365|
1366|        if STOPPERS.iter().any(|s| stripped.starts_with(s)) {
1367|            break;
1368|        }
1369|
1370|        if let Some(state) = detect_status_state(stripped) {
1371|            return Some(state.summary().to_string());
1372|        }
1373|    }
1374|
1375|    None
1376|}
1377|
1378|/// Extract the explicit "HEAD detached at/from <ref>" line from plain
1379|/// `git status` output.
1380|///
1381|/// Porcelain `-b` collapses a detached HEAD to the opaque `## HEAD (no branch)`,
1382|/// which an agent (or a distracted human) can misread as a branch literally
1383|/// named `HEAD`. The plain-status output keeps the explicit SHA/ref, so we
1384|/// surface that instead. Returns `None` when HEAD is on a branch.
1385|fn extract_detached_head(raw: &str) -> Option<String> {
1386|    raw.lines()
1387|        .map(str::trim)
1388|        .find(|l| l.starts_with("HEAD detached "))
1389|        .map(str::to_string)
1390|}
1391|
1392|/// Minimal filtering for git status with user-provided args
1393|fn filter_status_with_args(output: &str) -> String {
1394|    let mut result = Vec::new();
1395|
1396|    for line in output.lines() {
1397|        let trimmed = line.trim();
1398|
1399|        // Skip empty lines
1400|        if trimmed.is_empty() {
1401|            continue;
1402|        }
1403|
1404|        // Skip git hints - can appear at start or within line
1405|        if trimmed.starts_with("(use \"git")
1406|            || trimmed.starts_with("(create/copy files")
1407|            || trimmed.contains("(use \"git add")
1408|            || trimmed.contains("(use \"git restore")
1409|        {
1410|            continue;
1411|        }
1412|
1413|        // Special case: clean working tree
1414|        if trimmed.contains("nothing to commit") && trimmed.contains("working tree clean") {
1415|            result.push(trimmed.to_string());
1416|            break;
1417|        }
1418|
1419|        result.push(line.to_string());
1420|    }
1421|
1422|    if result.is_empty() {
1423|        "ok".to_string()
1424|    } else {
1425|        result.join("\n")
1426|    }
1427|}
1428|
1429|fn run_status(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1430|    let timer = tracking::TimedExecution::start();
1431|
1432|    // Keep a narrow compact path for no-arg status and branch/short-only flags.
1433|    // More complex explicit args still use the existing minimal-filter path.
1434|    if !uses_compact_status_path(args) {
1435|        let mut cmd = build_status_command(args, global_args);
1436|        let result = exec_capture(&mut cmd).context("Failed to run git status")?;
1437|
1438|        if !result.success() {
1439|            if !result.stderr.trim().is_empty() {
1440|                eprint!("{}", result.stderr);
1441|            }
1442|            timer.track(
1443|                &format!("git status {}", args.join(" ")),
1444|                &format!("rtk git status {}", args.join(" ")),
1445|                &result.stdout,
1446|                &result.stdout,
1447|            );
1448|            return Ok(result.exit_code);
1449|        }
1450|
1451|        if verbose > 0 || !result.stderr.is_empty() {
1452|            eprint!("{}", result.stderr);
1453|        }
1454|
1455|        // Apply minimal filtering: strip ANSI, remove hints, empty lines
1456|        let filtered = filter_status_with_args(&result.stdout);
1457|        print!("{}", filtered);
1458|
1459|        timer.track(
1460|            &format!("git status {}", args.join(" ")),
1461|            &format!("rtk git status {}", args.join(" ")),
1462|            &result.stdout,
1463|            &filtered,
1464|        );
1465|
1466|        return Ok(0);
1467|    }
1468|
1469|    let mut raw_cmd = git_cmd_c_locale(global_args);
1470|    raw_cmd.arg("status");
1471|    raw_cmd.args(args);
1472|    let raw_output = exec_capture(&mut raw_cmd)
1473|        .map(|r| r.stdout)
1474|        .unwrap_or_default();
1475|
1476|    let mut cmd = build_status_command(args, global_args);
1477|    let result = exec_capture(&mut cmd).context("Failed to run git status")?;
1478|
1479|    if !result.stderr.is_empty() && result.stderr.contains("not a git repository") {
1480|        let message = "Not a git repository".to_string();
1481|        eprintln!("{}", message);
1482|        let original_cmd = if args.is_empty() {
1483|            "git status".to_string()
1484|        } else {
1485|            format!("git status {}", args.join(" "))
1486|        };
1487|        let rtk_cmd = if args.is_empty() {
1488|            "rtk git status".to_string()
1489|        } else {
1490|            format!("rtk git status {}", args.join(" "))
1491|        };
1492|        timer.track(&original_cmd, &rtk_cmd, &raw_output, &message);
1493|        return Ok(result.exit_code);
1494|    }
1495|
1496|    let formatted = match extract_detached_head(&raw_output) {
1497|        Some(detached_ref) => format_status_output_detached(&result.stdout, &detached_ref),
1498|        None => format_status_output(&result.stdout),
1499|    };
1500|
1501|    // Surface in-progress state (rebase/merge/cherry-pick/bisect/am) from the
1502|    // plain-status output we already captured for tracking. Porcelain omits it
1503|    // and hiding it misleads the user about the true repo state.
1504|    let final_output = match extract_state_header(&raw_output) {
1505|        Some(state) => format!("{}\n{}", state, formatted),
1506|        None => formatted,
1507|    };
1508|
1509|    println!("{}", final_output);
1510|
1511|    let original_cmd = if args.is_empty() {
1512|        "git status".to_string()
1513|    } else {
1514|        format!("git status {}", args.join(" "))
1515|    };
1516|    let rtk_cmd = if args.is_empty() {
1517|        "rtk git status".to_string()
1518|    } else {
1519|        format!("rtk git status {}", args.join(" "))
1520|    };
1521|
1522|    timer.track(&original_cmd, &rtk_cmd, &raw_output, &final_output);
1523|
1524|    Ok(0)
1525|}
1526|
1527|fn run_add(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1528|    let timer = tracking::TimedExecution::start();
1529|
1530|    let mut cmd = git_cmd(global_args);
1531|    cmd.arg("add");
1532|
1533|    // Pass all arguments directly to git (flags like -A, -p, --all, etc.)
1534|    if args.is_empty() {
1535|        cmd.arg(".");
1536|    } else {
1537|        for arg in args {
1538|            cmd.arg(arg);
1539|        }
1540|    }
1541|
1542|    let result = exec_capture(&mut cmd).context("Failed to run git add")?;
1543|
1544|    if verbose > 0 {
1545|        eprintln!("git add executed");
1546|    }
1547|
1548|    let raw_output = format!("{}\n{}", result.stdout, result.stderr);
1549|
1550|    if result.success() {
1551|        // Count what was added
1552|        let mut stat_cmd = git_cmd(global_args);
1553|        stat_cmd.args(["diff", "--cached", "--stat", "--shortstat"]);
1554|        let stat_result = exec_capture(&mut stat_cmd).context("Failed to check staged files")?;
1555|
1556|        // Mirror git's own behaviour: a no-op `git add` is silent. Emitting a
1557|        // generic "ok" here is misleading — an agent can't tell "staged N files"
1558|        // from "staged nothing" when both print "ok".
1559|        let compact = if stat_result.stdout.trim().is_empty() {
1560|            String::new()
1561|        } else {
1562|            // Parse "1 file changed, 5 insertions(+)" format
1563|            let short = stat_result.stdout.lines().last().unwrap_or("").trim();
1564|            if short.is_empty() {
1565|                "ok".to_string()
1566|            } else {
1567|                format!("ok {}", short)
1568|            }
1569|        };
1570|
1571|        if !compact.is_empty() {
1572|            println!("{}", compact);
1573|        }
1574|
1575|        timer.track(
1576|            &format!("git add {}", args.join(" ")),
1577|            &format!("rtk git add {}", args.join(" ")),
1578|            &raw_output,
1579|            &compact,
1580|        );
1581|    } else {
1582|        eprintln!("FAILED: git add");
1583|        if !result.stderr.trim().is_empty() {
1584|            eprintln!("{}", result.stderr);
1585|        }
1586|        if !result.stdout.trim().is_empty() {
1587|            eprintln!("{}", result.stdout);
1588|        }
1589|        return Ok(result.exit_code);
1590|    }
1591|
1592|    Ok(0)
1593|}
1594|
1595|fn build_commit_command(args: &[String], global_args: &[String]) -> Command {
1596|    let mut cmd = git_cmd(global_args);
1597|    cmd.arg("commit");
1598|    for arg in args {
1599|        cmd.arg(arg);
1600|    }
1601|    cmd
1602|}
1603|
1604|fn run_commit(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1605|    let timer = tracking::TimedExecution::start();
1606|
1607|    let original_cmd = format!("git commit {}", args.join(" "));
1608|
1609|    if verbose > 0 {
1610|        eprintln!("{}", original_cmd);
1611|    }
1612|
1613|    let output = build_commit_command(args, global_args)
1614|        .stdin(Stdio::inherit())
1615|        .output()
1616|        .context("Failed to run git commit")?;
1617|
1618|    let stdout = String::from_utf8_lossy(&output.stdout);
1619|    let stderr = String::from_utf8_lossy(&output.stderr);
1620|    let exit_code = exit_code_from_output(&output, "git commit");
1621|    let raw_output = format!("{}\n{}", stdout, stderr);
1622|
1623|    if output.status.success() {
1624|        // Extract commit hash from output like "[main abc1234] message"
1625|        let compact = if let Some(line) = stdout.lines().next() {
1626|            if let Some(hash_start) = line.find(' ') {
1627|                let hash = line[1..hash_start].split(' ').next_back().unwrap_or("");
1628|                if !hash.is_empty() && hash.len() >= 7 {
1629|                    format!("ok {}", &hash[..7.min(hash.len())])
1630|                } else {
1631|                    "ok".to_string()
1632|                }
1633|            } else {
1634|                "ok".to_string()
1635|            }
1636|        } else {
1637|            "ok".to_string()
1638|        };
1639|
1640|        println!("{}", compact);
1641|
1642|        timer.track(&original_cmd, "rtk git commit", &raw_output, &compact);
1643|    } else if stderr.contains("nothing to commit") || stdout.contains("nothing to commit") {
1644|        println!("ok (nothing to commit)");
1645|        timer.track(
1646|            &original_cmd,
1647|            "rtk git commit",
1648|            &raw_output,
1649|            "ok (nothing to commit)",
1650|        );
1651|    } else {
1652|        if !stderr.trim().is_empty() {
1653|            eprint!("{}", stderr);
1654|        }
1655|        if !stdout.trim().is_empty() {
1656|            eprint!("{}", stdout);
1657|        }
1658|        timer.track(&original_cmd, "rtk git commit", &raw_output, &raw_output);
1659|        return Ok(exit_code);
1660|    }
1661|
1662|    Ok(0)
1663|}
1664|
1665|// Git push progress prefixes (stderr) — dropped from the stream.
1666|const GIT_PUSH_NOISE_PREFIXES: &[&str] = &[
1667|    "Enumerating objects:",
1668|    "Counting objects:",
1669|    "Compressing objects:",
1670|    "Writing objects:",
1671|    "Delta compression using",
1672|    "Total ",
1673|];
1674|
1675|#[derive(Default)]
1676|struct GitPushLineHandler {
1677|    up_to_date: bool,
1678|    pushed_ref: Option<String>,
1679|}
1680|
1681|impl LineHandler for GitPushLineHandler {
1682|    fn should_skip(&mut self, line: &str) -> bool {
1683|        if line.is_empty() {
1684|            return true;
1685|        }
1686|        let trimmed = line.trim_start();
1687|        GIT_PUSH_NOISE_PREFIXES
1688|            .iter()
1689|            .any(|p| trimmed.starts_with(p))
1690|    }
1691|
1692|    fn observe_line(&mut self, line: &str) {
1693|        if line.contains("Everything up-to-date") {
1694|            self.up_to_date = true;
1695|        }
1696|        if self.pushed_ref.is_none() {
1697|            if let Some(idx) = line.find(" -> ") {
1698|                let after = &line[idx + 4..];
1699|                if let Some(dest) = after.split_whitespace().next() {
1700|                    self.pushed_ref = Some(dest.to_string());
1701|                }
1702|            }
1703|        }
1704|    }
1705|
1706|    fn format_summary(&self, exit_code: i32, _raw: &str) -> Option<String> {
1707|        if exit_code != 0 {
1708|            return None;
1709|        }
1710|        let summary = if self.up_to_date {
1711|            "ok (up-to-date)".to_string()
1712|        } else if let Some(dest) = &self.pushed_ref {
1713|            format!("ok {}", dest)
1714|        } else {
1715|            "ok".to_string()
1716|        };
1717|        Some(format!("{}\n", summary))
1718|    }
1719|}
1720|
1721|fn run_push(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1722|    let timer = tracking::TimedExecution::start();
1723|
1724|    if verbose > 0 {
1725|        eprintln!("git push");
1726|    }
1727|
1728|    let mut cmd = git_cmd(global_args);
1729|    cmd.arg("push");
1730|    for arg in args {
1731|        cmd.arg(arg);
1732|    }
1733|
1734|    let cmd_label = format!("git push {}", args.join(" "));
1735|    let filter = LineStreamFilter::new(GitPushLineHandler::default());
1736|    let result = stream::run_streaming(
1737|        &mut cmd,
1738|        StdinMode::Inherit,
1739|        FilterMode::Streaming(Box::new(filter)),
1740|    )
1741|    .context("Failed to run git push")?;
1742|
1743|    timer.track(
1744|        &cmd_label,
1745|        &format!("rtk {}", cmd_label),
1746|        &result.raw,
1747|        &result.filtered,
1748|    );
1749|
1750|    Ok(result.exit_code)
1751|}
1752|
1753|fn run_pull(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1754|    let timer = tracking::TimedExecution::start();
1755|
1756|    if verbose > 0 {
1757|        eprintln!("git pull");
1758|    }
1759|
1760|    let mut cmd = git_cmd(global_args);
1761|    cmd.arg("pull");
1762|    for arg in args {
1763|        cmd.arg(arg);
1764|    }
1765|
1766|    let result = exec_capture(&mut cmd).context("Failed to run git pull")?;
1767|
1768|    let raw_output = format!("{}\n{}", result.stdout, result.stderr);
1769|
1770|    if result.success() {
1771|        let compact = if result.stdout.contains("Already up to date")
1772|            || result.stdout.contains("Already up-to-date")
1773|        {
1774|            "ok (up-to-date)".to_string()
1775|        } else {
1776|            // Count files changed
1777|            let mut files = 0;
1778|            let mut insertions = 0;
1779|            let mut deletions = 0;
1780|
1781|            for line in result.stdout.lines() {
1782|                if line.contains("file") && line.contains("changed") {
1783|                    // Parse "3 files changed, 10 insertions(+), 2 deletions(-)"
1784|                    for part in line.split(',') {
1785|                        let part = part.trim();
1786|                        if part.contains("file") {
1787|                            files = part
1788|                                .split_whitespace()
1789|                                .next()
1790|                                .and_then(|n| n.parse().ok())
1791|                                .unwrap_or(0);
1792|                        } else if part.contains("insertion") {
1793|                            insertions = part
1794|                                .split_whitespace()
1795|                                .next()
1796|                                .and_then(|n| n.parse().ok())
1797|                                .unwrap_or(0);
1798|                        } else if part.contains("deletion") {
1799|                            deletions = part
1800|                                .split_whitespace()
1801|                                .next()
1802|                                .and_then(|n| n.parse().ok())
1803|                                .unwrap_or(0);
1804|                        }
1805|                    }
1806|                }
1807|            }
1808|
1809|            if files > 0 {
1810|                format!("ok {} files +{} -{}", files, insertions, deletions)
1811|            } else {
1812|                "ok".to_string()
1813|            }
1814|        };
1815|
1816|        println!("{}", compact);
1817|
1818|        timer.track(
1819|            &format!("git pull {}", args.join(" ")),
1820|            &format!("rtk git pull {}", args.join(" ")),
1821|            &raw_output,
1822|            &compact,
1823|        );
1824|    } else {
1825|        eprintln!("FAILED: git pull");
1826|        if !result.stderr.trim().is_empty() {
1827|            eprintln!("{}", result.stderr);
1828|        }
1829|        if !result.stdout.trim().is_empty() {
1830|            eprintln!("{}", result.stdout);
1831|        }
1832|        return Ok(result.exit_code);
1833|    }
1834|
1835|    Ok(0)
1836|}
1837|
1838|fn run_branch(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1839|    let timer = tracking::TimedExecution::start();
1840|
1841|    if verbose > 0 {
1842|        eprintln!("git branch");
1843|    }
1844|
1845|    // Detect write operations: delete, rename, copy, upstream tracking
1846|    let has_action_flag = args.iter().any(|a| {
1847|        a == "-d"
1848|            || a == "-D"
1849|            || a == "-m"
1850|            || a == "-M"
1851|            || a == "-c"
1852|            || a == "-C"
1853|            || a == "--set-upstream-to"
1854|            || a.starts_with("--set-upstream-to=")
1855|            || a == "-u"
1856|            || a == "--unset-upstream"
1857|            || a == "--edit-description"
1858|    });
1859|
1860|    // Detect flags that produce specific output (not a branch list)
1861|    let has_show_flag = args.iter().any(|a| a == "--show-current");
1862|
1863|    // Detect list-mode flags
1864|    let has_list_flag = args.iter().any(|a| {
1865|        a == "-a"
1866|            || a == "--all"
1867|            || a == "-r"
1868|            || a == "--remotes"
1869|            || a == "--list"
1870|            || a == "--merged"
1871|            || a == "--no-merged"
1872|            || a == "--contains"
1873|            || a == "--no-contains"
1874|            || a == "--format"
1875|            || a.starts_with("--format=")
1876|            || a == "--sort"
1877|            || a.starts_with("--sort=")
1878|            || a == "--points-at"
1879|            || a.starts_with("--points-at=")
1880|    });
1881|
1882|    // Detect positional arguments (not flags) — indicates branch creation
1883|    let has_positional_arg = args.iter().any(|a| !a.starts_with('-'));
1884|
1885|    // --show-current: passthrough with raw stdout (not "ok")
1886|    if has_show_flag {
1887|        let mut cmd = git_cmd(global_args);
1888|        cmd.arg("branch");
1889|        for arg in args {
1890|            cmd.arg(arg);
1891|        }
1892|        let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1893|        let combined = result.combined();
1894|
1895|        let trimmed = result.stdout.trim();
1896|        timer.track(
1897|            &format!("git branch {}", args.join(" ")),
1898|            &format!("rtk git branch {}", args.join(" ")),
1899|            &combined,
1900|            trimmed,
1901|        );
1902|
1903|        if result.success() {
1904|            println!("{}", trimmed);
1905|        } else {
1906|            eprintln!("FAILED: git branch {}", args.join(" "));
1907|            if !result.stderr.trim().is_empty() {
1908|                eprintln!("{}", result.stderr);
1909|            }
1910|            return Ok(result.exit_code);
1911|        }
1912|        return Ok(0);
1913|    }
1914|
1915|    // Write operation: action flags, or positional args without list flags (= branch creation)
1916|    if has_action_flag || (has_positional_arg && !has_list_flag) {
1917|        let mut cmd = git_cmd(global_args);
1918|        cmd.arg("branch");
1919|        for arg in args {
1920|            cmd.arg(arg);
1921|        }
1922|        let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1923|        let combined = result.combined();
1924|
1925|        let msg = if result.success() { "ok" } else { &combined };
1926|
1927|        timer.track(
1928|            &format!("git branch {}", args.join(" ")),
1929|            &format!("rtk git branch {}", args.join(" ")),
1930|            &combined,
1931|            msg,
1932|        );
1933|
1934|        if result.success() {
1935|            println!("ok");
1936|        } else {
1937|            eprintln!("FAILED: git branch {}", args.join(" "));
1938|            if !result.stderr.trim().is_empty() {
1939|                eprintln!("{}", result.stderr);
1940|            }
1941|            if !result.stdout.trim().is_empty() {
1942|                eprintln!("{}", result.stdout);
1943|            }
1944|            return Ok(result.exit_code);
1945|        }
1946|        return Ok(0);
1947|    }
1948|
1949|    // List mode: show compact branch list
1950|    let mut cmd = git_cmd(global_args);
1951|    cmd.arg("branch");
1952|    if !has_list_flag {
1953|        cmd.arg("-a");
1954|    }
1955|    cmd.arg("--no-color");
1956|    for arg in args {
1957|        cmd.arg(arg);
1958|    }
1959|
1960|    let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1961|
1962|    if !result.success() {
1963|        if !result.stderr.trim().is_empty() {
1964|            eprint!("{}", result.stderr);
1965|        }
1966|        timer.track(
1967|            &format!("git branch {}", args.join(" ")),
1968|            &format!("rtk git branch {}", args.join(" ")),
1969|            &result.stdout,
1970|            &result.stdout,
1971|        );
1972|        return Ok(result.exit_code);
1973|    }
1974|
1975|    let filtered = filter_branch_output(&result.stdout);
1976|    println!("{}", filtered);
1977|
1978|    timer.track(
1979|        &format!("git branch {}", args.join(" ")),
1980|        &format!("rtk git branch {}", args.join(" ")),
1981|        &result.stdout,
1982|        &filtered,
1983|    );
1984|
1985|    Ok(0)
1986|}
1987|
1988|fn filter_branch_output(output: &str) -> String {
1989|    let mut current = String::new();
1990|    let mut local: Vec<String> = Vec::new();
1991|    let mut remote: Vec<String> = Vec::new();
1992|    let mut seen_remote: std::collections::HashSet<String> = std::collections::HashSet::new();
1993|
1994|    for line in output.lines() {
1995|        let line = line.trim();
1996|        if line.is_empty() {
1997|            continue;
1998|        }
1999|
2000|        if let Some(branch) = line.strip_prefix("* ") {
2001|