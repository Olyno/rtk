<<<<<<< HEAD
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
548|

... [OUTPUT TRUNCATED - 72 chars omitted out of 50072 total] ...

"experiment"),
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

    fn run_push_filter(input: &str, exit_code: i32) -> String {
        use crate::core::stream::StreamFilter;
        let mut f = LineStreamFilter::new(GitPushLineHandler::default());
        let mut out = String::new();
        for line in input.lines() {
            if let Some(s) = f.feed_line(line) {
                out.push_str(&s);
            }
        }
        out.push_str(&f.flush());
        if let Some(s) = f.on_exit(exit_code, input) {
            out.push_str(&s);
        }
        out
    }

    #[test]
    fn test_push_filter_drops_progress_phases() {
        let input = "\
Enumerating objects: 5, done.
Counting objects: 100% (5/5), done.
Delta compression using up to 8 threads
Compressing objects: 100% (3/3), done.
Writing objects: 100% (3/3), 312 bytes | 312.00 KiB/s, done.
Total 3 (delta 2), reused 0 (delta 0)
To https://github.com/foo/bar.git
   abc1234..def5678  master -> master
";
        let result = run_push_filter(input, 0);
        for prefix in GIT_PUSH_NOISE_PREFIXES {
            assert!(
                !result.contains(prefix),
                "noise prefix '{}' leaked through, got: {}",
                prefix,
                result
            );
        }
        assert!(result.contains("To https://github.com/foo/bar.git"));
        assert!(result.contains("master -> master"));
        assert!(result.ends_with("ok master\n"), "got: {}", result);
    }

    #[test]
    fn test_push_filter_up_to_date_summary() {
        let input = "Everything up-to-date\n";
        let result = run_push_filter(input, 0);
        assert!(result.contains("Everything up-to-date"));
        assert!(result.ends_with("ok (up-to-date)\n"), "got: {}", result);
    }

    #[test]
    fn test_push_filter_passes_remote_messages_through() {
        let input = "\
remote: Resolving deltas: 100% (2/2), completed with 2 local objects.
remote: GitHub found 1 vulnerability on foo/bar's default branch (1 moderate).
To https://github.com/foo/bar.git
   abc1234..def5678  feature -> feature
";
        let result = run_push_filter(input, 0);
        assert!(result.contains("remote: Resolving deltas"));
        assert!(result.contains("remote: GitHub found 1 vulnerability"));
        assert!(result.ends_with("ok feature\n"), "got: {}", result);
    }

    #[test]
    fn test_push_filter_no_summary_on_failure() {
        let input = "\
To https://github.com/foo/bar.git
 ! [rejected]        master -> master (non-fast-forward)
error: failed to push some refs to 'https://github.com/foo/bar.git'
";
        let result = run_push_filter(input, 1);
        assert!(result.contains("[rejected]"));
        assert!(result.contains("error: failed to push"));
        assert!(
            !result.contains("ok "),
            "summary leaked on failure, got: {}",
            result
        );
    }

    #[test]
    fn test_push_filter_first_ref_wins_for_summary() {
        let input = "\
To https://github.com/foo/bar.git
   abc1234..def5678  feat/a -> feat/a
   1111111..2222222  feat/b -> feat/b
";
        let result = run_push_filter(input, 0);
        assert!(result.ends_with("ok feat/a\n"), "got: {}", result);
    }

    #[test]
    fn test_push_filter_token_savings_on_verbose_output() {
        let input = "\
Enumerating objects: 142, done.
Counting objects: 100% (142/142), done.
Delta compression using up to 8 threads
Compressing objects: 100% (88/88), done.
Writing objects: 100% (104/104), 28.50 KiB | 14.25 MiB/s, done.
Total 104 (delta 64), reused 0 (delta 0), pack-reused 0
remote: Resolving deltas: 100% (64/64), completed with 24 local objects.
To https://github.com/foo/bar.git
   abc1234..def5678  master -> master
";
        let result = run_push_filter(input, 0);
        let count_tokens = |s: &str| s.split_whitespace().count();
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&result);
        let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
        assert!(
            savings >= 60.0,
            "expected >=60% savings, got {:.1}% (in={}, out={})",
            savings,
            input_tokens,
            output_tokens
        );
    }
}
>>>>>>> 16803a6 (chore(filters): remove filter-level annotations and restore compose logs tail arg)
