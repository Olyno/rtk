1|2|1|//! Filters ESLint and Biome linter output, grouping violations by rule.
3|2|
4|3|use crate::core::config;
5|4|use crate::core::stream::exec_capture;
6|5|use crate::core::tracking;
7|6|use crate::core::truncate::{CAP_ERRORS, CAP_WARNINGS};
8|7|use crate::core::utils::{package_manager_exec, resolved_command, truncate};
9|8|use crate::mypy_cmd;
10|9|use crate::ruff_cmd;
11|10|use anyhow::{Context, Result};
12|11|use serde::{Deserialize, Serialize};
13|12|use std::collections::HashMap;
14|13|
15|14|#[derive(Debug, Deserialize, Serialize)]
16|15|struct EslintMessage {
17|16|    #[serde(rename = "ruleId")]
18|17|    rule_id: Option<String>,
19|18|    severity: u8,
20|19|    message: String,
21|20|    line: usize,
22|21|    column: usize,
23|22|}
24|23|
25|24|#[derive(Debug, Deserialize, Serialize)]
26|25|struct EslintResult {
27|26|    #[serde(rename = "filePath")]
28|27|    file_path: String,
29|28|    messages: Vec<EslintMessage>,
30|29|    #[serde(rename = "errorCount")]
31|30|    error_count: usize,
32|31|    #[serde(rename = "warningCount")]
33|32|    warning_count: usize,
34|33|}
35|34|
36|35|#[derive(Debug, Deserialize)]
37|36|struct PylintDiagnostic {
38|37|    #[serde(rename = "type")]
39|38|    msg_type: String, // "warning", "error", "convention", "refactor"
40|39|    #[allow(dead_code)]
41|40|    module: String,
42|41|    #[allow(dead_code)]
43|42|    obj: String,
44|43|    #[allow(dead_code)]
45|44|    line: usize,
46|45|    #[allow(dead_code)]
47|46|    column: usize,
48|47|    path: String,
49|48|    symbol: String, // rule code like "unused-variable"
50|49|    #[allow(dead_code)]
51|50|    message: String,
52|51|    #[serde(rename = "message-id")]
53|52|    message_id: String, // e.g., "W0612"
54|53|}
55|54|
56|55|/// Check if a linter is Python-based (uses pip/pipx, not npm/pnpm)
57|56|fn is_python_linter(linter: &str) -> bool {
58|57|    matches!(linter, "ruff" | "pylint" | "mypy" | "flake8")
59|58|}
60|59|
61|60|/// Strip package manager prefixes (npx, bunx, pnpm, pnpm exec, yarn) from args.
62|61|/// Returns the number of args to skip.
63|62|fn strip_pm_prefix(args: &[String]) -> usize {
64|63|    let pm_names = ["npx", "bunx", "pnpm", "yarn"];
65|64|    let mut skip = 0;
66|65|    for arg in args {
67|66|        if pm_names.contains(&arg.as_str()) || arg == "exec" {
68|67|            skip += 1;
69|68|        } else {
70|69|            break;
71|70|        }
72|71|    }
73|72|    skip
74|73|}
75|74|
76|75|/// Detect the linter name from args (after stripping PM prefixes).
77|76|/// Returns the linter name and whether it was explicitly specified.
78|77|fn detect_linter(args: &[String]) -> (&str, bool) {
79|78|    let is_path_or_flag = args.is_empty()
80|79|        || args[0].starts_with('-')
81|80|        || args[0].contains('/')
82|81|        || args[0].contains('.');
83|82|
84|83|    if is_path_or_flag {
85|84|        ("eslint", false)
86|85|    } else {
87|86|        (&args[0], true)
88|87|    }
89|88|}
90|89|
91|90|pub fn run(args: &[String], verbose: u8) -> Result<i32> {
92|91|    let timer = tracking::TimedExecution::start();
93|92|
94|93|    let skip = strip_pm_prefix(args);
95|94|    let effective_args = &args[skip..];
96|95|
97|96|    let (linter, explicit) = detect_linter(effective_args);
98|97|
99|98|    // Python linters use resolved_command() directly (they're on PATH via pip/pipx)
100|99|    // JS linters use package_manager_exec (npx/pnpm exec)
101|100|    let mut cmd = if is_python_linter(linter) {
102|101|        resolved_command(linter)
103|102|    } else {
104|103|        package_manager_exec(linter)
105|104|    };
106|105|
107|106|    // Add format flags based on linter
108|107|    match linter {
109|108|        "eslint" => {
110|109|            cmd.arg("-f").arg("json");
111|110|        }
112|111|        // Force JSON output for ruff check
113|112|        "ruff" if !effective_args.contains(&"--output-format".to_string()) => {
114|113|            cmd.arg("check").arg("--output-format=json");
115|114|        }
116|115|        // Force JSON2 output for pylint
117|116|        "pylint" if !effective_args.contains(&"--output-format".to_string()) => {
118|117|            cmd.arg("--output-format=json2");
119|118|        }
120|119|        "mypy" => {
121|120|            // mypy uses default text output (no special flags)
122|121|        }
123|122|        _ => {
124|123|            // Other linters: no special formatting
125|124|        }
126|125|    }
127|126|
128|127|    // Add user arguments (skip first if it was the linter name, and skip "check" for ruff if we added it)
129|128|    let start_idx = if !explicit {
130|129|        0
131|130|    } else if linter == "ruff" && !effective_args.is_empty() && effective_args[0] == "ruff" {
132|131|        // Skip "ruff" and "check" if we already added "check"
133|132|        if effective_args.len() > 1 && effective_args[1] == "check" {
134|133|            2
135|134|        } else {
136|135|            1
137|136|        }
138|137|    } else {
139|138|        1
140|139|    };
141|140|
142|141|    for arg in &effective_args[start_idx..] {
143|142|        // Skip --output-format if we already added it
144|143|        if linter == "ruff" && arg.starts_with("--output-format") {
145|144|            continue;
146|145|        }
147|146|        if linter == "pylint" && arg.starts_with("--output-format") {
148|147|            continue;
149|148|        }
150|149|        cmd.arg(arg);
151|150|    }
152|151|
153|152|    // Default to current directory if no path specified (for ruff/pylint/mypy/eslint)
154|153|    if matches!(linter, "ruff" | "pylint" | "mypy" | "eslint") {
155|154|        let has_path = effective_args
156|155|            .iter()
157|156|            .skip(start_idx)
158|157|            .any(|a| !a.starts_with('-') && !a.contains('='));
159|158|        if !has_path {
160|159|            cmd.arg(".");
161|160|        }
162|161|    }
163|162|
164|163|    if verbose > 0 {
165|164|        eprintln!("Running: {} with structured output", linter);
166|165|    }
167|166|
168|167|    let result = exec_capture(&mut cmd).context(format!(
169|168|        "Failed to run {}. Is it installed? Try: pip install {} (or npm/pnpm for JS linters)",
170|169|        linter, linter
171|170|    ))?;
172|171|
173|172|    // Check if process was killed by signal (SIGABRT, SIGKILL, etc.)
174|173|    if !result.success() && result.exit_code > 128 {
175|174|        eprintln!("[warn] Linter process terminated abnormally (possibly out of memory)");
176|175|        if !result.stderr.is_empty() {
177|176|            eprintln!(
178|177|                "stderr: {}",
179|178|                result.stderr.lines().take(5).collect::<Vec<_>>().join("\n")
180|179|            );
181|180|        }
182|181|        return Ok(result.exit_code);
183|182|    }
184|183|
185|184|    let raw = format!("{}\n{}", result.stdout, result.stderr);
186|185|
187|186|    // Dispatch to appropriate filter based on linter
188|187|    let filtered = match linter {
189|188|        "eslint" => filter_eslint_json(&result.stdout),
190|189|        "ruff" => {
191|190|            // Reuse ruff_cmd's JSON parser
192|191|            if !result.stdout.trim().is_empty() {
193|192|                ruff_cmd::filter_ruff_check_json(&result.stdout)
194|193|            } else {
195|194|                "Ruff: No issues found".to_string()
196|195|            }
197|196|        }
198|197|        "pylint" => filter_pylint_json(&result.stdout),
199|198|        "mypy" => mypy_cmd::filter_mypy_output(&raw),
200|199|        _ => filter_generic_lint(&raw),
201|200|    };
202|201|
203|202|    if let Some(hint) = crate::core::tee::tee_and_hint(&raw, "lint", result.exit_code) {
204|203|        println!("{}\n{}", filtered, hint);
205|204|    } else {
206|205|        println!("{}", filtered);
207|206|    }
208|207|
209|208|    timer.track(
210|209|        &format!("{} {}", linter, args.join(" ")),
211|210|        &format!("rtk lint {} {}", linter, args.join(" ")),
212|211|        &raw,
213|212|        &filtered,
214|213|    );
215|214|
216|215|    if !result.success() {
217|216|        return Ok(result.exit_code);
218|217|    }
219|218|
220|219|    Ok(0)
221|220|}
222|221|
223|222|/// Filter ESLint JSON output - group by rule and file
224|223|fn filter_eslint_json(output: &str) -> String {
225|224|    let results: Result<Vec<EslintResult>, _> = serde_json::from_str(output);
226|225|
227|226|    let results = match results {
228|227|        Ok(r) => r,
229|228|        Err(e) => {
230|229|            // Fallback if JSON parsing fails
231|230|            return format!(
232|231|                "ESLint output (JSON parse failed: {})\n{}",
233|232|                e,
234|233|                truncate(output, config::limits().passthrough_max_chars)
235|234|            );
236|235|        }
237|236|    };
238|237|
239|238|    // Count total issues
240|239|    let total_errors: usize = results.iter().map(|r| r.error_count).sum();
241|240|    let total_warnings: usize = results.iter().map(|r| r.warning_count).sum();
242|241|    let total_files = results.iter().filter(|r| !r.messages.is_empty()).count();
243|242|
244|243|    if total_errors == 0 && total_warnings == 0 {
245|244|        return "ESLint: No issues found".to_string();
246|245|    }
247|246|
248|247|    // Group messages by rule
249|248|    let mut by_rule: HashMap<String, usize> = HashMap::new();
250|249|    for result in &results {
251|250|        for msg in &result.messages {
252|251|            if let Some(rule) = &msg.rule_id {
253|252|                *by_rule.entry(rule.clone()).or_insert(0) += 1;
254|253|            }
255|254|        }
256|255|    }
257|256|
258|257|    // Group by file
259|258|    let mut by_file: Vec<(&EslintResult, usize)> = results
260|259|        .iter()
261|260|        .filter(|r| !r.messages.is_empty())
262|261|        .map(|r| (r, r.messages.len()))
263|262|        .collect();
264|263|    by_file.sort_by_key(|b| std::cmp::Reverse(b.1));
265|264|
266|265|    // Build output
267|266|    let mut result = String::new();
268|267|    result.push_str(&format!(
269|268|        "ESLint: {} errors, {} warnings in {} files\n",
270|269|        total_errors, total_warnings, total_files
271|270|    ));
272|271|    result.push_str("═══════════════════════════════════════\n");
273|272|
274|273|    // Show top rules
275|274|    let mut rule_counts: Vec<_> = by_rule.iter().collect();
276|275|    rule_counts.sort_by(|a, b| b.1.cmp(a.1));
277|276|
278|277|    if !rule_counts.is_empty() {
279|278|        result.push_str("Top rules:\n");
280|279|        for (rule, count) in rule_counts.iter().take(10) {
281|280|            result.push_str(&format!("  {} ({}x)\n", rule, count));
282|281|        }
283|282|        result.push('\n');
284|283|    }
285|284|
286|285|286|    // Show top files with most issues, plus the top rules in each
287|287|    const MAX_FILES: usize = CAP_WARNINGS;
288|288|    result.push_str("Top files:\n");
289|289|    for (file_result, count) in by_file.iter().take(MAX_FILES) {
290|290|        let short_path = compact_path(&file_result.file_path);
291|291|        result.push_str(&format!("  {} ({} issues)\n", short_path, count));
292|292|
293|293|        let mut file_rules: HashMap<String, usize> = HashMap::new();
294|294|309|        for msg in &file_result.messages {
295|310|            if shown >= MAX_VIOLATIONS {
296|311|                result.push_str("    ...\n");
297|312|                break 'outer;
298|313|            }
299|314|315|        }
300|316|        let mut file_rule_counts: Vec<_> = file_rules.iter().collect();
301|317|        file_rule_counts.sort_by(|a, b| b.1.cmp(a.1));
302|318|        for (rule, count) in file_rule_counts.iter().take(3) {
303|319|            result.push_str(&format!("    {} ({})\n", rule, count));
304|320|        }
305|321|    }
306|322|
307|323|    if by_file.len() > MAX_FILES {
308|324|        result.push_str(&format!("\n… +{} more files\n", by_file.len() - MAX_FILES));
309|325|        let all_file_lines = by_file
310|326|            .iter()
311|327|            .map(|(r, count)| format!("{} ({} issues)", compact_path(&r.file_path), count))
312|328|            .collect::<Vec<_>>()
313|329|            .join("\n");
314|330|        if let Some(hint) =
315|331|            crate::core::tee::force_tee_tail_hint(&all_file_lines, "eslint-files", MAX_FILES + 1)
316|332|        {
317|333|            result.push_str(&format!("  {}\n", hint));
318|334|        }
319|335|357|    }
320|358|
321|359|    result.trim().to_string()
322|360|}
323|361|
324|362|/// Filter pylint JSON2 output - group by symbol and file
325|363|fn filter_pylint_json(output: &str) -> String {
326|364|    let diagnostics: Result<Vec<PylintDiagnostic>, _> = serde_json::from_str(output);
327|365|
328|366|    let diagnostics = match diagnostics {
329|367|        Ok(d) => d,
330|368|        Err(e) => {
331|369|            // Fallback if JSON parsing fails
332|370|            return format!(
333|371|                "Pylint output (JSON parse failed: {})\n{}",
334|372|                e,
335|373|                truncate(output, config::limits().passthrough_max_chars)
336|374|            );
337|375|        }
338|376|    };
339|377|
340|378|    if diagnostics.is_empty() {
341|379|        return "Pylint: No issues found".to_string();
342|380|    }
343|381|
344|382|    // Count by type
345|383|    let mut errors = 0;
346|384|    let mut warnings = 0;
347|385|    let mut conventions = 0;
348|386|    let mut refactors = 0;
349|387|
350|388|    for diag in &diagnostics {
351|389|        match diag.msg_type.as_str() {
352|390|            "error" => errors += 1,
353|391|            "warning" => warnings += 1,
354|392|            "convention" => conventions += 1,
355|393|            "refactor" => refactors += 1,
356|394|            _ => {}
357|395|        }
358|396|    }
359|397|
360|398|    // Count unique files
361|399|    let unique_files: std::collections::HashSet<_> = diagnostics.iter().map(|d| &d.path).collect();
362|400|    let total_files = unique_files.len();
363|401|
364|402|    // Group by symbol (rule code)
365|403|    let mut by_symbol: HashMap<String, usize> = HashMap::new();
366|404|    for diag in &diagnostics {
367|405|        let key = format!("{} ({})", diag.symbol, diag.message_id);
368|406|        *by_symbol.entry(key).or_insert(0) += 1;
369|407|    }
370|408|
371|409|    // Group by file
372|410|    let mut by_file: HashMap<&str, usize> = HashMap::new();
373|411|    for diag in &diagnostics {
374|412|        *by_file.entry(&diag.path).or_insert(0) += 1;
375|413|    }
376|414|
377|415|    let mut file_counts: Vec<_> = by_file.iter().collect();
378|416|    file_counts.sort_by(|a, b| b.1.cmp(a.1));
379|417|
380|418|    // Build output
381|419|    let mut result = String::new();
382|420|    result.push_str(&format!(
383|421|        "Pylint: {} issues in {} files\n",
384|422|        diagnostics.len(),
385|423|        total_files
386|424|    ));
387|425|
388|426|    if errors > 0 || warnings > 0 {
389|427|        result.push_str(&format!("  {} errors, {} warnings", errors, warnings));
390|428|        if conventions > 0 || refactors > 0 {
391|429|            result.push_str(&format!(
392|430|                ", {} conventions, {} refactors",
393|431|                conventions, refactors
394|432|            ));
395|433|        }
396|434|        result.push('\n');
397|435|    }
398|436|
399|437|    result.push_str("═══════════════════════════════════════\n");
400|438|
401|439|    // Show top symbols (rules)
402|440|    let mut symbol_counts: Vec<_> = by_symbol.iter().collect();
403|441|    symbol_counts.sort_by(|a, b| b.1.cmp(a.1));
404|442|
405|443|    if !symbol_counts.is_empty() {
406|444|        result.push_str("Top rules:\n");
407|445|        for (symbol, count) in symbol_counts.iter().take(10) {
408|446|            result.push_str(&format!("  {} ({}x)\n", symbol, count));
409|447|        }
410|448|        result.push('\n');
411|449|    }
412|450|
413|451|    // Show top files
414|452|    const MAX_FILES: usize = CAP_WARNINGS;
415|453|    result.push_str("Top files:\n");
416|454|    for (file, count) in file_counts.iter().take(MAX_FILES) {
417|455|        let short_path = compact_path(file);
418|456|        result.push_str(&format!("  {} ({} issues)\n", short_path, count));
419|457|
420|458|        // Show top 3 rules in this file
421|459|        let mut file_symbols: HashMap<String, usize> = HashMap::new();
422|460|        for diag in diagnostics.iter().filter(|d| &d.path == *file) {
423|461|            let key = format!("{} ({})", diag.symbol, diag.message_id);
424|462|            *file_symbols.entry(key).or_insert(0) += 1;
425|463|        }
426|464|
427|465|        let mut file_symbol_counts: Vec<_> = file_symbols.iter().collect();
428|466|        file_symbol_counts.sort_by(|a, b| b.1.cmp(a.1));
429|467|
430|468|        for (symbol, count) in file_symbol_counts.iter().take(3) {
431|469|            result.push_str(&format!("    {} ({})\n", symbol, count));
432|470|        }
433|471|    }
434|472|
435|473|    if file_counts.len() > MAX_FILES {
436|474|        result.push_str(&format!("\n… +{} more files\n", file_counts.len() - MAX_FILES));
437|475|        let all_file_lines = file_counts
438|476|            .iter()
439|477|            .map(|(file, count)| format!("{} ({} issues)", compact_path(file), count))
440|478|            .collect::<Vec<_>>()
441|479|            .join("\n");
442|480|        if let Some(hint) =
443|481|            crate::core::tee::force_tee_tail_hint(&all_file_lines, "pylint-files", MAX_FILES + 1)
444|482|        {
445|483|            result.push_str(&format!("  {}\n", hint));
446|484|        }
447|485|    }
448|486|
449|487|    result.trim().to_string()
450|488|}
451|489|
452|490|/// Filter generic linter output (fallback for non-ESLint linters)
453|491|fn filter_generic_lint(output: &str) -> String {
454|492|    let mut warnings = 0;
455|493|    let mut errors = 0;
456|494|    let mut issues: Vec<String> = Vec::new();
457|495|
458|496|    for line in output.lines() {
459|497|        let line_lower = line.to_lowercase();
460|498|        if line_lower.contains("warning") {
461|499|            warnings += 1;
462|500|            issues.push(line.to_string());
463|501|        }
464|502|        if line_lower.contains("error") && !line_lower.contains("0 error") {
465|503|            errors += 1;
466|504|            issues.push(line.to_string());
467|505|        }
468|506|    }
469|507|
470|508|    if errors == 0 && warnings == 0 {
471|509|        return "Lint: No issues found".to_string();
472|510|    }
473|511|
474|512|    let mut result = String::new();
475|513|    result.push_str(&format!("Lint: {} errors, {} warnings\n", errors, warnings));
476|514|    result.push_str("═══════════════════════════════════════\n");
477|515|
478|516|    const MAX_ISSUES: usize = CAP_ERRORS;
479|517|    for issue in issues.iter().take(MAX_ISSUES) {
480|518|        result.push_str(&format!("{}\n", truncate(issue, 100)));
481|519|    }
482|520|
483|521|    if issues.len() > MAX_ISSUES {
484|522|        result.push_str(&format!("\n… +{} more issues\n", issues.len() - MAX_ISSUES));
485|523|        let all_issues = issues.join("\n");
486|524|        if let Some(hint) =
487|525|            crate::core::tee::force_tee_tail_hint(&all_issues, "lint-issues", MAX_ISSUES + 1)
488|526|        {
489|527|            result.push_str(&format!("  {}\n", hint));
490|528|        }
491|529|    }
492|530|
493|531|    result.trim().to_string()
494|532|}
495|533|
496|534|/// Compact file path (remove common prefixes)
497|535|fn compact_path(path: &str) -> String {
498|536|    // Remove common prefixes like /Users/..., /home/..., C:\
499|537|    let path = path.replace('\\', "/");
500|538|
501|539|    if let Some(pos) = path.rfind("/src/") {
502|540|        format!("src/{}", &path[pos + 5..])
503|541|    } else if let Some(pos) = path.rfind("/lib/") {
504|542|        format!("lib/{}", &path[pos + 5..])
505|543|    } else if let Some(pos) = path.rfind('/') {
506|544|        path[pos + 1..].to_string()
507|545|    } else {
508|546|        path
509|547|    }
510|548|}
511|549|
512|550|#[cfg(test)]
513|551|mod tests {
514|552|    use super::*;
515|553|
516|554|    #[test]
517|555|    fn test_filter_eslint_json() {
518|556|        let json = r#"[
519|557|            {
520|558|                "filePath": "/Users/test/project/src/utils.ts",
521|559|                "messages": [
522|560|                    {
523|561|                        "ruleId": "prefer-const",
524|562|                        "severity": 1,
525|563|                        "message": "Use const instead of let",
526|564|                        "line": 10,
527|565|                        "column": 5
528|566|                    },
529|567|                    {
530|568|                        "ruleId": "prefer-const",
531|569|                        "severity": 1,
532|570|                        "message": "Use const instead of let",
533|571|                        "line": 15,
534|572|                        "column": 5
535|573|                    }
536|574|                ],
537|575|                "errorCount": 0,
538|576|                "warningCount": 2
539|577|            },
540|578|            {
541|579|                "filePath": "/Users/test/project/src/api.ts",
542|580|                "messages": [
543|581|                    {
544|582|                        "ruleId": "@typescript-eslint/no-unused-vars",
545|583|                        "severity": 2,
546|584|                        "message": "Variable x is unused",
547|585|                        "line": 20,
548|586|                        "column": 10
549|587|                    }
550|588|                ],
551|589|                "errorCount": 1,
552|590|                "warningCount": 0
553|591|            }
554|592|        ]"#;
555|593|
556|594|        let result = filter_eslint_json(json);
557|595|        assert!(result.contains("ESLint:"));
558|596|        assert!(result.contains("prefer-const"));
559|597|        assert!(result.contains("no-unused-vars"));
560|598|        assert!(result.contains("src/utils.ts"));
561|599|    }
562|600|
563|601|    #[test]
564|602|    fn test_compact_path() {
565|603|        assert_eq!(
566|604|            compact_path("/Users/foo/project/src/utils.ts"),
567|605|            "src/utils.ts"
568|606|        );
569|607|        assert_eq!(
570|608|            compact_path("C:\\Users\\project\\src\\api.ts"),
571|609|            "src/api.ts"
572|610|        );
573|611|        assert_eq!(compact_path("simple.ts"), "simple.ts");
574|612|    }
575|613|
576|614|    #[test]
577|615|    fn test_filter_pylint_json_no_issues() {
578|616|        let output = "[]";
579|617|        let result = filter_pylint_json(output);
580|618|        assert!(result.contains("Pylint"));
581|619|        assert!(result.contains("No issues found"));
582|620|    }
583|621|
584|622|    #[test]
585|623|    fn test_filter_pylint_json_with_issues() {
586|624|        let json = r#"[
587|625|            {
588|626|                "type": "warning",
589|627|                "module": "main",
590|628|                "obj": "",
591|629|                "line": 10,
592|630|                "column": 0,
593|631|                "path": "src/main.py",
594|632|                "symbol": "unused-variable",
595|633|                "message": "Unused variable 'x'",
596|634|                "message-id": "W0612"
597|635|            },
598|636|            {
599|637|                "type": "warning",
600|638|                "module": "main",
601|639|                "obj": "foo",
602|640|                "line": 15,
603|641|                "column": 4,
604|642|                "path": "src/main.py",
605|643|                "symbol": "unused-variable",
606|644|                "message": "Unused variable 'y'",
607|645|                "message-id": "W0612"
608|646|            },
609|647|            {
610|648|                "type": "error",
611|649|                "module": "utils",
612|650|                "obj": "bar",
613|651|                "line": 20,
614|652|                "column": 0,
615|653|                "path": "src/utils.py",
616|654|                "symbol": "undefined-variable",
617|655|                "message": "Undefined variable 'z'",
618|656|                "message-id": "E0602"
619|657|            }
620|658|        ]"#;
621|659|
622|660|        let result = filter_pylint_json(json);
623|661|        assert!(result.contains("3 issues"));
624|662|        assert!(result.contains("2 files"));
625|663|        assert!(result.contains("1 errors, 2 warnings"));
626|664|        assert!(result.contains("unused-variable (W0612)"));
627|665|        assert!(result.contains("undefined-variable (E0602)"));
628|666|        assert!(result.contains("main.py"));
629|667|        assert!(result.contains("utils.py"));
630|668|    }
631|669|
632|670|    #[test]
633|671|    fn test_strip_pm_prefix_npx() {
634|672|        let args: Vec<String> = vec!["npx".into(), "eslint".into(), "src/".into()];
635|673|        assert_eq!(strip_pm_prefix(&args), 1);
636|674|    }
637|675|
638|676|    #[test]
639|677|    fn test_strip_pm_prefix_bunx() {
640|678|        let args: Vec<String> = vec!["bunx".into(), "eslint".into(), ".".into()];
641|679|        assert_eq!(strip_pm_prefix(&args), 1);
642|680|    }
643|681|
644|682|    #[test]
645|683|    fn test_strip_pm_prefix_pnpm_exec() {
646|684|        let args: Vec<String> = vec!["pnpm".into(), "exec".into(), "eslint".into()];
647|685|        assert_eq!(strip_pm_prefix(&args), 2);
648|686|    }
649|687|
650|688|    #[test]
651|689|    fn test_strip_pm_prefix_none() {
652|690|        let args: Vec<String> = vec!["eslint".into(), "src/".into()];
653|691|        assert_eq!(strip_pm_prefix(&args), 0);
654|692|    }
655|693|
656|694|    #[test]
657|695|    fn test_strip_pm_prefix_empty() {
658|696|        let args: Vec<String> = vec![];
659|697|        assert_eq!(strip_pm_prefix(&args), 0);
660|698|    }
661|699|
662|700|    #[test]
663|701|    fn test_detect_linter_eslint() {
664|702|        let args: Vec<String> = vec!["eslint".into(), "src/".into()];
665|703|        let (linter, explicit) = detect_linter(&args);
666|704|        assert_eq!(linter, "eslint");
667|705|        assert!(explicit);
668|706|    }
669|707|
670|708|    #[test]
671|709|    fn test_detect_linter_default_on_path() {
672|710|        let args: Vec<String> = vec!["src/".into()];
673|711|        let (linter, explicit) = detect_linter(&args);
674|712|        assert_eq!(linter, "eslint");
675|713|        assert!(!explicit);
676|714|    }
677|715|
678|716|    #[test]
679|717|    fn test_detect_linter_default_on_flag() {
680|718|        let args: Vec<String> = vec!["--max-warnings=0".into()];
681|719|        let (linter, explicit) = detect_linter(&args);
682|720|        assert_eq!(linter, "eslint");
683|721|        assert!(!explicit);
684|722|    }
685|723|
686|724|    #[test]
687|725|    fn test_detect_linter_after_npx_strip() {
688|726|        // Simulates: rtk lint npx eslint src/ → after strip_pm_prefix, args = ["eslint", "src/"]
689|727|        let full_args: Vec<String> = vec!["npx".into(), "eslint".into(), "src/".into()];
690|728|        let skip = strip_pm_prefix(&full_args);
691|729|        let effective = &full_args[skip..];
692|730|        let (linter, _) = detect_linter(effective);
693|731|        assert_eq!(linter, "eslint");
694|732|    }
695|733|
696|734|    #[test]
697|735|    fn test_detect_linter_after_pnpm_exec_strip() {
698|736|        let full_args: Vec<String> =
699|737|            vec!["pnpm".into(), "exec".into(), "biome".into(), "check".into()];
700|738|        let skip = strip_pm_prefix(&full_args);
701|739|        let effective = &full_args[skip..];
702|740|        let (linter, _) = detect_linter(effective);
703|741|        assert_eq!(linter, "biome");
704|742|    }
705|743|
706|744|    #[test]
707|745|    fn test_is_python_linter() {
708|746|        assert!(is_python_linter("ruff"));
709|747|        assert!(is_python_linter("pylint"));
710|748|        assert!(is_python_linter("mypy"));
711|749|        assert!(is_python_linter("flake8"));
712|750|        assert!(!is_python_linter("eslint"));
713|751|        assert!(!is_python_linter("biome"));
714|752|        assert!(!is_python_linter("unknown"));
715|753|    }
716|754|}
717|755|
718|1404|