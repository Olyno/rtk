<<<<<<< HEAD
1|//! Filters ESLint and Biome linter output, grouping violations by rule.
2|
3|use crate::core::config;
4|use crate::core::stream::exec_capture;
5|use crate::core::tracking;
6|use crate::core::truncate::{CAP_ERRORS, CAP_WARNINGS};
7|use crate::core::utils::{package_manager_exec, resolved_command, truncate};
8|use crate::mypy_cmd;
9|use crate::ruff_cmd;
10|use anyhow::{Context, Result};
11|use serde::{Deserialize, Serialize};
12|use std::collections::HashMap;
13|
14|#[derive(Debug, Deserialize, Serialize)]
15|struct EslintMessage {
16|    #[serde(rename = "ruleId")]
17|    rule_id: Option<String>,
18|    severity: u8,
19|    message: String,
20|    line: usize,
21|    column: usize,
22|}
23|
24|#[derive(Debug, Deserialize, Serialize)]
25|struct EslintResult {
26|    #[serde(rename = "filePath")]
27|    file_path: String,
28|    messages: Vec<EslintMessage>,
29|    #[serde(rename = "errorCount")]
30|    error_count: usize,
31|    #[serde(rename = "warningCount")]
32|    warning_count: usize,
33|}
34|
35|#[derive(Debug, Deserialize)]
36|struct PylintDiagnostic {
37|    #[serde(rename = "type")]
38|    msg_type: String, // "warning", "error", "convention", "refactor"
39|    #[allow(dead_code)]
40|    module: String,
41|    #[allow(dead_code)]
42|    obj: String,
43|    #[allow(dead_code)]
44|    line: usize,
45|    #[allow(dead_code)]
46|    column: usize,
47|    path: String,
48|    symbol: String, // rule code like "unused-variable"
49|    #[allow(dead_code)]
50|    message: String,
51|    #[serde(rename = "message-id")]
52|    message_id: String, // e.g., "W0612"
53|}
54|
55|/// Check if a linter is Python-based (uses pip/pipx, not npm/pnpm)
56|fn is_python_linter(linter: &str) -> bool {
57|    matches!(linter, "ruff" | "pylint" | "mypy" | "flake8")
58|}
59|
60|/// Strip package manager prefixes (npx, bunx, pnpm, pnpm exec, yarn) from args.
61|/// Returns the number of args to skip.
62|fn strip_pm_prefix(args: &[String]) -> usize {
63|    let pm_names = ["npx", "bunx", "pnpm", "yarn"];
64|    let mut skip = 0;
65|    for arg in args {
66|        if pm_names.contains(&arg.as_str()) || arg == "exec" {
67|            skip += 1;
68|        } else {
69|            break;
70|        }
71|    }
72|    skip
73|}
74|
75|/// Detect the linter name from args (after stripping PM prefixes).
76|/// Returns the linter name and whether it was explicitly specified.
77|fn detect_linter(args: &[String]) -> (&str, bool) {
78|    let is_path_or_flag = args.is_empty()
79|        || args[0].starts_with('-')
80|        || args[0].contains('/')
81|        || args[0].contains('.');
82|
83|    if is_path_or_flag {
84|        ("eslint", false)
85|    } else {
86|        (&args[0], true)
87|    }
88|}
89|
90|pub fn run(args: &[String], verbose: u8) -> Result<i32> {
91|    let timer = tracking::TimedExecution::start();
92|
93|    let skip = strip_pm_prefix(args);
94|    let effective_args = &args[skip..];
95|
96|    let (linter, explicit) = detect_linter(effective_args);
97|
98|    // Python linters use resolved_command() directly (they're on PATH via pip/pipx)
99|    // JS linters use package_manager_exec (npx/pnpm exec)
100|    let mut cmd = if is_python_linter(linter) {
101|        resolved_command(linter)
102|    } else {
103|        package_manager_exec(linter)
104|    };
105|
106|    // Add format flags based on linter
107|    match linter {
108|        "eslint" => {
109|            cmd.arg("-f").arg("json");
110|        }
111|        // Force JSON output for ruff check
112|        "ruff" if !effective_args.contains(&"--output-format".to_string()) => {
113|            cmd.arg("check").arg("--output-format=json");
114|        }
115|        // Force JSON2 output for pylint
116|        "pylint" if !effective_args.contains(&"--output-format".to_string()) => {
117|            cmd.arg("--output-format=json2");
118|        }
119|        "mypy" => {
120|            // mypy uses default text output (no special flags)
121|        }
122|        _ => {
123|            // Other linters: no special formatting
124|        }
125|    }
126|
127|    // Add user arguments (skip first if it was the linter name, and skip "check" for ruff if we added it)
128|    let start_idx = if !explicit {
129|        0
130|    } else if linter == "ruff" && !effective_args.is_empty() && effective_args[0] == "ruff" {
131|        // Skip "ruff" and "check" if we already added "check"
132|        if effective_args.len() > 1 && effective_args[1] == "check" {
133|            2
134|        } else {
135|            1
136|        }
137|    } else {
138|        1
139|    };
140|
141|    for arg in &effective_args[start_idx..] {
142|        // Skip --output-format if we already added it
143|        if linter == "ruff" && arg.starts_with("--output-format") {
144|            continue;
145|        }
146|        if linter == "pylint" && arg.starts_with("--output-format") {
147|            continue;
148|        }
149|        cmd.arg(arg);
150|    }
151|
152|    // Default to current directory if no path specified (for ruff/pylint/mypy/eslint)
153|    if matches!(linter, "ruff" | "pylint" | "mypy" | "eslint") {
154|        let has_path = effective_args
155|            .iter()
156|            .skip(start_idx)
157|            .any(|a| !a.starts_with('-') && !a.contains('='));
158|        if !has_path {
159|            cmd.arg(".");
160|        }
161|    }
162|
163|    if verbose > 0 {
164|        eprintln!("Running: {} with structured output", linter);
165|    }
166|
167|    let result = exec_capture(&mut cmd).context(format!(
168|        "Failed to run {}. Is it installed? Try: pip install {} (or npm/pnpm for JS linters)",
169|        linter, linter
170|    ))?;
171|
172|    // Check if process was killed by signal (SIGABRT, SIGKILL, etc.)
173|    if !result.success() && result.exit_code > 128 {
174|        eprintln!("[warn] Linter process terminated abnormally (possibly out of memory)");
175|        if !result.stderr.is_empty() {
176|            eprintln!(
177|                "stderr: {}",
178|                result.stderr.lines().take(5).collect::<Vec<_>>().join("\n")
179|            );
180|        }
181|        return Ok(result.exit_code);
182|    }
183|
184|    let raw = format!("{}\n{}", result.stdout, result.stderr);
185|
186|    // Dispatch to appropriate filter based on linter
187|    let filtered = match linter {
188|        "eslint" => filter_eslint_json(&result.stdout),
189|        "ruff" => {
190|            // Reuse ruff_cmd's JSON parser
191|            if !result.stdout.trim().is_empty() {
192|                ruff_cmd::filter_ruff_check_json(&result.stdout)
193|            } else {
194|                "Ruff: No issues found".to_string()
195|            }
196|        }
197|        "pylint" => filter_pylint_json(&result.stdout),
198|        "mypy" => mypy_cmd::filter_mypy_output(&raw),
199|        _ => filter_generic_lint(&raw),
200|    };
201|
202|    if let Some(hint) = crate::core::tee::tee_and_hint(&raw, "lint", result.exit_code) {
203|        println!("{}\n{}", filtered, hint);
204|    } else {
205|        println!("{}", filtered);
206|    }
207|
208|    timer.track(
209|        &format!("{} {}", linter, args.join(" ")),
210|        &format!("rtk lint {} {}", linter, args.join(" ")),
211|        &raw,
212|        &filtered,
213|    );
214|
215|    if !result.success() {
216|        return Ok(result.exit_code);
217|    }
218|
219|    Ok(0)
220|}
221|
222|/// Filter ESLint JSON output - group by rule and file
223|fn filter_eslint_json(output: &str) -> String {
224|    let results: Result<Vec<EslintResult>, _> = serde_json::from_str(output);
225|
226|    let results = match results {
227|        Ok(r) => r,
228|        Err(e) => {
229|            // Fallback if JSON parsing fails
230|            return format!(
231|                "ESLint output (JSON parse failed: {})\n{}",
232|                e,
233|                truncate(output, config::limits().passthrough_max_chars)
234|            );
235|        }
236|    };
237|
238|    // Count total issues
239|    let total_errors: usize = results.iter().map(|r| r.error_count).sum();
240|    let total_warnings: usize = results.iter().map(|r| r.warning_count).sum();
241|    let total_files = results.iter().filter(|r| !r.messages.is_empty()).count();
242|
243|    if total_errors == 0 && total_warnings == 0 {
244|        return "ESLint: No issues found".to_string();
245|    }
246|
247|    // Group messages by rule
248|    let mut by_rule: HashMap<String, usize> = HashMap::new();
249|    for result in &results {
250|        for msg in &result.messages {
251|            if let Some(rule) = &msg.rule_id {
252|                *by_rule.entry(rule.clone()).or_insert(0) += 1;
253|            }
254|        }
255|    }
256|
257|    // Group by file
258|    let mut by_file: Vec<(&EslintResult, usize)> = results
259|        .iter()
260|        .filter(|r| !r.messages.is_empty())
261|        .map(|r| (r, r.messages.len()))
262|        .collect();
263|    by_file.sort_by_key(|b| std::cmp::Reverse(b.1));
264|
265|    // Build output
266|    let mut result = String::new();
267|    result.push_str(&format!(
268|        "ESLint: {} errors, {} warnings in {} files\n",
269|        total_errors, total_warnings, total_files
270|    ));
271|    result.push_str("═══════════════════════════════════════\n");
272|
273|    // Show top rules
274|    let mut rule_counts: Vec<_> = by_rule.iter().collect();
275|    rule_counts.sort_by(|a, b| b.1.cmp(a.1));
276|
277|    if !rule_counts.is_empty() {
278|        result.push_str("Top rules:\n");
279|        for (rule, count) in rule_counts.iter().take(10) {
280|            result.push_str(&format!("  {} ({}x)\n", rule, count));
281|        }
282|        result.push('\n');
283|    }
284|
285|286|    // Show top files with most issues, plus the top rules in each
287|    const MAX_FILES: usize = CAP_WARNINGS;
288|    result.push_str("Top files:\n");
289|    for (file_result, count) in by_file.iter().take(MAX_FILES) {
290|        let short_path = compact_path(&file_result.file_path);
291|        result.push_str(&format!("  {} ({} issues)\n", short_path, count));
292|
293|        let mut file_rules: HashMap<String, usize> = HashMap::new();
294|309|        for msg in &file_result.messages {
310|            if shown >= MAX_VIOLATIONS {
311|                result.push_str("    ...\n");
312|                break 'outer;
313|            }
314|315|        }
316|        let mut file_rule_counts: Vec<_> = file_rules.iter().collect();
317|        file_rule_counts.sort_by(|a, b| b.1.cmp(a.1));
318|        for (rule, count) in file_rule_counts.iter().take(3) {
319|            result.push_str(&format!("    {} ({})\n", rule, count));
320|        }
321|    }
322|
323|    if by_file.len() > MAX_FILES {
324|        result.push_str(&format!("\n… +{} more files\n", by_file.len() - MAX_FILES));
325|        let all_file_lines = by_file
326|            .iter()
327|            .map(|(r, count)| format!("{} ({} issues)", compact_path(&r.file_path), count))
328|            .collect::<Vec<_>>()
329|            .join("\n");
330|        if let Some(hint) =
331|            crate::core::tee::force_tee_tail_hint(&all_file_lines, "eslint-files", MAX_FILES + 1)
332|        {
333|            result.push_str(&format!("  {}\n", hint));
334|        }
335|357|    }
358|
359|    result.trim().to_string()
360|}
361|
362|/// Filter pylint JSON2 output - group by symbol and file
363|fn filter_pylint_json(output: &str) -> String {
364|    let diagnostics: Result<Vec<PylintDiagnostic>, _> = serde_json::from_str(output);
365|
366|    let diagnostics = match diagnostics {
367|        Ok(d) => d,
368|        Err(e) => {
369|            // Fallback if JSON parsing fails
370|            return format!(
371|                "Pylint output (JSON parse failed: {})\n{}",
372|                e,
373|                truncate(output, config::limits().passthrough_max_chars)
374|            );
375|        }
376|    };
377|
378|    if diagnostics.is_empty() {
379|        return "Pylint: No issues found".to_string();
380|    }
381|
382|    // Count by type
383|    let mut errors = 0;
384|    let mut warnings = 0;
385|    let mut conventions = 0;
386|    let mut refactors = 0;
387|
388|    for diag in &diagnostics {
389|        match diag.msg_type.as_str() {
390|            "error" => errors += 1,
391|            "warning" => warnings += 1,
392|            "convention" => conventions += 1,
393|            "refactor" => refactors += 1,
394|            _ => {}
395|        }
396|    }
397|
398|    // Count unique files
399|    let unique_files: std::collections::HashSet<_> = diagnostics.iter().map(|d| &d.path).collect();
400|    let total_files = unique_files.len();
401|
402|    // Group by symbol (rule code)
403|    let mut by_symbol: HashMap<String, usize> = HashMap::new();
404|    for diag in &diagnostics {
405|        let key = format!("{} ({})", diag.symbol, diag.message_id);
406|        *by_symbol.entry(key).or_insert(0) += 1;
407|    }
408|
409|    // Group by file
410|    let mut by_file: HashMap<&str, usize> = HashMap::new();
411|    for diag in &diagnostics {
412|        *by_file.entry(&diag.path).or_insert(0) += 1;
413|    }
414|
415|    let mut file_counts: Vec<_> = by_file.iter().collect();
416|    file_counts.sort_by(|a, b| b.1.cmp(a.1));
417|
418|    // Build output
419|    let mut result = String::new();
420|    result.push_str(&format!(
421|        "Pylint: {} issues in {} files\n",
422|        diagnostics.len(),
423|        total_files
424|    ));
425|
426|    if errors > 0 || warnings > 0 {
427|        result.push_str(&format!("  {} errors, {} warnings", errors, warnings));
428|        if conventions > 0 || refactors > 0 {
429|            result.push_str(&format!(
430|                ", {} conventions, {} refactors",
431|                conventions, refactors
432|            ));
433|        }
434|        result.push('\n');
435|    }
436|
437|    result.push_str("═══════════════════════════════════════\n");
438|
439|    // Show top symbols (rules)
440|    let mut symbol_counts: Vec<_> = by_symbol.iter().collect();
441|    symbol_counts.sort_by(|a, b| b.1.cmp(a.1));
442|
443|    if !symbol_counts.is_empty() {
444|        result.push_str("Top rules:\n");
445|        for (symbol, count) in symbol_counts.iter().take(10) {
446|            result.push_str(&format!("  {} ({}x)\n", symbol, count));
447|        }
448|        result.push('\n');
449|    }
450|
451|    // Show top files
452|    const MAX_FILES: usize = CAP_WARNINGS;
453|    result.push_str("Top files:\n");
454|    for (file, count) in file_counts.iter().take(MAX_FILES) {
455|        let short_path = compact_path(file);
456|        result.push_str(&format!("  {} ({} issues)\n", short_path, count));
457|
458|        // Show top 3 rules in this file
459|        let mut file_symbols: HashMap<String, usize> = HashMap::new();
460|        for diag in diagnostics.iter().filter(|d| &d.path == *file) {
461|            let key = format!("{} ({})", diag.symbol, diag.message_id);
462|            *file_symbols.entry(key).or_insert(0) += 1;
463|        }
464|
465|        let mut file_symbol_counts: Vec<_> = file_symbols.iter().collect();
466|        file_symbol_counts.sort_by(|a, b| b.1.cmp(a.1));
467|
468|        for (symbol, count) in file_symbol_counts.iter().take(3) {
469|            result.push_str(&format!("    {} ({})\n", symbol, count));
470|        }
471|    }
472|
473|    if file_counts.len() > MAX_FILES {
474|        result.push_str(&format!("\n… +{} more files\n", file_counts.len() - MAX_FILES));
475|        let all_file_lines = file_counts
476|            .iter()
477|            .map(|(file, count)| format!("{} ({} issues)", compact_path(file), count))
478|            .collect::<Vec<_>>()
479|            .join("\n");
480|        if let Some(hint) =
481|            crate::core::tee::force_tee_tail_hint(&all_file_lines, "pylint-files", MAX_FILES + 1)
482|        {
483|            result.push_str(&format!("  {}\n", hint));
484|        }
485|    }
486|
487|    result.trim().to_string()
488|}
489|
490|/// Filter generic linter output (fallback for non-ESLint linters)
491|fn filter_generic_lint(output: &str) -> String {
492|    let mut warnings = 0;
493|    let mut errors = 0;
494|    let mut issues: Vec<String> = Vec::new();
495|
496|    for line in output.lines() {
497|        let line_lower = line.to_lowercase();
498|        if line_lower.contains("warning") {
499|            warnings += 1;
500|            issues.push(line.to_string());
501|        }
502|        if line_lower.contains("error") && !line_lower.contains("0 error") {
503|            errors += 1;
504|            issues.push(line.to_string());
505|        }
506|    }
507|
508|    if errors == 0 && warnings == 0 {
509|        return "Lint: No issues found".to_string();
510|    }
511|
512|    let mut result = String::new();
513|    result.push_str(&format!("Lint: {} errors, {} warnings\n", errors, warnings));
514|    result.push_str("═══════════════════════════════════════\n");
515|
516|    const MAX_ISSUES: usize = CAP_ERRORS;
517|    for issue in issues.iter().take(MAX_ISSUES) {
518|        result.push_str(&format!("{}\n", truncate(issue, 100)));
519|    }
520|
521|    if issues.len() > MAX_ISSUES {
522|        result.push_str(&format!("\n… +{} more issues\n", issues.len() - MAX_ISSUES));
523|        let all_issues = issues.join("\n");
524|        if let Some(hint) =
525|            crate::core::tee::force_tee_tail_hint(&all_issues, "lint-issues", MAX_ISSUES + 1)
526|        {
527|            result.push_str(&format!("  {}\n", hint));
528|        }
529|    }
530|
531|    result.trim().to_string()
532|}
533|
534|/// Compact file path (remove common prefixes)
535|fn compact_path(path: &str) -> String {
536|    // Remove common prefixes like /Users/..., /home/..., C:\
537|    let path = path.replace('\\', "/");
538|
539|    if let Some(pos) = path.rfind("/src/") {
540|        format!("src/{}", &path[pos + 5..])
541|    } else if let Some(pos) = path.rfind("/lib/") {
542|        format!("lib/{}", &path[pos + 5..])
543|    } else if let Some(pos) = path.rfind('/') {
544|        path[pos + 1..].to_string()
545|    } else {
546|        path
547|    }
548|}
549|
550|#[cfg(test)]
551|mod tests {
552|    use super::*;
553|
554|    #[test]
555|    fn test_filter_eslint_json() {
556|        let json = r#"[
557|            {
558|                "filePath": "/Users/test/project/src/utils.ts",
559|                "messages": [
560|                    {
561|                        "ruleId": "prefer-const",
562|                        "severity": 1,
563|                        "message": "Use const instead of let",
564|                        "line": 10,
565|                        "column": 5
566|                    },
567|                    {
568|                        "ruleId": "prefer-const",
569|                        "severity": 1,
570|                        "message": "Use const instead of let",
571|                        "line": 15,
572|                        "column": 5
573|                    }
574|                ],
575|                "errorCount": 0,
576|                "warningCount": 2
577|            },
578|            {
579|                "filePath": "/Users/test/project/src/api.ts",
580|                "messages": [
581|                    {
582|                        "ruleId": "@typescript-eslint/no-unused-vars",
583|                        "severity": 2,
584|                        "message": "Variable x is unused",
585|                        "line": 20,
586|                        "column": 10
587|                    }
588|                ],
589|                "errorCount": 1,
590|                "warningCount": 0
591|            }
592|        ]"#;
593|
594|        let result = filter_eslint_json(json);
595|        assert!(result.contains("ESLint:"));
596|        assert!(result.contains("prefer-const"));
597|        assert!(result.contains("no-unused-vars"));
598|        assert!(result.contains("src/utils.ts"));
599|    }
600|
601|    #[test]
602|    fn test_compact_path() {
603|        assert_eq!(
604|            compact_path("/Users/foo/project/src/utils.ts"),
605|            "src/utils.ts"
606|        );
607|        assert_eq!(
608|            compact_path("C:\\Users\\project\\src\\api.ts"),
609|            "src/api.ts"
610|        );
611|        assert_eq!(compact_path("simple.ts"), "simple.ts");
612|    }
613|
614|    #[test]
615|    fn test_filter_pylint_json_no_issues() {
616|        let output = "[]";
617|        let result = filter_pylint_json(output);
618|        assert!(result.contains("Pylint"));
619|        assert!(result.contains("No issues found"));
620|    }
621|
622|    #[test]
623|    fn test_filter_pylint_json_with_issues() {
624|        let json = r#"[
625|            {
626|                "type": "warning",
627|                "module": "main",
628|                "obj": "",
629|                "line": 10,
630|                "column": 0,
631|                "path": "src/main.py",
632|                "symbol": "unused-variable",
633|                "message": "Unused variable 'x'",
634|                "message-id": "W0612"
635|            },
636|            {
637|                "type": "warning",
638|                "module": "main",
639|                "obj": "foo",
640|                "line": 15,
641|                "column": 4,
642|                "path": "src/main.py",
643|                "symbol": "unused-variable",
644|                "message": "Unused variable 'y'",
645|                "message-id": "W0612"
646|            },
647|            {
648|                "type": "error",
649|                "module": "utils",
650|                "obj": "bar",
651|                "line": 20,
652|                "column": 0,
653|                "path": "src/utils.py",
654|                "symbol": "undefined-variable",
655|                "message": "Undefined variable 'z'",
656|                "message-id": "E0602"
657|            }
658|        ]"#;
659|
660|        let result = filter_pylint_json(json);
661|        assert!(result.contains("3 issues"));
662|        assert!(result.contains("2 files"));
663|        assert!(result.contains("1 errors, 2 warnings"));
664|        assert!(result.contains("unused-variable (W0612)"));
665|        assert!(result.contains("undefined-variable (E0602)"));
666|        assert!(result.contains("main.py"));
667|        assert!(result.contains("utils.py"));
668|    }
669|
670|    #[test]
671|    fn test_strip_pm_prefix_npx() {
672|        let args: Vec<String> = vec!["npx".into(), "eslint".into(), "src/".into()];
673|        assert_eq!(strip_pm_prefix(&args), 1);
674|    }
675|
676|    #[test]
677|    fn test_strip_pm_prefix_bunx() {
678|        let args: Vec<String> = vec!["bunx".into(), "eslint".into(), ".".into()];
679|        assert_eq!(strip_pm_prefix(&args), 1);
680|    }
681|
682|    #[test]
683|    fn test_strip_pm_prefix_pnpm_exec() {
684|        let args: Vec<String> = vec!["pnpm".into(), "exec".into(), "eslint".into()];
685|        assert_eq!(strip_pm_prefix(&args), 2);
686|    }
687|
688|    #[test]
689|    fn test_strip_pm_prefix_none() {
690|        let args: Vec<String> = vec!["eslint".into(), "src/".into()];
691|        assert_eq!(strip_pm_prefix(&args), 0);
692|    }
693|
694|    #[test]
695|    fn test_strip_pm_prefix_empty() {
696|        let args: Vec<String> = vec![];
697|        assert_eq!(strip_pm_prefix(&args), 0);
698|    }
699|
700|    #[test]
701|    fn test_detect_linter_eslint() {
702|        let args: Vec<String> = vec!["eslint".into(), "src/".into()];
703|        let (linter, explicit) = detect_linter(&args);
704|        assert_eq!(linter, "eslint");
705|        assert!(explicit);
706|    }
707|
708|    #[test]
709|    fn test_detect_linter_default_on_path() {
710|        let args: Vec<String> = vec!["src/".into()];
711|        let (linter, explicit) = detect_linter(&args);
712|        assert_eq!(linter, "eslint");
713|        assert!(!explicit);
714|    }
715|
716|    #[test]
717|    fn test_detect_linter_default_on_flag() {
718|        let args: Vec<String> = vec!["--max-warnings=0".into()];
719|        let (linter, explicit) = detect_linter(&args);
720|        assert_eq!(linter, "eslint");
721|        assert!(!explicit);
722|    }
723|
724|    #[test]
725|    fn test_detect_linter_after_npx_strip() {
726|        // Simulates: rtk lint npx eslint src/ → after strip_pm_prefix, args = ["eslint", "src/"]
727|        let full_args: Vec<String> = vec!["npx".into(), "eslint".into(), "src/".into()];
728|        let skip = strip_pm_prefix(&full_args);
729|        let effective = &full_args[skip..];
730|        let (linter, _) = detect_linter(effective);
731|        assert_eq!(linter, "eslint");
732|    }
733|
734|    #[test]
735|    fn test_detect_linter_after_pnpm_exec_strip() {
736|        let full_args: Vec<String> =
737|            vec!["pnpm".into(), "exec".into(), "biome".into(), "check".into()];
738|        let skip = strip_pm_prefix(&full_args);
739|        let effective = &full_args[skip..];
740|        let (linter, _) = detect_linter(effective);
741|        assert_eq!(linter, "biome");
742|    }
743|
744|    #[test]
745|    fn test_is_python_linter() {
746|        assert!(is_python_linter("ruff"));
747|        assert!(is_python_linter("pylint"));
748|        assert!(is_python_linter("mypy"));
749|        assert!(is_python_linter("flake8"));
750|        assert!(!is_python_linter("eslint"));
751|        assert!(!is_python_linter("biome"));
752|        assert!(!is_python_linter("unknown"));
753|    }
754|}
755|
=======
//! Filters ESLint and Biome linter output, grouping violations by rule.

use crate::core::config;
use crate::core::stream::exec_capture;
use crate::core::tracking;
use crate::core::utils::{package_manager_exec, resolved_command, truncate};
use crate::mypy_cmd;
use crate::ruff_cmd;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
struct EslintMessage {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    severity: u8,
    message: String,
    line: usize,
    column: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct EslintResult {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<EslintMessage>,
    #[serde(rename = "errorCount")]
    error_count: usize,
    #[serde(rename = "warningCount")]
    warning_count: usize,
}

#[derive(Debug, Deserialize)]
struct PylintDiagnostic {
    #[serde(rename = "type")]
    msg_type: String, // "warning", "error", "convention", "refactor"
    #[allow(dead_code)]
    module: String,
    #[allow(dead_code)]
    obj: String,
    #[allow(dead_code)]
    line: usize,
    #[allow(dead_code)]
    column: usize,
    path: String,
    symbol: String, // rule code like "unused-variable"
    #[allow(dead_code)]
    message: String,
    #[serde(rename = "message-id")]
    message_id: String, // e.g., "W0612"
}

/// Check if a linter is Python-based (uses pip/pipx, not npm/pnpm)
fn is_python_linter(linter: &str) -> bool {
    matches!(linter, "ruff" | "pylint" | "mypy" | "flake8")
}

/// Strip package manager prefixes (npx, bunx, pnpm, pnpm exec, yarn) from args.
/// Returns the number of args to skip.
fn strip_pm_prefix(args: &[String]) -> usize {
    let pm_names = ["npx", "bunx", "pnpm", "yarn"];
    let mut skip = 0;
    for arg in args {
        if pm_names.contains(&arg.as_str()) || arg == "exec" {
            skip += 1;
        } else {
            break;
        }
    }
    skip
}

/// Detect the linter name from args (after stripping PM prefixes).
/// Returns the linter name and whether it was explicitly specified.
fn detect_linter(args: &[String]) -> (&str, bool) {
    let is_path_or_flag = args.is_empty()
        || args[0].starts_with('-')
        || args[0].contains('/')
        || args[0].contains('.');

    if is_path_or_flag {
        ("eslint", false)
    } else {
        (&args[0], true)
    }
}

pub fn run(args: &[String], verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    let skip = strip_pm_prefix(args);
    let effective_args = &args[skip..];

    let (linter, explicit) = detect_linter(effective_args);

    // Python linters use resolved_command() directly (they're on PATH via pip/pipx)
    // JS linters use package_manager_exec (npx/pnpm exec)
    let mut cmd = if is_python_linter(linter) {
        resolved_command(linter)
    } else {
        package_manager_exec(linter)
    };

    // Add format flags based on linter
    match linter {
        "eslint" => {
            cmd.arg("-f").arg("json");
        }
        // Force JSON output for ruff check
        "ruff" if !effective_args.contains(&"--output-format".to_string()) => {
            cmd.arg("check").arg("--output-format=json");
        }
        // Force JSON2 output for pylint
        "pylint" if !effective_args.contains(&"--output-format".to_string()) => {
            cmd.arg("--output-format=json2");
        }
        "mypy" => {
            // mypy uses default text output (no special flags)
        }
        _ => {
            // Other linters: no special formatting
        }
    }

    // Add user arguments (skip first if it was the linter name, and skip "check" for ruff if we added it)
    let start_idx = if !explicit {
        0
    } else if linter == "ruff" && !effective_args.is_empty() && effective_args[0] == "ruff" {
        // Skip "ruff" and "check" if we already added "check"
        if effective_args.len() > 1 && effective_args[1] == "check" {
            2
        } else {
            1
        }
    } else {
        1
    };

    for arg in &effective_args[start_idx..] {
        // Skip --output-format if we already added it
        if linter == "ruff" && arg.starts_with("--output-format") {
            continue;
        }
        if linter == "pylint" && arg.starts_with("--output-format") {
            continue;
        }
        cmd.arg(arg);
    }

    // Default to current directory if no path specified (for ruff/pylint/mypy/eslint)
    if matches!(linter, "ruff" | "pylint" | "mypy" | "eslint") {
        let has_path = effective_args
            .iter()
            .skip(start_idx)
            .any(|a| !a.starts_with('-') && !a.contains('='));
        if !has_path {
            cmd.arg(".");
        }
    }

    if verbose > 0 {
        eprintln!("Running: {} with structured output", linter);
    }

    let result = exec_capture(&mut cmd).context(format!(
        "Failed to run {}. Is it installed? Try: pip install {} (or npm/pnpm for JS linters)",
        linter, linter
    ))?;

    // Check if process was killed by signal (SIGABRT, SIGKILL, etc.)
    if !result.success() && result.exit_code > 128 {
        eprintln!("[warn] Linter process terminated abnormally (possibly out of memory)");
        if !result.stderr.is_empty() {
            eprintln!(
                "stderr: {}",
                result.stderr.lines().take(5).collect::<Vec<_>>().join("\n")
            );
        }
        return Ok(result.exit_code);
    }

    let raw = format!("{}\n{}", result.stdout, result.stderr);

    // Dispatch to appropriate filter based on linter
    let filtered = match linter {
        "eslint" => filter_eslint_json(&result.stdout),
        "ruff" => {
            // Reuse ruff_cmd's JSON parser
            if !result.stdout.trim().is_empty() {
                ruff_cmd::filter_ruff_check_json(&result.stdout)
            } else {
                "Ruff: No issues found".to_string()
            }
        }
        "pylint" => filter_pylint_json(&result.stdout),
        "mypy" => mypy_cmd::filter_mypy_output(&raw),
        _ => filter_generic_lint(&raw),
    };

    if let Some(hint) = crate::core::tee::tee_and_hint(&raw, "lint", result.exit_code) {
        println!("{}\n{}", filtered, hint);
    } else {
        println!("{}", filtered);
    }

    timer.track(
        &format!("{} {}", linter, args.join(" ")),
        &format!("rtk lint {} {}", linter, args.join(" ")),
        &raw,
        &filtered,
    );

    if !result.success() {
        return Ok(result.exit_code);
    }

    Ok(0)
}

/// Filter ESLint JSON output - group by rule and file
fn filter_eslint_json(output: &str) -> String {
    let results: Result<Vec<EslintResult>, _> = serde_json::from_str(output);

    let results = match results {
        Ok(r) => r,
        Err(e) => {
            // Fallback if JSON parsing fails
            return format!(
                "ESLint output (JSON parse failed: {})\n{}",
                e,
                truncate(output, config::limits().passthrough_max_chars)
            );
        }
    };

    // Count total issues
    let total_errors: usize = results.iter().map(|r| r.error_count).sum();
    let total_warnings: usize = results.iter().map(|r| r.warning_count).sum();
    let total_files = results.iter().filter(|r| !r.messages.is_empty()).count();

    if total_errors == 0 && total_warnings == 0 {
        return "ESLint: No issues found".to_string();
    }

    // Group messages by rule
    let mut by_rule: HashMap<String, usize> = HashMap::new();
    for result in &results {
        for msg in &result.messages {
            if let Some(rule) = &msg.rule_id {
                *by_rule.entry(rule.clone()).or_insert(0) += 1;
            }
        }
    }

    // Group by file
    let mut by_file: Vec<(&EslintResult, usize)> = results
        .iter()
        .filter(|r| !r.messages.is_empty())
        .map(|r| (r, r.messages.len()))
        .collect();
    by_file.sort_by_key(|b| std::cmp::Reverse(b.1));

    // Build output
    let mut result = String::new();
    result.push_str(&format!(
        "ESLint: {} errors, {} warnings in {} files\n",
        total_errors, total_warnings, total_files
    ));
    result.push_str("═══════════════════════════════════════\n");

    // Show top rules
    let mut rule_counts: Vec<_> = by_rule.iter().collect();
    rule_counts.sort_by(|a, b| b.1.cmp(a.1));

    if !rule_counts.is_empty() {
        result.push_str("Top rules:\n");
        for (rule, count) in rule_counts.iter().take(10) {
            result.push_str(&format!("  {} ({}x)\n", rule, count));
        }
        result.push('\n');
    }

    // Show top files with most issues, plus the top rules in each
    result.push_str("Top files:\n");
    for (file_result, count) in by_file.iter().take(10) {
        let short_path = compact_path(&file_result.file_path);
        result.push_str(&format!("  {} ({} issues)\n", short_path, count));

        let mut file_rules: HashMap<String, usize> = HashMap::new();
        for msg in &file_result.messages {
            if let Some(rule) = &msg.rule_id {
                *file_rules.entry(rule.clone()).or_insert(0) += 1;
            }
        }
        let mut file_rule_counts: Vec<_> = file_rules.iter().collect();
        file_rule_counts.sort_by(|a, b| b.1.cmp(a.1));
        for (rule, count) in file_rule_counts.iter().take(3) {
            result.push_str(&format!("    {} ({})\n", rule, count));
        }
    }

    if by_file.len() > 10 {
        result.push_str(&format!("\n... +{} more files\n", by_file.len() - 10));
    }

    result.trim().to_string()
}

/// Filter pylint JSON2 output - group by symbol and file
fn filter_pylint_json(output: &str) -> String {
    let diagnostics: Result<Vec<PylintDiagnostic>, _> = serde_json::from_str(output);

    let diagnostics = match diagnostics {
        Ok(d) => d,
        Err(e) => {
            // Fallback if JSON parsing fails
            return format!(
                "Pylint output (JSON parse failed: {})\n{}",
                e,
                truncate(output, config::limits().passthrough_max_chars)
            );
        }
    };

    if diagnostics.is_empty() {
        return "Pylint: No issues found".to_string();
    }

    // Count by type
    let mut errors = 0;
    let mut warnings = 0;
    let mut conventions = 0;
    let mut refactors = 0;

    for diag in &diagnostics {
        match diag.msg_type.as_str() {
            "error" => errors += 1,
            "warning" => warnings += 1,
            "convention" => conventions += 1,
            "refactor" => refactors += 1,
            _ => {}
        }
    }

    // Count unique files
    let unique_files: std::collections::HashSet<_> = diagnostics.iter().map(|d| &d.path).collect();
    let total_files = unique_files.len();

    // Group by symbol (rule code)
    let mut by_symbol: HashMap<String, usize> = HashMap::new();
    for diag in &diagnostics {
        let key = format!("{} ({})", diag.symbol, diag.message_id);
        *by_symbol.entry(key).or_insert(0) += 1;
    }

    // Group by file
    let mut by_file: HashMap<&str, usize> = HashMap::new();
    for diag in &diagnostics {
        *by_file.entry(&diag.path).or_insert(0) += 1;
    }

    let mut file_counts: Vec<_> = by_file.iter().collect();
    file_counts.sort_by(|a, b| b.1.cmp(a.1));

    // Build output
    let mut result = String::new();
    result.push_str(&format!(
        "Pylint: {} issues in {} files\n",
        diagnostics.len(),
        total_files
    ));

    if errors > 0 || warnings > 0 {
        result.push_str(&format!("  {} errors, {} warnings", errors, warnings));
        if conventions > 0 || refactors > 0 {
            result.push_str(&format!(
                ", {} conventions, {} refactors",
                conventions, refactors
            ));
        }
        result.push('\n');
    }

    result.push_str("═══════════════════════════════════════\n");

    // Show top symbols (rules)
    let mut symbol_counts: Vec<_> = by_symbol.iter().collect();
    symbol_counts.sort_by(|a, b| b.1.cmp(a.1));

    if !symbol_counts.is_empty() {
        result.push_str("Top rules:\n");
        for (symbol, count) in symbol_counts.iter().take(10) {
            result.push_str(&format!("  {} ({}x)\n", symbol, count));
        }
        result.push('\n');
    }

    // Show top files
    result.push_str("Top files:\n");
    for (file, count) in file_counts.iter().take(10) {
        let short_path = compact_path(file);
        result.push_str(&format!("  {} ({} issues)\n", short_path, count));

        // Show top 3 rules in this file
        let mut file_symbols: HashMap<String, usize> = HashMap::new();
        for diag in diagnostics.iter().filter(|d| &d.path == *file) {
            let key = format!("{} ({})", diag.symbol, diag.message_id);
            *file_symbols.entry(key).or_insert(0) += 1;
        }

        let mut file_symbol_counts: Vec<_> = file_symbols.iter().collect();
        file_symbol_counts.sort_by(|a, b| b.1.cmp(a.1));

        for (symbol, count) in file_symbol_counts.iter().take(3) {
            result.push_str(&format!("    {} ({})\n", symbol, count));
        }
    }

    if file_counts.len() > 10 {
        result.push_str(&format!("\n... +{} more files\n", file_counts.len() - 10));
    }

    result.trim().to_string()
}

/// Filter generic linter output (fallback for non-ESLint linters)
fn filter_generic_lint(output: &str) -> String {
    let mut warnings = 0;
    let mut errors = 0;
    let mut issues: Vec<String> = Vec::new();

    for line in output.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("warning") {
            warnings += 1;
            issues.push(line.to_string());
        }
        if line_lower.contains("error") && !line_lower.contains("0 error") {
            errors += 1;
            issues.push(line.to_string());
        }
    }

    if errors == 0 && warnings == 0 {
        return "Lint: No issues found".to_string();
    }

    let mut result = String::new();
    result.push_str(&format!("Lint: {} errors, {} warnings\n", errors, warnings));
    result.push_str("═══════════════════════════════════════\n");

    for issue in issues.iter().take(20) {
        result.push_str(&format!("{}\n", truncate(issue, 100)));
    }

    if issues.len() > 20 {
        result.push_str(&format!("\n... +{} more issues\n", issues.len() - 20));
    }

    result.trim().to_string()
}

/// Compact file path (remove common prefixes)
fn compact_path(path: &str) -> String {
    // Remove common prefixes like /Users/..., /home/..., C:\
    let path = path.replace('\\', "/");

    if let Some(pos) = path.rfind("/src/") {
        format!("src/{}", &path[pos + 5..])
    } else if let Some(pos) = path.rfind("/lib/") {
        format!("lib/{}", &path[pos + 5..])
    } else if let Some(pos) = path.rfind('/') {
        path[pos + 1..].to_string()
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_eslint_json() {
        let json = r#"[
            {
                "filePath": "/Users/test/project/src/utils.ts",
                "messages": [
                    {
                        "ruleId": "prefer-const",
                        "severity": 1,
                        "message": "Use const instead of let",
                        "line": 10,
                        "column": 5
                    },
                    {
                        "ruleId": "prefer-const",
                        "severity": 1,
                        "message": "Use const instead of let",
                        "line": 15,
                        "column": 5
                    }
                ],
                "errorCount": 0,
                "warningCount": 2
            },
            {
                "filePath": "/Users/test/project/src/api.ts",
                "messages": [
                    {
                        "ruleId": "@typescript-eslint/no-unused-vars",
                        "severity": 2,
                        "message": "Variable x is unused",
                        "line": 20,
                        "column": 10
                    }
                ],
                "errorCount": 1,
                "warningCount": 0
            }
        ]"#;

        let result = filter_eslint_json(json);
        assert!(result.contains("ESLint:"));
        assert!(result.contains("prefer-const"));
        assert!(result.contains("no-unused-vars"));
        assert!(result.contains("src/utils.ts"));
    }

    #[test]
    fn test_compact_path() {
        assert_eq!(
            compact_path("/Users/foo/project/src/utils.ts"),
            "src/utils.ts"
        );
        assert_eq!(
            compact_path("C:\\Users\\project\\src\\api.ts"),
            "src/api.ts"
        );
        assert_eq!(compact_path("simple.ts"), "simple.ts");
    }

    #[test]
    fn test_filter_pylint_json_no_issues() {
        let output = "[]";
        let result = filter_pylint_json(output);
        assert!(result.contains("Pylint"));
        assert!(result.contains("No issues found"));
    }

    #[test]
    fn test_filter_pylint_json_with_issues() {
        let json = r#"[
            {
                "type": "warning",
                "module": "main",
                "obj": "",
                "line": 10,
                "column": 0,
                "path": "src/main.py",
                "symbol": "unused-variable",
                "message": "Unused variable 'x'",
                "message-id": "W0612"
            },
            {
                "type": "warning",
                "module": "main",
                "obj": "foo",
                "line": 15,
                "column": 4,
                "path": "src/main.py",
                "symbol": "unused-variable",
                "message": "Unused variable 'y'",
                "message-id": "W0612"
            },
            {
                "type": "error",
                "module": "utils",
                "obj": "bar",
                "line": 20,
                "column": 0,
                "path": "src/utils.py",
                "symbol": "undefined-variable",
                "message": "Undefined variable 'z'",
                "message-id": "E0602"
            }
        ]"#;

        let result = filter_pylint_json(json);
        assert!(result.contains("3 issues"));
        assert!(result.contains("2 files"));
        assert!(result.contains("1 errors, 2 warnings"));
        assert!(result.contains("unused-variable (W0612)"));
        assert!(result.contains("undefined-variable (E0602)"));
        assert!(result.contains("main.py"));
        assert!(result.contains("utils.py"));
    }

    #[test]
    fn test_strip_pm_prefix_npx() {
        let args: Vec<String> = vec!["npx".into(), "eslint".into(), "src/".into()];
        assert_eq!(strip_pm_prefix(&args), 1);
    }

    #[test]
    fn test_strip_pm_prefix_bunx() {
        let args: Vec<String> = vec!["bunx".into(), "eslint".into(), ".".into()];
        assert_eq!(strip_pm_prefix(&args), 1);
    }

    #[test]
    fn test_strip_pm_prefix_pnpm_exec() {
        let args: Vec<String> = vec!["pnpm".into(), "exec".into(), "eslint".into()];
        assert_eq!(strip_pm_prefix(&args), 2);
    }

    #[test]
    fn test_strip_pm_prefix_none() {
        let args: Vec<String> = vec!["eslint".into(), "src/".into()];
        assert_eq!(strip_pm_prefix(&args), 0);
    }

    #[test]
    fn test_strip_pm_prefix_empty() {
        let args: Vec<String> = vec![];
        assert_eq!(strip_pm_prefix(&args), 0);
    }

    #[test]
    fn test_detect_linter_eslint() {
        let args: Vec<String> = vec!["eslint".into(), "src/".into()];
        let (linter, explicit) = detect_linter(&args);
        assert_eq!(linter, "eslint");
        assert!(explicit);
    }

    #[test]
    fn test_detect_linter_default_on_path() {
        let args: Vec<String> = vec!["src/".into()];
        let (linter, explicit) = detect_linter(&args);
        assert_eq!(linter, "eslint");
        assert!(!explicit);
    }

    #[test]
    fn test_detect_linter_default_on_flag() {
        let args: Vec<String> = vec!["--max-warnings=0".into()];
        let (linter, explicit) = detect_linter(&args);
        assert_eq!(linter, "eslint");
        assert!(!explicit);
    }

    #[test]
    fn test_detect_linter_after_npx_strip() {
        // Simulates: rtk lint npx eslint src/ → after strip_pm_prefix, args = ["eslint", "src/"]
        let full_args: Vec<String> = vec!["npx".into(), "eslint".into(), "src/".into()];
        let skip = strip_pm_prefix(&full_args);
        let effective = &full_args[skip..];
        let (linter, _) = detect_linter(effective);
        assert_eq!(linter, "eslint");
    }

    #[test]
    fn test_detect_linter_after_pnpm_exec_strip() {
        let full_args: Vec<String> =
            vec!["pnpm".into(), "exec".into(), "biome".into(), "check".into()];
        let skip = strip_pm_prefix(&full_args);
        let effective = &full_args[skip..];
        let (linter, _) = detect_linter(effective);
        assert_eq!(linter, "biome");
    }

    #[test]
    fn test_is_python_linter() {
        assert!(is_python_linter("ruff"));
        assert!(is_python_linter("pylint"));
        assert!(is_python_linter("mypy"));
        assert!(is_python_linter("flake8"));
        assert!(!is_python_linter("eslint"));
        assert!(!is_python_linter("biome"));
        assert!(!is_python_linter("unknown"));
    }
}
>>>>>>> 16803a6 (chore(filters): remove filter-level annotations and restore compose logs tail arg)
