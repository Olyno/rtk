<<<<<<< HEAD
1|//! Filters git output — log, status, diff, and more — keeping just the essential info.
2|
3|use crate::core::stream::{
4|    self, exec_capture, CaptureResult, FilterMode, LineHandler, LineStreamFilter, StdinMode,
5|};
6|use crate::core::tracking;
7|use crate::core::truncate::CAP_WARNINGS;
8|use crate::core::utils::{exit_code_from_output, exit_code_from_status, resolved_command};
9|use anyhow::{Context, Result};
10|use std::ffi::OsString;
11|use std::process::Command;
12|use std::process::Stdio;
13|
14|#[derive(Debug, Clone)]
15|pub enum GitCommand {
16|    Diff,
17|    Log,
18|    Status,
19|    Show,
20|    Add,
21|    Commit,
22|    Push,
23|    Pull,
24|    Branch,
25|    Fetch,
26|    Stash { subcommand: Option<String> },
27|    Worktree,
28|}
29|
30|/// Create a git Command with global options (e.g. -C, -c, --git-dir, --work-tree)
31|/// prepended before any subcommand arguments.
32|fn git_cmd(global_args: &[String]) -> Command {
33|    let mut cmd = resolved_command("git");
34|    for arg in global_args {
35|        cmd.arg(arg);
36|    }
37|    cmd
38|}
39|
40|/// Create a git Command for internal parsing that must be locale-stable.
41|///
42|/// We only use this for non-user-facing parses where RTK depends on git's
43|/// English status phrases. User-visible passthrough output keeps the user's
44|/// locale.
45|fn git_cmd_c_locale(global_args: &[String]) -> Command {
46|    let mut cmd = git_cmd(global_args);
47|    cmd.env("LC_ALL", "C");
48|    cmd
49|}
50|
51|fn uses_compact_status_path(args: &[String]) -> bool {
52|    if args.is_empty() {
53|        return true;
54|    }
55|
56|    let mut saw_branch = false;
57|    for arg in args {
58|        match arg.as_str() {
59|            "-b" | "--branch" => saw_branch = true,
60|            "-sb" | "-bs" => return true,
61|            "-s" | "--short" => {}
62|            _ => return false,
63|        }
64|    }
65|
66|    saw_branch
67|}
68|
69|fn build_status_command(args: &[String], global_args: &[String]) -> Command {
70|    let mut cmd = git_cmd(global_args);
71|    cmd.arg("status");
72|    if uses_compact_status_path(args) {
73|        cmd.args(["--porcelain", "-b"]);
74|    } else {
75|        cmd.args(args);
76|    }
77|    cmd
78|}
79|
80|pub fn run(
81|    cmd: GitCommand,
82|    args: &[String],
83|    max_lines: Option<usize>,
84|    verbose: u8,
85|    global_args: &[String],
86|) -> Result<i32> {
87|    match cmd {
88|        GitCommand::Diff => run_diff(args, max_lines, verbose, global_args),
89|        GitCommand::Log => run_log(args, max_lines, verbose, global_args),
90|        GitCommand::Status => run_status(args, verbose, global_args),
91|        GitCommand::Show => run_show(args, max_lines, verbose, global_args),
92|        GitCommand::Add => run_add(args, verbose, global_args),
93|        GitCommand::Commit => run_commit(args, verbose, global_args),
94|        GitCommand::Push => run_push(args, verbose, global_args),
95|        GitCommand::Pull => run_pull(args, verbose, global_args),
96|        GitCommand::Branch => run_branch(args, verbose, global_args),
97|        GitCommand::Fetch => run_fetch(args, verbose, global_args),
98|        GitCommand::Stash { subcommand } => {
99|            run_stash(subcommand.as_deref(), args, verbose, global_args)
100|        }
101|        GitCommand::Worktree => run_worktree(args, verbose, global_args),
102|    }
103|}
104|
105|/// Re-insert `--` before the first path-like argument when clap has consumed it.
106|///
107|/// clap's `trailing_var_arg = true` silently drops `--` when it appears as the
108|/// first positional argument (before any other positional).  This means:
109|///   `rtk git diff -- file` → args = ["file"]   (clap ate `--`)
110|///   `rtk git diff HEAD -- file` → args = ["HEAD", "--", "file"]  (preserved)
111|///
112|/// Without the `--` separator git may treat an unambiguous path as a revision and
113|/// emit "fatal: ambiguous argument".  We re-insert `--` before the first path-like
114|/// argument; see `normalize_diff_args_impl` for the detection rules.
115|fn normalize_diff_args(args: &[String]) -> Vec<String> {
116|    normalize_diff_args_impl(args, |p| std::path::Path::new(p).exists())
117|}
118|
119|/// Testable core of `normalize_diff_args` — accepts an injectable filesystem existence checker.
120|///
121|/// The path-detection logic is:
122|/// 1. Explicit path prefixes (`.`, `~`) → always a path, no filesystem check needed.
123|/// 2. Contains path separator (`/`, `\`) → use `path_exists` to distinguish branch names
124|///    (e.g. `feature/auth`) from real paths (e.g. `src/main.rs`).
125|/// 3. Bare word with no separator → never a path (avoids injecting `--` when a file
126|///    happens to share a name with a branch or ref, e.g. a file named `main`).
127|fn normalize_diff_args_impl<F>(args: &[String], path_exists: F) -> Vec<String>
128|where
129|    F: Fn(&str) -> bool,
130|{
131|    // Already has `--` — nothing to do
132|    if args.iter().any(|a| a == "--") {
133|        return args.to_vec();
134|    }
135|    let path_start = args.iter().position(|arg| {
136|        if arg.starts_with('-') {
137|            return false;
138|        }
139|        // Explicit path prefixes — always treat as path regardless of existence
140|        if arg.starts_with('.') || arg.starts_with('~') {
141|            return true;
142|        }
143|        // Contains path separator — use filesystem check to distinguish
144|        // branch names (feature/auth) from real paths (src/main.rs)
145|        if arg.contains('/') || arg.contains('\\') {
146|            return path_exists(arg);
147|        }
148|        // Bare word (no separator, no special prefix) — never inject `--`
149|        // This avoids misidentifying a ref/branch as a path even if a same-named
150|        // file happens to exist on disk.
151|        false
152|    });
153|    match path_start {
154|        Some(idx) => {
155|            let mut out = args[..idx].to_vec();
156|            out.push("--".to_string());
157|            out.extend_from_slice(&args[idx..]);
158|            out
159|        }
160|        None => args.to_vec(),
161|    }
162|}
163|
164|fn run_diff(
165|    args: &[String],
166|    max_lines: Option<usize>,
167|    verbose: u8,
168|    global_args: &[String],
169|) -> Result<i32> {
170|    let timer = tracking::TimedExecution::start();
171|
172|    // Re-insert `--` when clap's trailing_var_arg consumed it (issue #1215)
173|    let args = &normalize_diff_args(args);
174|
175|    // Check if user wants stat output
176|    let wants_stat = args
177|        .iter()
178|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
179|
180|    // Check if user wants compact diff (default RTK behavior)
181|    let wants_compact = !args.iter().any(|arg| arg == "--no-compact");
182|
183|    if wants_stat || !wants_compact {
184|        // User wants stat or explicitly no compacting - pass through directly
185|        let mut cmd = git_cmd(global_args);
186|        cmd.arg("diff");
187|        for arg in args {
188|            if arg == "--no-compact" {
189|                continue; // RTK flag, not a git flag
190|            }
191|            cmd.arg(arg);
192|        }
193|
194|        let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
195|
196|        if !result.success() {
197|            eprintln!("{}", result.stderr);
198|            return Ok(result.exit_code);
199|        }
200|
201|        println!("{}", result.stdout.trim());
202|
203|        timer.track(
204|            &format!("git diff {}", args.join(" ")),
205|            &format!("rtk git diff {} (passthrough)", args.join(" ")),
206|            &result.stdout,
207|            &result.stdout,
208|        );
209|
210|        return Ok(0);
211|    }
212|
213|    // Default RTK behavior: stat first, then compacted diff
214|    let mut cmd = git_cmd(global_args);
215|    cmd.arg("diff").arg("--stat");
216|
217|    for arg in args {
218|        cmd.arg(arg);
219|    }
220|
221|    let result = exec_capture(&mut cmd).context("Failed to run git diff")?;
222|
223|    if !result.success() {
224|        if !result.stderr.trim().is_empty() {
225|            eprint!("{}", result.stderr);
226|        }
227|        timer.track(
228|            &format!("git diff {}", args.join(" ")),
229|            &format!("rtk git diff {}", args.join(" ")),
230|            &result.stdout,
231|            &result.stdout,
232|        );
233|        return Ok(result.exit_code);
234|    }
235|
236|    if verbose > 0 {
237|        eprintln!("Git diff summary:");
238|    }
239|
240|    // Print stat summary first
241|    println!("{}", result.stdout.trim());
242|
243|    // Now get actual diff but compact it
244|    let mut diff_cmd = git_cmd(global_args);
245|    diff_cmd.arg("diff");
246|    for arg in args {
247|        diff_cmd.arg(arg);
248|    }
249|
250|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git diff")?;
251|
252|    let mut final_output = result.stdout.clone();
253|    if !diff_result.stdout.is_empty() {
254|        println!("\n--- Changes ---");
255|        let compacted = compact_diff(&diff_result.stdout, max_lines.unwrap_or(500));
256|        println!("{}", compacted);
257|        final_output.push_str("\n--- Changes ---\n");
258|        final_output.push_str(&compacted);
259|    }
260|
261|    timer.track(
262|        &format!("git diff {}", args.join(" ")),
263|        &format!("rtk git diff {}", args.join(" ")),
264|        &format!("{}\n{}", result.stdout, diff_result.stdout),
265|        &final_output,
266|    );
267|
268|    Ok(0)
269|}
270|
271|fn run_show(
272|    args: &[String],
273|    max_lines: Option<usize>,
274|    verbose: u8,
275|    global_args: &[String],
276|) -> Result<i32> {
277|    let timer = tracking::TimedExecution::start();
278|
279|    // If user wants --stat or --format only, pass through
280|    let wants_stat_only = args
281|        .iter()
282|        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");
283|
284|    let wants_format = args
285|        .iter()
286|        .any(|arg| arg.starts_with("--pretty") || arg.starts_with("--format"));
287|
288|    // `git show rev:path` prints a blob, not a commit diff. In this mode we should
289|    // pass through directly to avoid duplicated output from compact-show steps.
290|    let wants_blob_show = args.iter().any(|arg| is_blob_show_arg(arg));
291|
292|    if wants_stat_only || wants_format || wants_blob_show {
293|        let mut cmd = git_cmd(global_args);
294|        cmd.arg("show");
295|        for arg in args {
296|            cmd.arg(arg);
297|        }
298|        let result = exec_capture(&mut cmd).context("Failed to run git show")?;
299|        if !result.success() {
300|            eprintln!("{}", result.stderr);
301|            return Ok(result.exit_code);
302|        }
303|        if wants_blob_show {
304|            print!("{}", result.stdout);
305|        } else {
306|            println!("{}", result.stdout.trim());
307|        }
308|
309|        timer.track(
310|            &format!("git show {}", args.join(" ")),
311|            &format!("rtk git show {} (passthrough)", args.join(" ")),
312|            &result.stdout,
313|            &result.stdout,
314|        );
315|
316|        return Ok(0);
317|    }
318|
319|    // Get raw output for tracking
320|    let mut raw_cmd = git_cmd(global_args);
321|    raw_cmd.arg("show");
322|    for arg in args {
323|        raw_cmd.arg(arg);
324|    }
325|    let raw_output = exec_capture(&mut raw_cmd)
326|        .map(|r| r.stdout)
327|        .unwrap_or_default();
328|
329|    // Step 1: one-line commit summary
330|    let mut summary_cmd = git_cmd(global_args);
331|    summary_cmd.args(["show", "--no-patch", "--pretty=format:%h %s (%ar) <%an>"]);
332|    for arg in args {
333|        summary_cmd.arg(arg);
334|    }
335|    let summary_result = exec_capture(&mut summary_cmd).context("Failed to run git show")?;
336|    if !summary_result.success() {
337|        eprintln!("{}", summary_result.stderr);
338|        return Ok(summary_result.exit_code);
339|    }
340|    println!("{}", summary_result.stdout.trim());
341|
342|    // Step 2: --stat summary
343|    let mut stat_cmd = git_cmd(global_args);
344|    stat_cmd.args(["show", "--stat", "--pretty=format:"]);
345|    for arg in args {
346|        stat_cmd.arg(arg);
347|    }
348|    let stat_result = exec_capture(&mut stat_cmd).context("Failed to run git show --stat")?;
349|    let stat_text = stat_result.stdout.trim();
350|    if !stat_text.is_empty() {
351|        println!("{}", stat_text);
352|    }
353|
354|    // Step 3: compacted diff
355|    let mut diff_cmd = git_cmd(global_args);
356|    diff_cmd.args(["show", "--pretty=format:"]);
357|    for arg in args {
358|        diff_cmd.arg(arg);
359|    }
360|    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git show (diff)")?;
361|    let diff_text = diff_result.stdout.trim();
362|
363|    let mut final_output = summary_result.stdout.clone();
364|    if !diff_text.is_empty() {
365|        if verbose > 0 {
366|            println!("\n--- Changes ---");
367|        }
368|        let compacted = compact_diff(diff_text, max_lines.unwrap_or(500));
369|        println!("{}", compacted);
370|        final_output.push_str(&format!("\n{}", compacted));
371|    }
372|
373|    timer.track(
374|        &format!("git show {}", args.join(" ")),
375|        &format!("rtk git show {}", args.join(" ")),
376|        &raw_output,
377|        &final_output,
378|    );
379|
380|    Ok(0)
381|}
382|
383|fn is_blob_show_arg(arg: &str) -> bool {
384|    // Detect `rev:path` style arguments while ignoring flags like `--pretty=format:...`.
385|    !arg.starts_with('-') && arg.contains(':')
386|}
387|
388|pub(crate) fn compact_diff(diff: &str, max_lines: usize) -> String {
389|    let mut result = Vec::new();
390|    let mut current_file = String::new();
391|    let mut added = 0;
392|    let mut removed = 0;
393|    let mut in_hunk = false;
394|    let mut hunk_shown = 0;
395|    let mut hunk_skipped = 0usize;
396|    let max_hunk_lines = 100;
397|    let mut was_truncated = false;
398|
399|    for line in diff.lines() {
400|        if line.starts_with("diff --git") {
401|            // Flush hunk truncation before starting a new file
402|            if hunk_skipped > 0 {
403|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
404|                was_truncated = true;
405|                hunk_skipped = 0;
406|            }
407|            if !current_file.is_empty() && (added > 0 || removed > 0) {
408|                result.push(format!("  +{} -{}", added, removed));
409|            }
410|            current_file = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
411|            result.push(format!("\n{}", current_file));
412|            added = 0;
413|            removed = 0;
414|            in_hunk = false;
415|            hunk_shown = 0;
416|        } else if line.starts_with("@@") {
417|            // Flush hunk truncation before starting a new hunk
418|            if hunk_skipped > 0 {
419|                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
420|                was_truncated = true;
421|                hunk_skipped = 0;
422|            }
423|            in_hunk = true;
424|            hunk_shown = 0;
425|            // Preserve the full unified diff hunk header, including trailing
426|            // function / symbol context after the second @@ marker.
427|            result.push(format!("  {}", line));
428|        } else if in_hunk {
429|            if line.starts_with('+') && !line.starts_with("+++") {
430|                added += 1;
431|                if hunk_shown < max_hunk_lines {
432|                    result.push(format!("  {}", line));
433|                    hunk_shown += 1;
434|                } else {
435|                    hunk_skipped += 1;
436|                }
437|            } else if line.starts_with('-') && !line.starts_with("---") {
438|                removed += 1;
439|                if hunk_shown < max_hunk_lines {
440|                    result.push(format!("  {}", line));
441|                    hunk_shown += 1;
442|                } else {
443|                    hunk_skipped += 1;
444|                }
445|            } else if hunk_shown < max_hunk_lines && !line.starts_with("\\") {
446|                // Context line
447|                if hunk_shown > 0 {
448|                    result.push(format!("  {}", line));
449|                    hunk_shown += 1;
450|                }
451|            }
452|        }
453|
454|        if result.len() >= max_lines {
455|            result.push("\n... (more changes truncated)".to_string());
456|            was_truncated = true;
457|            break;
458|        }
459|    }
460|
461|    // Flush last hunk
462|    if hunk_skipped > 0 {
463|        result.push(format!("  ... ({} lines truncated)", hunk_skipped));
464|        was_truncated = true;
465|    }
466|
467|    if !current_file.is_empty() && (added > 0 || removed > 0) {
468|        result.push(format!("  +{} -{}", added, removed));
469|    }
470|
471|    if was_truncated {
472|        result.push("[full diff: rtk git diff --no-compact]".to_string());
473|    }
474|
475|    result.join("\n")
476|}
477|
478|fn run_log(
479|    args: &[String],
480|    _max_lines: Option<usize>,
481|    verbose: u8,
482|    global_args: &[String],
483|) -> Result<i32> {
484|    let timer = tracking::TimedExecution::start();
485|
486|    let mut cmd = git_cmd(global_args);
487|    cmd.arg("log");
488|
489|    // Check if user provided format flags
490|    let has_format_flag = args.iter().any(|arg| {
491|        arg.starts_with("--oneline") || arg.starts_with("--pretty") || arg.starts_with("--format")
492|    });
493|
494|    // Check if user provided limit flag (-N, -n N, --max-count=N, --max-count N)
495|    let has_limit_flag = args.iter().any(|arg| {
496|        (arg.starts_with('-') && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
497|            || arg == "-n"
498|            || arg.starts_with("--max-count")
499|    });
500|
501|    // Apply RTK defaults only if user didn't specify them
502|    // Use %b (body) to preserve first line of commit body for agent context
503|    // (BREAKING CHANGE, Closes #xxx, design notes)
504|    if !has_format_flag {
505|        cmd.args(["--pretty=format:%h %s (%ar) <%an>%n%b%n---END---"]);
506|    }
507|
508|    // Determine limit: respect user's explicit -N flag, use sensible defaults otherwise
509|    let (limit, user_set_limit) = if has_limit_flag {
510|        // User explicitly passed -N / -n N / --max-count=N → respect their choice
511|        let n = parse_user_limit(args).unwrap_or(10);
512|        (n, true)
513|    } else if has_format_flag {
514|        // --oneline / --pretty without -N: user wants compact output, allow more
515|        cmd.arg("-50");
516|        (50, false)
517|    } else {
518|        // No flags at all: default to 10
519|        cmd.arg("-10");
520|        (10, false)
521|    };
522|
523|    // Only add --no-merges if user didn't explicitly request merge commits
524|    let wants_merges = args
525|        .iter()
526|        .any(|arg| arg == "--merges" || arg == "--min-parents=2");
527|    if !wants_merges {
528|        cmd.arg("--no-merges");
529|    }
530|
531|    // Pass all user arguments
532|    for arg in args {
533|        cmd.arg(arg);
534|    }
535|
536|    let result = exec_capture(&mut cmd).context("Failed to run git log")?;
537|
538|    if !result.success() {
539|        eprintln!("{}", result.stderr);
540|        return Ok(result.exit_code);
541|    }
542|
543|    if verbose > 0 {
544|        eprintln!("Git log output:");
545|    }
546|
547|    // Post-process: truncate long messages, cap lines only if RTK set the default
548|    let filtered = filter_log_output(&result.stdout, limit, user_set_limit, has_format_flag);
549|    println!("{}", filtered);
550|
551|    timer.track(
552|        &format!("git log {}", args.join(" ")),
553|        &format!("rtk git log {}", args.join(" ")),
554|        &result.stdout,
555|        &filtered,
556|    );
557|
558|    Ok(0)
559|}
560|
561|/// Filter git log output: truncate long messages, cap lines
562|/// Parse the user-specified limit from git log args.
563|/// Handles: -20, -n 20, --max-count=20, --max-count 20
564|fn parse_user_limit(args: &[String]) -> Option<usize> {
565|    let mut iter = args.iter();
566|    while let Some(arg) = iter.next() {
567|        // -20 (combined digit form)
568|        if arg.starts_with('-')
569|            && arg.len() > 1
570|            && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
571|        {
572|            if let Ok(n) = arg[1..].parse::<usize>() {
573|                return Some(n);
574|            }
575|        }
576|        // -n 20 (two-token form)
577|        if arg == "-n" {
578|            if let Some(next) = iter.next() {
579|                if let Ok(n) = next.parse::<usize>() {
580|                    return Some(n);
581|                }
582|            }
583|        }
584|        // --max-count=20
585|        if let Some(rest) = arg.strip_prefix("--max-count=") {
586|            if let Ok(n) = rest.parse::<usize>() {
587|                return Some(n);
588|            }
589|        }
590|        // --max-count 20 (two-token form)
591|        if arg == "--max-count" {
592|            if let Some(next) = iter.next() {
593|                if let Ok(n) = next.parse::<usize>() {
594|                    return Some(n);
595|                }
596|            }
597|        }
598|    }
599|    None
600|}
601|
602|/// When `user_set_limit` is true, the user explicitly passed `-N` to git log,
603|/// so we skip line capping (git already returns exactly N commits) and use a
604|/// wider truncation threshold (120 chars) to preserve commit context that LLMs
605|/// need for rebase/squash operations.
606|pub(crate) fn filter_log_output(
607|    output: &str,
608|    limit: usize,
609|    user_set_limit: bool,
610|    user_format: bool,
611|) -> String {
612|    let truncate_width = if user_set_limit { 120 } else { 80 };
613|
614|    // When user specified their own format (--oneline, --pretty, --format),
615|    // RTK did not inject ---END--- markers. Use simple line-based truncation.
616|    if user_format {
617|        let lines: Vec<&str> = output.lines().collect();
618|        let max_lines = if user_set_limit { lines.len() } else { limit };
619|        return lines
620|            .iter()
621|            .take(max_lines)
622|            .map(|l| truncate_line(l, truncate_width))
623|            .collect::<Vec<_>>()
624|            .join("\n");
625|    }
626|
627|    // RTK injected format: split output into commit blocks separated by ---END---
628|    let commits: Vec<&str> = output.split("---END---").collect();
629|    let max_commits = if user_set_limit { commits.len() } else { limit };
630|
631|    let mut result = Vec::new();
632|    for block in commits.iter().take(max_commits) {
633|        let block = block.trim();
634|        if block.is_empty() {
635|            continue;
636|        }
637|        let mut lines = block.lines();
638|        // First line is the header: hash subject (date) <author>
639|        let header = match lines.next() {
640|            Some(h) => truncate_line(h.trim(), truncate_width),
641|            None => continue,
642|        };
643|        // Remaining lines are the body — keep up to 3 non-empty, non-trailer lines
644|        let all_body_lines: Vec<&str> = lines
645|            .map(|l| l.trim())
646|            .filter(|l| {
647|                !l.is_empty()
648|                    && !l.starts_with("Signed-off-by:")
649|                    && !l.starts_with("Co-authored-by:")
650|            })
651|            .collect();
652|        let body_omitted = all_body_lines.len().saturating_sub(3);
653|        let body_lines = &all_body_lines[..all_body_lines.len().min(3)];
654|
655|        if body_lines.is_empty() {
656|            result.push(header);
657|        } else {
658|            let mut entry = header;
659|            for body in body_lines {
660|                entry.push_str(&format!("\n  {}", truncate_line(body, truncate_width)));
661|            }
662|            if body_omitted > 0 {
663|                entry.push_str(&format!("\n  [+{} lines omitted]", body_omitted));
664|            }
665|            result.push(entry);
666|        }
667|    }
668|
669|    result.join("\n").trim().to_string()
670|}
671|
672|/// Truncate a single line to `width` characters, appending "..." if needed
673|fn truncate_line(line: &str, width: usize) -> String {
674|    if line.chars().count() > width {
675|        let truncated: String = line.chars().take(width - 3).collect();
676|        format!("{}...", truncated)
677|    } else {
678|        line.to_string()
679|    }
680|}
681|
682|pub(crate) fn format_status_output(porcelain: &str) -> String {
683|    format_status_inner(porcelain, None)
684|}
685|
686|pub(crate) fn format_status_output_detached(porcelain: &str, detached_ref: &str) -> String {
687|    format_status_inner(porcelain, Some(detached_ref))
688|}
689|
690|fn format_status_inner(porcelain: &str, detached: Option<&str>) -> String {
691|    let lines: Vec<&str> = porcelain
692|        .lines()
693|        .filter(|line| !line.trim().is_empty())
694|        .collect();
695|
696|    if lines.is_empty() {
697|        return "Clean working tree".to_string();
698|    }
699|
700|    let mut output = Vec::new();
701|
702|    if let Some(branch_line) = lines.first() {
703|        if branch_line.starts_with("##") {
704|            let branch = branch_line.trim_start_matches("## ");
705|            let display = detached.unwrap_or(branch);
706|            output.push(format!("* {}", display));
707|        } else {
708|            output.push((*branch_line).to_string());
709|        }
710|    }
711|
712|    for line in lines.iter().skip(1) {
713|        output.push((*line).to_string());
714|    }
715|
716|    if lines.len() == 1 && lines[0].starts_with("##") {
717|        output.push("clean — nothing to commit".to_string());
718|    }
719|
720|    output.join("\n")
721|}
722|
723|#[derive(Debug, Clone, Copy, PartialEq, Eq)]
724|enum GitStatusState {
725|    Rebase,
726|    MergeConflicts,
727|    MergeReadyToCommit,
728|    CherryPick,
729|    Revert,
730|    Bisect,
731|    Am,
732|    SparseCheckout,
733|}
734|
735|impl GitStatusState {
736|    fn summary(self) -> &'static str {
737|        match self {
738|            Self::Rebase => "rebase in progress",
739|            Self::MergeConflicts => "merge in progress. unresolved conflicts",
740|            Self::MergeReadyToCommit => "merge in progress. no conflicts",
741|            Self::CherryPick => "cherry-pick in progress",
742|            Self::Revert => "revert in progress",
743|            Self::Bisect => "bisect in progress",
744|            Self::Am => "am session in progress",
745|            Self::SparseCheckout => "sparse checkout enabled",
746|        }
747|    }
748|}
749|
750|const REBASE_INDICATORS: &[&str] = &[
751|    "rebase in progress",
752|    "You are currently rebasing",
753|    "You are currently editing",
754|    "You are currently splitting",
755|    "Last command done",
756|    "Next command to do",
757|    "No commands remaining",
758|];
759|
760|fn detect_status_state(line: &str) -> Option<GitStatusState> {
761|    if line.contains("All conflicts fixed but you are still merging") {
762|        Some(GitStatusState::MergeReadyToCommit)
763|    } else if line.contains("You have unmerged paths") {
764|        Some(GitStatusState::MergeConflicts)
765|    } else if line.contains("You are currently cherry-picking") {
766|        Some(GitStatusState::CherryPick)
767|    } else if line.contains("You are currently reverting") {
768|        Some(GitStatusState::Revert)
769|    } else if line.contains("You are currently bisecting") {
770|        Some(GitStatusState::Bisect)
771|    } else if line.contains("You are in the middle of an am session") {
772|        Some(GitStatusState::Am)
773|    } else if line.contains("You are in a sparse checkout") {
774|        Some(GitStatusState::SparseCheckout)
775|    } else if REBASE_INDICATORS.iter().any(|i| line.contains(i)) {
776|        Some(GitStatusState::Rebase)
777|    } else {
778|        None
779|    }
780|}
781|
782|/// Extract a compact in-progress state summary from plain `git status` output.
783|///
784|/// Compact mode runs `git status --porcelain -b`, which omits the state header
785|/// git prints for rebase / merge / cherry-pick / revert / bisect / am / sparse
786|/// checkout. Hiding that block is a correctness bug — e.g. during an interactive
787|/// rebase edit, the user sees a "clean" status and misses "You are currently
788|/// editing a commit while rebasing ...".
789|///
790|/// This helper walks the plain-status output we already capture for tracking
791|/// and emits a compact, RTK-style summary rather than dumping git's full prose.
792|/// Returns `None` when no state is in progress.
793|fn extract_state_header(raw: &str) -> Option<String> {
794|    // Headers of the file-change blocks — everything relevant to state appears
795|    // above these in git's output, so they double as a terminator.
796|    const STOPPERS: &[&str] = &[
797|        "Changes to be committed:",
798|        "Changes not staged for commit:",
799|        "Untracked files:",
800|        "Unmerged paths:",
801|        "no changes added to commit",
802|        "nothing to commit",
803|        "nothing added to commit",
804|    ];
805|
806|    for line in raw.lines() {
807|        let stripped = line.trim();
808|
809|        if STOPPERS.iter().any(|s| stripped.starts_with(s)) {
810|            break;
811|        }
812|
813|        if let Some(state) = detect_status_state(stripped) {
814|            return Some(state.summary().to_string());
815|        }
816|    }
817|
818|    None
819|}
820|
821|/// Extract the explicit "HEAD detached at/from <ref>" line from plain
822|/// `git status` output.
823|///
824|/// Porcelain `-b` collapses a detached HEAD to the opaque `## HEAD (no branch)`,
825|/// which an agent (or a distracted human) can misread as a branch literally
826|/// named `HEAD`. The plain-status output keeps the explicit SHA/ref, so we
827|/// surface that instead. Returns `None` when HEAD is on a branch.
828|fn extract_detached_head(raw: &str) -> Option<String> {
829|    raw.lines()
830|        .map(str::trim)
831|        .find(|l| l.starts_with("HEAD detached "))
832|        .map(str::to_string)
833|}
834|
835|/// Minimal filtering for git status with user-provided args
836|fn filter_status_with_args(output: &str) -> String {
837|    let mut result = Vec::new();
838|
839|    for line in output.lines() {
840|        let trimmed = line.trim();
841|
842|        // Skip empty lines
843|        if trimmed.is_empty() {
844|            continue;
845|        }
846|
847|        // Skip git hints - can appear at start or within line
848|        if trimmed.starts_with("(use \"git")
849|            || trimmed.starts_with("(create/copy files")
850|            || trimmed.contains("(use \"git add")
851|            || trimmed.contains("(use \"git restore")
852|        {
853|            continue;
854|        }
855|
856|        // Special case: clean working tree
857|        if trimmed.contains("nothing to commit") && trimmed.contains("working tree clean") {
858|            result.push(trimmed.to_string());
859|            break;
860|        }
861|
862|        result.push(line.to_string());
863|    }
864|
865|    if result.is_empty() {
866|        "ok".to_string()
867|    } else {
868|        result.join("\n")
869|    }
870|}
871|
872|fn run_status(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
873|    let timer = tracking::TimedExecution::start();
874|
875|    // Keep a narrow compact path for no-arg status and branch/short-only flags.
876|    // More complex explicit args still use the existing minimal-filter path.
877|    if !uses_compact_status_path(args) {
878|        let mut cmd = build_status_command(args, global_args);
879|        let result = exec_capture(&mut cmd).context("Failed to run git status")?;
880|
881|        if !result.success() {
882|            if !result.stderr.trim().is_empty() {
883|                eprint!("{}", result.stderr);
884|            }
885|            timer.track(
886|                &format!("git status {}", args.join(" ")),
887|                &format!("rtk git status {}", args.join(" ")),
888|                &result.stdout,
889|                &result.stdout,
890|            );
891|            return Ok(result.exit_code);
892|        }
893|
894|        if verbose > 0 || !result.stderr.is_empty() {
895|            eprint!("{}", result.stderr);
896|        }
897|
898|        // Apply minimal filtering: strip ANSI, remove hints, empty lines
899|        let filtered = filter_status_with_args(&result.stdout);
900|        print!("{}", filtered);
901|
902|        timer.track(
903|            &format!("git status {}", args.join(" ")),
904|            &format!("rtk git status {}", args.join(" ")),
905|            &result.stdout,
906|            &filtered,
907|        );
908|
909|        return Ok(0);
910|    }
911|
912|    let mut raw_cmd = git_cmd_c_locale(global_args);
913|    raw_cmd.arg("status");
914|    raw_cmd.args(args);
915|    let raw_output = exec_capture(&mut raw_cmd)
916|        .map(|r| r.stdout)
917|        .unwrap_or_default();
918|
919|    let mut cmd = build_status_command(args, global_args);
920|    let result = exec_capture(&mut cmd).context("Failed to run git status")?;
921|
922|    if !result.stderr.is_empty() && result.stderr.contains("not a git repository") {
923|        let message = "Not a git repository".to_string();
924|        eprintln!("{}", message);
925|        let original_cmd = if args.is_empty() {
926|            "git status".to_string()
927|        } else {
928|            format!("git status {}", args.join(" "))
929|        };
930|        let rtk_cmd = if args.is_empty() {
931|            "rtk git status".to_string()
932|        } else {
933|            format!("rtk git status {}", args.join(" "))
934|        };
935|        timer.track(&original_cmd, &rtk_cmd, &raw_output, &message);
936|        return Ok(result.exit_code);
937|    }
938|
939|940|    let formatted = match extract_detached_head(&raw_output) {
941|        Some(detached_ref) => format_status_output_detached(&result.stdout, &detached_ref),
942|        None => format_status_output(&result.stdout),
943|    };
944|953|
954|    // Surface in-progress state (rebase/merge/cherry-pick/bisect/am) from the
955|    // plain-status output we already captured for tracking. Porcelain omits it
956|    // and hiding it misleads the user about the true repo state.
957|    let final_output = match extract_state_header(&raw_output) {
958|        Some(state) => format!("{}\n{}", state, formatted),
959|        None => formatted,
960|    };
961|
962|    println!("{}", final_output);
963|
964|    let original_cmd = if args.is_empty() {
965|        "git status".to_string()
966|    } else {
967|        format!("git status {}", args.join(" "))
968|    };
969|    let rtk_cmd = if args.is_empty() {
970|        "rtk git status".to_string()
971|    } else {
972|        format!("rtk git status {}", args.join(" "))
973|    };
974|
975|    timer.track(&original_cmd, &rtk_cmd, &raw_output, &final_output);
976|
977|    Ok(0)
978|}
979|
980|fn run_add(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
981|    let timer = tracking::TimedExecution::start();
982|
983|    let mut cmd = git_cmd(global_args);
984|    cmd.arg("add");
985|
986|    // Pass all arguments directly to git (flags like -A, -p, --all, etc.)
987|    if args.is_empty() {
988|        cmd.arg(".");
989|    } else {
990|        for arg in args {
991|            cmd.arg(arg);
992|        }
993|    }
994|
995|    let result = exec_capture(&mut cmd).context("Failed to run git add")?;
996|
997|    if verbose > 0 {
998|        eprintln!("git add executed");
999|    }
1000|
1001|    let raw_output = format!("{}\n{}", result.stdout, result.stderr);
1002|
1003|    if result.success() {
1004|        // Count what was added
1005|        let mut stat_cmd = git_cmd(global_args);
1006|        stat_cmd.args(["diff", "--cached", "--stat", "--shortstat"]);
1007|        let stat_result = exec_capture(&mut stat_cmd).context("Failed to check staged files")?;
1008|
1009|        // Mirror git's own behaviour: a no-op `git add` is silent. Emitting a
1010|        // generic "ok" here is misleading — an agent can't tell "staged N files"
1011|        // from "staged nothing" when both print "ok".
1012|        let compact = if stat_result.stdout.trim().is_empty() {
1013|            String::new()
1014|        } else {
1015|            // Parse "1 file changed, 5 insertions(+)" format
1016|            let short = stat_result.stdout.lines().last().unwrap_or("").trim();
1017|            if short.is_empty() {
1018|                "ok".to_string()
1019|            } else {
1020|                format!("ok {}", short)
1021|            }
1022|        };
1023|
1024|        if !compact.is_empty() {
1025|            println!("{}", compact);
1026|        }
1027|
1028|        timer.track(
1029|            &format!("git add {}", args.join(" ")),
1030|            &format!("rtk git add {}", args.join(" ")),
1031|            &raw_output,
1032|            &compact,
1033|        );
1034|    } else {
1035|        eprintln!("FAILED: git add");
1036|        if !result.stderr.trim().is_empty() {
1037|            eprintln!("{}", result.stderr);
1038|        }
1039|        if !result.stdout.trim().is_empty() {
1040|            eprintln!("{}", result.stdout);
1041|        }
1042|        return Ok(result.exit_code);
1043|    }
1044|
1045|    Ok(0)
1046|}
1047|
1048|fn build_commit_command(args: &[String], global_args: &[String]) -> Command {
1049|    let mut cmd = git_cmd(global_args);
1050|    cmd.arg("commit");
1051|    for arg in args {
1052|        cmd.arg(arg);
1053|    }
1054|    cmd
1055|}
1056|
1057|fn run_commit(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1058|    let timer = tracking::TimedExecution::start();
1059|
1060|    let original_cmd = format!("git commit {}", args.join(" "));
1061|
1062|    if verbose > 0 {
1063|        eprintln!("{}", original_cmd);
1064|    }
1065|
1066|    let output = build_commit_command(args, global_args)
1067|        .stdin(Stdio::inherit())
1068|        .output()
1069|        .context("Failed to run git commit")?;
1070|
1071|    let stdout = String::from_utf8_lossy(&output.stdout);
1072|    let stderr = String::from_utf8_lossy(&output.stderr);
1073|    let exit_code = exit_code_from_output(&output, "git commit");
1074|    let raw_output = format!("{}\n{}", stdout, stderr);
1075|
1076|    if output.status.success() {
1077|        // Extract commit hash from output like "[main abc1234] message"
1078|        let compact = if let Some(line) = stdout.lines().next() {
1079|            if let Some(hash_start) = line.find(' ') {
1080|                let hash = line[1..hash_start].split(' ').next_back().unwrap_or("");
1081|                if !hash.is_empty() && hash.len() >= 7 {
1082|                    format!("ok {}", &hash[..7.min(hash.len())])
1083|                } else {
1084|                    "ok".to_string()
1085|                }
1086|            } else {
1087|                "ok".to_string()
1088|            }
1089|        } else {
1090|            "ok".to_string()
1091|        };
1092|
1093|        println!("{}", compact);
1094|
1095|        timer.track(&original_cmd, "rtk git commit", &raw_output, &compact);
1096|    } else if stderr.contains("nothing to commit") || stdout.contains("nothing to commit") {
1097|        println!("ok (nothing to commit)");
1098|        timer.track(
1099|            &original_cmd,
1100|            "rtk git commit",
1101|            &raw_output,
1102|            "ok (nothing to commit)",
1103|        );
1104|    } else {
1105|        if !stderr.trim().is_empty() {
1106|            eprint!("{}", stderr);
1107|        }
1108|        if !stdout.trim().is_empty() {
1109|            eprint!("{}", stdout);
1110|        }
1111|        timer.track(&original_cmd, "rtk git commit", &raw_output, &raw_output);
1112|        return Ok(exit_code);
1113|    }
1114|
1115|    Ok(0)
1116|}
1117|
1118|// Git push progress prefixes (stderr) — dropped from the stream.
1119|const GIT_PUSH_NOISE_PREFIXES: &[&str] = &[
1120|    "Enumerating objects:",
1121|    "Counting objects:",
1122|    "Compressing objects:",
1123|    "Writing objects:",
1124|    "Delta compression using",
1125|    "Total ",
1126|];
1127|
1128|#[derive(Default)]
1129|struct GitPushLineHandler {
1130|    up_to_date: bool,
1131|    pushed_ref: Option<String>,
1132|}
1133|
1134|impl LineHandler for GitPushLineHandler {
1135|    fn should_skip(&mut self, line: &str) -> bool {
1136|        if line.is_empty() {
1137|            return true;
1138|        }
1139|        let trimmed = line.trim_start();
1140|        GIT_PUSH_NOISE_PREFIXES
1141|            .iter()
1142|            .any(|p| trimmed.starts_with(p))
1143|    }
1144|
1145|    fn observe_line(&mut self, line: &str) {
1146|        if line.contains("Everything up-to-date") {
1147|            self.up_to_date = true;
1148|        }
1149|        if self.pushed_ref.is_none() {
1150|            if let Some(idx) = line.find(" -> ") {
1151|                let after = &line[idx + 4..];
1152|                if let Some(dest) = after.split_whitespace().next() {
1153|                    self.pushed_ref = Some(dest.to_string());
1154|                }
1155|            }
1156|        }
1157|    }
1158|
1159|    fn format_summary(&self, exit_code: i32, _raw: &str) -> Option<String> {
1160|        if exit_code != 0 {
1161|            return None;
1162|        }
1163|        let summary = if self.up_to_date {
1164|            "ok (up-to-date)".to_string()
1165|        } else if let Some(dest) = &self.pushed_ref {
1166|            format!("ok {}", dest)
1167|        } else {
1168|            "ok".to_string()
1169|        };
1170|        Some(format!("{}\n", summary))
1171|    }
1172|}
1173|
1174|fn run_push(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1175|    let timer = tracking::TimedExecution::start();
1176|
1177|    if verbose > 0 {
1178|        eprintln!("git push");
1179|    }
1180|
1181|    let mut cmd = git_cmd(global_args);
1182|    cmd.arg("push");
1183|    for arg in args {
1184|        cmd.arg(arg);
1185|    }
1186|
1187|    let cmd_label = format!("git push {}", args.join(" "));
1188|    let filter = LineStreamFilter::new(GitPushLineHandler::default());
1189|    let result = stream::run_streaming(
1190|        &mut cmd,
1191|        StdinMode::Inherit,
1192|        FilterMode::Streaming(Box::new(filter)),
1193|    )
1194|    .context("Failed to run git push")?;
1195|
1196|    timer.track(
1197|        &cmd_label,
1198|        &format!("rtk {}", cmd_label),
1199|        &result.raw,
1200|        &result.filtered,
1201|    );
1202|
1203|    Ok(result.exit_code)
1204|}
1205|
1206|fn run_pull(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1207|    let timer = tracking::TimedExecution::start();
1208|
1209|    if verbose > 0 {
1210|        eprintln!("git pull");
1211|    }
1212|
1213|    let mut cmd = git_cmd(global_args);
1214|    cmd.arg("pull");
1215|    for arg in args {
1216|        cmd.arg(arg);
1217|    }
1218|
1219|    let result = exec_capture(&mut cmd).context("Failed to run git pull")?;
1220|
1221|    let raw_output = format!("{}\n{}", result.stdout, result.stderr);
1222|
1223|    if result.success() {
1224|        let compact = if result.stdout.contains("Already up to date")
1225|            || result.stdout.contains("Already up-to-date")
1226|        {
1227|            "ok (up-to-date)".to_string()
1228|        } else {
1229|            // Count files changed
1230|            let mut files = 0;
1231|            let mut insertions = 0;
1232|            let mut deletions = 0;
1233|
1234|            for line in result.stdout.lines() {
1235|                if line.contains("file") && line.contains("changed") {
1236|                    // Parse "3 files changed, 10 insertions(+), 2 deletions(-)"
1237|                    for part in line.split(',') {
1238|                        let part = part.trim();
1239|                        if part.contains("file") {
1240|                            files = part
1241|                                .split_whitespace()
1242|                                .next()
1243|                                .and_then(|n| n.parse().ok())
1244|                                .unwrap_or(0);
1245|                        } else if part.contains("insertion") {
1246|                            insertions = part
1247|                                .split_whitespace()
1248|                                .next()
1249|                                .and_then(|n| n.parse().ok())
1250|                                .unwrap_or(0);
1251|                        } else if part.contains("deletion") {
1252|                            deletions = part
1253|                                .split_whitespace()
1254|                                .next()
1255|                                .and_then(|n| n.parse().ok())
1256|                                .unwrap_or(0);
1257|                        }
1258|                    }
1259|                }
1260|            }
1261|
1262|            if files > 0 {
1263|                format!("ok {} files +{} -{}", files, insertions, deletions)
1264|            } else {
1265|                "ok".to_string()
1266|            }
1267|        };
1268|
1269|        println!("{}", compact);
1270|
1271|        timer.track(
1272|            &format!("git pull {}", args.join(" ")),
1273|            &format!("rtk git pull {}", args.join(" ")),
1274|            &raw_output,
1275|            &compact,
1276|        );
1277|    } else {
1278|        eprintln!("FAILED: git pull");
1279|        if !result.stderr.trim().is_empty() {
1280|            eprintln!("{}", result.stderr);
1281|        }
1282|        if !result.stdout.trim().is_empty() {
1283|            eprintln!("{}", result.stdout);
1284|        }
1285|        return Ok(result.exit_code);
1286|    }
1287|
1288|    Ok(0)
1289|}
1290|
1291|fn run_branch(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1292|    let timer = tracking::TimedExecution::start();
1293|
1294|    if verbose > 0 {
1295|        eprintln!("git branch");
1296|    }
1297|
1298|    // Detect write operations: delete, rename, copy, upstream tracking
1299|    let has_action_flag = args.iter().any(|a| {
1300|        a == "-d"
1301|            || a == "-D"
1302|            || a == "-m"
1303|            || a == "-M"
1304|            || a == "-c"
1305|            || a == "-C"
1306|            || a == "--set-upstream-to"
1307|            || a.starts_with("--set-upstream-to=")
1308|            || a == "-u"
1309|            || a == "--unset-upstream"
1310|            || a == "--edit-description"
1311|    });
1312|
1313|    // Detect flags that produce specific output (not a branch list)
1314|    let has_show_flag = args.iter().any(|a| a == "--show-current");
1315|
1316|    // Detect list-mode flags
1317|    let has_list_flag = args.iter().any(|a| {
1318|        a == "-a"
1319|            || a == "--all"
1320|            || a == "-r"
1321|            || a == "--remotes"
1322|            || a == "--list"
1323|            || a == "--merged"
1324|            || a == "--no-merged"
1325|            || a == "--contains"
1326|            || a == "--no-contains"
1327|            || a == "--format"
1328|            || a.starts_with("--format=")
1329|            || a == "--sort"
1330|            || a.starts_with("--sort=")
1331|            || a == "--points-at"
1332|            || a.starts_with("--points-at=")
1333|    });
1334|
1335|    // Detect positional arguments (not flags) — indicates branch creation
1336|    let has_positional_arg = args.iter().any(|a| !a.starts_with('-'));
1337|
1338|    // --show-current: passthrough with raw stdout (not "ok")
1339|    if has_show_flag {
1340|        let mut cmd = git_cmd(global_args);
1341|        cmd.arg("branch");
1342|        for arg in args {
1343|            cmd.arg(arg);
1344|        }
1345|        let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1346|        let combined = result.combined();
1347|
1348|        let trimmed = result.stdout.trim();
1349|        timer.track(
1350|            &format!("git branch {}", args.join(" ")),
1351|            &format!("rtk git branch {}", args.join(" ")),
1352|            &combined,
1353|            trimmed,
1354|        );
1355|
1356|        if result.success() {
1357|            println!("{}", trimmed);
1358|        } else {
1359|            eprintln!("FAILED: git branch {}", args.join(" "));
1360|            if !result.stderr.trim().is_empty() {
1361|                eprintln!("{}", result.stderr);
1362|            }
1363|            return Ok(result.exit_code);
1364|        }
1365|        return Ok(0);
1366|    }
1367|
1368|    // Write operation: action flags, or positional args without list flags (= branch creation)
1369|    if has_action_flag || (has_positional_arg && !has_list_flag) {
1370|        let mut cmd = git_cmd(global_args);
1371|        cmd.arg("branch");
1372|        for arg in args {
1373|            cmd.arg(arg);
1374|        }
1375|        let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1376|        let combined = result.combined();
1377|
1378|        let msg = if result.success() { "ok" } else { &combined };
1379|
1380|        timer.track(
1381|            &format!("git branch {}", args.join(" ")),
1382|            &format!("rtk git branch {}", args.join(" ")),
1383|            &combined,
1384|            msg,
1385|        );
1386|
1387|        if result.success() {
1388|            println!("ok");
1389|        } else {
1390|            eprintln!("FAILED: git branch {}", args.join(" "));
1391|            if !result.stderr.trim().is_empty() {
1392|                eprintln!("{}", result.stderr);
1393|            }
1394|            if !result.stdout.trim().is_empty() {
1395|                eprintln!("{}", result.stdout);
1396|            }
1397|            return Ok(result.exit_code);
1398|        }
1399|        return Ok(0);
1400|    }
1401|
1402|    // List mode: show compact branch list
1403|    let mut cmd = git_cmd(global_args);
1404|    cmd.arg("branch");
1405|    if !has_list_flag {
1406|        cmd.arg("-a");
1407|    }
1408|    cmd.arg("--no-color");
1409|    for arg in args {
1410|        cmd.arg(arg);
1411|    }
1412|
1413|    let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
1414|
1415|    if !result.success() {
1416|        if !result.stderr.trim().is_empty() {
1417|            eprint!("{}", result.stderr);
1418|        }
1419|        timer.track(
1420|            &format!("git branch {}", args.join(" ")),
1421|            &format!("rtk git branch {}", args.join(" ")),
1422|            &result.stdout,
1423|            &result.stdout,
1424|        );
1425|        return Ok(result.exit_code);
1426|    }
1427|
1428|    let filtered = filter_branch_output(&result.stdout);
1429|    println!("{}", filtered);
1430|
1431|    timer.track(
1432|        &format!("git branch {}", args.join(" ")),
1433|        &format!("rtk git branch {}", args.join(" ")),
1434|        &result.stdout,
1435|        &filtered,
1436|    );
1437|
1438|    Ok(0)
1439|}
1440|
1441|fn filter_branch_output(output: &str) -> String {
1442|    let mut current = String::new();
1443|    let mut local: Vec<String> = Vec::new();
1444|    let mut remote: Vec<String> = Vec::new();
1445|    let mut seen_remote: std::collections::HashSet<String> = std::collections::HashSet::new();
1446|
1447|    for line in output.lines() {
1448|        let line = line.trim();
1449|        if line.is_empty() {
1450|            continue;
1451|        }
1452|
1453|        if let Some(branch) = line.strip_prefix("* ") {
1454|            current = branch.to_string();
1455|        } else if let Some(rest) = line.strip_prefix("remotes/") {
1456|            if let Some(slash_pos) = rest.find('/') {
1457|                let branch = &rest[slash_pos + 1..];
1458|                if branch.starts_with("HEAD ") {
1459|                    continue;
1460|                }
1461|                if seen_remote.insert(branch.to_string()) {
1462|                    remote.push(branch.to_string());
1463|                }
1464|            }
1465|        } else {
1466|            local.push(line.to_string());
1467|        }
1468|    }
1469|
1470|    let mut result = Vec::new();
1471|    result.push(format!("* {}", current));
1472|
1473|    if !local.is_empty() {
1474|        for b in &local {
1475|            result.push(format!("  {}", b));
1476|        }
1477|    }
1478|
1479|    if !remote.is_empty() {
1480|        let remote_only: Vec<&String> = remote
1481|            .iter()
1482|            .filter(|r| *r != &current && !local.contains(r))
1483|            .collect();
1484|        if !remote_only.is_empty() {
1485|            const MAX_REMOTE_BRANCHES: usize = CAP_WARNINGS;
1486|            result.push(format!("  remote-only ({}):", remote_only.len()));
1487|            for b in remote_only.iter().take(MAX_REMOTE_BRANCHES) {
1488|                result.push(format!("    {}", b));
1489|            }
1490|            if remote_only.len() > MAX_REMOTE_BRANCHES {
1491|                result.push(format!(
1492|                    "    ... +{} more",
1493|                    remote_only.len() - MAX_REMOTE_BRANCHES
1494|                ));
1495|            }
1496|        }
1497|    }
1498|
1499|    result.join("\n")
1500|}
1501|
1502|fn run_fetch(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1503|    let timer = tracking::TimedExecution::start();
1504|
1505|    if verbose > 0 {
1506|        eprintln!("git fetch");
1507|    }
1508|
1509|    let mut cmd = git_cmd(global_args);
1510|    cmd.arg("fetch");
1511|    for arg in args {
1512|        cmd.arg(arg);
1513|    }
1514|
1515|    let result = exec_capture(&mut cmd).context("Failed to run git fetch")?;
1516|    let raw = result.combined();
1517|
1518|    if !result.success() {
1519|        eprintln!("FAILED: git fetch");
1520|        if !result.stderr.trim().is_empty() {
1521|            eprintln!("{}", result.stderr);
1522|        }
1523|        return Ok(result.exit_code);
1524|    }
1525|
1526|    // Count new refs from stderr (git fetch outputs to stderr)
1527|    let new_refs: usize = result
1528|        .stderr
1529|        .lines()
1530|        .filter(|l| l.contains("->") || l.contains("[new"))
1531|        .count();
1532|
1533|    let msg = if new_refs > 0 {
1534|        format!("ok fetched ({} new refs)", new_refs)
1535|    } else {
1536|        "ok fetched".to_string()
1537|    };
1538|
1539|    println!("{}", msg);
1540|    timer.track("git fetch", "rtk git fetch", &raw, &msg);
1541|
1542|    Ok(0)
1543|}
1544|
1545|/// Format status message for stash operations.
1546|/// - For create operations (push/save): checks for "No local changes"
1547|/// - For other operations: uses "ok stash <subcommand>" format
1548|fn format_stash_message(subcommand: Option<&str>, result: &CaptureResult) -> String {
1549|    match subcommand {
1550|        None | Some("push") | Some("save") => {
1551|1552|            // A successful stash collapses to "ok stashed" (the WIP ref/sha git
1553|            // prints isn't needed to `git stash pop`). But a no-op must NOT look
1554|            // like success — pass git's "No local changes to save" through so the
1555|            // agent can tell nothing was stashed.
1556|            if result.combined().contains("No local changes") {
1557|                "No local changes to save".to_string()
1558|            } else {
1559|1568|                "ok stashed".to_string()
1569|            } else {
1570|                trimmed.to_string()
1571|            }
1572|        }
1573|        Some(sub) => format!("ok stash {}", sub),
1574|    }
1575|}
1576|
1577|fn run_stash(
1578|    subcommand: Option<&str>,
1579|    args: &[String],
1580|    verbose: u8,
1581|    global_args: &[String],
1582|) -> Result<i32> {
1583|    let timer = tracking::TimedExecution::start();
1584|
1585|    if verbose > 0 {
1586|        eprintln!("git stash {:?}", subcommand);
1587|    }
1588|
1589|    match subcommand {
1590|        Some("list") => {
1591|            let mut cmd = git_cmd(global_args);
1592|            cmd.args(["stash", "list"]);
1593|            let result = exec_capture(&mut cmd).context("Failed to run git stash list")?;
1594|
1595|            if result.stdout.trim().is_empty() {
1596|                let msg = "No stashes";
1597|                println!("{}", msg);
1598|                timer.track("git stash list", "rtk git stash list", &result.stdout, msg);
1599|                return Ok(0);
1600|            }
1601|
1602|            let filtered = filter_stash_list(&result.stdout);
1603|            println!("{}", filtered);
1604|            timer.track(
1605|                "git stash list",
1606|                "rtk git stash list",
1607|                &result.stdout,
1608|                &filtered,
1609|            );
1610|        }
1611|        Some("show") => {
1612|            let mut cmd = git_cmd(global_args);
1613|            cmd.args(["stash", "show", "-p"]);
1614|            for arg in args {
1615|                cmd.arg(arg);
1616|            }
1617|            let result = exec_capture(&mut cmd).context("Failed to run git stash show")?;
1618|
1619|            let filtered = if result.stdout.trim().is_empty() {
1620|                let msg = "Empty stash";
1621|                println!("{}", msg);
1622|                msg.to_string()
1623|            } else {
1624|                let compacted = compact_diff(&result.stdout, 100);
1625|                println!("{}", compacted);
1626|                compacted
1627|            };
1628|
1629|            timer.track(
1630|                "git stash show",
1631|                "rtk git stash show",
1632|                &result.stdout,
1633|                &filtered,
1634|            );
1635|        }
1636|        Some("apply") | Some("branch") | Some("clear") | Some("create") | Some("drop")
1637|        | Some("export") | Some("import") | Some("pop") | Some("store") => {
1638|            let sub = subcommand.unwrap();
1639|            let mut cmd = git_cmd(global_args);
1640|            cmd.args(["stash", sub]);
1641|            for arg in args {
1642|                cmd.arg(arg);
1643|            }
1644|            let result = exec_capture(&mut cmd).context("Failed to run git stash")?;
1645|            let combined = result.combined();
1646|
1647|            let msg = if result.success() {
1648|                let msg = format_stash_message(subcommand, &result);
1649|                println!("{}", msg);
1650|                msg
1651|            } else {
1652|                eprintln!("FAILED: git stash {}", sub);
1653|                if !result.stderr.trim().is_empty() {
1654|                    eprintln!("{}", result.stderr);
1655|                }
1656|                combined.clone()
1657|            };
1658|
1659|            timer.track(
1660|                &format!("git stash {}", sub),
1661|                &format!("rtk git stash {}", sub),
1662|                &combined,
1663|                &msg,
1664|            );
1665|
1666|            if !result.success() {
1667|                return Ok(result.exit_code);
1668|            }
1669|        }
1670|        // Default: "git stash [push] [--] [<pathspec>...]" or "git stash save [<message>]"
1671|        Some(_) | None => {
1672|            let (sub, arg) = match subcommand {
1673|                Some("save") => ("save", None),
1674|                Some("push") => ("push", None),
1675|                Some(s) => ("push", Some(s)),
1676|                None => ("push", None),
1677|            };
1678|            let mut cmd = git_cmd(global_args);
1679|            cmd.args(["stash", sub]);
1680|            if let Some(arg) = arg {
1681|                cmd.arg(arg);
1682|            }
1683|            for arg in args {
1684|                cmd.arg(arg);
1685|            }
1686|            let result = exec_capture(&mut cmd).context("Failed to run git stash")?;
1687|            let combined = result.combined();
1688|
1689|            let msg = if result.success() {
1690|                let msg = format_stash_message(subcommand, &result);
1691|                println!("{}", msg);
1692|                msg
1693|            } else {
1694|                eprintln!("FAILED: git stash {}", sub);
1695|                if !result.stderr.trim().is_empty() {
1696|                    eprintln!("{}", result.stderr);
1697|                }
1698|                combined.clone()
1699|            };
1700|
1701|            timer.track(
1702|                &format!("git stash {}", sub),
1703|                &format!("rtk git stash {}", sub),
1704|                &combined,
1705|                &msg,
1706|            );
1707|
1708|            if !result.success() {
1709|                return Ok(result.exit_code);
1710|            }
1711|        }
1712|    }
1713|
1714|    Ok(0)
1715|}
1716|
1717|fn filter_stash_list(output: &str) -> String {
1718|    // Format: "stash@{0}: WIP on main: abc1234 commit message"
1719|    let mut result = Vec::new();
1720|    for line in output.lines() {
1721|        if let Some(colon_pos) = line.find(": ") {
1722|            let index = &line[..colon_pos];
1723|            let rest = &line[colon_pos + 2..];
1724|            // Compact: strip "WIP on branch:" prefix if present
1725|            let message = if let Some(second_colon) = rest.find(": ") {
1726|                rest[second_colon + 2..].trim()
1727|            } else {
1728|                rest.trim()
1729|            };
1730|            result.push(format!("{}: {}", index, message));
1731|        } else {
1732|            result.push(line.to_string());
1733|        }
1734|    }
1735|    result.join("\n")
1736|}
1737|
1738|fn run_worktree(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
1739|    let timer = tracking::TimedExecution::start();
1740|
1741|    if verbose > 0 {
1742|        eprintln!("git worktree list");
1743|    }
1744|
1745|    // If args contain "add", "remove", "prune" etc., pass through
1746|    let has_action = args.iter().any(|a| {
1747|        a == "add" || a == "remove" || a == "prune" || a == "lock" || a == "unlock" || a == "move"
1748|    });
1749|
1750|    if has_action {
1751|        let mut cmd = git_cmd(global_args);
1752|        cmd.arg("worktree");
1753|        for arg in args {
1754|            cmd.arg(arg);
1755|        }
1756|        let result = exec_capture(&mut cmd).context("Failed to run git worktree")?;
1757|        let combined = result.combined();
1758|
1759|        let msg = if result.success() { "ok" } else { &combined };
1760|
1761|        timer.track(
1762|            &format!("git worktree {}", args.join(" ")),
1763|            &format!("rtk git worktree {}", args.join(" ")),
1764|            &combined,
1765|            msg,
1766|        );
1767|
1768|        if result.success() {
1769|            println!("ok");
1770|        } else {
1771|            eprintln!("FAILED: git worktree {}", args.join(" "));
1772|            if !result.stderr.trim().is_empty() {
1773|                eprintln!("{}", result.stderr);
1774|            }
1775|            return Ok(result.exit_code);
1776|        }
1777|        return Ok(0);
1778|    }
1779|
1780|    // Default: list mode
1781|    let mut cmd = git_cmd(global_args);
1782|    cmd.args(["worktree", "list"]);
1783|    let result = exec_capture(&mut cmd).context("Failed to run git worktree list")?;
1784|
1785|    let filtered = filter_worktree_list(&result.stdout);
1786|    println!("{}", filtered);
1787|    timer.track(
1788|        "git worktree list",
1789|        "rtk git worktree",
1790|        &result.stdout,
1791|        &filtered,
1792|    );
1793|
1794|    Ok(0)
1795|}
1796|
1797|fn filter_worktree_list(output: &str) -> String {
1798|    let home = dirs::home_dir()
1799|        .map(|h| h.to_string_lossy().to_string())
1800|        .unwrap_or_default();
1801|
1802|    let mut result = Vec::new();
1803|    for line in output.lines() {
1804|        if line.trim().is_empty() {
1805|            continue;
1806|        }
1807|        // Format: "/path/to/worktree  abc1234 [branch]"
1808|        let parts: Vec<&str> = line.split_whitespace().collect();
1809|        if parts.len() >= 3 {
1810|            let mut path = parts[0].to_string();
1811|            if !home.is_empty() && path.starts_with(&home) {
1812|                path = format!("~{}", &path[home.len()..]);
1813|            }
1814|            let hash = parts[1];
1815|            let branch = parts[2..].join(" ");
1816|            result.push(format!("{} {} {}", path, hash, branch));
1817|        } else {
1818|            result.push(line.to_string());
1819|        }
1820|    }
1821|    result.join("\n")
1822|}
1823|
1824|/// Runs an unsupported git subcommand by passing it through directly
1825|pub fn run_passthrough(args: &[OsString], global_args: &[String], verbose: u8) -> Result<i32> {
1826|    let timer = tracking::TimedExecution::start();
1827|
1828|    if verbose > 0 {
1829|        eprintln!("git passthrough: {:?}", args);
1830|    }
1831|    let status = git_cmd(global_args)
1832|        .args(args)
1833|        .status()
1834|        .context("Failed to run git")?;
1835|
1836|    let args_str = tracking::args_display(args);
1837|    timer.track_passthrough(
1838|        &format!("git {}", args_str),
1839|        &format!("rtk git {} (passthrough)", args_str),
1840|    );
1841|
1842|    if !status.success() {
1843|        return Ok(exit_code_from_status(&status, "git"));
1844|    }
1845|    Ok(0)
1846|}
1847|
1848|#[cfg(test)]
1849|mod tests {
1850|    use super::*;
1851|
1852|    #[test]
1853|    fn test_git_cmd_no_global_args() {
1854|        let cmd = git_cmd(&[]);
1855|        let program = cmd.get_program().to_string_lossy().to_string();
1856|        // On Windows, resolved_command returns full path (e.g. "C:\Program Files\Git\bin\git.exe")
1857|        let basename = std::path::Path::new(&program)
1858|            .file_stem()
1859|            .unwrap()
1860|            .to_string_lossy()
1861|            .to_string();
1862|        assert_eq!(basename, "git");
1863|        let args: Vec<_> = cmd.get_args().collect();
1864|        assert!(args.is_empty());
1865|    }
1866|
1867|    #[test]
1868|    fn test_git_cmd_with_directory() {
1869|        let global_args = vec!["-C".to_string(), "/tmp".to_string()];
1870|        let cmd = git_cmd(&global_args);
1871|        let args: Vec<_> = cmd.get_args().collect();
1872|        assert_eq!(args, vec!["-C", "/tmp"]);
1873|    }
1874|
1875|    #[test]
1876|    fn test_git_cmd_with_multiple_global_args() {
1877|        let global_args = vec![
1878|            "-C".to_string(),
1879|            "/tmp".to_string(),
1880|            "-c".to_string(),
1881|            "user.name=test".to_string(),
1882|            "--git-dir".to_string(),
1883|            "/foo/.git".to_string(),
1884|        ];
1885|        let cmd = git_cmd(&global_args);
1886|        let args: Vec<_> = cmd.get_args().collect();
1887|        assert_eq!(
1888|            args,
1889|            vec![
1890|                "-C",
1891|                "/tmp",
1892|                "-c",
1893|                "user.name=test",
1894|                "--git-dir",
1895|                "/foo/.git"
1896|            ]
1897|        );
1898|    }
1899|
1900|    #[test]
1901|    fn test_git_cmd_with_boolean_flags() {
1902|        let global_args = vec!["--no-pager".to_string(), "--bare".to_string()];
1903|        let cmd = git_cmd(&global_args);
1904|        let args: Vec<_> = cmd.get_args().collect();
1905|        assert_eq!(args, vec!["--no-pager", "--bare"]);
1906|    }
1907|
1908|    #[test]
1909|    fn test_git_cmd_c_locale_sets_stable_env() {
1910|        let cmd = git_cmd_c_locale(&[]);
1911|        let envs: Vec<_> = cmd
1912|            .get_envs()
1913|            .map(|(key, value)| {
1914|                (
1915|                    key.to_string_lossy().to_string(),
1916|                    value.expect("env value").to_string_lossy().to_string(),
1917|                )
1918|            })
1919|            .collect();
1920|        assert!(envs.contains(&("LC_ALL".to_string(), "C".to_string())));
1921|    }
1922|
1923|    #[test]
1924|    fn test_build_status_command_default_compact() {
1925|        let cmd = build_status_command(&[], &[]);
1926|        let args: Vec<_> = cmd.get_args().collect();
1927|        assert_eq!(args, vec!["status", "--porcelain", "-b"]);
1928|    }
1929|
1930|    #[test]
1931|    fn test_uses_compact_status_path_for_branch_and_short_flags() {
1932|        assert!(uses_compact_status_path(&["-b".to_string()]));
1933|        assert!(uses_compact_status_path(&["--branch".to_string()]));
1934|        assert!(uses_compact_status_path(&["-sb".to_string()]));
1935|        assert!(uses_compact_status_path(&["-s".to_string(), "-b".to_string()]));
1936|        assert!(uses_compact_status_path(&["--short".to_string(), "--branch".to_string()]));
1937|        assert!(!uses_compact_status_path(&["-s".to_string()]));
1938|        assert!(!uses_compact_status_path(&["--short".to_string()]));
1939|        assert!(!uses_compact_status_path(&["--porcelain".to_string()]));
1940|        assert!(!uses_compact_status_path(&["-uno".to_string()]));
1941|    }
1942|
1943|    #[test]
1944|    fn test_build_status_command_with_user_args_passthrough() {
1945|        let args = vec!["--short".to_string(), "--branch".to_string()];
1946|        let cmd = build_status_command(&args, &[]);
1947|        let cmd_args: Vec<_> = cmd.get_args().collect();
1948|        assert_eq!(cmd_args, vec!["status", "--porcelain", "-b"]);
1949|    }
1950|
1951|    #[test]
1952|    fn test_build_status_command_with_incompatible_user_args_passthrough() {
1953|        let args = vec!["--porcelain".to_string(), "-uno".to_string()];
1954|        let cmd = build_status_command(&args, &[]);
1955|        let cmd_args: Vec<_> = cmd.get_args().collect();
1956|        assert_eq!(cmd_args, vec!["status", "--porcelain", "-uno"]);
1957|    }
1958|
1959|    #[test]
1960|    fn test_compact_diff() {
1961|        let diff = r#"diff --git a/foo.rs b/foo.rs
1962|--- a/foo.rs
1963|+++ b/foo.rs
1964|@@ -1,3 +1,4 @@
1965| fn main() {
1966|+    println!("hello");
1967| }
1968|"#;
1969|        let result = compact_diff(diff, 100);
1970|        assert!(result.contains("foo.rs"));
1971|        assert!(result.contains("+"));
1972|    }
1973|
1974|    #[test]
1975|    fn test_compact_diff_preserves_full_hunk_header_context() {
1976|        let diff = r#"diff --git a/foo.rs b/foo.rs
1977|--- a/foo.rs
1978|+++ b/foo.rs
1979|@@ -10,3 +10,4 @@ fn important_context() {
1980| fn main() {
1981|+    println!("hello");
1982| }
1983|"#;
1984|        let result = compact_diff(diff, 100);
1985|        assert!(
1986|            result.contains("@@ -10,3 +10,4 @@ fn important_context() {"),
1987|            "Expected full hunk header with trailing context, got:\n{}",
1988|            result
1989|        );
1990|    }
1991|
1992|    #[test]
1993|    fn test_compact_diff_increased_hunk_limit() {
1994|        // Build a hunk with 25 changed lines — should NOT be truncated with limit 30
1995|        let mut diff =
1996|            "diff --git a/big.rs b/big.rs\n--- a/big.rs\n+++ b/big.rs\n@@ -1,25 +1,25 @@\n"
1997|                .to_string();
1998|        for i in 1..=25 {
1999|            diff.push_str(&format!("+line{}\n", i));
2000|        }
2001|
=======
//! Filters git output — log, status, diff, and more — keeping just the essential info.

use crate::core::stream::{exec_capture, CaptureResult};
use crate::core::tracking;
use crate::core::utils::{exit_code_from_output, exit_code_from_status, resolved_command};
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;
use std::process::Stdio;

#[derive(Debug, Clone)]
pub enum GitCommand {
    Diff,
    Log,
    Status,
    Show,
    Add,
    Commit,
    Push,
    Pull,
    Branch,
    Fetch,
    Stash { subcommand: Option<String> },
    Worktree,
}

/// Create a git Command with global options (e.g. -C, -c, --git-dir, --work-tree)
/// prepended before any subcommand arguments.
fn git_cmd(global_args: &[String]) -> Command {
    let mut cmd = resolved_command("git");
    for arg in global_args {
        cmd.arg(arg);
    }
    cmd
}

/// Create a git Command for internal parsing that must be locale-stable.
///
/// We only use this for non-user-facing parses where RTK depends on git's
/// English status phrases. User-visible passthrough output keeps the user's
/// locale.
fn git_cmd_c_locale(global_args: &[String]) -> Command {
    let mut cmd = git_cmd(global_args);
    cmd.env("LC_ALL", "C");
    cmd
}

fn uses_compact_status_path(args: &[String]) -> bool {
    if args.is_empty() {
        return true;
    }

    let mut saw_branch = false;
    for arg in args {
        match arg.as_str() {
            "-b" | "--branch" => saw_branch = true,
            "-sb" | "-bs" => return true,
            "-s" | "--short" => {}
            _ => return false,
        }
    }

    saw_branch
}

fn build_status_command(args: &[String], global_args: &[String]) -> Command {
    let mut cmd = git_cmd(global_args);
    cmd.arg("status");
    if uses_compact_status_path(args) {
        cmd.args(["--porcelain", "-b", "-uall"]);
    } else {
        cmd.args(args);
    }
    cmd
}

pub fn run(
    cmd: GitCommand,
    args: &[String],
    max_lines: Option<usize>,
    verbose: u8,
    global_args: &[String],
) -> Result<i32> {
    match cmd {
        GitCommand::Diff => run_diff(args, max_lines, verbose, global_args),
        GitCommand::Log => run_log(args, max_lines, verbose, global_args),
        GitCommand::Status => run_status(args, verbose, global_args),
        GitCommand::Show => run_show(args, max_lines, verbose, global_args),
        GitCommand::Add => run_add(args, verbose, global_args),
        GitCommand::Commit => run_commit(args, verbose, global_args),
        GitCommand::Push => run_push(args, verbose, global_args),
        GitCommand::Pull => run_pull(args, verbose, global_args),
        GitCommand::Branch => run_branch(args, verbose, global_args),
        GitCommand::Fetch => run_fetch(args, verbose, global_args),
        GitCommand::Stash { subcommand } => {
            run_stash(subcommand.as_deref(), args, verbose, global_args)
        }
        GitCommand::Worktree => run_worktree(args, verbose, global_args),
    }
}

/// Re-insert `--` before the first path-like argument when clap has consumed it.
///
/// clap's `trailing_var_arg = true` silently drops `--` when it appears as the
/// first positional argument (before any other positional).  This means:
///   `rtk git diff -- file` → args = ["file"]   (clap ate `--`)
///   `rtk git diff HEAD -- file` → args = ["HEAD", "--", "file"]  (preserved)
///
/// Without the `--` separator git may treat an unambiguous path as a revision and
/// emit "fatal: ambiguous argument".  We re-insert `--` before the first path-like
/// argument; see `normalize_diff_args_impl` for the detection rules.
fn normalize_diff_args(args: &[String]) -> Vec<String> {
    normalize_diff_args_impl(args, |p| std::path::Path::new(p).exists())
}

/// Testable core of `normalize_diff_args` — accepts an injectable filesystem existence checker.
///
/// The path-detection logic is:
/// 1. Explicit path prefixes (`.`, `~`) → always a path, no filesystem check needed.
/// 2. Contains path separator (`/`, `\`) → use `path_exists` to distinguish branch names
///    (e.g. `feature/auth`) from real paths (e.g. `src/main.rs`).
/// 3. Bare word with no separator → never a path (avoids injecting `--` when a file
///    happens to share a name with a branch or ref, e.g. a file named `main`).
fn normalize_diff_args_impl<F>(args: &[String], path_exists: F) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    // Already has `--` — nothing to do
    if args.iter().any(|a| a == "--") {
        return args.to_vec();
    }
    let path_start = args.iter().position(|arg| {
        if arg.starts_with('-') {
            return false;
        }
        // Explicit path prefixes — always treat as path regardless of existence
        if arg.starts_with('.') || arg.starts_with('~') {
            return true;
        }
        // Contains path separator — use filesystem check to distinguish
        // branch names (feature/auth) from real paths (src/main.rs)
        if arg.contains('/') || arg.contains('\\') {
            return path_exists(arg);
        }
        // Bare word (no separator, no special prefix) — never inject `--`
        // This avoids misidentifying a ref/branch as a path even if a same-named
        // file happens to exist on disk.
        false
    });
    match path_start {
        Some(idx) => {
            let mut out = args[..idx].to_vec();
            out.push("--".to_string());
            out.extend_from_slice(&args[idx..]);
            out
        }
        None => args.to_vec(),
    }
}

fn run_diff(
    args: &[String],
    max_lines: Option<usize>,
    verbose: u8,
    global_args: &[String],
) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    // Re-insert `--` when clap's trailing_var_arg consumed it (issue #1215)
    let args = &normalize_diff_args(args);

    // Check if user wants stat output
    let wants_stat = args
        .iter()
        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");

    // Check if user wants compact diff (default RTK behavior)
    let wants_compact = !args.iter().any(|arg| arg == "--no-compact");

    if wants_stat || !wants_compact {
        // User wants stat or explicitly no compacting - pass through directly
        let mut cmd = git_cmd(global_args);
        cmd.arg("diff");
        for arg in args {
            if arg == "--no-compact" {
                continue; // RTK flag, not a git flag
            }
            cmd.arg(arg);
        }

        let result = exec_capture(&mut cmd).context("Failed to run git diff")?;

        if !result.success() {
            eprintln!("{}", result.stderr);
            return Ok(result.exit_code);
        }

        println!("{}", result.stdout.trim());

        timer.track(
            &format!("git diff {}", args.join(" ")),
            &format!("rtk git diff {} (passthrough)", args.join(" ")),
            &result.stdout,
            &result.stdout,
        );

        return Ok(0);
    }

    // Default RTK behavior: stat first, then compacted diff
    let mut cmd = git_cmd(global_args);
    cmd.arg("diff").arg("--stat");

    for arg in args {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to run git diff")?;

    if !result.success() {
        if !result.stderr.trim().is_empty() {
            eprint!("{}", result.stderr);
        }
        timer.track(
            &format!("git diff {}", args.join(" ")),
            &format!("rtk git diff {}", args.join(" ")),
            &result.stdout,
            &result.stdout,
        );
        return Ok(result.exit_code);
    }

    if verbose > 0 {
        eprintln!("Git diff summary:");
    }

    // Print stat summary first
    println!("{}", result.stdout.trim());

    // Now get actual diff but compact it
    let mut diff_cmd = git_cmd(global_args);
    diff_cmd.arg("diff");
    for arg in args {
        diff_cmd.arg(arg);
    }

    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git diff")?;

    let mut final_output = result.stdout.clone();
    if !diff_result.stdout.is_empty() {
        println!("\n--- Changes ---");
        let compacted = compact_diff(&diff_result.stdout, max_lines.unwrap_or(500));
        println!("{}", compacted);
        final_output.push_str("\n--- Changes ---\n");
        final_output.push_str(&compacted);
    }

    timer.track(
        &format!("git diff {}", args.join(" ")),
        &format!("rtk git diff {}", args.join(" ")),
        &format!("{}\n{}", result.stdout, diff_result.stdout),
        &final_output,
    );

    Ok(0)
}

fn run_show(
    args: &[String],
    max_lines: Option<usize>,
    verbose: u8,
    global_args: &[String],
) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    // If user wants --stat or --format only, pass through
    let wants_stat_only = args
        .iter()
        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");

    let wants_format = args
        .iter()
        .any(|arg| arg.starts_with("--pretty") || arg.starts_with("--format"));

    // `git show rev:path` prints a blob, not a commit diff. In this mode we should
    // pass through directly to avoid duplicated output from compact-show steps.
    let wants_blob_show = args.iter().any(|arg| is_blob_show_arg(arg));

    if wants_stat_only || wants_format || wants_blob_show {
        let mut cmd = git_cmd(global_args);
        cmd.arg("show");
        for arg in args {
            cmd.arg(arg);
        }
        let result = exec_capture(&mut cmd).context("Failed to run git show")?;
        if !result.success() {
            eprintln!("{}", result.stderr);
            return Ok(result.exit_code);
        }
        if wants_blob_show {
            print!("{}", result.stdout);
        } else {
            println!("{}", result.stdout.trim());
        }

        timer.track(
            &format!("git show {}", args.join(" ")),
            &format!("rtk git show {} (passthrough)", args.join(" ")),
            &result.stdout,
            &result.stdout,
        );

        return Ok(0);
    }

    // Get raw output for tracking
    let mut raw_cmd = git_cmd(global_args);
    raw_cmd.arg("show");
    for arg in args {
        raw_cmd.arg(arg);
    }
    let raw_output = exec_capture(&mut raw_cmd)
        .map(|r| r.stdout)
        .unwrap_or_default();

    // Step 1: one-line commit summary
    let mut summary_cmd = git_cmd(global_args);
    summary_cmd.args(["show", "--no-patch", "--pretty=format:%h %s (%ar) <%an>"]);
    for arg in args {
        summary_cmd.arg(arg);
    }
    let summary_result = exec_capture(&mut summary_cmd).context("Failed to run git show")?;
    if !summary_result.success() {
        eprintln!("{}", summary_result.stderr);
        return Ok(summary_result.exit_code);
    }
    println!("{}", summary_result.stdout.trim());

    // Step 2: --stat summary
    let mut stat_cmd = git_cmd(global_args);
    stat_cmd.args(["show", "--stat", "--pretty=format:"]);
    for arg in args {
        stat_cmd.arg(arg);
    }
    let stat_result = exec_capture(&mut stat_cmd).context("Failed to run git show --stat")?;
    let stat_text = stat_result.stdout.trim();
    if !stat_text.is_empty() {
        println!("{}", stat_text);
    }

    // Step 3: compacted diff
    let mut diff_cmd = git_cmd(global_args);
    diff_cmd.args(["show", "--pretty=format:"]);
    for arg in args {
        diff_cmd.arg(arg);
    }
    let diff_result = exec_capture(&mut diff_cmd).context("Failed to run git show (diff)")?;
    let diff_text = diff_result.stdout.trim();

    let mut final_output = summary_result.stdout.clone();
    if !diff_text.is_empty() {
        if verbose > 0 {
            println!("\n--- Changes ---");
        }
        let compacted = compact_diff(diff_text, max_lines.unwrap_or(500));
        println!("{}", compacted);
        final_output.push_str(&format!("\n{}", compacted));
    }

    timer.track(
        &format!("git show {}", args.join(" ")),
        &format!("rtk git show {}", args.join(" ")),
        &raw_output,
        &final_output,
    );

    Ok(0)
}

fn is_blob_show_arg(arg: &str) -> bool {
    // Detect `rev:path` style arguments while ignoring flags like `--pretty=format:...`.
    !arg.starts_with('-') && arg.contains(':')
}

pub(crate) fn compact_diff(diff: &str, max_lines: usize) -> String {
    let mut result = Vec::new();
    let mut current_file = String::new();
    let mut added = 0;
    let mut removed = 0;
    let mut in_hunk = false;
    let mut hunk_shown = 0;
    let mut hunk_skipped = 0usize;
    let max_hunk_lines = 100;
    let mut was_truncated = false;

    for line in diff.lines() {
        if line.starts_with("diff --git") {
            // Flush hunk truncation before starting a new file
            if hunk_skipped > 0 {
                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
                was_truncated = true;
                hunk_skipped = 0;
            }
            if !current_file.is_empty() && (added > 0 || removed > 0) {
                result.push(format!("  +{} -{}", added, removed));
            }
            current_file = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
            result.push(format!("\n{}", current_file));
            added = 0;
            removed = 0;
            in_hunk = false;
            hunk_shown = 0;
        } else if line.starts_with("@@") {
            // Flush hunk truncation before starting a new hunk
            if hunk_skipped > 0 {
                result.push(format!("  ... ({} lines truncated)", hunk_skipped));
                was_truncated = true;
                hunk_skipped = 0;
            }
            in_hunk = true;
            hunk_shown = 0;
            // Preserve the full unified diff hunk header, including trailing
            // function / symbol context after the second @@ marker.
            result.push(format!("  {}", line));
        } else if in_hunk {
            if line.starts_with('+') && !line.starts_with("+++") {
                added += 1;
                if hunk_shown < max_hunk_lines {
                    result.push(format!("  {}", line));
                    hunk_shown += 1;
                } else {
                    hunk_skipped += 1;
                }
            } else if line.starts_with('-') && !line.starts_with("---") {
                removed += 1;
                if hunk_shown < max_hunk_lines {
                    result.push(format!("  {}", line));
                    hunk_shown += 1;
                } else {
                    hunk_skipped += 1;
                }
            } else if hunk_shown < max_hunk_lines && !line.starts_with("\\") {
                // Context line
                if hunk_shown > 0 {
                    result.push(format!("  {}", line));
                    hunk_shown += 1;
                }
            }
        }

        if result.len() >= max_lines {
            result.push("\n... (more changes truncated)".to_string());
            was_truncated = true;
            break;
        }
    }

    // Flush last hunk
    if hunk_skipped > 0 {
        result.push(format!("  ... ({} lines truncated)", hunk_skipped));
        was_truncated = true;
    }

    if !current_file.is_empty() && (added > 0 || removed > 0) {
        result.push(format!("  +{} -{}", added, removed));
    }

    if was_truncated {
        result.push("[full diff: rtk git diff --no-compact]".to_string());
    }

    result.join("\n")
}

fn run_log(
    args: &[String],
    _max_lines: Option<usize>,
    verbose: u8,
    global_args: &[String],
) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = git_cmd(global_args);
    cmd.arg("log");

    // Check if user provided format flags
    let has_format_flag = args.iter().any(|arg| {
        arg.starts_with("--oneline") || arg.starts_with("--pretty") || arg.starts_with("--format")
    });

    // Check if user provided limit flag (-N, -n N, --max-count=N, --max-count N)
    let has_limit_flag = args.iter().any(|arg| {
        (arg.starts_with('-') && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
            || arg == "-n"
            || arg.starts_with("--max-count")
    });

    // Apply RTK defaults only if user didn't specify them
    // Use %b (body) to preserve first line of commit body for agent context
    // (BREAKING CHANGE, Closes #xxx, design notes)
    if !has_format_flag {
        cmd.args(["--pretty=format:%h %s (%ar) <%an>%n%b%n---END---"]);
    }

    // Determine limit: respect user's explicit -N flag, use sensible defaults otherwise
    let (limit, user_set_limit) = if has_limit_flag {
        // User explicitly passed -N / -n N / --max-count=N → respect their choice
        let n = parse_user_limit(args).unwrap_or(10);
        (n, true)
    } else if has_format_flag {
        // --oneline / --pretty without -N: user wants compact output, allow more
        cmd.arg("-50");
        (50, false)
    } else {
        // No flags at all: default to 10
        cmd.arg("-10");
        (10, false)
    };

    // Only add --no-merges if user didn't explicitly request merge commits
    let wants_merges = args
        .iter()
        .any(|arg| arg == "--merges" || arg == "--min-parents=2");
    if !wants_merges {
        cmd.arg("--no-merges");
    }

    // Pass all user arguments
    for arg in args {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to run git log")?;

    if !result.success() {
        eprintln!("{}", result.stderr);
        return Ok(result.exit_code);
    }

    if verbose > 0 {
        eprintln!("Git log output:");
    }

    // Post-process: truncate long messages, cap lines only if RTK set the default
    let filtered = filter_log_output(&result.stdout, limit, user_set_limit, has_format_flag);
    println!("{}", filtered);

    timer.track(
        &format!("git log {}", args.join(" ")),
        &format!("rtk git log {}", args.join(" ")),
        &result.stdout,
        &filtered,
    );

    Ok(0)
}

/// Filter git log output: truncate long messages, cap lines
/// Parse the user-specified limit from git log args.
/// Handles: -20, -n 20, --max-count=20, --max-count 20
fn parse_user_limit(args: &[String]) -> Option<usize> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        // -20 (combined digit form)
        if arg.starts_with('-')
            && arg.len() > 1
            && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
        {
            if let Ok(n) = arg[1..].parse::<usize>() {
                return Some(n);
            }
        }
        // -n 20 (two-token form)
        if arg == "-n" {
            if let Some(next) = iter.next() {
                if let Ok(n) = next.parse::<usize>() {
                    return Some(n);
                }
            }
        }
        // --max-count=20
        if let Some(rest) = arg.strip_prefix("--max-count=") {
            if let Ok(n) = rest.parse::<usize>() {
                return Some(n);
            }
        }
        // --max-count 20 (two-token form)
        if arg == "--max-count" {
            if let Some(next) = iter.next() {
                if let Ok(n) = next.parse::<usize>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// When `user_set_limit` is true, the user explicitly passed `-N` to git log,
/// so we skip line capping (git already returns exactly N commits) and use a
/// wider truncation threshold (120 chars) to preserve commit context that LLMs
/// need for rebase/squash operations.
pub(crate) fn filter_log_output(
    output: &str,
    limit: usize,
    user_set_limit: bool,
    user_format: bool,
) -> String {
    let truncate_width = if user_set_limit { 120 } else { 80 };

    // When user specified their own format (--oneline, --pretty, --format),
    // RTK did not inject ---END--- markers. Use simple line-based truncation.
    if user_format {
        let lines: Vec<&str> = output.lines().collect();
        let max_lines = if user_set_limit { lines.len() } else { limit };
        return lines
            .iter()
            .take(max_lines)
            .map(|l| truncate_line(l, truncate_width))
            .collect::<Vec<_>>()
            .join("\n");
    }

    // RTK injected format: split output into commit blocks separated by ---END---
    let commits: Vec<&str> = output.split("---END---").collect();
    let max_commits = if user_set_limit { commits.len() } else { limit };

    let mut result = Vec::new();
    for block in commits.iter().take(max_commits) {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let mut lines = block.lines();
        // First line is the header: hash subject (date) <author>
        let header = match lines.next() {
            Some(h) => truncate_line(h.trim(), truncate_width),
            None => continue,
        };
        // Remaining lines are the body — keep up to 3 non-empty, non-trailer lines
        let all_body_lines: Vec<&str> = lines
            .map(|l| l.trim())
            .filter(|l| {
                !l.is_empty()
                    && !l.starts_with("Signed-off-by:")
                    && !l.starts_with("Co-authored-by:")
            })
            .collect();
        let body_omitted = all_body_lines.len().saturating_sub(3);
        let body_lines = &all_body_lines[..all_body_lines.len().min(3)];

        if body_lines.is_empty() {
            result.push(header);
        } else {
            let mut entry = header;
            for body in body_lines {
                entry.push_str(&format!("\n  {}", truncate_line(body, truncate_width)));
            }
            if body_omitted > 0 {
                entry.push_str(&format!("\n  [+{} lines omitted]", body_omitted));
            }
            result.push(entry);
        }
    }

    result.join("\n").trim().to_string()
}

/// Truncate a single line to `width` characters, appending "..." if needed
fn truncate_line(line: &str, width: usize) -> String {
    if line.chars().count() > width {
        let truncated: String = line.chars().take(width - 3).collect();
        format!("{}...", truncated)
    } else {
        line.to_string()
    }
}

/// Preserve RTK's branch/clean framing while keeping porcelain file lines intact.
pub(crate) fn format_status_output(porcelain: &str) -> String {
    let lines: Vec<&str> = porcelain
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    if lines.is_empty() {
        return "Clean working tree".to_string();
    }

    let mut output = Vec::new();

    if let Some(branch_line) = lines.first() {
        if branch_line.starts_with("##") {
            let branch = branch_line.trim_start_matches("## ");
            output.push(format!("* {}", branch));
        } else {
            output.push((*branch_line).to_string());
        }
    }

    for line in lines.iter().skip(1) {
        output.push((*line).to_string());
    }

    if lines.len() == 1 && lines[0].starts_with("##") {
        output.push("clean — nothing to commit".to_string());
    }

    output.join("\n")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitStatusState {
    Rebase,
    MergeConflicts,
    MergeReadyToCommit,
    CherryPick,
    Revert,
    Bisect,
    Am,
    SparseCheckout,
}

impl GitStatusState {
    fn summary(self) -> &'static str {
        match self {
            Self::Rebase => "rebase in progress",
            Self::MergeConflicts => "merge in progress. unresolved conflicts",
            Self::MergeReadyToCommit => "merge in progress. no conflicts",
            Self::CherryPick => "cherry-pick in progress",
            Self::Revert => "revert in progress",
            Self::Bisect => "bisect in progress",
            Self::Am => "am session in progress",
            Self::SparseCheckout => "sparse checkout enabled",
        }
    }
}

const REBASE_INDICATORS: &[&str] = &[
    "rebase in progress",
    "You are currently rebasing",
    "You are currently editing",
    "You are currently splitting",
    "Last command done",
    "Next command to do",
    "No commands remaining",
];

fn detect_status_state(line: &str) -> Option<GitStatusState> {
    if line.contains("All conflicts fixed but you are still merging") {
        Some(GitStatusState::MergeReadyToCommit)
    } else if line.contains("You have unmerged paths") {
        Some(GitStatusState::MergeConflicts)
    } else if line.contains("You are currently cherry-picking") {
        Some(GitStatusState::CherryPick)
    } else if line.contains("You are currently reverting") {
        Some(GitStatusState::Revert)
    } else if line.contains("You are currently bisecting") {
        Some(GitStatusState::Bisect)
    } else if line.contains("You are in the middle of an am session") {
        Some(GitStatusState::Am)
    } else if line.contains("You are in a sparse checkout") {
        Some(GitStatusState::SparseCheckout)
    } else if REBASE_INDICATORS.iter().any(|i| line.contains(i)) {
        Some(GitStatusState::Rebase)
    } else {
        None
    }
}

/// Extract a compact in-progress state summary from plain `git status` output.
///
/// Compact mode runs `git status --porcelain -b`, which omits the state header
/// git prints for rebase / merge / cherry-pick / revert / bisect / am / sparse
/// checkout. Hiding that block is a correctness bug — e.g. during an interactive
/// rebase edit, the user sees a "clean" status and misses "You are currently
/// editing a commit while rebasing ...".
///
/// This helper walks the plain-status output we already capture for tracking
/// and emits a compact, RTK-style summary rather than dumping git's full prose.
/// Returns `None` when no state is in progress.
fn extract_state_header(raw: &str) -> Option<String> {
    // Headers of the file-change blocks — everything relevant to state appears
    // above these in git's output, so they double as a terminator.
    const STOPPERS: &[&str] = &[
        "Changes to be committed:",
        "Changes not staged for commit:",
        "Untracked files:",
        "Unmerged paths:",
        "no changes added to commit",
        "nothing to commit",
        "nothing added to commit",
    ];

    for line in raw.lines() {
        let stripped = line.trim();

        if STOPPERS.iter().any(|s| stripped.starts_with(s)) {
            break;
        }

        if let Some(state) = detect_status_state(stripped) {
            return Some(state.summary().to_string());
        }
    }

    None
}

/// Extract the explicit "HEAD detached at/from <ref>" line from plain
/// `git status` output.
///
/// Porcelain `-b` collapses a detached HEAD to the opaque `## HEAD (no branch)`,
/// which an agent (or a distracted human) can misread as a branch literally
/// named `HEAD`. The plain-status output keeps the explicit SHA/ref, so we
/// surface that instead. Returns `None` when HEAD is on a branch.
fn extract_detached_head(raw: &str) -> Option<String> {
    raw.lines()
        .map(str::trim)
        .find(|l| l.starts_with("HEAD detached "))
        .map(str::to_string)
}

/// Minimal filtering for git status with user-provided args
fn filter_status_with_args(output: &str) -> String {
    let mut result = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Skip git hints - can appear at start or within line
        if trimmed.starts_with("(use \"git")
            || trimmed.starts_with("(create/copy files")
            || trimmed.contains("(use \"git add")
            || trimmed.contains("(use \"git restore")
        {
            continue;
        }

        // Special case: clean working tree
        if trimmed.contains("nothing to commit") && trimmed.contains("working tree clean") {
            result.push(trimmed.to_string());
            break;
        }

        result.push(line.to_string());
    }

    if result.is_empty() {
        "ok".to_string()
    } else {
        result.join("\n")
    }
}

fn run_status(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    // Keep a narrow compact path for no-arg status and branch/short-only flags.
    // More complex explicit args still use the existing minimal-filter path.
    if !uses_compact_status_path(args) {
        let mut cmd = build_status_command(args, global_args);
        let result = exec_capture(&mut cmd).context("Failed to run git status")?;

        if !result.success() {
            if !result.stderr.trim().is_empty() {
                eprint!("{}", result.stderr);
            }
            timer.track(
                &format!("git status {}", args.join(" ")),
                &format!("rtk git status {}", args.join(" ")),
                &result.stdout,
                &result.stdout,
            );
            return Ok(result.exit_code);
        }

        if verbose > 0 || !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
        }

        // Apply minimal filtering: strip ANSI, remove hints, empty lines
        let filtered = filter_status_with_args(&result.stdout);
        print!("{}", filtered);

        timer.track(
            &format!("git status {}", args.join(" ")),
            &format!("rtk git status {}", args.join(" ")),
            &result.stdout,
            &filtered,
        );

        return Ok(0);
    }

    let mut raw_cmd = git_cmd_c_locale(global_args);
    raw_cmd.arg("status");
    raw_cmd.args(args);
    let raw_output = exec_capture(&mut raw_cmd)
        .map(|r| r.stdout)
        .unwrap_or_default();

    let mut cmd = build_status_command(args, global_args);
    let result = exec_capture(&mut cmd).context("Failed to run git status")?;

    if !result.stderr.is_empty() && result.stderr.contains("not a git repository") {
        let message = "Not a git repository".to_string();
        eprintln!("{}", message);
        let original_cmd = if args.is_empty() {
            "git status".to_string()
        } else {
            format!("git status {}", args.join(" "))
        };
        let rtk_cmd = if args.is_empty() {
            "rtk git status".to_string()
        } else {
            format!("rtk git status {}", args.join(" "))
        };
        timer.track(&original_cmd, &rtk_cmd, &raw_output, &message);
        return Ok(result.exit_code);
    }

    let mut formatted = format_status_output(&result.stdout);

    // Porcelain `-b` reduces a detached HEAD to "## HEAD (no branch)"; restore
    // the explicit "HEAD detached at <sha>" from the plain status we captured.
    if let Some(detached) = extract_detached_head(&raw_output) {
        formatted = formatted.replacen("* HEAD (no branch)", &format!("* {detached}"), 1);
    }

    // Surface in-progress state (rebase/merge/cherry-pick/bisect/am) from the
    // plain-status output we already captured for tracking. Porcelain omits it
    // and hiding it misleads the user about the true repo state.
    let final_output = match extract_state_header(&raw_output) {
        Some(state) => format!("{}\n{}", state, formatted),
        None => formatted,
    };

    println!("{}", final_output);

    let original_cmd = if args.is_empty() {
        "git status".to_string()
    } else {
        format!("git status {}", args.join(" "))
    };
    let rtk_cmd = if args.is_empty() {
        "rtk git status".to_string()
    } else {
        format!("rtk git status {}", args.join(" "))
    };

    timer.track(&original_cmd, &rtk_cmd, &raw_output, &final_output);

    Ok(0)
}

fn run_add(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = git_cmd(global_args);
    cmd.arg("add");

    // Pass all arguments directly to git (flags like -A, -p, --all, etc.)
    if args.is_empty() {
        cmd.arg(".");
    } else {
        for arg in args {
            cmd.arg(arg);
        }
    }

    let result = exec_capture(&mut cmd).context("Failed to run git add")?;

    if verbose > 0 {
        eprintln!("git add executed");
    }

    let raw_output = format!("{}\n{}", result.stdout, result.stderr);

    if result.success() {
        // Count what was added
        let mut stat_cmd = git_cmd(global_args);
        stat_cmd.args(["diff", "--cached", "--stat", "--shortstat"]);
        let stat_result = exec_capture(&mut stat_cmd).context("Failed to check staged files")?;

        // Mirror git's own behaviour: a no-op `git add` is silent. Emitting a
        // generic "ok" here is misleading — an agent can't tell "staged N files"
        // from "staged nothing" when both print "ok".
        let compact = if stat_result.stdout.trim().is_empty() {
            String::new()
        } else {
            // Parse "1 file changed, 5 insertions(+)" format
            let short = stat_result.stdout.lines().last().unwrap_or("").trim();
            if short.is_empty() {
                "ok".to_string()
            } else {
                format!("ok {}", short)
            }
        };

        if !compact.is_empty() {
            println!("{}", compact);
        }

        timer.track(
            &format!("git add {}", args.join(" ")),
            &format!("rtk git add {}", args.join(" ")),
            &raw_output,
            &compact,
        );
    } else {
        eprintln!("FAILED: git add");
        if !result.stderr.trim().is_empty() {
            eprintln!("{}", result.stderr);
        }
        if !result.stdout.trim().is_empty() {
            eprintln!("{}", result.stdout);
        }
        return Ok(result.exit_code);
    }

    Ok(0)
}

fn build_commit_command(args: &[String], global_args: &[String]) -> Command {
    let mut cmd = git_cmd(global_args);
    cmd.arg("commit");
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

fn run_commit(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    let original_cmd = format!("git commit {}", args.join(" "));

    if verbose > 0 {
        eprintln!("{}", original_cmd);
    }

    let output = build_commit_command(args, global_args)
        .stdin(Stdio::inherit())
        .output()
        .context("Failed to run git commit")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = exit_code_from_output(&output, "git commit");
    let raw_output = format!("{}\n{}", stdout, stderr);

    if output.status.success() {
        // Extract commit hash from output like "[main abc1234] message"
        let compact = if let Some(line) = stdout.lines().next() {
            if let Some(hash_start) = line.find(' ') {
                let hash = line[1..hash_start].split(' ').next_back().unwrap_or("");
                if !hash.is_empty() && hash.len() >= 7 {
                    format!("ok {}", &hash[..7.min(hash.len())])
                } else {
                    "ok".to_string()
                }
            } else {
                "ok".to_string()
            }
        } else {
            "ok".to_string()
        };

        println!("{}", compact);

        timer.track(&original_cmd, "rtk git commit", &raw_output, &compact);
    } else if stderr.contains("nothing to commit") || stdout.contains("nothing to commit") {
        println!("ok (nothing to commit)");
        timer.track(
            &original_cmd,
            "rtk git commit",
            &raw_output,
            "ok (nothing to commit)",
        );
    } else {
        if !stderr.trim().is_empty() {
            eprint!("{}", stderr);
        }
        if !stdout.trim().is_empty() {
            eprint!("{}", stdout);
        }
        timer.track(&original_cmd, "rtk git commit", &raw_output, &raw_output);
        return Ok(exit_code);
    }

    Ok(0)
}

fn run_push(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git push");
    }

    let mut cmd = git_cmd(global_args);
    cmd.arg("push");
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd
        .stdin(Stdio::inherit())
        .output()
        .context("Failed to run git push")?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = format!("{}{}", stdout, stderr);

    if output.status.success() {
        let compact = if stderr.contains("Everything up-to-date") {
            "ok (up-to-date)".to_string()
        } else {
            let mut push_info = String::new();
            for line in stderr.lines() {
                if line.contains("->") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        push_info = format!("ok {}", parts[parts.len() - 1]);
                        break;
                    }
                }
            }
            if !push_info.is_empty() {
                push_info
            } else {
                "ok".to_string()
            }
        };

        println!("{}", compact);

        timer.track(
            &format!("git push {}", args.join(" ")),
            &format!("rtk git push {}", args.join(" ")),
            &raw,
            &compact,
        );
    } else {
        eprintln!("FAILED: git push");
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr);
        }
        if !stdout.trim().is_empty() {
            eprintln!("{}", stdout);
        }
        return Ok(exit_code_from_output(&output, "git push"));
    }

    Ok(0)
}

fn run_pull(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git pull");
    }

    let mut cmd = git_cmd(global_args);
    cmd.arg("pull");
    for arg in args {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to run git pull")?;

    let raw_output = format!("{}\n{}", result.stdout, result.stderr);

    if result.success() {
        let compact = if result.stdout.contains("Already up to date")
            || result.stdout.contains("Already up-to-date")
        {
            "ok (up-to-date)".to_string()
        } else {
            // Count files changed
            let mut files = 0;
            let mut insertions = 0;
            let mut deletions = 0;

            for line in result.stdout.lines() {
                if line.contains("file") && line.contains("changed") {
                    // Parse "3 files changed, 10 insertions(+), 2 deletions(-)"
                    for part in line.split(',') {
                        let part = part.trim();
                        if part.contains("file") {
                            files = part
                                .split_whitespace()
                                .next()
                                .and_then(|n| n.parse().ok())
                                .unwrap_or(0);
                        } else if part.contains("insertion") {
                            insertions = part
                                .split_whitespace()
                                .next()
                                .and_then(|n| n.parse().ok())
                                .unwrap_or(0);
                        } else if part.contains("deletion") {
                            deletions = part
                                .split_whitespace()
                                .next()
                                .and_then(|n| n.parse().ok())
                                .unwrap_or(0);
                        }
                    }
                }
            }

            if files > 0 {
                format!("ok {} files +{} -{}", files, insertions, deletions)
            } else {
                "ok".to_string()
            }
        };

        println!("{}", compact);

        timer.track(
            &format!("git pull {}", args.join(" ")),
            &format!("rtk git pull {}", args.join(" ")),
            &raw_output,
            &compact,
        );
    } else {
        eprintln!("FAILED: git pull");
        if !result.stderr.trim().is_empty() {
            eprintln!("{}", result.stderr);
        }
        if !result.stdout.trim().is_empty() {
            eprintln!("{}", result.stdout);
        }
        return Ok(result.exit_code);
    }

    Ok(0)
}

fn run_branch(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git branch");
    }

    // Detect write operations: delete, rename, copy, upstream tracking
    let has_action_flag = args.iter().any(|a| {
        a == "-d"
            || a == "-D"
            || a == "-m"
            || a == "-M"
            || a == "-c"
            || a == "-C"
            || a == "--set-upstream-to"
            || a.starts_with("--set-upstream-to=")
            || a == "-u"
            || a == "--unset-upstream"
            || a == "--edit-description"
    });

    // Detect flags that produce specific output (not a branch list)
    let has_show_flag = args.iter().any(|a| a == "--show-current");

    // Detect list-mode flags
    let has_list_flag = args.iter().any(|a| {
        a == "-a"
            || a == "--all"
            || a == "-r"
            || a == "--remotes"
            || a == "--list"
            || a == "--merged"
            || a == "--no-merged"
            || a == "--contains"
            || a == "--no-contains"
            || a == "--format"
            || a.starts_with("--format=")
            || a == "--sort"
            || a.starts_with("--sort=")
            || a == "--points-at"
            || a.starts_with("--points-at=")
    });

    // Detect positional arguments (not flags) — indicates branch creation
    let has_positional_arg = args.iter().any(|a| !a.starts_with('-'));

    // --show-current: passthrough with raw stdout (not "ok")
    if has_show_flag {
        let mut cmd = git_cmd(global_args);
        cmd.arg("branch");
        for arg in args {
            cmd.arg(arg);
        }
        let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
        let combined = result.combined();

        let trimmed = result.stdout.trim();
        timer.track(
            &format!("git branch {}", args.join(" ")),
            &format!("rtk git branch {}", args.join(" ")),
            &combined,
            trimmed,
        );

        if result.success() {
            println!("{}", trimmed);
        } else {
            eprintln!("FAILED: git branch {}", args.join(" "));
            if !result.stderr.trim().is_empty() {
                eprintln!("{}", result.stderr);
            }
            return Ok(result.exit_code);
        }
        return Ok(0);
    }

    // Write operation: action flags, or positional args without list flags (= branch creation)
    if has_action_flag || (has_positional_arg && !has_list_flag) {
        let mut cmd = git_cmd(global_args);
        cmd.arg("branch");
        for arg in args {
            cmd.arg(arg);
        }
        let result = exec_capture(&mut cmd).context("Failed to run git branch")?;
        let combined = result.combined();

        let msg = if result.success() { "ok" } else { &combined };

        timer.track(
            &format!("git branch {}", args.join(" ")),
            &format!("rtk git branch {}", args.join(" ")),
            &combined,
            msg,
        );

        if result.success() {
            println!("ok");
        } else {
            eprintln!("FAILED: git branch {}", args.join(" "));
            if !result.stderr.trim().is_empty() {
                eprintln!("{}", result.stderr);
            }
            if !result.stdout.trim().is_empty() {
                eprintln!("{}", result.stdout);
            }
            return Ok(result.exit_code);
        }
        return Ok(0);
    }

    // List mode: show compact branch list
    let mut cmd = git_cmd(global_args);
    cmd.arg("branch");
    if !has_list_flag {
        cmd.arg("-a");
    }
    cmd.arg("--no-color");
    for arg in args {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to run git branch")?;

    if !result.success() {
        if !result.stderr.trim().is_empty() {
            eprint!("{}", result.stderr);
        }
        timer.track(
            &format!("git branch {}", args.join(" ")),
            &format!("rtk git branch {}", args.join(" ")),
            &result.stdout,
            &result.stdout,
        );
        return Ok(result.exit_code);
    }

    let filtered = filter_branch_output(&result.stdout);
    println!("{}", filtered);

    timer.track(
        &format!("git branch {}", args.join(" ")),
        &format!("rtk git branch {}", args.join(" ")),
        &result.stdout,
        &filtered,
    );

    Ok(0)
}

fn filter_branch_output(output: &str) -> String {
    let mut current = String::new();
    let mut local: Vec<String> = Vec::new();
    let mut remote: Vec<String> = Vec::new();
    let mut seen_remote: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(branch) = line.strip_prefix("* ") {
            current = branch.to_string();
        } else if let Some(rest) = line.strip_prefix("remotes/") {
            if let Some(slash_pos) = rest.find('/') {
                let branch = &rest[slash_pos + 1..];
                if branch.starts_with("HEAD ") {
                    continue;
                }
                if seen_remote.insert(branch.to_string()) {
                    remote.push(branch.to_string());
                }
            }
        } else {
            local.push(line.to_string());
        }
    }

    let mut result = Vec::new();
    result.push(format!("* {}", current));

    if !local.is_empty() {
        for b in &local {
            result.push(format!("  {}", b));
        }
    }

    if !remote.is_empty() {
        let remote_only: Vec<&String> = remote
            .iter()
            .filter(|r| *r != &current && !local.contains(r))
            .collect();
        if !remote_only.is_empty() {
            result.push(format!("  remote-only ({}):", remote_only.len()));
            for b in remote_only.iter().take(10) {
                result.push(format!("    {}", b));
            }
            if remote_only.len() > 10 {
                result.push(format!("    ... +{} more", remote_only.len() - 10));
            }
        }
    }

    result.join("\n")
}

fn run_fetch(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git fetch");
    }

    let mut cmd = git_cmd(global_args);
    cmd.arg("fetch");
    for arg in args {
        cmd.arg(arg);
    }

    let result = exec_capture(&mut cmd).context("Failed to run git fetch")?;
    let raw = result.combined();

    if !result.success() {
        eprintln!("FAILED: git fetch");
        if !result.stderr.trim().is_empty() {
            eprintln!("{}", result.stderr);
        }
        return Ok(result.exit_code);
    }

    // Count new refs from stderr (git fetch outputs to stderr)
    let new_refs: usize = result
        .stderr
        .lines()
        .filter(|l| l.contains("->") || l.contains("[new"))
        .count();

    let msg = if new_refs > 0 {
        format!("ok fetched ({} new refs)", new_refs)
    } else {
        "ok fetched".to_string()
    };

    println!("{}", msg);
    timer.track("git fetch", "rtk git fetch", &raw, &msg);

    Ok(0)
}

/// Format status message for stash operations.
/// - For create operations (push/save): checks for "No local changes"
/// - For other operations: uses "ok stash <subcommand>" format
fn format_stash_message(subcommand: Option<&str>, result: &CaptureResult) -> String {
    match subcommand {
        None | Some("push") | Some("save") => {
            // A successful stash collapses to "ok stashed" (the WIP ref/sha git
            // prints isn't needed to `git stash pop`). But a no-op must NOT look
            // like success — pass git's "No local changes to save" through so the
            // agent can tell nothing was stashed.
            if result.combined().contains("No local changes") {
                "No local changes to save".to_string()
            } else {
                "ok stashed".to_string()
            }
        }
        Some(sub) => format!("ok stash {}", sub),
    }
}

fn run_stash(
    subcommand: Option<&str>,
    args: &[String],
    verbose: u8,
    global_args: &[String],
) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git stash {:?}", subcommand);
    }

    match subcommand {
        Some("list") => {
            let mut cmd = git_cmd(global_args);
            cmd.args(["stash", "list"]);
            let result = exec_capture(&mut cmd).context("Failed to run git stash list")?;

            if result.stdout.trim().is_empty() {
                let msg = "No stashes";
                println!("{}", msg);
                timer.track("git stash list", "rtk git stash list", &result.stdout, msg);
                return Ok(0);
            }

            let filtered = filter_stash_list(&result.stdout);
            println!("{}", filtered);
            timer.track(
                "git stash list",
                "rtk git stash list",
                &result.stdout,
                &filtered,
            );
        }
        Some("show") => {
            let mut cmd = git_cmd(global_args);
            cmd.args(["stash", "show", "-p"]);
            for arg in args {
                cmd.arg(arg);
            }
            let result = exec_capture(&mut cmd).context("Failed to run git stash show")?;

            let filtered = if result.stdout.trim().is_empty() {
                let msg = "Empty stash";
                println!("{}", msg);
                msg.to_string()
            } else {
                let compacted = compact_diff(&result.stdout, 100);
                println!("{}", compacted);
                compacted
            };

            timer.track(
                "git stash show",
                "rtk git stash show",
                &result.stdout,
                &filtered,
            );
        }
        Some("apply") | Some("branch") | Some("clear") | Some("create") | Some("drop")
        | Some("export") | Some("import") | Some("pop") | Some("store") => {
            let sub = subcommand.unwrap();
            let mut cmd = git_cmd(global_args);
            cmd.args(["stash", sub]);
            for arg in args {
                cmd.arg(arg);
            }
            let result = exec_capture(&mut cmd).context("Failed to run git stash")?;
            let combined = result.combined();

            let msg = if result.success() {
                let msg = format_stash_message(subcommand, &result);
                println!("{}", msg);
                msg
            } else {
                eprintln!("FAILED: git stash {}", sub);
                if !result.stderr.trim().is_empty() {
                    eprintln!("{}", result.stderr);
                }
                combined.clone()
            };

            timer.track(
                &format!("git stash {}", sub),
                &format!("rtk git stash {}", sub),
                &combined,
                &msg,
            );

            if !result.success() {
                return Ok(result.exit_code);
            }
        }
        // Default: "git stash [push] [--] [<pathspec>...]" or "git stash save [<message>]"
        Some(_) | None => {
            let (sub, arg) = match subcommand {
                Some("save") => ("save", None),
                Some("push") => ("push", None),
                Some(s) => ("push", Some(s)),
                None => ("push", None),
            };
            let mut cmd = git_cmd(global_args);
            cmd.args(["stash", sub]);
            if let Some(arg) = arg {
                cmd.arg(arg);
            }
            for arg in args {
                cmd.arg(arg);
            }
            let result = exec_capture(&mut cmd).context("Failed to run git stash")?;
            let combined = result.combined();

            let msg = if result.success() {
                let msg = format_stash_message(subcommand, &result);
                println!("{}", msg);
                msg
            } else {
                eprintln!("FAILED: git stash {}", sub);
                if !result.stderr.trim().is_empty() {
                    eprintln!("{}", result.stderr);
                }
                combined.clone()
            };

            timer.track(
                &format!("git stash {}", sub),
                &format!("rtk git stash {}", sub),
                &combined,
                &msg,
            );

            if !result.success() {
                return Ok(result.exit_code);
            }
        }
    }

    Ok(0)
}

fn filter_stash_list(output: &str) -> String {
    // Format: "stash@{0}: WIP on main: abc1234 commit message"
    let mut result = Vec::new();
    for line in output.lines() {
        if let Some(colon_pos) = line.find(": ") {
            let index = &line[..colon_pos];
            let rest = &line[colon_pos + 2..];
            // Compact: strip "WIP on branch:" prefix if present
            let message = if let Some(second_colon) = rest.find(": ") {
                rest[second_colon + 2..].trim()
            } else {
                rest.trim()
            };
            result.push(format!("{}: {}", index, message));
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

fn run_worktree(args: &[String], verbose: u8, global_args: &[String]) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git worktree list");
    }

    // If args contain "add", "remove", "prune" etc., pass through
    let has_action = args.iter().any(|a| {
        a == "add" || a == "remove" || a == "prune" || a == "lock" || a == "unlock" || a == "move"
    });

    if has_action {
        let mut cmd = git_cmd(global_args);
        cmd.arg("worktree");
        for arg in args {
            cmd.arg(arg);
        }
        let result = exec_capture(&mut cmd).context("Failed to run git worktree")?;
        let combined = result.combined();

        let msg = if result.success() { "ok" } else { &combined };

        timer.track(
            &format!("git worktree {}", args.join(" ")),
            &format!("rtk git worktree {}", args.join(" ")),
            &combined,
            msg,
        );

        if result.success() {
            println!("ok");
        } else {
            eprintln!("FAILED: git worktree {}", args.join(" "));
            if !result.stderr.trim().is_empty() {
                eprintln!("{}", result.stderr);
            }
            return Ok(result.exit_code);
        }
        return Ok(0);
    }

    // Default: list mode
    let mut cmd = git_cmd(global_args);
    cmd.args(["worktree", "list"]);
    let result = exec_capture(&mut cmd).context("Failed to run git worktree list")?;

    let filtered = filter_worktree_list(&result.stdout);
    println!("{}", filtered);
    timer.track(
        "git worktree list",
        "rtk git worktree",
        &result.stdout,
        &filtered,
    );

    Ok(0)
}

fn filter_worktree_list(output: &str) -> String {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut result = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // Format: "/path/to/worktree  abc1234 [branch]"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let mut path = parts[0].to_string();
            if !home.is_empty() && path.starts_with(&home) {
                path = format!("~{}", &path[home.len()..]);
            }
            let hash = parts[1];
            let branch = parts[2..].join(" ");
            result.push(format!("{} {} {}", path, hash, branch));
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

/// Runs an unsupported git subcommand by passing it through directly
pub fn run_passthrough(args: &[OsString], global_args: &[String], verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git passthrough: {:?}", args);
    }
    let status = git_cmd(global_args)
        .args(args)
        .status()
        .context("Failed to run git")?;

    let args_str = tracking::args_display(args);
    timer.track_passthrough(
        &format!("git {}", args_str),
        &format!("rtk git {} (passthrough)", args_str),
    );

    if !status.success() {
        return Ok(exit_code_from_status(&status, "git"));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_cmd_no_global_args() {
        let cmd = git_cmd(&[]);
        let program = cmd.get_program().to_string_lossy().to_string();
        // On Windows, resolved_command returns full path (e.g. "C:\Program Files\Git\bin\git.exe")
        let basename = std::path::Path::new(&program)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(basename, "git");
        let args: Vec<_> = cmd.get_args().collect();
        assert!(args.is_empty());
    }

    #[test]
    fn test_git_cmd_with_directory() {
        let global_args = vec!["-C".to_string(), "/tmp".to_string()];
        let cmd = git_cmd(&global_args);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, vec!["-C", "/tmp"]);
    }

    #[test]
    fn test_git_cmd_with_multiple_global_args() {
        let global_args = vec![
            "-C".to_string(),
            "/tmp".to_string(),
            "-c".to_string(),
            "user.name=test".to_string(),
            "--git-dir".to_string(),
            "/foo/.git".to_string(),
        ];
        let cmd = git_cmd(&global_args);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(
            args,
            vec![
                "-C",
                "/tmp",
                "-c",
                "user.name=test",
                "--git-dir",
                "/foo/.git"
            ]
        );
    }

    #[test]
    fn test_git_cmd_with_boolean_flags() {
        let global_args = vec!["--no-pager".to_string(), "--bare".to_string()];
        let cmd = git_cmd(&global_args);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, vec!["--no-pager", "--bare"]);
    }

    #[test]
    fn test_git_cmd_c_locale_sets_stable_env() {
        let cmd = git_cmd_c_locale(&[]);
        let envs: Vec<_> = cmd
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().to_string(),
                    value.expect("env value").to_string_lossy().to_string(),
                )
            })
            .collect();
        assert!(envs.contains(&("LC_ALL".to_string(), "C".to_string())));
    }

    #[test]
    fn test_build_status_command_default_includes_uall() {
        let cmd = build_status_command(&[], &[]);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, vec!["status", "--porcelain", "-b", "-uall"]);
    }

    #[test]
    fn test_uses_compact_status_path_for_branch_and_short_flags() {
        assert!(uses_compact_status_path(&["-b".to_string()]));
        assert!(uses_compact_status_path(&["--branch".to_string()]));
        assert!(uses_compact_status_path(&["-sb".to_string()]));
        assert!(uses_compact_status_path(&["-s".to_string(), "-b".to_string()]));
        assert!(uses_compact_status_path(&["--short".to_string(), "--branch".to_string()]));
        assert!(!uses_compact_status_path(&["-s".to_string()]));
        assert!(!uses_compact_status_path(&["--short".to_string()]));
        assert!(!uses_compact_status_path(&["--porcelain".to_string()]));
        assert!(!uses_compact_status_path(&["-uno".to_string()]));
    }

    #[test]
    fn test_build_status_command_with_user_args_passthrough() {
        let args = vec!["--short".to_string(), "--branch".to_string()];
        let cmd = build_status_command(&args, &[]);
        let cmd_args: Vec<_> = cmd.get_args().collect();
        assert_eq!(cmd_args, vec!["status", "--porcelain", "-b", "-uall"]);
    }

    #[test]
    fn test_build_status_command_with_incompatible_user_args_passthrough() {
        let args = vec!["--porcelain".to_string(), "-uno".to_string()];
        let cmd = build_status_command(&args, &[]);
        let cmd_args: Vec<_> = cmd.get_args().collect();
        assert_eq!(cmd_args, vec!["status", "--porcelain", "-uno"]);
    }

    #[test]
    fn test_compact_diff() {
        let diff = r#"diff --git a/foo.rs b/foo.rs
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("hello");
 }
"#;
        let result = compact_diff(diff, 100);
        assert!(result.contains("foo.rs"));
        assert!(result.contains("+"));
    }

    #[test]
    fn test_compact_diff_preserves_full_hunk_header_context() {
        let diff = r#"diff --git a/foo.rs b/foo.rs
--- a/foo.rs
+++ b/foo.rs
@@ -10,3 +10,4 @@ fn important_context() {
 fn main() {
+    println!("hello");
 }
"#;
        let result = compact_diff(diff, 100);
        assert!(
            result.contains("@@ -10,3 +10,4 @@ fn important_context() {"),
            "Expected full hunk header with trailing context, got:\n{}",
            result
        );
    }

    #[test]
    fn test_compact_diff_increased_hunk_limit() {
        // Build a hunk with 25 changed lines — should NOT be truncated with limit 30
        let mut diff =
            "diff --git a/big.rs b/big.rs\n--- a/big.rs\n+++ b/big.rs\n@@ -1,25 +1,25 @@\n"
                .to_string();
        for i in 1..=25 {
            diff.push_str(&format!("+line{}\n", i));
        }
        let result = compact_diff(&diff, 500);
        assert!(
            !result.contains("... (truncated)"),
            "25 lines should not be truncated with max_hunk_lines=30"
        );
        assert!(result.contains("+line25"));
    }

    #[test]
    fn test_compact_diff_increased_total_limit() {
        // Build a diff with 150 output result lines across multiple files — should NOT be cut at 100
        let mut diff = String::new();
        for f in 1..=5 {
            diff.push_str(&format!("diff --git a/file{f}.rs b/file{f}.rs\n--- a/file{f}.rs\n+++ b/file{f}.rs\n@@ -1,20 +1,20 @@\n"));
            for i in 1..=20 {
                diff.push_str(&format!("+line{f}_{i}\n"));
            }
        }
        let result = compact_diff(&diff, 500);
        assert!(
            !result.contains("more changes truncated"),
            "5 files × 20 lines should not exceed max_lines=500"
        );
    }

    // ----- normalize_diff_args (issue #1215 + branch-name fix #1431) -----
    //
    // Tests use normalize_diff_args_impl with a mock path-existence checker so
    // they don't depend on the real filesystem.

    fn exists_mock<'a>(existing: &'a [&'a str]) -> impl Fn(&str) -> bool + 'a {
        move |p| existing.contains(&p)
    }

    /// Baseline: `--` already present → no-op, args unchanged.
    #[test]
    fn test_normalize_diff_args_noop_when_separator_present() {
        let args = vec![
            "HEAD".to_string(),
            "--".to_string(),
            "src/main.rs".to_string(),
        ];
        assert_eq!(normalize_diff_args_impl(&args, exists_mock(&[])), args);
    }

    /// Core regression (issue #1215): clap ate `--` before a real file path.
    /// When the path exists on disk, `--` must be re-inserted.
    #[test]
    fn test_normalize_diff_args_reinserts_separator_before_existing_path() {
        let args = vec!["apps/client/frontend/src/MyComponent.tsx".to_string()];
        let normalized = normalize_diff_args_impl(
            &args,
            exists_mock(&["apps/client/frontend/src/MyComponent.tsx"]),
        );
        assert_eq!(
            normalized,
            vec![
                "--".to_string(),
                "apps/client/frontend/src/MyComponent.tsx".to_string()
            ],
            "-- must be injected before an existing path"
        );
    }

    /// Ref before path: ["HEAD", "src/foo.rs"] where src/foo.rs exists → inject after HEAD.
    #[test]
    fn test_normalize_diff_args_reinserts_separator_after_ref() {
        let args = vec!["HEAD".to_string(), "src/foo.rs".to_string()];
        let normalized = normalize_diff_args_impl(&args, exists_mock(&["src/foo.rs"]));
        assert_eq!(
            normalized,
            vec![
                "HEAD".to_string(),
                "--".to_string(),
                "src/foo.rs".to_string()
            ]
        );
    }

    /// Flags before path: ["--cached", "src/foo.rs"] where src/foo.rs exists.
    #[test]
    fn test_normalize_diff_args_reinserts_separator_after_flag() {
        let args = vec!["--cached".to_string(), "src/foo.rs".to_string()];
        let normalized = normalize_diff_args_impl(&args, exists_mock(&["src/foo.rs"]));
        assert_eq!(
            normalized,
            vec![
                "--cached".to_string(),
                "--".to_string(),
                "src/foo.rs".to_string()
            ]
        );
    }

    /// Pure flags (no paths) → no injection.
    #[test]
    fn test_normalize_diff_args_no_injection_for_pure_flags() {
        let args = vec!["--stat".to_string(), "--cached".to_string()];
        assert_eq!(normalize_diff_args_impl(&args, exists_mock(&[])), args);
    }

    /// Dotfile that exists on disk → inject `--`.
    #[test]
    fn test_normalize_diff_args_dotfile_is_path() {
        let args = vec![".gitignore".to_string()];
        let normalized = normalize_diff_args_impl(&args, exists_mock(&[".gitignore"]));
        assert_eq!(normalized, vec!["--".to_string(), ".gitignore".to_string()]);
    }

    /// A bare ref (HEAD) that doesn't exist as a file → no injection.
    #[test]
    fn test_normalize_diff_args_no_injection_for_bare_ref() {
        let args = vec!["HEAD".to_string()];
        assert_eq!(normalize_diff_args_impl(&args, exists_mock(&[])), args);
    }

    /// Branch name with `/` that does NOT exist as a file → no injection.
    /// Regression for issue #1431: `rtk git diff feature/user-auth` must not inject `--`.
    #[test]
    fn test_normalize_diff_args_no_injection_for_branch_with_slash() {
        let args = vec!["feature/user-auth".to_string()];
        assert_eq!(
            normalize_diff_args_impl(&args, exists_mock(&[])),
            args,
            "branch names containing '/' must not trigger -- injection"
        );
    }

    /// Range syntax with `/` → no injection.
    /// Regression: `rtk git diff main...feature/user-auth` produced no output.
    #[test]
    fn test_normalize_diff_args_no_injection_for_range_with_slash() {
        let args = vec!["main...feature/user-auth".to_string()];
        assert_eq!(
            normalize_diff_args_impl(&args, exists_mock(&[])),
            args,
            "revision ranges like main...feature/user-auth must not trigger -- injection"
        );
    }

    /// Bare word that happens to exist as a file on disk → still no injection.
    /// A file named "main" must not cause `--` to be injected when the user
    /// intends `rtk git diff main` as a branch comparison.
    #[test]
    fn test_normalize_diff_args_no_injection_for_bare_word_even_if_file_exists() {
        let args = vec!["main".to_string()];
        assert_eq!(
            normalize_diff_args_impl(&args, exists_mock(&["main"])),
            args,
            "bare words must never trigger -- injection even when a same-named file exists"
        );
    }

    #[test]
    fn test_is_blob_show_arg() {
        assert!(is_blob_show_arg("develop:modules/pairs_backtest.py"));
        assert!(is_blob_show_arg("HEAD:src/main.rs"));
        assert!(!is_blob_show_arg("--pretty=format:%h"));
        assert!(!is_blob_show_arg("--format=short"));
        assert!(!is_blob_show_arg("HEAD"));
    }

    #[test]
    fn test_filter_branch_output() {
        let output = "* main\n  feature/auth\n  fix/bug-123\n  remotes/origin/HEAD -> origin/main\n  remotes/origin/main\n  remotes/origin/feature/auth\n  remotes/origin/release/v2\n";
        let result = filter_branch_output(output);
        assert!(result.contains("* main"));
        assert!(result.contains("feature/auth"));
        assert!(result.contains("fix/bug-123"));
        // remote-only should show release/v2 but not main or feature/auth (already local)
        assert!(result.contains("remote-only"));
        assert!(result.contains("release/v2"));
    }

    #[test]
    fn test_filter_branch_no_remotes() {
        let output = "* main\n  develop\n";
        let result = filter_branch_output(output);
        assert!(result.contains("* main"));
        assert!(result.contains("develop"));
        assert!(!result.contains("remote-only"));
    }

    #[test]
    fn test_filter_branch_multi_remote() {
        let output = "* main\n  develop\n  remotes/origin/HEAD -> origin/main\n  remotes/origin/main\n  remotes/origin/feature-x\n  remotes/upstream/main\n  remotes/upstream/release-v3\n  remotes/fork/main\n  remotes/fork/experiment\n";
        let result = filter_branch_output(output);
        assert!(result.contains("* main"));
        assert!(result.contains("develop"));
        assert!(
            result.contains("feature-x"),
            "origin branch shown: {}",
            result
        );
        assert!(
            result.contains("release-v3"),
            "upstream branch shown: {}",
            result
        );
        assert!(
            result.contains("experiment"),
            "fork branch shown: {}",
            result
        );
        assert!(
            !result.contains("remotes/"),
            "remote prefix stripped: {}",
            result
        );
        let main_count = result.matches("main").count();
        assert!(
            main_count <= 2,
            "main deduplicated across remotes (found {} occurrences): {}",
            main_count,
            result
        );
    }

    #[test]
    fn test_filter_stash_list() {
        let output =
            "stash@{0}: WIP on main: abc1234 fix login\nstash@{1}: On feature: def5678 wip\n";
        let result = filter_stash_list(output);
        assert!(result.contains("stash@{0}: abc1234 fix login"));
        assert!(result.contains("stash@{1}: def5678 wip"));
    }

    #[test]
    fn test_filter_worktree_list() {
        let output =
            "/home/user/project  abc1234 [main]\n/home/user/worktrees/feat  def5678 [feature]\n";
        let result = filter_worktree_list(output);
        assert!(result.contains("abc1234"));
        assert!(result.contains("[main]"));
        assert!(result.contains("[feature]"));
    }

    #[test]
    fn test_format_status_output_clean() {
        let porcelain = "## main...origin/main\n";
        let result = format_status_output(porcelain);
        assert_eq!(result, "* main...origin/main\nclean — nothing to commit");
    }

    #[test]
    fn test_extract_state_header_clean_returns_none() {
        let raw = "On branch main\nYour branch is up to date with 'origin/main'.\n\nnothing to commit, working tree clean\n";
        assert_eq!(extract_state_header(raw), None);
    }

    #[test]
    fn test_extract_state_header_no_state_with_changes_returns_none() {
        let raw = "On branch main\nChanges not staged for commit:\n  (use \"git add <file>...\" to update what will be committed)\n\tmodified:   src/main.rs\n\nno changes added to commit\n";
        assert_eq!(extract_state_header(raw), None);
    }

    #[test]
    fn test_extract_state_header_editing_while_rebasing() {
        let raw = "On branch feature\n\ninteractive rebase in progress; onto abc1234\nLast command done (1 command done):\n   edit abc123 some message\nNo commands remaining.\nYou are currently editing a commit while rebasing branch 'feature' on 'abc1234'.\n  (use \"git commit --amend\" to amend the current commit)\n  (use \"git rebase --continue\" once you are satisfied with your changes)\n\nnothing to commit, working tree clean\n";
        let out = extract_state_header(raw).expect("state expected");
        assert_eq!(out, "rebase in progress");
    }

    #[test]
    fn test_extract_state_header_merge_unresolved() {
        let raw = "On branch main\nYou have unmerged paths.\n  (fix conflicts and run \"git commit\")\n  (use \"git merge --abort\" to abort the merge)\n\nUnmerged paths:\n\tboth modified:   src/main.rs\n";
        let out = extract_state_header(raw).expect("state expected");
        assert_eq!(out, "merge in progress. unresolved conflicts");
    }

    #[test]
    fn test_extract_state_header_cherry_pick() {
        let raw = "On branch main\n\nYou are currently cherry-picking commit abc1234.\n  (fix conflicts and run \"git cherry-pick --continue\")\n  (use \"git cherry-pick --abort\" to cancel the cherry-pick operation)\n\nnothing to commit, working tree clean\n";
        let out = extract_state_header(raw).expect("state expected");
        assert_eq!(out, "cherry-pick in progress");
    }

    #[test]
    fn test_extract_state_header_bisect() {
        let raw = "On branch main\n\nYou are currently bisecting, started from branch 'main'.\n  (use \"git bisect reset\" to get back to the original branch)\n\nnothing to commit, working tree clean\n";
        let out = extract_state_header(raw).expect("state expected");
        assert_eq!(out, "bisect in progress");
    }

    #[test]
    fn test_extract_state_header_revert() {
        let raw = "On branch main\n\nYou are currently reverting commit abc1234.\n  (fix conflicts and run \"git revert --continue\")\n  (use \"git revert --abort\" to cancel the revert operation)\n\nnothing to commit, working tree clean\n";
        let out = extract_state_header(raw).expect("state expected");
        assert_eq!(out, "revert in progress");
    }

    #[test]
    fn test_extract_state_header_merge_in_middle() {
        let raw = "On branch main\n\nAll conflicts fixed but you are still merging.\n  (use \"git commit\" to conclude merge)\n\nChanges to be committed:\n\tmodified:   src/main.rs\n";
        let out = extract_state_header(raw).expect("state expected");
        assert_eq!(out, "merge in progress. no conflicts");
    }

    #[test]
    fn test_extract_state_header_am_session() {
        let raw = "On branch main\n\nYou are in the middle of an am session.\n  (use \"git am --continue\" to continue)\n  (use \"git am --abort\" to restore the original branch)\n\nnothing to commit, working tree clean\n";
        let out = extract_state_header(raw).expect("state expected");
        assert_eq!(out, "am session in progress");
    }

    #[test]
    fn test_extract_state_header_sparse_checkout() {
        let raw = "On branch main\n\nYou are in a sparse checkout with 17% of tracked files present.\n\nnothing to commit, working tree clean\n";
        let out = extract_state_header(raw).expect("state expected");
        assert_eq!(out, "sparse checkout enabled");
    }

    #[test]
    fn test_format_status_output_preserves_nested_untracked_paths() {
        let porcelain = "## main\n?? tmp/c.txt\n?? tmp/nested/d.txt\n";
        let result = format_status_output(porcelain);
        assert!(result.contains("* main"));
        assert!(result.contains("?? tmp/c.txt"));
        assert!(result.contains("?? tmp/nested/d.txt"));
        assert!(
            result.lines().all(|line| line != "?? tmp/"),
            "Nested untracked files must not collapse back to a directory marker:\n{}",
            result
        );
    }

    #[test]
    fn test_format_status_output_mixed_changes() {
        let porcelain = r#"## main
M  staged.rs
 M modified.rs
A  added.rs
?? untracked.txt
"#;
        let result = format_status_output(porcelain);
        assert!(result.contains("* main"));
        assert!(result.contains("M  staged.rs"));
        assert!(result.contains(" M modified.rs"));
        assert!(result.contains("A  added.rs"));
        assert!(result.contains("?? untracked.txt"));
        assert!(!result.contains("Staged"));
        assert!(!result.contains("Modified"));
        assert!(!result.contains("Untracked"));
    }

    #[test]
    fn test_format_status_output_preserves_rename_and_conflict_lines() {
        let porcelain = "## main\nR  old.rs -> new.rs\nUU conflict.rs\nMM mixed.rs\n";
        let result = format_status_output(porcelain);
        assert!(result.contains("* main"));
        assert!(result.contains("R  old.rs -> new.rs"));
        assert!(result.contains("UU conflict.rs"));
        assert!(result.contains("MM mixed.rs"));
        assert!(!result.contains("conflicts:"));
    }

    #[test]
    fn test_run_passthrough_accepts_args() {
        // Test that run_passthrough compiles and has correct signature
        let _args: Vec<OsString> = vec![OsString::from("tag"), OsString::from("--list")];
        // Compile-time verification that the function exists with correct signature
    }

    #[test]
    fn test_filter_log_output() {
        let output = "abc1234 This is a commit message (2 days ago) <author>\n\n---END---\ndef5678 Another commit (1 week ago) <other>\n\n---END---\n";
        let result = filter_log_output(output, 10, false, false);
        assert!(result.contains("abc1234"));
        assert!(result.contains("def5678"));
        assert_eq!(result.lines().count(), 2);
    }

    #[test]
    fn test_filter_log_output_with_body() {
        // Commit with body: first non-trailer body line should appear indented
        let output = "abc1234 feat: add feature (2 days ago) <author>\nBREAKING CHANGE: removed old API\nSigned-off-by: Author <a@b.com>\n---END---\ndef5678 fix: typo (1 day ago) <other>\n\n---END---\n";
        let result = filter_log_output(output, 10, false, false);
        assert!(result.contains("abc1234"));
        assert!(result.contains("BREAKING CHANGE: removed old API"));
        assert!(!result.contains("Signed-off-by:"));
        // def5678 has no body — just header
        assert!(result.contains("def5678"));
        // 3 lines: header1, body1 indented, header2
        assert_eq!(result.lines().count(), 3);
    }

    #[test]
    fn test_filter_log_output_skips_trailers() {
        // Body with only trailers should not produce a body line
        let output = "abc1234 chore: bump (1 day ago) <bot>\nSigned-off-by: Bot <bot@ci>\nCo-authored-by: Human <h@b>\n---END---\n";
        let result = filter_log_output(output, 10, false, false);
        assert!(result.contains("abc1234"));
        assert!(!result.contains("Signed-off-by:"));
        assert!(!result.contains("Co-authored-by:"));
        assert_eq!(result.lines().count(), 1);
    }

    #[test]
    fn test_filter_log_output_truncate_long() {
        let long_line = "abc1234 ".to_string() + &"x".repeat(100) + " (2 days ago) <author>";
        let result = filter_log_output(&long_line, 10, false, false);
        assert!(result.chars().count() < long_line.chars().count());
        assert!(result.contains("..."));
        assert!(result.chars().count() <= 80);
    }

    #[test]
    fn test_filter_log_output_cap_lines() {
        let output = (0..20)
            .map(|i| format!("hash{} message {} (1 day ago) <author>\n\n---END---", i, i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = filter_log_output(&output, 5, false, false);
        assert_eq!(result.lines().count(), 5);
    }

    #[test]
    fn test_filter_log_output_user_limit_no_cap() {
        // When user explicitly passes -N, all N lines should be returned (no re-truncation)
        let output = (0..20)
            .map(|i| format!("hash{} message {} (1 day ago) <author>\n\n---END---", i, i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = filter_log_output(&output, 20, true, false);
        assert_eq!(
            result.lines().count(),
            20,
            "User's -20 should return all 20 lines"
        );
    }

    #[test]
    fn test_filter_log_output_user_limit_wider_truncation() {
        // When user explicitly passes -N, lines up to 120 chars should NOT be truncated
        let line_90_chars = format!("abc1234 {} (2 days ago) <author>", "x".repeat(60));
        assert!(line_90_chars.chars().count() > 80);
        assert!(line_90_chars.chars().count() < 120);

        let result_default = filter_log_output(&line_90_chars, 10, false, false);
        let result_user = filter_log_output(&line_90_chars, 10, true, false);

        // Default truncates at 80 chars
        assert!(
            result_default.contains("..."),
            "Default should truncate at 80 chars"
        );
        // User-set limit uses wider threshold (120 chars)
        assert!(
            !result_user.contains("..."),
            "User limit should not truncate 90-char line"
        );
    }

    #[test]
    fn test_parse_user_limit_combined() {
        let args: Vec<String> = vec!["-20".into()];
        assert_eq!(parse_user_limit(&args), Some(20));
    }

    #[test]
    fn test_parse_user_limit_n_space() {
        let args: Vec<String> = vec!["-n".into(), "15".into()];
        assert_eq!(parse_user_limit(&args), Some(15));
    }

    #[test]
    fn test_parse_user_limit_max_count_eq() {
        let args: Vec<String> = vec!["--max-count=30".into()];
        assert_eq!(parse_user_limit(&args), Some(30));
    }

    #[test]
    fn test_parse_user_limit_max_count_space() {
        let args: Vec<String> = vec!["--max-count".into(), "25".into()];
        assert_eq!(parse_user_limit(&args), Some(25));
    }

    #[test]
    fn test_parse_user_limit_none() {
        let args: Vec<String> = vec!["--oneline".into()];
        assert_eq!(parse_user_limit(&args), None);
    }

    #[test]
    fn test_filter_log_output_token_savings() {
        fn count_tokens(text: &str) -> usize {
            text.split_whitespace().count()
        }
        // Simulate verbose git log output (default format with full metadata)
        let input = (0..20)
            .map(|i| {
                format!(
                    "commit abc123{:02x}\nAuthor: User Name <user@example.com>\nDate:   Mon Mar 10 10:00:00 2026 +0000\n\n    fix: commit message number {}\n\n    Extended body with details about the change.\n",
                    i, i
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let output = filter_log_output(&input, 10, false, false);
        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(&input) as f64 * 100.0);
        assert!(
            savings >= 60.0,
            "Expected ≥60% token savings, got {:.1}%",
            savings
        );
    }

    #[test]
    fn test_filter_status_with_args() {
        let output = r#"On branch main
Your branch is up to date with 'origin/main'.

Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
  (use "git restore <file>..." to discard changes in working directory)
	modified:   src/main.rs

no changes added to commit (use "git add" and/or "git commit -a")
"#;
        let result = filter_status_with_args(output);
        eprintln!("Result:\n{}", result);
        assert!(result.contains("On branch main"));
        assert!(result.contains("modified:   src/main.rs"));
        assert!(
            !result.contains("(use \"git"),
            "Result should not contain git hints"
        );
    }

    #[test]
    fn test_filter_status_with_args_clean() {
        let output = "nothing to commit, working tree clean\n";
        let result = filter_status_with_args(output);
        assert!(result.contains("nothing to commit"));
    }

    #[test]
    fn test_filter_log_output_multibyte() {
        // Thai characters: each is 3 bytes. A line with >80 bytes but few chars
        let thai_msg = format!("abc1234 {} (2 days ago) <author>", "ก".repeat(30));
        let result = filter_log_output(&thai_msg, 10, false, false);
        // Should not panic
        assert!(result.contains("abc1234"));
        // The line has 30 Thai chars + other text, so > 80 chars total
        // truncate_line now counts chars, not bytes
        // 30 Thai + ~33 other = 63 chars < 80 threshold, so no truncation
        assert!(result.contains("abc1234"));
    }

    #[test]
    fn test_filter_log_output_emoji() {
        let emoji_msg = "abc1234 🎉🎊🎈🎁🎂🎄🎃🎆🎇✨🎉🎊🎈🎁🎂🎄🎃🎆🎇✨ (1 day ago) <user>";
        let result = filter_log_output(emoji_msg, 10, false, false);
        // Should not panic
        // 20 emoji + ~30 other chars = ~50 chars < 80, no truncation needed
        assert!(result.contains("abc1234"));
    }

    #[test]
    fn test_format_status_output_thai_filename() {
        let porcelain = "## main\n M สวัสดี.txt\n?? ทดสอบ.rs\n";
        let result = format_status_output(porcelain);
        // Should not panic
        assert!(result.contains("* main"));
        assert!(result.contains("สวัสดี.txt"));
        assert!(result.contains("ทดสอบ.rs"));
    }

    #[test]
    fn test_format_status_output_emoji_filename() {
        let porcelain = "## main\nA  🎉-party.txt\n M 日本語ファイル.rs\n";
        let result = format_status_output(porcelain);
        assert!(result.contains("* main"));
    }

    /// Regression test: --oneline and other user format flags must preserve all commits.
    /// Before fix, filter_log_output split on ---END--- which doesn't exist when
    /// the user specifies their own format, resulting in only 2 commits surviving.
    #[test]
    fn test_filter_log_output_user_format_oneline() {
        let oneline_output = "abc1234 feat: add feature\n\
                              def5678 fix: typo\n\
                              ghi9012 chore: bump deps\n\
                              jkl3456 docs: update readme\n\
                              mno7890 test: add tests\n";

        let result = filter_log_output(oneline_output, 10, false, true);
        // All 5 lines must survive — no ---END--- splitting
        assert_eq!(result.lines().count(), 5);
        assert!(result.contains("abc1234"));
        assert!(result.contains("mno7890"));
    }

    #[test]
    fn test_filter_log_output_user_format_with_limit() {
        let oneline_output = "abc1234 feat: add feature\n\
                              def5678 fix: typo\n\
                              ghi9012 chore: bump deps\n\
                              jkl3456 docs: update readme\n\
                              mno7890 test: add tests\n";

        // user_set_limit=true means respect all lines (no cap)
        let result = filter_log_output(oneline_output, 3, true, true);
        assert_eq!(result.lines().count(), 5);

        // user_set_limit=false means cap at limit
        let result = filter_log_output(oneline_output, 3, false, true);
        assert_eq!(result.lines().count(), 3);
    }

    /// Regression test: `git branch <name>` must create, not list.
    /// Before fix, positional args fell into list mode which added `-a`,
    /// turning creation into a pattern-filtered listing (silent no-op).
    #[test]
    #[ignore] // Integration test: requires git repo
    fn test_branch_creation_not_swallowed() {
        let branch = "test-rtk-create-branch-regression";
        // Create branch via run_branch
        run_branch(&[branch.to_string()], 0, &[]).expect("run_branch should succeed");
        // Verify it exists
        let output = Command::new("git")
            .args(["branch", "--list", branch])
            .output()
            .expect("git branch --list should work");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(branch),
            "Branch '{}' was not created. run_branch silently swallowed the creation.",
            branch
        );
        // Cleanup
        let _ = Command::new("git").args(["branch", "-d", branch]).output();
    }

    /// Regression test: `git branch <name> <commit>` must create from commit.
    #[test]
    #[ignore] // Integration test: requires git repo
    fn test_branch_creation_from_commit() {
        let branch = "test-rtk-create-from-commit";
        run_branch(&[branch.to_string(), "HEAD".to_string()], 0, &[])
            .expect("run_branch with start-point should succeed");
        let output = Command::new("git")
            .args(["branch", "--list", branch])
            .output()
            .expect("git branch --list should work");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(branch),
            "Branch '{}' was not created from commit.",
            branch
        );
        let _ = Command::new("git").args(["branch", "-d", branch]).output();
    }

    #[test]
    fn test_commit_single_message() {
        let args = vec!["-m".to_string(), "fix: typo".to_string()];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cmd_args, vec!["commit", "-m", "fix: typo"]);
    }

    #[test]
    fn test_commit_multiple_messages() {
        let args = vec![
            "-m".to_string(),
            "feat: add multi-paragraph support".to_string(),
            "-m".to_string(),
            "This allows git commit -m \"title\" -m \"body\".".to_string(),
        ];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            cmd_args,
            vec![
                "commit",
                "-m",
                "feat: add multi-paragraph support",
                "-m",
                "This allows git commit -m \"title\" -m \"body\"."
            ]
        );
    }

    // #327: git commit -am "msg" must pass -am through to git
    #[test]
    fn test_commit_am_flag() {
        let args = vec!["-am".to_string(), "quick fix".to_string()];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cmd_args, vec!["commit", "-am", "quick fix"]);
    }

    #[test]
    fn test_commit_amend() {
        let args = vec![
            "--amend".to_string(),
            "-m".to_string(),
            "new msg".to_string(),
        ];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cmd_args, vec!["commit", "--amend", "-m", "new msg"]);
    }

    #[test]
    #[ignore] // Requires `cargo build` first — run with `cargo test --ignored`
    fn test_git_status_not_a_repo_exits_nonzero() {
        // Run rtk git status in a directory that is not a git repo
        let tmp = std::env::temp_dir().join("rtk_test_not_a_repo");
        let _ = std::fs::create_dir_all(&tmp);

        // Build the path to the test binary
        let bin_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join("rtk");
        assert!(
            bin_path.exists(),
            "Debug binary not found at {:?} — run `cargo build` first",
            bin_path
        );
        let output = std::process::Command::new(&bin_path)
            .args(["git", "status"])
            .current_dir(&tmp)
            .output()
            .expect("Failed to run rtk");

        // Should exit with non-zero (128 from git)
        assert!(
            !output.status.success(),
            "Expected non-zero exit code for git status outside a repo, got {:?}",
            output.status.code()
        );

        // Message should be on stderr, not stdout
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stderr.to_lowercase().contains("not a git repository"),
            "Expected 'not a git repository' on stderr, got stderr={:?}, stdout={:?}",
            stderr,
            stdout
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- truncation accuracy ---

    #[test]
    fn test_format_status_output_shows_every_file_when_many_are_dirty() {
        let mut porcelain = String::from("## main...origin/main\n");
        for i in 0..25 {
            porcelain.push_str(&format!("M  staged_file_{}.rs\n", i));
        }
        let result = format_status_output(&porcelain);
        assert!(
            result.contains("staged_file_24.rs"),
            "Expected the last staged file to remain visible, got:\n{}",
            result
        );
        assert!(
            result.lines().count() == 26,
            "Expected branch + all 25 staged files, got:\n{}",
            result
        );
        assert!(
            !result.contains("... +"),
            "Status output must not hide dirty paths behind overflow markers:\n{}",
            result
        );
    }

    #[test]
    fn test_compact_diff_recovery_hint_present() {
        // A hunk with 110 lines exceeds max_hunk_lines (100), triggers truncation
        // The recovery hint must appear so LLMs can re-fetch the full diff
        let mut diff = String::new();
        diff.push_str("diff --git a/large.rs b/large.rs\n");
        diff.push_str("--- a/large.rs\n");
        diff.push_str("+++ b/large.rs\n");
        diff.push_str("@@ -1,150 +1,150 @@\n");
        for i in 0..110 {
            diff.push_str(&format!("+added line {}\n", i));
        }
        let result = compact_diff(&diff, 500);
        assert!(
            result.contains("[full diff: rtk git diff --no-compact]"),
            "Expected recovery hint when hunk is truncated, got:\n{}",
            result
        );
    }

    #[test]
    fn test_compact_diff_hunk_truncation_count_accurate() {
        // 150 change lines in one hunk: 100 shown, 50 silently dropped
        // Must report the exact count, not just "(truncated)"
        let mut diff = String::from(
            "diff --git a/large.rs b/large.rs\n--- a/large.rs\n+++ b/large.rs\n@@ -1,150 +1,150 @@\n",
        );
        for i in 0..150 {
            diff.push_str(&format!("+line {}\n", i));
        }
        let result = compact_diff(&diff, 500);
        assert!(
            result.contains("50 lines truncated"),
            "Expected '50 lines truncated' (150 - 100 = 50), got:\n{}",
            result
        );
    }

    #[test]
    fn test_filter_log_output_body_omission_indicator() {
        // Commit with 6 meaningful body lines: only 3 shown, must signal "+3 lines omitted"
        let body_lines = (1..=6)
            .map(|i| format!("body line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let output = format!(
            "abc1234 feat: big change (1 day ago) <author>\n{}\n---END---\n",
            body_lines
        );
        let result = filter_log_output(&output, 10, false, false);
        assert!(
            result.contains("+3 lines omitted"),
            "Expected '+3 lines omitted' when 6 body lines truncated to 3, got:\n{}",
            result
        );
    }
}
>>>>>>> 16803a6 (chore(filters): remove filter-level annotations and restore compose logs tail arg)
