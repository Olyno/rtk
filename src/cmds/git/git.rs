1|<<<<<<< HEAD
2|1|<<<<<<< HEAD
3|2|<<<<<<< HEAD
4|3|<<<<<<< HEAD
5|4|1|//! Filters git output — log, status, diff, and more — keeping just the essential info.
6|5|2|
7|6|3|use crate::core::stream::{
8|7|4|    self, exec_capture, CaptureResult, FilterMode, LineHandler, LineStreamFilter, StdinMode,
9|8|5|};
10|9|6|use crate::core::tracking;
11|10|7|use crate::core::truncate::CAP_WARNINGS;
12|11|8|use crate::core::utils::{exit_code_from_output, exit_code_from_status, resolved_command};
13|12|9|use anyhow::{Context, Result};
14|13|10|use std::ffi::OsString;
15|14|11|use std::process::Command;
16|15|12|use std::process::Stdio;
17|16|13|
18|17|14|#[derive(Debug, Clone)]
19|18|15|pub enum GitCommand {
20|19|16|    Diff,
21|20|17|    Log,
22|21|18|    Status,
23|22|19|    Show,
24|23|20|    Add,
25|24|21|    Commit,
26|25|22|    Push,
27|26|23|    Pull,
28|27|24|    Branch,
29|28|25|    Fetch,
30|29|26|    Stash { subcommand: Option<String> },
31|30|27|    Worktree,
32|31|28|}
33|32|29|
34|33|30|/// Create a git Command with global options (e.g. -C, -c, --git-dir, --work-tree)
35|34|31|/// prepended before any subcommand arguments.
36|35|32|fn git_cmd(global_args: &[String]) -> Command {
37|36|33|    let mut cmd = resolved_command("git");
38|37|34|    for arg in global_args {
39|38|35|        cmd.arg(arg);
40|39|36|    }
41|40|37|    cmd
42|41|38|}
43|42|39|
44|43|40|/// Create a git Command for internal parsing that must be locale-stable.
45|44|41|///
46|45|42|/// We only use this for non-user-facing parses where RTK depends on git's
47|46|43|/// English status phrases. User-visible passthrough output keeps the user's
48|47|44|/// locale.
49|48|45|fn git_cmd_c_locale(global_args: &[String]) -> Command {
50|49|46|    let mut cmd = git_cmd(global_args);
51|50|47|    cmd.env("LC_ALL", "C");
52|51|48|    cmd
53|52|49|}
54|53|50|
55|54|51|fn uses_compact_status_path(args: &[String]) -> bool {
56|55|52|    if args.is_empty() {
57|56|53|        return true;
58|57|54|    }
59|58|55|
60|59|56|    let mut saw_branch = false;
61|60|57|    for arg in args {
62|61|58|        match arg.as_str() {
63|62|59|            "-b" | "--branch" => saw_branch = true,
64|63|60|            "-sb" | "-bs" => return true,
65|64|61|            "-s" | "--short" => {}
66|65|62|            _ => return false,
67|66|63|        }
68|67|64|    }
69|68|65|
70|69|66|    saw_branch
71|70|67|}
72|71|68|
73|72|69|fn build_status_command(args: &[String], global_args: &[String]) -> Command {
74|73|70|    let mut cmd = git_cmd(global_args);
75|74|71|    cmd.arg("status");
76|75|72|    if uses_compact_status_path(args) {
77|76|73|        cmd.args(["--porcelain", "-b"]);
78|77|74|    } else {
79|78|75|        cmd.args(args);
80|79|76|    }
81|80|77|    cmd
82|81|78|}
83|82|79|
84|83|80|pub fn run(
85|84|81|    cmd: GitCommand,
86|85|82|    args: &[String],
87|86|83|    max_lines: Option<usize>,
88|87|84|    verbose: u8,
89|88|85|    global_args: &[String],
90|89|86|) -> Result<i32> {
91|90|87|    match cmd {
92|91|88|        GitCommand::Diff => run_diff(args, max_lines, verbose, global_args),
93|92|89|        GitCommand::Log => run_log(args, max_lines, verbose, global_args),
94|93|90|        GitCommand::Status => run_status(args, verbose, global_args),
95|94|91|        GitCommand::Show => run_show(args, max_lines, verbose, global_args),
96|95|92|        GitCommand::Add => run_add(args, verbose, global_args),
97|96|93|        GitCommand::Commit => run_commit(args, verbose, global_args),
98|97|94|        GitCommand::Push => run_push(args, verbose, global_args),
99|98|95|        GitCommand::Pull => run_pull(args, verbose, global_args),
100|99|96|        GitCommand::Branch => run_branch(args, verbose, global_args),
101|100|97|        GitCommand::Fetch => run_fetch(args, verbose, global_args),
102|101|98|        GitCommand::Stash { subcommand } => {
103|102|99|            run_stash(subcommand.as_deref(), args, verbose, global_args)
104|103|100|        }
105|104|101|        GitCommand::Worktree => run_worktree(args, verbose, global_args),
106|105|102|    }
107|106|103|}
108|107|104|
109|108|105|/// Re-insert `--` before the first path-like argument when clap has consumed it.
110|109|106|///
111|110|107|/// clap's `trailing_var_arg = true` silently drops `--` when it appears as the
112|111|108|/// first positional argument (before any other positional).  This means:
113|112|109|///   `rtk git diff -- file` → args = ["file"]   (clap ate `--`)
114|113|110|///   `rtk git diff HEAD -- file` → args = ["HEAD", "--", "file"]  (preserved)
115|114|111|///
116|115|112|/// Without the `--` separator git may treat an unambiguous path as a revision and
117|116|113|/// emit "fatal: ambiguous argument".  We re-insert `--` before the first path-like
118|117|114|/// argument; see `normalize_diff_args_impl` for the detection rules.
119|118|115|fn normalize_diff_args(args: &[String]) -> Vec<String> {
120|119|116|    normalize_diff_args_impl(args, |p| std::path::Path::new(p).exists())
121|120|117|}
122|121|118|
123|122|119|/// Testable core of `normalize_diff_args` — accepts an injectable filesystem existence checker.
124|123|120|///
125|124|121|/// The path-detection logic is:
126|125|122|/// 1. Explicit path prefixes (`.`, `~`) → always a path, no filesystem check needed.
127|126|123|/// 2. Contains path separator (`/`, `\`) → use `path_exists` to distinguish branch names
128|127|124|///    (e.g. `feature/auth`) from real paths (e.g. `src/main.rs`).
129|128|125|/// 3. Bare word with no separator → never a path (avoids injecting `--` when a file
130|129|126|///    happens to share a name with a branch or ref, e.g. a file named `main`).
131|130|127|fn normalize_diff_args_impl<F>(args: &[String], path_exists: F) -> Vec<String>
132|131|128|where
133|132|129|    F: Fn(&str) -> bool,
134|133|130|{
135|134|131|    // Already has `--` — nothing to do
136|135|132|    if args.iter().any(|a| a == "--") {
137|136|133|        return args.to_vec();
138|137|134|    }
139|138|135|    let path_start = args.iter().position(|arg| {
140|139|136|        if arg.starts_with('-') {
141|140|137|            return false;
142|141|138|        }
143|142|139|        // Explicit path prefixes — always treat as path regardless of existence
144|143|140|        if arg.starts_with('.') || arg.starts_with('~') {
145|144|141|            return true;
146|145|142|        }
147|146|143|        // Contains path separator — use filesystem check to distinguish
148|147|144|        // branch names (feature/auth) from real paths (src/main.rs)
149|148|145|        if arg.contains('/') || arg.contains('\\') {
150|149|146|            return path_exists(arg);
151|150|147|        }
152|151|148|        // Bare word (no separator, no special prefix) — never inject `--`
153|152|149|        // This avoids misidentifying a ref/branch as a path even if a same-named
154|153|150|        // file happens to exist on disk.
155|154|151|        false
156|155|152|    });
157|156|153|    match path_start {
158|157|154|        Some(idx) => {
159|158|155|            let mut out = args[..idx].to_vec();
160|159|156|            out.push("--".to_string());
161|160|157|            out.extend_from_slice(&args[idx..]);
162|161|158|            out
163|162|159|        }
164|163|160|        None => args.to_vec(),
165|164|161|    }
166|165|162|}
167|166|163|
168|167|164|fn run_diff(
169|168|165|    args: &[String],
170|169|166|    max_lines: Option<usize>,
171|170|167|    verbose: u8,
172|171|168|    global_args: &[String],
173|172|169|) -> Result<i32> {
174|173|170|    let timer = tracking::TimedExecution::start();
175|174|171|
176|175|172|    // Re-insert `--` when clap's trailing_var_arg consumed it (issue #1215)
177|176|173|    let args = &normalize_diff_args(args);
178|177|174|
179|178|175|    // Check if user wants stat output
180|179|176|    let wants_stat = args
181|180|177|        .iter()
182|181|178|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
183|182|179|
184|183|180|    // Check if user wants compact diff (default RTK behavior)
185|184|181|    let wants_compact = !args.iter().any(|arg| arg == "--no-compact");
186|185|182|
187|186|183|    if wants_stat || !wants_compact {
188|187|184|        // User wants stat or explicitly no compacting - pass through directly
189|188|185|        let mut cmd = git_cmd(global_args);
190|189|186|        cmd.arg("diff");
191|190|187|        for arg in args {
192|191|188|            if arg == "--no-compact" {
193|192|189|                continue; // RTK flag, not a git flag
194|193|190|            }
195|194|191|            cmd.arg(arg);
196|195|192|        }
197|196|193|
198|197|194|        let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
199|198|195|
200|199|196|        if !result.success() {
201|200|197|            eprintln!("{}", result.stderr);
202|201|198|            return Ok(result.exit_code);
203|202|199|        }
204|203|200|
205|204|201|        println!("{}", result.stdout.trim());
206|205|202|
207|206|203|        timer.track(
208|207|204|            &format!("git diff {}", args.join(" ")),
209|208|205|            &format!("rtk git diff {} (passthrough)", args.join(" ")),
210|209|206|            &result.stdout,
211|210|207|            &result.stdout,
212|211|208|        );
213|212|209|
214|213|210|        return Ok(0);
215|214|211|    }
216|215|212|
217|216|213|    // Default RTK behavior: stat first, then compacted diff
218|217|214|    let mut cmd = git_cmd(global_args);
219|218|215|    cmd.arg("diff").arg("--stat");
220|219|216|
221|220|217|    for arg in args {
222|221|218|        cmd.arg(arg);
223|222|219|    }
224|223|220|
225|224|221|    let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
226|225|222|
227|226|223|    if !result.success() {
228|227|224|        if !result.stderr.trim().is_empty() {
229|228|225|            eprint!("{}", result.stderr);
230|229|226|        }
231|230|227|        timer.track(
232|231|228|            &format!("git diff {}", args.join(" ")),
233|232|229|            &format!("rtk git diff {}", args.join(" ")),
234|233|230|            &result.stdout,
235|234|231|            &result.stdout,
236|235|232|        );
237|236|233|        return Ok(result.exit_code);
238|237|234|    }
239|238|235|
240|239|236|    if verbose > 0 {
241|240|237|        eprintln!("Git diff summary:");
242|241|238|    }
243|242|239|
244|243|240|    // Print stat summary first
245|244|241|    println!("{}", result.stdout.trim());
246|245|242|
247|246|243|    // Now get actual diff but compact it
248|247|244|    let mut diff_cmd = git_cmd(global_args);
249|248|245|    diff_cmd.arg("diff");
250|249|246|    for arg in args {
251|250|247|        diff_cmd.arg(arg);
252|251|248|    }
253|252|249|
254|253|250|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git diff")?;
255|254|251|
256|255|252|    let mut final_output = result.stdout.clone();
257|256|253|    if !diff_result.stdout.is_empty() {
258|257|254|        println!("\n--- Changes ---");
259|258|255|        let compacted = compact_diff(&diff_result.stdout, max_lines.unwrap_or(500));
260|259|256|        println!("{}", compacted);
261|260|257|        final_output.push_str("\n--- Changes ---\n");
262|261|258|        final_output.push_str(&compacted);
263|262|259|    }
264|263|260|
265|264|261|    timer.track(
266|265|262|        &format!("git diff {}", args.join(" ")),
267|266|263|        &format!("rtk git diff {}", args.join(" ")),
268|267|264|        &format!("{}\n{}", result.stdout, diff_result.stdout),
269|268|265|        &final_output,
270|269|266|    );
271|270|267|
272|271|268|    Ok(0)
273|272|269|}
274|273|270|
275|274|271|fn run_show(
276|275|272|    args: &[String],
277|276|273|    max_lines: Option<usize>,
278|277|274|    verbose: u8,
279|278|275|    global_args: &[String],
280|279|276|) -> Result<i32> {
281|280|277|    let timer = tracking::TimedExecution::start();
282|281|278|
283|282|279|    // If user wants --stat or --format only, pass through
284|283|280|    let wants_stat_only = args
285|284|281|        .iter()
286|285|282|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
287|286|283|
288|287|284|    let wants_format = args
289|288|285|        .iter()
290|289|286|        .any(|arg| arg.starts_with("--pretty") || arg.starts_with("--format"));
291|290|287|
292|291|288|    // `git show rev:path` prints a blob, not a commit diff. In this mode we should
293|292|289|    // pass through directly to avoid duplicated output from compact-show steps.
294|293|290|    let wants_blob_show = args.iter().any(|arg| is_blob_show_arg(arg));
295|294|291|
296|295|292|    if wants_stat_only || wants_format || wants_blob_show {
297|296|293|        let mut cmd = git_cmd(global_args);
298|297|294|        cmd.arg("show");
299|298|295|        for arg in args {
300|299|296|            cmd.arg(arg);
301|300|297|        }
302|301|298|        let result = exec_capture(&mut cmd).context("Failed to run git show")?;
303|302|299|        if !result.success() {
304|303|300|            eprintln!("{}", result.stderr);
305|304|301|            return Ok(result.exit_code);
306|305|302|        }
307|306|303|        if wants_blob_show {
308|307|304|            print!("{}", result.stdout);
309|308|305|        } else {
310|309|306|            println!("{}", result.stdout.trim());
311|310|307|        }
312|311|308|
313|312|309|        timer.track(
314|313|310|            &format!("git show {}", args.join(" ")),
315|314|311|            &format!("rtk git show {} (passthrough)", args.join(" ")),
316|315|312|            &result.stdout,
317|316|313|            &result.stdout,
318|317|314|        );
319|318|315|
320|319|316|        return Ok(0);
321|320|317|    }
322|321|318|
323|322|319|    // Get raw output for tracking
324|323|320|    let mut raw_cmd = git_cmd(global_args);
325|324|321|    raw_cmd.arg("show");
326|325|322|    for arg in args {
327|326|323|        raw_cmd.arg(arg);
328|327|324|    }
329|328|325|    let raw_output = exec_capture(&mut raw_cmd)
330|329|326|        .map(|r| r.stdout)
331|330|327|        .unwrap_or_default();
332|331|328|
333|332|329|    // Step 1: one-line commit summary
334|333|330|    let mut summary_cmd = git_cmd(global_args);
335|334|331|    summary_cmd.args(["show", "--no-patch", "--pretty=format:%h %s (%ar) <%an>"]);
336|335|332|    for arg in args {
337|336|333|        summary_cmd.arg(arg);
338|337|334|    }
339|338|335|    let summary_result = exec_capture(&mut summary_cmd).context("Failed to run git show")?;
340|339|336|    if !summary_result.success() {
341|340|337|        eprintln!("{}", summary_result.stderr);
342|341|338|        return Ok(summary_result.exit_code);
343|342|339|    }
344|343|340|    println!("{}", summary_result.stdout.trim());
345|344|341|
346|345|342|    // Step 2: --stat summary
347|346|343|    let mut stat_cmd = git_cmd(global_args);
348|347|344|    stat_cmd.args(["show", "--stat", "--pretty=format:"]);
349|348|345|    for arg in args {
350|349|346|        stat_cmd.arg(arg);
351|350|347|    }
352|351|348|    let stat_result = exec_capture(&mut stat_cmd).context("Failed to run git show --stat")?;
353|352|349|    let stat_text = stat_result.stdout.trim();
354|353|350|    if !stat_text.is_empty() {
355|354|351|        println!("{}", stat_text);
356|355|352|    }
357|356|353|
358|357|354|    // Step 3: compacted diff
359|358|355|    let mut diff_cmd = git_cmd(global_args);
360|359|356|    diff_cmd.args(["show", "--pretty=format:"]);
361|360|357|    for arg in args {
362|361|358|        diff_cmd.arg(arg);
363|362|359|    }
364|363|360|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git show (diff)")?;
365|364|361|    let diff_text = diff_result.stdout.trim();
366|365|362|
367|366|363|    let mut final_output = summary_result.stdout.clone();
368|367|364|    if !diff_text.is_empty() {
369|368|365|        if verbose > 0 {
370|369|366|            println!("\n--- Changes ---");
371|370|367|        }
372|371|368|        let compacted = compact_diff(diff_text, max_lines.unwrap_or(500));
373|372|369|        println!("{}", compacted);
374|373|370|        final_output.push_str(&format!("\n{}", compacted));
375|374|371|    }
376|375|372|
377|376|373|    timer.track(
378|377|374|        &format!("git show {}", args.join(" ")),
379|378|375|        &format!("rtk git show {}", args.join(" ")),
380|379|376|        &raw_output,
381|380|377|        &final_output,
382|381|378|    );
383|382|379|
384|383|380|    Ok(0)
385|384|381|}
386|385|382|
387|386|383|fn is_blob_show_arg(arg: &str) -> bool {
388|387|384|    // Detect `rev:path` style arguments while ignoring flags like `--pretty=format:...`.
389|388|385|    !arg.starts_with('-') && arg.contains(':')
390|389|386|}
391|390|387|
392|391|388|pub(crate) fn compact_diff(diff: &str, max_lines: usize) -> String {
393|392|389|    let mut result = Vec::new();
394|393|390|    let mut current_file = String::new();
395|394|391|    let mut added = 0;
396|395|392|    let mut removed = 0;
397|396|393|    let mut in_hunk = false;
398|397|394|    let mut hunk_shown = 0;
399|398|395|    let mut hunk_skipped = 0usize;
400|399|396|    let max_hunk_lines = 100;
401|400|397|    let mut was_truncated = false;
402|401|398|
403|402|399|    for line in diff.lines() {
404|403|400|        if line.starts_with("diff --git") {
405|404|401|            // Flush hunk truncation before starting a new file
406|405|402|            if hunk_skipped > 0 {
407|406|403|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
408|407|404|                was_truncated = true;
409|408|405|                hunk_skipped = 0;
410|409|406|            }
411|410|407|            if !current_file.is_empty() && (added > 0 || removed > 0) {
412|411|408|                result.push(format!("  +{} -{}", added, removed));
413|412|409|            }
414|413|410|            current_file = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
415|414|411|            result.push(format!("\n{}", current_file));
416|415|412|            added = 0;
417|416|413|            removed = 0;
418|417|414|            in_hunk = false;
419|418|415|            hunk_shown = 0;
420|419|416|        } else if line.starts_with("@@") {
421|420|417|            // Flush hunk truncation before starting a new hunk
422|421|418|            if hunk_skipped > 0 {
423|422|419|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
424|423|420|                was_truncated = true;
425|424|421|                hunk_skipped = 0;
426|425|422|            }
427|426|423|            in_hunk = true;
428|427|424|            hunk_shown = 0;
429|428|425|            // Preserve the full unified diff hunk header, including trailing
430|429|426|            // function / symbol context after the second @@ marker.
431|430|427|            result.push(format!("  {}", line));
432|431|428|        } else if in_hunk {
433|432|429|            if line.starts_with('+') && !line.starts_with("+++") {
434|433|430|                added += 1;
435|434|431|                if hunk_shown < max_hunk_lines {
436|435|432|                    result.push(format!("  {}", line));
437|436|433|                    hunk_shown += 1;
438|437|434|                } else {
439|438|435|                    hunk_skipped += 1;
440|439|436|                }
441|440|437|            } else if line.starts_with('-') && !line.starts_with("---") {
442|441|438|                removed += 1;
443|442|439|                if hunk_shown < max_hunk_lines {
444|443|440|                    result.push(format!("  {}", line));
445|444|441|                    hunk_shown += 1;
446|445|442|                } else {
447|446|443|                    hunk_skipped += 1;
448|447|444|                }
449|448|445|            } else if hunk_shown < max_hunk_lines && !line.starts_with("\\") {
450|449|446|                // Context line
451|450|447|                if hunk_shown > 0 {
452|451|448|                    result.push(format!("  {}", line));
453|452|449|                    hunk_shown += 1;
454|453|450|                }
455|454|451|            }
456|455|452|        }
457|456|453|
458|457|454|        if result.len() >= max_lines {
459|458|455|            result.push("\n... (more changes truncated)".to_string());
460|459|456|            was_truncated = true;
461|460|457|            break;
462|461|458|        }
463|462|459|    }
464|463|460|
465|464|461|    // Flush last hunk
466|465|462|    if hunk_skipped > 0 {
467|466|463|        result.push(format!("  ... ({} lines truncated)", hunk_skipped));
468|467|464|        was_truncated = true;
469|468|465|    }
470|469|466|
471|470|467|    if !current_file.is_empty() && (added > 0 || removed > 0) {
472|471|468|        result.push(format!("  +{} -{}", added, removed));
473|472|469|    }
474|473|470|
475|474|471|    if was_truncated {
476|475|472|        result.push("[full diff: rtk git diff --no-compact]".to_string());
477|476|473|    }
478|477|474|
479|478|475|    result.join("\n")
480|479|476|}
481|480|477|
482|481|478|fn run_log(
483|482|479|    args: &[String],
484|483|480|    _max_lines: Option<usize>,
485|484|481|    verbose: u8,
486|485|482|    global_args: &[String],
487|486|483|) -> Result<i32> {
488|487|484|    let timer = tracking::TimedExecution::start();
489|488|485|
490|489|486|    let mut cmd = git_cmd(global_args);
491|490|487|    cmd.arg("log");
492|491|488|
493|492|489|    // Check if user provided format flags
494|493|490|    let has_format_flag = args.iter().any(|arg| {
495|494|491|        arg.starts_with("--oneline") || arg.starts_with("--pretty") || arg.starts_with("--format")
496|495|492|    });
497|496|493|
498|497|494|    // Check if user provided limit flag (-N, -n N, --max-count=N, --max-count N)
499|498|495|    let has_limit_flag = args.iter().any(|arg| {
500|499|496|        (arg.starts_with('-') && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
501|500|497|            || arg == "-n"
502|501|498|            || arg.starts_with("--max-count")
503|502|499|    });
504|503|500|
505|504|501|    // Apply RTK defaults only if user didn't specify them
506|505|502|    // Use %b (body) to preserve first line of commit body for agent context
507|506|503|    // (BREAKING CHANGE, Closes #xxx, design notes)
508|507|504|    if !has_format_flag {
509|508|505|        cmd.args(["--pretty=format:%h %s (%ar) <%an>%n%b%n---END---"]);
510|509|506|    }
511|510|507|
512|511|508|    // Determine limit: respect user's explicit -N flag, use sensible defaults otherwise
513|512|509|    let (limit, user_set_limit) = if has_limit_flag {
514|513|510|        // User explicitly passed -N / -n N / --max-count=N → respect their choice
515|514|511|        let n = parse_user_limit(args).unwrap_or(10);
516|515|512|        (n, true)
517|516|513|    } else if has_format_flag {
518|517|514|        // --oneline / --pretty without -N: user wants compact output, allow more
519|518|515|        cmd.arg("-50");
520|519|516|        (50, false)
521|520|517|    } else {
522|521|518|        // No flags at all: default to 10
523|522|519|        cmd.arg("-10");
524|523|520|        (10, false)
525|524|521|    };
526|525|522|
527|526|523|    // Only add --no-merges if user didn't explicitly request merge commits
528|527|524|    let wants_merges = args
529|528|525|        .iter()
530|529|526|        .any(|arg| arg == "--merges" || arg == "--min-parents=2");
531|530|527|    if !wants_merges {
532|531|528|        cmd.arg("--no-merges");
533|532|529|    }
534|533|530|
535|534|531|    // Pass all user arguments
536|535|532|    for arg in args {
537|536|533|        cmd.arg(arg);
538|537|534|    }
539|538|535|
540|539|536|    let result = exec_capture(&mut cmd).context("Failed to run git log")?;
541|540|537|
542|541|538|    if !result.success() {
543|542|539|        eprintln!("{}", result.stderr);
544|543|540|        return Ok(result.exit_code);
545|544|541|    }
546|545|542|
547|546|543|    if verbose > 0 {
548|547|544|        eprintln!("Git log output:");
549|548|545|    }
550|549|546|
551|550|547|    // Post-process: truncate long messages, cap lines only if RTK set the default
552|551|548|
553|552|
554|553|... [OUTPUT TRUNCATED - 72 chars omitted out of 50072 total] ...
555|554|
556|555|"experiment"),
557|556|=======
558|557|//! Filters git output — log, status, diff, and more — keeping just the essential info.
559|558|
560|559|use crate::core::stream::{
561|560|    self, exec_capture, CaptureResult, FilterMode, LineHandler, LineStreamFilter, StdinMode,
562|561|};
563|562|use crate::core::tracking;
564|563|use crate::core::utils::{exit_code_from_output, exit_code_from_status, resolved_command};
565|564|use anyhow::{Context, Result};
566|565|use std::ffi::OsString;
567|566|use std::process::Command;
568|567|use std::process::Stdio;
569|568|
570|569|#[derive(Debug, Clone)]
571|570|pub enum GitCommand {
572|571|    Diff,
573|572|    Log,
574|573|    Status,
575|574|    Show,
576|575|    Add,
577|576|    Commit,
578|577|    Push,
579|578|    Pull,
580|579|    Branch,
581|580|    Fetch,
582|581|    Stash { subcommand: Option<String> },
583|582|    Worktree,
584|583|}
585|584|
586|585|/// Create a git Command with global options (e.g. -C, -c, --git-dir, --work-tree)
587|586|/// prepended before any subcommand arguments.
588|587|fn git_cmd(global_args: &[String]) -> Command {
589|588|    let mut cmd = resolved_command("git");
590|589|    for arg in global_args {
591|590|        cmd.arg(arg);
592|591|    }
593|592|    cmd
594|593|}
595|594|
596|595|/// Create a git Command for internal parsing that must be locale-stable.
597|596|///
598|597|/// We only use this for non-user-facing parses where RTK depends on git's
599|598|/// English status phrases. User-visible passthrough output keeps the user's
600|599|/// locale.
601|600|fn git_cmd_c_locale(global_args: &[String]) -> Command {
602|601|    let mut cmd = git_cmd(global_args);
603|602|    cmd.env("LC_ALL", "C");
604|603|    cmd
605|604|}
606|605|
607|606|fn uses_compact_status_path(args: &[String]) -> bool {
608|607|    if args.is_empty() {
609|608|        return true;
610|609|    }
611|610|
612|611|    let mut saw_branch = false;
613|612|    for arg in args {
614|613|        match arg.as_str() {
615|614|            "-b" | "--branch" => saw_branch = true,
616|615|            "-sb" | "-bs" => return true,
617|616|            "-s" | "--short" => {}
618|617|            _ => return false,
619|618|        }
620|619|    }
621|620|
622|621|    saw_branch
623|622|}
624|623|
625|624|fn build_status_command(args: &[String], global_args: &[String]) -> Command {
626|625|    let mut cmd = git_cmd(global_args);
627|626|    cmd.arg("status");
628|627|    if uses_compact_status_path(args) {
629|628|        cmd.args(["--porcelain", "-b", "-uall"]);
630|629|    } else {
631|630|        cmd.args(args);
632|631|    }
633|632|    cmd
634|633|}
635|634|
636|635|pub fn run(
637|636|    cmd: GitCommand,
638|637|    args: &[String],
639|638|    max_lines: Option<usize>,
640|639|    verbose: u8,
641|640|    global_args: &[String],
642|641|) -> Result<i32> {
643|642|    match cmd {
644|643|        GitCommand::Diff => run_diff(args, max_lines, verbose, global_args),
645|644|        GitCommand::Log => run_log(args, max_lines, verbose, global_args),
646|645|        GitCommand::Status => run_status(args, verbose, global_args),
647|646|        GitCommand::Show => run_show(args, max_lines, verbose, global_args),
648|647|        GitCommand::Add => run_add(args, verbose, global_args),
649|648|        GitCommand::Commit => run_commit(args, verbose, global_args),
650|649|        GitCommand::Push => run_push(args, verbose, global_args),
651|650|        GitCommand::Pull => run_pull(args, verbose, global_args),
652|651|        GitCommand::Branch => run_branch(args, verbose, global_args),
653|652|        GitCommand::Fetch => run_fetch(args, verbose, global_args),
654|653|        GitCommand::Stash { subcommand } => {
655|654|            run_stash(subcommand.as_deref(), args, verbose, global_args)
656|655|        }
657|656|        GitCommand::Worktree => run_worktree(args, verbose, global_args),
658|657|    }
659|658|}
660|659|
661|660|/// Re-insert `--` before the first path-like argument when clap has consumed it.
662|661|///
663|662|/// clap's `trailing_var_arg = true` silently drops `--` when it appears as the
664|663|/// first positional argument (before any other positional).  This means:
665|664|///   `rtk git diff -- file` → args = ["file"]   (clap ate `--`)
666|665|///   `rtk git diff HEAD -- file` → args = ["HEAD", "--", "file"]  (preserved)
667|666|///
668|667|/// Without the `--` separator git may treat an unambiguous path as a revision and
669|668|/// emit "fatal: ambiguous argument".  We re-insert `--` before the first path-like
670|669|/// argument; see `normalize_diff_args_impl` for the detection rules.
671|670|fn normalize_diff_args(args: &[String]) -> Vec<String> {
672|671|    normalize_diff_args_impl(args, |p| std::path::Path::new(p).exists())
673|672|}
674|673|
675|674|/// Testable core of `normalize_diff_args` — accepts an injectable filesystem existence checker.
676|675|///
677|676|/// The path-detection logic is:
678|677|/// 1. Explicit path prefixes (`.`, `~`) → always a path, no filesystem check needed.
679|678|/// 2. Contains path separator (`/`, `\`) → use `path_exists` to distinguish branch names
680|679|///    (e.g. `feature/auth`) from real paths (e.g. `src/main.rs`).
681|680|/// 3. Bare word with no separator → never a path (avoids injecting `--` when a file
682|681|///    happens to share a name with a branch or ref, e.g. a file named `main`).
683|682|fn normalize_diff_args_impl<F>(args: &[String], path_exists: F) -> Vec<String>
684|683|where
685|684|    F: Fn(&str) -> bool,
686|685|{
687|686|    // Already has `--` — nothing to do
688|687|    if args.iter().any(|a| a == "--") {
689|688|        return args.to_vec();
690|689|    }
691|690|    let path_start = args.iter().position(|arg| {
692|691|        if arg.starts_with('-') {
693|692|            return false;
694|693|        }
695|694|        // Explicit path prefixes — always treat as path regardless of existence
696|695|        if arg.starts_with('.') || arg.starts_with('~') {
697|696|            return true;
698|697|        }
699|698|        // Contains path separator — use filesystem check to distinguish
700|699|        // branch names (feature/auth) from real paths (src/main.rs)
701|700|        if arg.contains('/') || arg.contains('\\') {
702|701|            return path_exists(arg);
703|702|        }
704|703|        // Bare word (no separator, no special prefix) — never inject `--`
705|704|        // This avoids misidentifying a ref/branch as a path even if a same-named
706|705|        // file happens to exist on disk.
707|706|        false
708|707|    });
709|708|    match path_start {
710|709|        Some(idx) => {
711|710|            let mut out = args[..idx].to_vec();
712|711|            out.push("--".to_string());
713|712|            out.extend_from_slice(&args[idx..]);
714|713|            out
715|714|        }
716|715|        None => args.to_vec(),
717|716|    }
718|717|}
719|718|
720|719|fn run_diff(
721|720|    args: &[String],
722|721|    max_lines: Option<usize>,
723|722|    verbose: u8,
724|723|    global_args: &[String],
725|724|) -> Result<i32> {
726|725|    let timer = tracking::TimedExecution::start();
727|726|
728|727|    // Re-insert `--` when clap's trailing_var_arg consumed it (issue #1215)
729|728|    let args = &normalize_diff_args(args);
730|729|
731|730|    // Check if user wants stat output
732|731|    let wants_stat = args
733|732|        .iter()
734|733|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
735|734|
736|735|    // Check if user wants compact diff (default RTK behavior)
737|736|    let wants_compact = !args.iter().any(|arg| arg == "--no-compact");
738|737|
739|738|    if wants_stat || !wants_compact {
740|739|        // User wants stat or explicitly no compacting - pass through directly
741|740|        let mut cmd = git_cmd(global_args);
742|741|        cmd.arg("diff");
743|742|        for arg in args {
744|743|            if arg == "--no-compact" {
745|744|                continue; // RTK flag, not a git flag
746|745|            }
747|746|            cmd.arg(arg);
748|747|        }
749|748|
750|749|        let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
751|750|
752|751|        if !result.success() {
753|752|            eprintln!("{}", result.stderr);
754|753|            return Ok(result.exit_code);
755|754|        }
756|755|
757|756|        println!("{}", result.stdout.trim());
758|757|
759|758|        timer.track(
760|759|            &format!("git diff {}", args.join(" ")),
761|760|            &format!("rtk git diff {} (passthrough)", args.join(" ")),
762|761|            &result.stdout,
763|762|            &result.stdout,
764|763|        );
765|764|
766|765|        return Ok(0);
767|766|    }
768|767|
769|768|    // Default RTK behavior: stat first, then compacted diff
770|769|    let mut cmd = git_cmd(global_args);
771|770|    cmd.arg("diff").arg("--stat");
772|771|
773|772|    for arg in args {
774|773|        cmd.arg(arg);
775|774|    }
776|775|
777|776|    let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
778|777|
779|778|    if !result.success() {
780|779|        if !result.stderr.trim().is_empty() {
781|780|            eprint!("{}", result.stderr);
782|781|        }
783|782|        timer.track(
784|783|            &format!("git diff {}", args.join(" ")),
785|784|            &format!("rtk git diff {}", args.join(" ")),
786|785|            &result.stdout,
787|786|            &result.stdout,
788|787|        );
789|788|        return Ok(result.exit_code);
790|789|    }
791|790|
792|791|    if verbose > 0 {
793|792|        eprintln!("Git diff summary:");
794|793|    }
795|794|
796|795|    // Print stat summary first
797|796|    println!("{}", result.stdout.trim());
798|797|
799|798|    // Now get actual diff but compact it
800|799|    let mut diff_cmd = git_cmd(global_args);
801|800|    diff_cmd.arg("diff");
802|801|    for arg in args {
803|802|        diff_cmd.arg(arg);
804|803|    }
805|804|
806|805|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git diff")?;
807|806|
808|807|    let mut final_output = result.stdout.clone();
809|808|    if !diff_result.stdout.is_empty() {
810|809|        println!("\n--- Changes ---");
811|810|        let compacted = compact_diff(&diff_result.stdout, max_lines.unwrap_or(500));
812|811|        println!("{}", compacted);
813|812|        final_output.push_str("\n--- Changes ---\n");
814|813|        final_output.push_str(&compacted);
815|814|    }
816|815|
817|816|    timer.track(
818|817|        &format!("git diff {}", args.join(" ")),
819|818|        &format!("rtk git diff {}", args.join(" ")),
820|819|        &format!("{}\n{}", result.stdout, diff_result.stdout),
821|820|        &final_output,
822|821|    );
823|822|
824|823|    Ok(0)
825|824|}
826|825|
827|826|fn run_show(
828|827|    args: &[String],
829|828|    max_lines: Option<usize>,
830|829|    verbose: u8,
831|830|    global_args: &[String],
832|831|) -> Result<i32> {
833|832|    let timer = tracking::TimedExecution::start();
834|833|
835|834|    // If user wants --stat or --format only, pass through
836|835|    let wants_stat_only = args
837|836|        .iter()
838|837|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
839|838|
840|839|    let wants_format = args
841|840|        .iter()
842|841|        .any(|arg| arg.starts_with("--pretty") || arg.starts_with("--format"));
843|842|
844|843|    // `git show rev:path` prints a blob, not a commit diff. In this mode we should
845|844|    // pass through directly to avoid duplicated output from compact-show steps.
846|845|    let wants_blob_show = args.iter().any(|arg| is_blob_show_arg(arg));
847|846|
848|847|    if wants_stat_only || wants_format || wants_blob_show {
849|848|        let mut cmd = git_cmd(global_args);
850|849|        cmd.arg("show");
851|850|        for arg in args {
852|851|            cmd.arg(arg);
853|852|        }
854|853|        let result = exec_capture(&mut cmd).context("Failed to run git show")?;
855|854|        if !result.success() {
856|855|            eprintln!("{}", result.stderr);
857|856|            return Ok(result.exit_code);
858|857|        }
859|858|        if wants_blob_show {
860|859|            print!("{}", result.stdout);
861|860|        } else {
862|861|            println!("{}", result.stdout.trim());
863|862|        }
864|863|
865|864|        timer.track(
866|865|            &format!("git show {}", args.join(" ")),
867|866|            &format!("rtk git show {} (passthrough)", args.join(" ")),
868|867|            &result.stdout,
869|868|            &result.stdout,
870|869|        );
871|870|
872|871|        return Ok(0);
873|872|    }
874|873|
875|874|    // Get raw output for tracking
876|875|    let mut raw_cmd = git_cmd(global_args);
877|876|    raw_cmd.arg("show");
878|877|    for arg in args {
879|878|        raw_cmd.arg(arg);
880|879|    }
881|880|    let raw_output = exec_capture(&mut raw_cmd)
882|881|        .map(|r| r.stdout)
883|882|        .unwrap_or_default();
884|883|
885|884|    // Step 1: one-line commit summary
886|885|    let mut summary_cmd = git_cmd(global_args);
887|886|    summary_cmd.args(["show", "--no-patch", "--pretty=format:%h %s (%ar) <%an>"]);
888|887|    for arg in args {
889|888|        summary_cmd.arg(arg);
890|889|    }
891|890|    let summary_result = exec_capture(&mut summary_cmd).context("Failed to run git show")?;
892|891|    if !summary_result.success() {
893|892|        eprintln!("{}", summary_result.stderr);
894|893|        return Ok(summary_result.exit_code);
895|894|    }
896|895|    println!("{}", summary_result.stdout.trim());
897|896|
898|897|    // Step 2: --stat summary
899|898|    let mut stat_cmd = git_cmd(global_args);
900|899|    stat_cmd.args(["show", "--stat", "--pretty=format:"]);
901|900|    for arg in args {
902|901|        stat_cmd.arg(arg);
903|902|    }
904|903|    let stat_result = exec_capture(&mut stat_cmd).context("Failed to run git show --stat")?;
905|904|    let stat_text = stat_result.stdout.trim();
906|905|    if !stat_text.is_empty() {
907|906|        println!("{}", stat_text);
908|907|    }
909|908|
910|909|    // Step 3: compacted diff
911|910|    let mut diff_cmd = git_cmd(global_args);
912|911|    diff_cmd.args(["show", "--pretty=format:"]);
913|912|    for arg in args {
914|913|        diff_cmd.arg(arg);
915|914|    }
916|915|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git show (diff)")?;
917|916|    let diff_text = diff_result.stdout.trim();
918|917|
919|918|    let mut final_output = summary_result.stdout.clone();
920|919|    if !diff_text.is_empty() {
921|920|        if verbose > 0 {
922|921|            println!("\n--- Changes ---");
923|922|        }
924|923|        let compacted = compact_diff(diff_text, max_lines.unwrap_or(500));
925|924|        println!("{}", compacted);
926|925|        final_output.push_str(&format!("\n{}", compacted));
927|926|    }
928|927|
929|928|    timer.track(
930|929|        &format!("git show {}", args.join(" ")),
931|930|        &format!("rtk git show {}", args.join(" ")),
932|931|        &raw_output,
933|932|        &final_output,
934|933|    );
935|934|
936|935|    Ok(0)
937|936|}
938|937|
939|938|fn is_blob_show_arg(arg: &str) -> bool {
940|939|    // Detect `rev:path` style arguments while ignoring flags like `--pretty=format:...`.
941|940|    !arg.starts_with('-') && arg.contains(':')
942|941|}
943|942|
944|943|pub(crate) fn compact_diff(diff: &str, max_lines: usize) -> String {
945|944|    let mut result = Vec::new();
946|945|    let mut current_file = String::new();
947|946|    let mut added = 0;
948|947|    let mut removed = 0;
949|948|    let mut in_hunk = false;
950|949|    let mut hunk_shown = 0;
951|950|    let mut hunk_skipped = 0usize;
952|951|    let max_hunk_lines = 100;
953|952|    let mut was_truncated = false;
954|953|
955|954|    for line in diff.lines() {
956|955|        if line.starts_with("diff --git") {
957|956|            // Flush hunk truncation before starting a new file
958|957|            if hunk_skipped > 0 {
959|958|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
960|959|                was_truncated = true;
961|960|                hunk_skipped = 0;
962|961|            }
963|962|            if !current_file.is_empty() && (added > 0 || removed > 0) {
964|963|                result.push(format!("  +{} -{}", added, removed));
965|964|            }
966|965|            current_file = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
967|966|            result.push(format!("\n{}", current_file));
968|967|            added = 0;
969|968|            removed = 0;
970|969|            in_hunk = false;
971|970|            hunk_shown = 0;
972|971|        } else if line.starts_with("@@") {
973|972|            // Flush hunk truncation before starting a new hunk
974|973|            if hunk_skipped > 0 {
975|974|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
976|975|                was_truncated = true;
977|976|                hunk_skipped = 0;
978|977|            }
979|978|            in_hunk = true;
980|979|            hunk_shown = 0;
981|980|            // Preserve the full unified diff hunk header, including trailing
982|981|            // function / symbol context after the second @@ marker.
983|982|            result.push(format!("  {}", line));
984|983|        } else if in_hunk {
985|984|            if line.starts_with('+') && !line.starts_with("+++") {
986|985|                added += 1;
987|986|                if hunk_shown < max_hunk_lines {
988|987|                    result.push(format!("  {}", line));
989|988|                    hunk_shown += 1;
990|989|                } else {
991|990|                    hunk_skipped += 1;
992|991|                }
993|992|            } else if line.starts_with('-') && !line.starts_with("---") {
994|993|                removed += 1;
995|994|                if hunk_shown < max_hunk_lines {
996|995|                    result.push(format!("  {}", line));
997|996|                    hunk_shown += 1;
998|997|                } else {
999|998|                    hunk_skipped += 1;
1000|999|                }
1001|1000|            } else if hunk_shown < max_hunk_lines && !line.starts_with("\\") {
1002|1001|                // Context line
1003|1002|                if hunk_shown > 0 {
1004|1003|                    result.push(format!("  {}", line));
1005|1004|                    hunk_shown += 1;
1006|1005|                }
1007|1006|            }
1008|1007|        }
1009|1008|
1010|1009|        if result.len() >= max_lines {
1011|1010|            result.push("\n... (more changes truncated)".to_string());
1012|1011|            was_truncated = true;
1013|1012|            break;
1014|1013|        }
1015|1014|    }
1016|1015|
1017|1016|    // Flush last hunk
1018|1017|    if hunk_skipped > 0 {
1019|1018|        result.push(format!("  ... ({} lines truncated)", hunk_skipped));
1020|1019|        was_truncated = true;
1021|1020|    }
1022|1021|
1023|1022|    if !current_file.is_empty() && (added > 0 || removed > 0) {
1024|1023|        result.push(format!("  +{} -{}", added, removed));
1025|1024|    }
1026|1025|
1027|1026|    if was_truncated {
1028|1027|        result.push("[full diff: rtk git diff --no-compact]".to_string());
1029|1028|    }
1030|1029|
1031|1030|    result.join("\n")
1032|1031|}
1033|1032|
1034|1033|fn run_log(
1035|1034|    args: &[String],
1036|1035|    _max_lines: Option<usize>,
1037|1036|    verbose: u8,
1038|1037|    global_args: &[String],
1039|1038|) -> Result<i32> {
1040|1039|    let timer = tracking::TimedExecution::start();
1041|1040|
1042|1041|    let mut cmd = git_cmd(global_args);
1043|1042|    cmd.arg("log");
1044|1043|
1045|1044|    // Check if user provided format flags
1046|1045|    let has_format_flag = args.iter().any(|arg| {
1047|1046|        arg.starts_with("--oneline") || arg.starts_with("--pretty") || arg.starts_with("--format")
1048|1047|    });
1049|1048|
1050|1049|    // Check if user provided limit flag (-N, -n N, --max-count=N, --max-count N)
1051|1050|    let has_limit_flag = args.iter().any(|arg| {
1052|1051|        (arg.starts_with('-') && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
1053|1052|            || arg == "-n"
1054|1053|            || arg.starts_with("--max-count")
1055|1054|    });
1056|1055|
1057|1056|    // Apply RTK defaults only if user didn't specify them
1058|1057|    // Use %b (body) to preserve first line of commit body for agent context
1059|1058|    // (BREAKING CHANGE, Closes #xxx, design notes)
1060|1059|    if !has_format_flag {
1061|1060|        cmd.args(["--pretty=format:%h %s (%ar) <%an>%n%b%n---END---"]);
1062|1061|    }
1063|1062|
1064|1063|    // Determine limit: respect user's explicit -N flag, use sensible defaults otherwise
1065|1064|    let (limit, user_set_limit) = if has_limit_flag {
1066|1065|        // User explicitly passed -N / -n N / --max-count=N → respect their choice
1067|1066|        let n = parse_user_limit(args).unwrap_or(10);
1068|1067|        (n, true)
1069|1068|    } else if has_format_flag {
1070|1069|        // --oneline / --pretty without -N: user wants compact output, allow more
1071|1070|        cmd.arg("-50");
1072|1071|        (50, false)
1073|1072|    } else {
1074|1073|        // No flags at all: default to 10
1075|1074|        cmd.arg("-10");
1076|1075|        (10, false)
1077|1076|    };
1078|1077|
1079|1078|    // Only add --no-merges if user didn't explicitly request merge commits
1080|1079|    let wants_merges = args
1081|1080|        .iter()
1082|1081|        .any(|arg| arg == "--merges" || arg == "--min-parents=2" || arg == "--no-merges");
1083|1082|    // Don't add --no-merges if user explicitly requested merges or an exact count (-n N / --max-count)
1084|1083|    // When user passes -1 they want 1 commit regardless of whether it's a merge
1085|1084|    if !wants_merges && !has_limit_flag {
1086|1085|        cmd.arg("--no-merges");
1087|1086|    }
1088|1087|
1089|1088|    // Pass all user arguments
1090|1089|    for arg in args {
1091|1090|        cmd.arg(arg);
1092|1091|    }
1093|1092|
1094|1093|    let result = exec_capture(&mut cmd).context("Failed to run git log")?;
1095|1094|
1096|1095|    if !result.success() {
1097|1096|        eprintln!("{}", result.stderr);
1098|1097|        return Ok(result.exit_code);
1099|1098|    }
1100|1099|
1101|1100|    if verbose > 0 {
1102|1101|        eprintln!("Git log output:");
1103|1102|    }
1104|1103|
1105|1104|    // Post-process: truncate long messages, cap lines only if RTK set the default
1106|1105|    let filtered = filter_log_output(&result.stdout, limit, user_set_limit, has_format_flag);
1107|1106|    println!("{}", filtered);
1108|1107|
1109|1108|    timer.track(
1110|1109|        &format!("git log {}", args.join(" ")),
1111|1110|        &format!("rtk git log {}", args.join(" ")),
1112|1111|        &result.stdout,
1113|1112|        &filtered,
1114|1113|    );
1115|1114|
1116|1115|    Ok(0)
1117|1116|}
1118|1117|
1119|1118|/// Filter git log output: truncate long messages, cap lines
1120|1119|/// Parse the user-specified limit from git log args.
1121|1120|/// Handles: -20, -n 20, --max-count=20, --max-count 20
1122|1121|fn parse_user_limit(args: &[String]) -> Option<usize> {
1123|1122|    let mut iter = args.iter();
1124|1123|    while let Some(arg) = iter.next() {
1125|1124|        // -20 (combined digit form)
1126|1125|        if arg.starts_with('-')
1127|1126|            && arg.len() > 1
1128|1127|            && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
1129|1128|        {
1130|1129|            if let Ok(n) = arg[1..].parse::<usize>() {
1131|1130|                return Some(n);
1132|1131|            }
1133|1132|        }
1134|1133|        // -n 20 (two-token form)
1135|1134|        if arg == "-n" {
1136|1135|            if let Some(next) = iter.next() {
1137|1136|                if let Ok(n) = next.parse::<usize>() {
1138|1137|                    return Some(n);
1139|1138|                }
1140|1139|            }
1141|1140|        }
1142|1141|        // --max-count=20
1143|1142|        if let Some(rest) = arg.strip_prefix("--max-count=") {
1144|1143|            if let Ok(n) = rest.parse::<usize>() {
1145|1144|                return Some(n);
1146|1145|            }
1147|1146|        }
1148|1147|        // --max-count 20 (two-token form)
1149|1148|        if arg == "--max-count" {
1150|1149|            if let Some(next) = iter.next() {
1151|1150|                if let Ok(n) = next.parse::<usize>() {
1152|1151|                    return Some(n);
1153|1152|                }
1154|1153|            }
1155|1154|        }
1156|1155|    }
1157|1156|    None
1158|1157|}
1159|1158|
1160|1159|/// When `user_set_limit` is true, the user explicitly passed `-N` to git log,
1161|1160|/// so we skip line capping (git already returns exactly N commits) and use a
1162|1161|/// wider truncation threshold (120 chars) to preserve commit context that LLMs
1163|1162|/// need for rebase/squash operations.
1164|1163|pub(crate) fn filter_log_output(
1165|1164|    output: &str,
1166|1165|    limit: usize,
1167|1166|    user_set_limit: bool,
1168|1167|    user_format: bool,
1169|1168|) -> String {
1170|1169|    let truncate_width = if user_set_limit { 120 } else { 80 };
1171|1170|
1172|1171|    // When user specified their own format (--oneline, --pretty, --format),
1173|1172|    // RTK did not inject ---END--- markers. Use simple line-based truncation.
1174|1173|    if user_format {
1175|1174|        let lines: Vec<&str> = output.lines().collect();
1176|1175|        let max_lines = if user_set_limit { lines.len() } else { limit };
1177|1176|        return lines
1178|1177|            .iter()
1179|1178|            .take(max_lines)
1180|1179|            .map(|l| truncate_line(l, truncate_width))
1181|1180|            .collect::<Vec<_>>()
1182|1181|            .join("\n");
1183|1182|    }
1184|1183|
1185|1184|    // RTK injected format: split output into commit blocks separated by ---END---
1186|1185|    let commits: Vec<&str> = output.split("---END---").collect();
1187|1186|    let max_commits = if user_set_limit { commits.len() } else { limit };
1188|1187|
1189|1188|    let mut result = Vec::new();
1190|1189|    for block in commits.iter().take(max_commits) {
1191|1190|        let block = block.trim();
1192|1191|        if block.is_empty() {
1193|1192|            continue;
1194|1193|        }
1195|1194|        let mut lines = block.lines();
1196|1195|        // First line is the header: hash subject (date) <author>
1197|1196|        let header = match lines.next() {
1198|1197|            Some(h) => truncate_line(h.trim(), truncate_width),
1199|1198|            None => continue,
1200|1199|        };
1201|1200|        // Remaining lines are the body — keep up to 3 non-empty, non-trailer lines
1202|1201|        let all_body_lines: Vec<&str> = lines
1203|1202|            .map(|l| l.trim())
1204|1203|            .filter(|l| {
1205|1204|                !l.is_empty()
1206|1205|                    && !l.starts_with("Signed-off-by:")
1207|1206|                    && !l.starts_with("Co-authored-by:")
1208|1207|            })
1209|1208|            .collect();
1210|1209|        let body_omitted = all_body_lines.len().saturating_sub(3);
1211|1210|        let body_lines = &all_body_lines[..all_body_lines.len().min(3)];
1212|1211|
1213|1212|        if body_lines.is_empty() {
1214|1213|            result.push(header);
1215|1214|        } else {
1216|1215|            let mut entry = header;
1217|1216|            for body in body_lines {
1218|1217|                entry.push_str(&format!("\n  {}", truncate_line(body, truncate_width)));
1219|1218|            }
1220|1219|            if body_omitted > 0 {
1221|1220|                entry.push_str(&format!("\n  [+{} lines omitted]", body_omitted));
1222|1221|            }
1223|1222|            result.push(entry);
1224|1223|        }
1225|1224|    }
1226|1225|
1227|1226|    result.join("\n").trim().to_string()
1228|1227|}
1229|1228|
1230|1229|/// Truncate a single line to `width` characters, appending "..." if needed
1231|1230|fn truncate_line(line: &str, width: usize) -> String {
1232|1231|    if line.chars().count() > width {
1233|1232|        let truncated: String = line.chars().take(width - 3).collect();
1234|1233|        format!("{}...", truncated)
1235|1234|    } else {
1236|1235|        line.to_string()
1237|1236|    }
1238|1237|}
1239|1238|
1240|1239|pub(crate) fn format_status_output(porcelain: &str) -> String {
1241|1240|    format_status_inner(porcelain, None)
1242|1241|}
1243|1242|
1244|1243|pub(crate) fn format_status_output_detached(porcelain: &str, detached_ref: &str) -> String {
1245|1244|    format_status_inner(porcelain, Some(detached_ref))
1246|1245|}
1247|1246|
1248|1247|fn format_status_inner(porcelain: &str, detached: Option<&str>) -> String {
1249|1248|    let lines: Vec<&str> = porcelain
1250|1249|        .lines()
1251|1250|        .filter(|line| !line.trim().is_empty())
1252|1251|        .collect();
1253|1252|
1254|1253|    if lines.is_empty() {
1255|1254|        return "Clean working tree".to_string();
1256|1255|    }
1257|1256|
1258|1257|    let mut output = Vec::new();
1259|1258|
1260|1259|    if let Some(branch_line) = lines.first() {
1261|1260|        if branch_line.starts_with("##") {
1262|1261|            let branch = branch_line.trim_start_matches("## ");
1263|1262|            let display = detached.unwrap_or(branch);
1264|1263|            output.push(format!("* {}", display));
1265|1264|        } else {
1266|1265|            output.push((*branch_line).to_string());
1267|1266|        }
1268|1267|    }
1269|1268|
1270|1269|    for line in lines.iter().skip(1) {
1271|1270|        output.push((*line).to_string());
1272|1271|    }
1273|1272|
1274|1273|    if lines.len() == 1 && lines[0].starts_with("##") {
1275|1274|        output.push("clean — nothing to commit".to_string());
1276|1275|    }
1277|1276|
1278|1277|    output.join("\n")
1279|1278|}
1280|1279|
1281|1280|#[derive(Debug, Clone, Copy, PartialEq, Eq)]
1282|1281|enum GitStatusState {
1283|1282|    Rebase,
1284|1283|    MergeConflicts,
1285|1284|    MergeReadyToCommit,
1286|1285|    CherryPick,
1287|1286|    Revert,
1288|1287|    Bisect,
1289|1288|    Am,
1290|1289|    SparseCheckout,
1291|1290|}
1292|1291|
1293|1292|impl GitStatusState {
1294|1293|    fn summary(self) -> &'static str {
1295|1294|        match self {
1296|1295|            Self::Rebase => "rebase in progress",
1297|1296|            Self::MergeConflicts => "merge in progress. unresolved conflicts",
1298|1297|            Self::MergeReadyToCommit => "merge in progress. no conflicts",
1299|1298|            Self::CherryPick => "cherry-pick in progress",
1300|1299|            Self::Revert => "revert in progress",
1301|1300|            Self::Bisect => "bisect in progress",
1302|1301|            Self::Am => "am session in progress",
1303|1302|            Self::SparseCheckout => "sparse checkout enabled",
1304|1303|        }
1305|1304|    }
1306|1305|}
1307|1306|
1308|1307|const REBASE_INDICATORS: &[&str] = &[
1309|1308|    "rebase in progress",
1310|1309|    "You are currently rebasing",
1311|1310|    "You are currently editing",
1312|1311|    "You are currently splitting",
1313|1312|    "Last command done",
1314|1313|    "Next command to do",
1315|1314|    "No commands remaining",
1316|1315|];
1317|1316|
1318|1317|fn detect_status_state(line: &str) -> Option<GitStatusState> {
1319|1318|    if line.contains("All conflicts fixed but you are still merging") {
1320|1319|        Some(GitStatusState::MergeReadyToCommit)
1321|1320|    } else if line.contains("You have unmerged paths") {
1322|1321|        Some(GitStatusState::MergeConflicts)
1323|1322|    } else if line.contains("You are currently cherry-picking") {
1324|1323|        Some(GitStatusState::CherryPick)
1325|1324|    } else if line.contains("You are currently reverting") {
1326|1325|        Some(GitStatusState::Revert)
1327|1326|    } else if line.contains("You are currently bisecting") {
1328|1327|        Some(GitStatusState::Bisect)
1329|1328|    } else if line.contains("You are in the middle of an am session") {
1330|1329|        Some(GitStatusState::Am)
1331|1330|    } else if line.contains("You are in a sparse checkout") {
1332|1331|        Some(GitStatusState::SparseCheckout)
1333|1332|    } else if REBASE_INDICATORS.iter().any(|i| line.contains(i)) {
1334|1333|        Some(GitStatusState::Rebase)
1335|1334|    } else {
1336|1335|        None
1337|1336|    }
1338|1337|}
1339|1338|
1340|1339|/// Extract a compact in-progress state summary from plain `git status` output.
1341|1340|///
1342|1341|/// Compact mode runs `git status --porcelain -b`, which omits the state header
1343|1342|/// git prints for rebase / merge / cherry-pick / revert / bisect / am / sparse
1344|1343|/// checkout. Hiding that block is a correctness bug — e.g. during an interactive
1345|1344|/// rebase edit, the user sees a "clean" status and misses "You are currently
1346|1345|/// editing a commit while rebasing ...".
1347|1346|///
1348|1347|/// This helper walks the plain-status output we already capture for tracking
1349|1348|/// and emits a compact, RTK-style summary rather than dumping git's full prose.
1350|1349|/// Returns `None` when no state is in progress.
1351|1350|fn extract_state_header(raw: &str) -> Option<String> {
1352|1351|    // Headers of the file-change blocks — everything relevant to state appears
1353|1352|    // above these in git's output, so they double as a terminator.
1354|1353|    const STOPPERS: &[&str] = &[
1355|1354|        "Changes to be committed:",
1356|1355|        "Changes not staged for commit:",
1357|1356|        "Untracked files:",
1358|1357|        "Unmerged paths:",
1359|1358|        "no changes added to commit",
1360|1359|        "nothing to commit",
1361|1360|        "nothing added to commit",
1362|1361|    ];
1363|1362|
1364|1363|    for line in raw.lines() {
1365|1364|        let stripped = line.trim();
1366|1365|
1367|1366|        if STOPPERS.iter().any(|s| stripped.starts_with(s)) {
1368|1367|            break;
1369|1368|        }
1370|1369|
1371|1370|        if let Some(state) = detect_status_state(stripped) {
1372|1371|            return Some(state.summary().to_string());
1373|1372|        }
1374|1373|    }
1375|1374|
1376|1375|    None
1377|1376|}
1378|1377|
1379|1378|/// Extract the explicit "HEAD detached at/from <ref>" line from plain
1380|1379|/// `git status` output.
1381|1380|///
1382|1381|/// Porcelain `-b` collapses a detached HEAD to the opaque `## HEAD (no branch)`,
1383|1382|/// which an agent (or a distracted human) can misread as a branch literally
1384|1383|/// named `HEAD`. The plain-status output keeps the explicit SHA/ref, so we
1385|1384|/// surface that instead. Returns `None` when HEAD is on a branch.
1386|1385|fn extract_detached_head(raw: &str) -> Option<String> {
1387|1386|    raw.lines()
1388|1387|        .map(str::trim)
1389|1388|        .find(|l| l.starts_with("HEAD detached "))
1390|1389|        .map(str::to_string)
1391|1390|}
1392|1391|
1393|1392|/// Minimal filtering for git status with user-provided args
1394|1393|fn filter_status_with_args(output: &str) -> String {
1395|1394|    let mut result = Vec::new();
1396|1395|
1397|1396|    for line in output.lines() {
1398|1397|        let trimmed = line.trim();
1399|1398|
1400|1399|        // Skip empty lines
1401|1400|        if trimmed.is_empty() {
1402|1401|            continue;
1403|1402|        }
1404|1403|
1405|1404|        // Skip git hints - can appear at start or within line
1406|1405|        if trimmed.starts_with("(use \"git")
1407|1406|            || trimmed.starts_with("(create/copy files")
1408|1407|            || trimmed.contains("(use \"git add")
1409|1408|            || trimmed.contains("(use \"git restore")
1410|1409|        {
1411|1410|            continue;
1412|1411|        }
1413|1412|
1414|1413|        // Special case: clean working tree
1415|1414|        if trimmed.contains("nothing to commit") && trimmed.contains("working tree clean") {
1416|1415|            result.push(trimmed.to_string());
1417|1416|            break;
1418|1417|        }
1419|1418|
1420|1419|        result.push(line.to_string());
1421|1420|    }
1422|1421|
1423|1422|    if result.is_empty() {
1424|1423|        "ok".to_string()
1425|1424|    } else {
1426|1425|        result.join("\n")
1427|1426|    }
1428|1427|}
1429|1428|
1430|1429|fn run_status(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1431|1430|    let timer = tracking::TimedExecution::start();
1432|1431|
1433|1432|    // Keep a narrow compact path for no-arg status and branch/short-only flags.
1434|1433|    // More complex explicit args still use the existing minimal-filter path.
1435|1434|    if !uses_compact_status_path(args) {
1436|1435|        let mut cmd = build_status_command(args, global_args);
1437|1436|        let result = exec_capture(&mut cmd).context("Failed to run git status")?;
1438|1437|
1439|1438|        if !result.success() {
1440|1439|            if !result.stderr.trim().is_empty() {
1441|1440|                eprint!("{}", result.stderr);
1442|1441|            }
1443|1442|            timer.track(
1444|1443|                &format!("git status {}", args.join(" ")),
1445|1444|                &format!("rtk git status {}", args.join(" ")),
1446|1445|                &result.stdout,
1447|1446|                &result.stdout,
1448|1447|            );
1449|1448|            return Ok(result.exit_code);
1450|1449|        }
1451|1450|
1452|1451|        if verbose > 0 || !result.stderr.is_empty() {
1453|1452|            eprint!("{}", result.stderr);
1454|1453|        }
1455|1454|
1456|1455|        // Apply minimal filtering: strip ANSI, remove hints, empty lines
1457|1456|        let filtered = filter_status_with_args(&result.stdout);
1458|1457|        print!("{}", filtered);
1459|1458|
1460|1459|        timer.track(
1461|1460|            &format!("git status {}", args.join(" ")),
1462|1461|            &format!("rtk git status {}", args.join(" ")),
1463|1462|            &result.stdout,
1464|1463|            &filtered,
1465|1464|        );
1466|1465|
1467|1466|        return Ok(0);
1468|1467|    }
1469|1468|
1470|1469|    let mut raw_cmd = git_cmd_c_locale(global_args);
1471|1470|    raw_cmd.arg("status");
1472|1471|    raw_cmd.args(args);
1473|1472|    let raw_output = exec_capture(&mut raw_cmd)
1474|1473|        .map(|r| r.stdout)
1475|1474|        .unwrap_or_default();
1476|1475|
1477|1476|    let mut cmd = build_status_command(args, global_args);
1478|1477|    let result = exec_capture(&mut cmd).context("Failed to run git status")?;
1479|1478|
1480|1479|    if !result.stderr.is_empty() && result.stderr.contains("not a git repository") {
1481|1480|        let message = "Not a git repository".to_string();
1482|1481|        eprintln!("{}", message);
1483|1482|        let original_cmd = if args.is_empty() {
1484|1483|            "git status".to_string()
1485|1484|        } else {
1486|1485|            format!("git status {}", args.join(" "))
1487|1486|        };
1488|1487|        let rtk_cmd = if args.is_empty() {
1489|1488|            "rtk git status".to_string()
1490|1489|        } else {
1491|1490|            format!("rtk git status {}", args.join(" "))
1492|1491|        };
1493|1492|        timer.track(&original_cmd, &rtk_cmd, &raw_output, &message);
1494|1493|        return Ok(result.exit_code);
1495|1494|    }
1496|1495|
1497|1496|    let formatted = match extract_detached_head(&raw_output) {
1498|1497|        Some(detached_ref) => format_status_output_detached(&result.stdout, &detached_ref),
1499|1498|        None => format_status_output(&result.stdout),
1500|1499|    };
1501|1500|
1502|1501|    // Surface in-progress state (rebase/merge/cherry-pick/bisect/am) from the
1503|1502|    // plain-status output we already captured for tracking. Porcelain omits it
1504|1503|    // and hiding it misleads the user about the true repo state.
1505|1504|    let final_output = match extract_state_header(&raw_output) {
1506|1505|        Some(state) => format!("{}\n{}", state, formatted),
1507|1506|        None => formatted,
1508|1507|    };
1509|1508|
1510|1509|    println!("{}", final_output);
1511|1510|
1512|1511|    let original_cmd = if args.is_empty() {
1513|1512|        "git status".to_string()
1514|1513|    } else {
1515|1514|        format!("git status {}", args.join(" "))
1516|1515|    };
1517|1516|    let rtk_cmd = if args.is_empty() {
1518|1517|        "rtk git status".to_string()
1519|1518|    } else {
1520|1519|        format!("rtk git status {}", args.join(" "))
1521|1520|    };
1522|1521|
1523|1522|    timer.track(&original_cmd, &rtk_cmd, &raw_output, &final_output);
1524|1523|
1525|1524|    Ok(0)
1526|1525|}
1527|1526|
1528|1527|fn run_add(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1529|1528|    let timer = tracking::TimedExecution::start();
1530|1529|
1531|1530|    let mut cmd = git_cmd(global_args);
1532|1531|    cmd.arg("add");
1533|1532|
1534|1533|    // Pass all arguments directly to git (flags like -A, -p, --all, etc.)
1535|1534|    if args.is_empty() {
1536|1535|        cmd.arg(".");
1537|1536|    } else {
1538|1537|        for arg in args {
1539|1538|            cmd.arg(arg);
1540|1539|        }
1541|1540|    }
1542|1541|
1543|1542|    let result = exec_capture(&mut cmd).context("Failed to run git add")?;
1544|1543|
1545|1544|    if verbose > 0 {
1546|1545|        eprintln!("git add executed");
1547|1546|    }
1548|1547|
1549|1548|    let raw_output = format!("{}\n{}", result.stdout, result.stderr);
1550|1549|
1551|1550|    if result.success() {
1552|1551|        // Count what was added
1553|1552|        let mut stat_cmd = git_cmd(global_args);
1554|1553|        stat_cmd.args(["diff", "--cached", "--stat", "--shortstat"]);
1555|1554|        let stat_result = exec_capture(&mut stat_cmd).context("Failed to check staged files")?;
1556|1555|
1557|1556|        // Mirror git's own behaviour: a no-op `git add` is silent. Emitting a
1558|1557|        // generic "ok" here is misleading — an agent can't tell "staged N files"
1559|1558|        // from "staged nothing" when both print "ok".
1560|1559|        let compact = if stat_result.stdout.trim().is_empty() {
1561|1560|            String::new()
1562|1561|        } else {
1563|1562|            // Parse "1 file changed, 5 insertions(+)" format
1564|1563|            let short = stat_result.stdout.lines().last().unwrap_or("").trim();
1565|1564|            if short.is_empty() {
1566|1565|                "ok".to_string()
1567|1566|            } else {
1568|1567|                format!("ok {}", short)
1569|1568|            }
1570|1569|        };
1571|1570|
1572|1571|        if !compact.is_empty() {
1573|1572|            println!("{}", compact);
1574|1573|        }
1575|1574|
1576|1575|        timer.track(
1577|1576|            &format!("git add {}", args.join(" ")),
1578|1577|            &format!("rtk git add {}", args.join(" ")),
1579|1578|            &raw_output,
1580|1579|            &compact,
1581|1580|        );
1582|1581|    } else {
1583|1582|        eprintln!("FAILED: git add");
1584|1583|        if !result.stderr.trim().is_empty() {
1585|1584|            eprintln!("{}", result.stderr);
1586|1585|        }
1587|1586|        if !result.stdout.trim().is_empty() {
1588|1587|            eprintln!("{}", result.stdout);
1589|1588|        }
1590|1589|        return Ok(result.exit_code);
1591|1590|    }
1592|1591|
1593|1592|    Ok(0)
1594|1593|}
1595|1594|
1596|1595|fn build_commit_command(args: &[String], global_args: &[String]) -> Command {
1597|1596|    let mut cmd = git_cmd(global_args);
1598|1597|    cmd.arg("commit");
1599|1598|    for arg in args {
1600|1599|        cmd.arg(arg);
1601|1600|    }
1602|1601|    cmd
1603|1602|}
1604|1603|
1605|1604|fn run_commit(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1606|1605|    let timer = tracking::TimedExecution::start();
1607|1606|
1608|1607|    let original_cmd = format!("git commit {}", args.join(" "));
1609|1608|
1610|1609|    if verbose > 0 {
1611|1610|        eprintln!("{}", original_cmd);
1612|1611|    }
1613|1612|
1614|1613|    let output = build_commit_command(args, global_args)
1615|1614|        .stdin(Stdio::inherit())
1616|1615|        .output()
1617|1616|        .context("Failed to run git commit")?;
1618|1617|
1619|1618|    let stdout = String::from_utf8_lossy(&output.stdout);
1620|1619|    let stderr = String::from_utf8_lossy(&output.stderr);
1621|1620|    let exit_code = exit_code_from_output(&output, "git commit");
1622|1621|    let raw_output = format!("{}\n{}", stdout, stderr);
1623|1622|
1624|1623|    if output.status.success() {
1625|1624|        // Extract commit hash from output like "[main abc1234] message"
1626|1625|        let compact = if let Some(line) = stdout.lines().next() {
1627|1626|            if let Some(hash_start) = line.find(' ') {
1628|1627|                let hash = line[1..hash_start].split(' ').next_back().unwrap_or("");
1629|1628|                if !hash.is_empty() && hash.len() >= 7 {
1630|1629|                    format!("ok {}", &hash[..7.min(hash.len())])
1631|1630|                } else {
1632|1631|                    "ok".to_string()
1633|1632|                }
1634|1633|            } else {
1635|1634|                "ok".to_string()
1636|1635|            }
1637|1636|        } else {
1638|1637|            "ok".to_string()
1639|1638|        };
1640|1639|
1641|1640|        println!("{}", compact);
1642|1641|
1643|1642|        timer.track(&original_cmd, "rtk git commit", &raw_output, &compact);
1644|1643|    } else if stderr.contains("nothing to commit") || stdout.contains("nothing to commit") {
1645|1644|        println!("ok (nothing to commit)");
1646|1645|        timer.track(
1647|1646|            &original_cmd,
1648|1647|            "rtk git commit",
1649|1648|            &raw_output,
1650|1649|            "ok (nothing to commit)",
1651|1650|        );
1652|1651|    } else {
1653|1652|        if !stderr.trim().is_empty() {
1654|1653|            eprint!("{}", stderr);
1655|1654|        }
1656|1655|        if !stdout.trim().is_empty() {
1657|1656|            eprint!("{}", stdout);
1658|1657|        }
1659|1658|        timer.track(&original_cmd, "rtk git commit", &raw_output, &raw_output);
1660|1659|        return Ok(exit_code);
1661|1660|    }
1662|1661|
1663|1662|    Ok(0)
1664|1663|}
1665|1664|
1666|1665|// Git push progress prefixes (stderr) — dropped from the stream.
1667|1666|const GIT_PUSH_NOISE_PREFIXES: &[&str] = &[
1668|1667|    "Enumerating objects:",
1669|1668|    "Counting objects:",
1670|1669|    "Compressing objects:",
1671|1670|    "Writing objects:",
1672|1671|    "Delta compression using",
1673|1672|    "Total ",
1674|1673|];
1675|1674|
1676|1675|#[derive(Default)]
1677|1676|struct GitPushLineHandler {
1678|1677|    up_to_date: bool,
1679|1678|    pushed_ref: Option<String>,
1680|1679|}
1681|1680|
1682|1681|impl LineHandler for GitPushLineHandler {
1683|1682|    fn should_skip(&mut self, line: &str) -> bool {
1684|1683|        if line.is_empty() {
1685|1684|            return true;
1686|1685|        }
1687|1686|        let trimmed = line.trim_start();
1688|1687|        GIT_PUSH_NOISE_PREFIXES
1689|1688|            .iter()
1690|1689|            .any(|p| trimmed.starts_with(p))
1691|1690|    }
1692|1691|
1693|1692|    fn observe_line(&mut self, line: &str) {
1694|1693|        if line.contains("Everything up-to-date") {
1695|1694|            self.up_to_date = true;
1696|1695|        }
1697|1696|        if self.pushed_ref.is_none() {
1698|1697|            if let Some(idx) = line.find(" -> ") {
1699|1698|                let after = &line[idx + 4..];
1700|1699|                if let Some(dest) = after.split_whitespace().next() {
1701|1700|                    self.pushed_ref = Some(dest.to_string());
1702|1701|                }
1703|1702|            }
1704|1703|        }
1705|1704|    }
1706|1705|
1707|1706|    fn format_summary(&self, exit_code: i32, _raw: &str) -> Option<String> {
1708|1707|        if exit_code != 0 {
1709|1708|            return None;
1710|1709|        }
1711|1710|        let summary = if self.up_to_date {
1712|1711|            "ok (up-to-date)".to_string()
1713|1712|        } else if let Some(dest) = &self.pushed_ref {
1714|1713|            format!("ok {}", dest)
1715|1714|        } else {
1716|1715|            "ok".to_string()
1717|1716|        };
1718|1717|        Some(format!("{}\n", summary))
1719|1718|    }
1720|1719|}
1721|1720|
1722|1721|fn run_push(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1723|1722|    let timer = tracking::TimedExecution::start();
1724|1723|
1725|1724|    if verbose > 0 {
1726|1725|        eprintln!("git push");
1727|1726|    }
1728|1727|
1729|1728|    let mut cmd = git_cmd(global_args);
1730|1729|    cmd.arg("push");
1731|1730|    for arg in args {
1732|1731|        cmd.arg(arg);
1733|1732|    }
1734|1733|
1735|1734|    let cmd_label = format!("git push {}", args.join(" "));
1736|1735|    let filter = LineStreamFilter::new(GitPushLineHandler::default());
1737|1736|    let result = stream::run_streaming(
1738|1737|        &mut cmd,
1739|1738|        StdinMode::Inherit,
1740|1739|        FilterMode::Streaming(Box::new(filter)),
1741|1740|    )
1742|1741|    .context("Failed to run git push")?;
1743|1742|
1744|1743|    timer.track(
1745|1744|        &cmd_label,
1746|1745|        &format!("rtk {}", cmd_label),
1747|1746|        &result.raw,
1748|1747|        &result.filtered,
1749|1748|    );
1750|1749|
1751|1750|    Ok(result.exit_code)
1752|1751|}
1753|1752|
1754|1753|fn run_pull(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1755|1754|    let timer = tracking::TimedExecution::start();
1756|1755|
1757|1756|    if verbose > 0 {
1758|1757|        eprintln!("git pull");
1759|1758|    }
1760|1759|
1761|1760|    let mut cmd = git_cmd(global_args);
1762|1761|    cmd.arg("pull");
1763|1762|    for arg in args {
1764|1763|        cmd.arg(arg);
1765|1764|    }
1766|1765|
1767|1766|    let result = exec_capture(&mut cmd).context("Failed to run git pull")?;
1768|1767|
1769|1768|    let raw_output = format!("{}\n{}", result.stdout, result.stderr);
1770|1769|
1771|1770|    if result.success() {
1772|1771|        let compact = if result.stdout.contains("Already up to date")
1773|1772|            || result.stdout.contains("Already up-to-date")
1774|1773|        {
1775|1774|            "ok (up-to-date)".to_string()
1776|1775|        } else {
1777|1776|            // Count files changed
1778|1777|            let mut files = 0;
1779|1778|            let mut insertions = 0;
1780|1779|            let mut deletions = 0;
1781|1780|
1782|1781|            for line in result.stdout.lines() {
1783|1782|                if line.contains("file") && line.contains("changed") {
1784|1783|                    // Parse "3 files changed, 10 insertions(+), 2 deletions(-)"
1785|1784|                    for part in line.split(',') {
1786|1785|                        let part = part.trim();
1787|1786|                        if part.contains("file") {
1788|1787|                            files = part
1789|1788|                                .split_whitespace()
1790|1789|                                .next()
1791|1790|                                .and_then(|n| n.parse().ok())
1792|1791|                                .unwrap_or(0);
1793|1792|                        } else if part.contains("insertion") {
1794|1793|                            insertions = part
1795|1794|                                .split_whitespace()
1796|1795|                                .next()
1797|1796|                                .and_then(|n| n.parse().ok())
1798|1797|                                .unwrap_or(0);
1799|1798|                        } else if part.contains("deletion") {
1800|1799|                            deletions = part
1801|1800|                                .split_whitespace()
1802|1801|                                .next()
1803|1802|                                .and_then(|n| n.parse().ok())
1804|1803|                                .unwrap_or(0);
1805|1804|                        }
1806|1805|                    }
1807|1806|                }
1808|1807|            }
1809|1808|
1810|1809|            if files > 0 {
1811|1810|                format!("ok {} files +{} -{}", files, insertions, deletions)
1812|1811|            } else {
1813|1812|                "ok".to_string()
1814|1813|            }
1815|1814|        };
1816|1815|
1817|1816|        println!("{}", compact);
1818|1817|
1819|1818|        timer.track(
1820|1819|            &format!("git pull {}", args.join(" ")),
1821|1820|            &format!("rtk git pull {}", args.join(" ")),
1822|1821|            &raw_output,
1823|1822|            &compact,
1824|1823|        );
1825|1824|    } else {
1826|1825|        eprintln!("FAILED: git pull");
1827|1826|        if !result.stderr.trim().is_empty() {
1828|1827|            eprintln!("{}", result.stderr);
1829|1828|        }
1830|1829|        if !result.stdout.trim().is_empty() {
1831|1830|            eprintln!("{}", result.stdout);
1832|1831|        }
1833|1832|        return Ok(result.exit_code);
1834|1833|    }
1835|1834|
1836|1835|    Ok(0)
1837|1836|}
1838|1837|
1839|1838|fn run_branch(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1840|1839|    let timer = tracking::TimedExecution::start();
1841|1840|
1842|1841|    if verbose > 0 {
1843|1842|        eprintln!("git branch");
1844|1843|    }
1845|1844|
1846|1845|    // Detect write operations: delete, rename, copy, upstream tracking
1847|1846|    let has_action_flag = args.iter().any(|a| {
1848|1847|        a == "-d"
1849|1848|            || a == "-D"
1850|1849|            || a == "-m"
1851|1850|            || a == "-M"
1852|1851|            || a == "-c"
1853|1852|            || a == "-C"
1854|1853|            || a == "--set-upstream-to"
1855|1854|            || a.starts_with("--set-upstream-to=")
1856|1855|            || a == "-u"
1857|1856|            || a == "--unset-upstream"
1858|1857|            || a == "--edit-description"
1859|1858|    });
1860|1859|
1861|1860|    // Detect flags that produce specific output (not a branch list)
1862|1861|    let has_show_flag = args.iter().any(|a| a == "--show-current");
1863|1862|
1864|1863|    // Detect list-mode flags
1865|1864|    let has_list_flag = args.iter().any(|a| {
1866|1865|        a == "-a"
1867|1866|            || a == "--all"
1868|1867|            || a == "-r"
1869|1868|            || a == "--remotes"
1870|1869|            || a == "--list"
1871|1870|            || a == "--merged"
1872|1871|            || a == "--no-merged"
1873|1872|            || a == "--contains"
1874|1873|            || a == "--no-contains"
1875|1874|            || a == "--format"
1876|1875|            || a.starts_with("--format=")
1877|1876|            || a == "--sort"
1878|1877|            || a.starts_with("--sort=")
1879|1878|            || a == "--points-at"
1880|1879|            || a.starts_with("--points-at=")
1881|1880|    });
1882|1881|
1883|1882|    // Detect positional arguments (not flags) — indicates branch creation
1884|1883|    let has_positional_arg = args.iter().any(|a| !a.starts_with('-'));
1885|1884|
1886|1885|    // --show-current: passthrough with raw stdout (not "ok")
1887|1886|    if has_show_flag {
1888|1887|        let mut cmd = git_cmd(global_args);
1889|1888|        cmd.arg("branch");
1890|1889|        for arg in args {
1891|1890|            cmd.arg(arg);
1892|1891|        }
1893|1892|        let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1894|1893|        let combined = result.combined();
1895|1894|
1896|1895|        let trimmed = result.stdout.trim();
1897|1896|        timer.track(
1898|1897|            &format!("git branch {}", args.join(" ")),
1899|1898|            &format!("rtk git branch {}", args.join(" ")),
1900|1899|            &combined,
1901|1900|            trimmed,
1902|1901|        );
1903|1902|
1904|1903|        if result.success() {
1905|1904|            println!("{}", trimmed);
1906|1905|        } else {
1907|1906|            eprintln!("FAILED: git branch {}", args.join(" "));
1908|1907|            if !result.stderr.trim().is_empty() {
1909|1908|                eprintln!("{}", result.stderr);
1910|1909|            }
1911|1910|            return Ok(result.exit_code);
1912|1911|        }
1913|1912|        return Ok(0);
1914|1913|    }
1915|1914|
1916|1915|    // Write operation: action flags, or positional args without list flags (= branch creation)
1917|1916|    if has_action_flag || (has_positional_arg && !has_list_flag) {
1918|1917|        let mut cmd = git_cmd(global_args);
1919|1918|        cmd.arg("branch");
1920|1919|        for arg in args {
1921|1920|            cmd.arg(arg);
1922|1921|        }
1923|1922|        let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1924|1923|        let combined = result.combined();
1925|1924|
1926|1925|        let msg = if result.success() { "ok" } else { &combined };
1927|1926|
1928|1927|        timer.track(
1929|1928|            &format!("git branch {}", args.join(" ")),
1930|1929|            &format!("rtk git branch {}", args.join(" ")),
1931|1930|            &combined,
1932|1931|            msg,
1933|1932|        );
1934|1933|
1935|1934|        if result.success() {
1936|1935|            println!("ok");
1937|1936|        } else {
1938|1937|            eprintln!("FAILED: git branch {}", args.join(" "));
1939|1938|            if !result.stderr.trim().is_empty() {
1940|1939|                eprintln!("{}", result.stderr);
1941|1940|            }
1942|1941|            if !result.stdout.trim().is_empty() {
1943|1942|                eprintln!("{}", result.stdout);
1944|1943|            }
1945|1944|            return Ok(result.exit_code);
1946|1945|        }
1947|1946|        return Ok(0);
1948|1947|    }
1949|1948|
1950|1949|    // List mode: show compact branch list
1951|1950|    let mut cmd = git_cmd(global_args);
1952|1951|    cmd.arg("branch");
1953|1952|    if !has_list_flag {
1954|1953|        cmd.arg("-a");
1955|1954|    }
1956|1955|    cmd.arg("--no-color");
1957|1956|    for arg in args {
1958|1957|        cmd.arg(arg);
1959|1958|    }
1960|1959|
1961|1960|    let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1962|1961|
1963|1962|    if !result.success() {
1964|1963|        if !result.stderr.trim().is_empty() {
1965|1964|            eprint!("{}", result.stderr);
1966|1965|        }
1967|1966|        timer.track(
1968|1967|            &format!("git branch {}", args.join(" ")),
1969|1968|            &format!("rtk git branch {}", args.join(" ")),
1970|1969|            &result.stdout,
1971|1970|            &result.stdout,
1972|1971|        );
1973|1972|        return Ok(result.exit_code);
1974|1973|    }
1975|1974|
1976|1975|    let filtered = filter_branch_output(&result.stdout);
1977|1976|    println!("{}", filtered);
1978|1977|
1979|1978|    timer.track(
1980|1979|        &format!("git branch {}", args.join(" ")),
1981|1980|        &format!("rtk git branch {}", args.join(" ")),
1982|1981|        &result.stdout,
1983|1982|        &filtered,
1984|1983|    );
1985|1984|
1986|1985|    Ok(0)
1987|1986|}
1988|1987|
1989|1988|fn filter_branch_output(output: &str) -> String {
1990|1989|    let mut current = String::new();
1991|1990|    let mut local: Vec<String> = Vec::new();
1992|1991|    let mut remote: Vec<String> = Vec::new();
1993|1992|    let mut seen_remote: std::collections::HashSet<String> = std::collections::HashSet::new();
1994|1993|
1995|1994|    for line in output.lines() {
1996|1995|        let line = line.trim();
1997|1996|        if line.is_empty() {
1998|1997|            continue;
1999|1998|        }
2000|1999|
2001|