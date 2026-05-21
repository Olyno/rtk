1|2|1|//! Filters pytest output to show only failures and the summary line.
3|2|
4|3|use crate::core::runner;
5|4|use crate::core::truncate::CAP_WARNINGS;
6|5|use crate::core::utils::{resolved_command, tool_exists, truncate};
7|6|use anyhow::Result;
8|7|
9|8|const MAX_XFAIL: usize = CAP_WARNINGS;
10|9|const MAX_PYTEST_FAILURES: usize = CAP_WARNINGS;
11|10|
12|11|#[derive(Debug, PartialEq)]
13|12|enum ParseState {
14|13|    Header,
15|14|    TestProgress,
16|15|    Failures,
17|16|    Summary,
18|17|}
19|18|
20|19|pub fn run(args: &[String], verbose: u8) -> Result<i32> {
21|20|    let mut cmd = if tool_exists("pytest") {
22|21|        resolved_command("pytest")
23|22|    } else {
24|23|        let mut c = resolved_command("python");
25|24|        c.arg("-m").arg("pytest");
26|25|        c
27|26|    };
28|27|
29|28|    let has_tb_flag = args.iter().any(|a| a.starts_with("--tb"));
30|29|    let has_quiet_flag = args.iter().any(|a| a == "-q" || a == "--quiet");
31|30|    // Only treat a short `-r…` as pytest's report flag (not `--randomly-seed` etc.)
32|31|    let has_report_flag = args.iter().any(|a| a.starts_with("-r") && !a.starts_with("--"));
33|32|
34|33|    if !has_tb_flag {
35|34|        cmd.arg("--tb=short");
36|35|    }
37|36|    if !has_quiet_flag {
38|37|        cmd.arg("-q");
39|38|    }
40|39|    // Surface xfailed/xpassed (and their reasons) in the short summary section
41|40|    // so the compact output can report expected failures and — crucially —
42|41|    // unexpected passes (XPASS), which signal a behavior change.
43|42|    if !has_report_flag {
44|43|        cmd.arg("-rxX");
45|44|    }
46|45|
47|46|    for arg in args {
48|47|        cmd.arg(arg);
49|48|    }
50|49|
51|50|    if verbose > 0 {
52|51|        eprintln!("Running: pytest --tb=short -q {}", args.join(" "));
53|52|    }
54|53|
55|54|    runner::run_filtered(
56|55|        cmd,
57|56|        "pytest",
58|57|        &args.join(" "),
59|58|        filter_pytest_output,
60|59|        runner::RunOptions::stdout_only().tee("pytest"),
61|60|    )
62|61|}
63|62|
64|63|pub(crate) fn filter_pytest_output(output: &str) -> String {
65|64|    let mut state = ParseState::Header;
66|65|    let mut test_files: Vec<String> = Vec::new();
67|66|    let mut failures: Vec<String> = Vec::new();
68|67|    let mut current_failure: Vec<String> = Vec::new();
69|68|    let mut xfail_lines: Vec<String> = Vec::new();
70|69|    let mut summary_line = String::new();
71|70|
72|71|    for line in output.lines() {
73|72|        let trimmed = line.trim();
74|73|
75|74|        // State transitions
76|75|        if trimmed.starts_with("===") && trimmed.contains("test session starts") {
77|76|            state = ParseState::Header;
78|77|            continue;
79|78|        } else if trimmed.starts_with("===") && trimmed.contains("FAILURES") {
80|79|            state = ParseState::Failures;
81|80|            continue;
82|81|        } else if trimmed.starts_with("===") && trimmed.contains("short test summary") {
83|82|            state = ParseState::Summary;
84|83|            // Save current failure if any
85|84|            if !current_failure.is_empty() {
86|85|                failures.push(current_failure.join("\n"));
87|86|                current_failure.clear();
88|87|            }
89|88|            continue;
90|89|        } else if trimmed.starts_with("===")
91|90|            && (trimmed.contains("passed")
92|91|                || trimmed.contains("failed")
93|92|                || trimmed.contains("skipped"))
94|93|        {
95|94|            summary_line = trimmed.to_string();
96|95|            continue;
97|96|        // quiet mode (-q): bare summary without === wrapper, e.g. "5 failed, 1698 passed, 2 skipped in 108.89s"
98|97|        } else if summary_line.is_empty()
99|98|            && !trimmed.starts_with("===")
100|99|            && !trimmed.starts_with("FAILED")
101|100|            && !trimmed.starts_with("ERROR")
102|101|            && (trimmed.contains(" passed")
103|102|                || trimmed.contains(" failed")
104|103|                || trimmed.contains(" skipped"))
105|104|            && trimmed.contains(" in ")
106|105|        {
107|106|            summary_line = trimmed.to_string();
108|107|            continue;
109|108|        }
110|109|
111|110|        // Process based on state
112|111|        match state {
113|112|            ParseState::Header => {
114|113|                if trimmed.starts_with("collected") {
115|114|                    state = ParseState::TestProgress;
116|115|                }
117|116|            }
118|117|            ParseState::TestProgress => {
119|118|                // Lines like "tests/test_foo.py ....  [ 40%]"
120|119|                if !trimmed.is_empty()
121|120|                    && !trimmed.starts_with("===")
122|121|                    && (trimmed.contains(".py") || trimmed.contains("%]"))
123|122|                {
124|123|                    test_files.push(trimmed.to_string());
125|124|                }
126|125|            }
127|126|            ParseState::Failures => {
128|127|                // Collect failure details
129|128|                if trimmed.starts_with("___") {
130|129|                    // New failure section
131|130|                    if !current_failure.is_empty() {
132|131|                        failures.push(current_failure.join("\n"));
133|132|                        current_failure.clear();
134|133|                    }
135|134|                    current_failure.push(trimmed.to_string());
136|135|                } else if !trimmed.is_empty() && !trimmed.starts_with("===") {
137|136|                    current_failure.push(trimmed.to_string());
138|137|                }
139|138|            }
140|139|            ParseState::Summary => {
141|140|                // FAILED test lines
142|141|                if trimmed.starts_with("FAILED") || trimmed.starts_with("ERROR") {
143|142|                    failures.push(trimmed.to_string());
144|143|                } else if trimmed.starts_with("XFAIL") || trimmed.starts_with("XPASS") {
145|144|145|148|                    xfail_lines.push(trimmed.to_string());
146|149|                }
147|150|            }
148|151|        }
149|152|    }
150|153|
151|154|    // Save last failure if any
152|155|    if !current_failure.is_empty() {
153|156|        failures.push(current_failure.join("\n"));
154|157|    }
155|158|
156|159|    // Build compact output
157|160|    build_pytest_summary(&summary_line, &test_files, &failures, &xfail_lines)
158|161|}
159|162|
160|163|#[derive(Default)]
161|164|struct PytestCounts {
162|165|    passed: usize,
163|166|    failed: usize,
164|167|    skipped: usize,
165|168|    xfailed: usize,
166|169|    xpassed: usize,
167|170|}
168|171|
169|172|fn build_pytest_summary(
170|173|    summary: &str,
171|174|    _test_files: &[String],
172|175|    failures: &[String],
173|176|    xfail_lines: &[String],
174|177|) -> String {
175|178|    let counts = parse_summary_line(summary);
176|179|    let PytestCounts {
177|180|        passed,
178|181|        failed,
179|182|        skipped,
180|183|        xfailed,
181|184|        xpassed,
182|185|    } = counts;
183|186|
184|187|    if passed == 0 && failed == 0 && skipped == 0 && xfailed == 0 && xpassed == 0 {
185|188|        return "Pytest: No tests collected".to_string();
186|189|    }
187|190|
188|191|    let extras_present = skipped > 0 || xfailed > 0 || xpassed > 0 || !xfail_lines.is_empty();
189|192|
190|193|    if failed == 0 && passed > 0 && !extras_present {
191|194|        return format!("Pytest: {} passed", passed);
192|195|    }
193|196|
194|197|    let mut result = String::new();
195|198|    result.push_str(&format!("Pytest: {} passed, {} failed", passed, failed));
196|199|    if skipped > 0 {
197|200|        result.push_str(&format!(", {} skipped", skipped));
198|201|    }
199|202|    if xfailed > 0 {
200|203|        result.push_str(&format!(", {} xfailed", xfailed));
201|204|    }
202|205|    if xpassed > 0 {
203|206|        result.push_str(&format!(", {} xpassed", xpassed));
204|207|    }
205|208|    result.push('\n');
206|209|    result.push_str("═══════════════════════════════════════\n");
207|210|
208|211|    // Surface xfail/xpass entries (with their reasons) — XPASS in particular
209|212|    // signals that something expected-to-fail now passes.
210|213|    if !xfail_lines.is_empty() {
211|214|        result.push_str("\nExpected-failure outcomes:\n");
212|215|216|        for line in xfail_lines.iter().take(MAX_XFAIL) {
213|217|            result.push_str(&format!("  {}\n", truncate(line, 120)));
214|218|        }
215|219|        if xfail_lines.len() > MAX_XFAIL {
216|220|            result.push_str(&format!("  … +{} more\n", xfail_lines.len() - MAX_XFAIL));
217|221|            let all_xfail = xfail_lines.join("\n");
218|222|            if let Some(hint) = crate::core::tee::force_tee_tail_hint(&all_xfail, "pytest-xfail", MAX_XFAIL + 1) {
219|223|                result.push_str(&format!("  {}\n", hint));
220|224|            }
221|225|232|        }
222|233|    }
223|234|
224|235|    if failures.is_empty() {
225|236|        return result.trim().to_string();
226|237|    }
227|238|
228|239|    // Show failures (limit to key information)
229|240|    result.push_str("\nFailures:\n");
230|241|
231|242|    for (i, failure) in failures.iter().take(MAX_PYTEST_FAILURES).enumerate() {
232|243|        // Extract test name and key error info
233|244|        let lines: Vec<&str> = failure.lines().collect();
234|245|
235|246|        // First line is usually test name (after ___)
236|247|        if let Some(first_line) = lines.first() {
237|248|            if first_line.starts_with("___") {
238|249|                // Extract test name between ___
239|250|                let test_name = first_line.trim_matches('_').trim();
240|251|                result.push_str(&format!("{}. [FAIL] {}\n", i + 1, test_name));
241|252|            } else if first_line.starts_with("FAILED") {
242|253|                // Summary format: "FAILED tests/test_foo.py::test_bar - AssertionError"
243|254|                let parts: Vec<&str> = first_line.split(" - ").collect();
244|255|                if let Some(test_path) = parts.first() {
245|256|                    let test_name = test_path.trim_start_matches("FAILED ");
246|257|                    result.push_str(&format!("{}. [FAIL] {}\n", i + 1, test_name));
247|258|                }
248|259|                if parts.len() > 1 {
249|260|                    result.push_str(&format!("     {}\n", truncate(parts[1], 100)));
250|261|                }
251|262|                continue;
252|263|            }
253|264|        }
254|265|
255|266|        // Show relevant error lines (assertions, errors, file locations)
256|267|        let mut relevant_lines = 0;
257|268|        for line in &lines[1..] {
258|269|            let line_lower = line.to_lowercase();
259|270|            let is_relevant = line.trim().starts_with('>')
260|271|                || line.trim().starts_with('E')
261|272|                || line_lower.contains("assert")
262|273|                || line_lower.contains("error")
263|274|                || line.contains(".py:");
264|275|
265|276|            if is_relevant && relevant_lines < 3 {
266|277|                result.push_str(&format!("     {}\n", truncate(line, 100)));
267|278|                relevant_lines += 1;
268|279|            }
269|280|        }
270|281|
271|282|        if i < failures.len() - 1 {
272|283|            result.push('\n');
273|284|        }
274|285|    }
275|286|
276|287|    if failures.len() > MAX_PYTEST_FAILURES {
277|288|        result.push_str(&format!(
278|289|            "\n… +{} more failures\n",
279|290|            failures.len() - MAX_PYTEST_FAILURES
280|291|        ));
281|292|        let all_failures = failures.join("\n\n");
282|293|        if let Some(hint) = crate::core::tee::force_tee_hint(&all_failures, "pytest-failures") {
283|294|            result.push_str(&format!("  {}\n", hint));
284|295|        }
285|296|    }
286|297|
287|298|    result.trim().to_string()
288|299|}
289|300|
290|301|fn parse_summary_line(summary: &str) -> PytestCounts {
291|302|    let mut counts = PytestCounts::default();
292|303|
293|304|    // Parse lines like "=== 4 passed, 1 failed, 2 xfailed, 1 xpassed in 0.50s ==="
294|305|    for part in summary.split(',') {
295|306|        let words: Vec<&str> = part.split_whitespace().collect();
296|307|        for (i, word) in words.iter().enumerate() {
297|308|            if i == 0 {
298|309|                continue;
299|310|            }
300|311|            let Ok(n) = words[i - 1].parse::<usize>() else {
301|312|                continue;
302|313|            };
303|314|            // Order matters: "xpassed"/"xfailed" contain "passed"/"failed".
304|315|            if word.contains("xpassed") {
305|316|                counts.xpassed = n;
306|317|            } else if word.contains("xfailed") {
307|318|                counts.xfailed = n;
308|319|            } else if word.contains("passed") {
309|320|                counts.passed = n;
310|321|            } else if word.contains("failed") {
311|322|                counts.failed = n;
312|323|            } else if word.contains("skipped") {
313|324|                counts.skipped = n;
314|325|            }
315|326|        }
316|327|    }
317|328|
318|329|    counts
319|330|}
320|331|
321|332|#[cfg(test)]
322|333|mod tests {
323|334|    use super::*;
324|335|
325|336|    #[test]
326|337|    fn test_filter_pytest_all_pass() {
327|338|        let output = r#"=== test session starts ===
328|339|platform darwin -- Python 3.11.0
329|340|collected 5 items
330|341|
331|342|tests/test_foo.py .....                                            [100%]
332|343|
333|344|=== 5 passed in 0.50s ==="#;
334|345|
335|346|        let result = filter_pytest_output(output);
336|347|        assert!(result.contains("Pytest"));
337|348|        assert!(result.contains("5 passed"));
338|349|    }
339|350|
340|351|    #[test]
341|352|    fn test_filter_pytest_with_failures() {
342|353|        let output = r#"=== test session starts ===
343|354|collected 5 items
344|355|
345|356|tests/test_foo.py ..F..                                            [100%]
346|357|
347|358|=== FAILURES ===
348|359|___ test_something ___
349|360|
350|361|    def test_something():
351|362|>       assert False
352|363|E       assert False
353|364|
354|365|tests/test_foo.py:10: AssertionError
355|366|
356|367|=== short test summary info ===
357|368|FAILED tests/test_foo.py::test_something - assert False
358|369|=== 4 passed, 1 failed in 0.50s ==="#;
359|370|
360|371|        let result = filter_pytest_output(output);
361|372|        assert!(result.contains("4 passed, 1 failed"));
362|373|        assert!(result.contains("test_something"));
363|374|        assert!(result.contains("assert False"));
364|375|    }
365|376|
366|377|    #[test]
367|378|    fn test_filter_pytest_multiple_failures() {
368|379|        let output = r#"=== test session starts ===
369|380|collected 3 items
370|381|
371|382|tests/test_foo.py FFF                                              [100%]
372|383|
373|384|=== FAILURES ===
374|385|___ test_one ___
375|386|E   AssertionError: expected 5
376|387|
377|388|___ test_two ___
378|389|E   ValueError: invalid value
379|390|
380|391|=== short test summary info ===
381|392|FAILED tests/test_foo.py::test_one - AssertionError: expected 5
382|393|FAILED tests/test_foo.py::test_two - ValueError: invalid value
383|394|FAILED tests/test_foo.py::test_three - KeyError
384|395|=== 3 failed in 0.20s ==="#;
385|396|
386|397|        let result = filter_pytest_output(output);
387|398|        assert!(result.contains("3 failed"));
388|399|        assert!(result.contains("test_one"));
389|400|        assert!(result.contains("test_two"));
390|401|        assert!(result.contains("expected 5"));
391|402|    }
392|403|
393|404|    #[test]
394|405|    fn test_filter_pytest_no_tests() {
395|406|        let output = r#"=== test session starts ===
396|407|collected 0 items
397|408|
398|409|=== no tests ran in 0.00s ==="#;
399|410|
400|411|        let result = filter_pytest_output(output);
401|412|        assert!(result.contains("No tests collected"));
402|413|    }
403|414|
404|415|    #[test]
405|416|    fn test_parse_summary_line() {
406|417|        let c = parse_summary_line("=== 5 passed in 0.50s ===");
407|418|        assert_eq!((c.passed, c.failed, c.skipped), (5, 0, 0));
408|419|
409|420|        let c = parse_summary_line("=== 4 passed, 1 failed in 0.50s ===");
410|421|        assert_eq!((c.passed, c.failed, c.skipped), (4, 1, 0));
411|422|
412|423|        let c = parse_summary_line("=== 3 passed, 1 failed, 2 skipped in 1.0s ===");
413|424|        assert_eq!((c.passed, c.failed, c.skipped), (3, 1, 2));
414|425|
415|426|        let c = parse_summary_line("=== 2 passed, 1 failed, 2 xfailed, 1 xpassed in 1.0s ===");
416|427|        assert_eq!(
417|428|            (c.passed, c.failed, c.xfailed, c.xpassed),
418|429|            (2, 1, 2, 1)
419|430|431|        );
420|432|    }
421|433|
422|434|    #[test]
423|435|    fn test_filter_pytest_xfail_caps_and_tee_hint() {
424|436|        let mut lines = String::from("=== test session starts ===\ncollected 30 items\n\n");
425|437|        lines.push_str("test_x.py ");
426|438|        for _ in 0..15 {
427|439|            lines.push('x');
428|440|        }
429|441|        lines.push_str("\n\n=== short test summary info ===\n");
430|442|        for i in 0..15 {
431|443|            lines.push_str(&format!(
432|444|                "XFAIL test_x.py::test_case_{i} - known issue #{i}\n"
433|445|            ));
434|446|        }
435|447|        lines.push_str("=== 0 passed, 15 xfailed in 0.05s ===\n");
436|448|
437|449|        let result = filter_pytest_output(&lines);
438|450|        let xfail_in_section = result
439|451|            .split("Expected-failure outcomes:")
440|452|            .nth(1)
441|453|            .unwrap_or("");
442|454|        let listed = xfail_in_section
443|455|            .lines()
444|456|            .filter(|l| l.trim().starts_with("XFAIL"))
445|457|            .count();
446|458|        assert!(
447|459|            listed <= 10,
448|460|            "MAX_XFAIL cap not enforced: listed {listed}"
449|461|463|        );
450|464|        assert!(result.contains("… +5 more"), "missing '+N more': {result}");
451|465|    }
452|466|
453|467|    #[test]
454|468|    fn test_filter_pytest_xfail_xpass() {
455|469|        let output = r#"=== test session starts ===
456|470|collected 5 items
457|471|
458|472|test_math.py ..xxX                                                 [100%]
459|473|
460|474|=== short test summary info ===
461|475|XFAIL test_math.py::test_division_by_zero - known bug in division
462|476|XFAIL test_math.py::test_float_precision - float precision issue — bug #42
463|477|XPASS test_math.py::test_unexpected_pass - this should fail but currently passes
464|478|=== 2 passed, 2 xfailed, 1 xpassed in 0.05s ==="#;
465|479|
466|480|        let result = filter_pytest_output(output);
467|481|        assert!(result.contains("xfailed"), "got: {result}");
468|482|        assert!(result.contains("xpassed"), "got: {result}");
469|483|        assert!(result.contains("XPASS"), "got: {result}");
470|484|        assert!(result.contains("float precision"), "got: {result}");
471|485|        assert!(result.contains("test_division_by_zero"), "got: {result}");
472|486|    }
473|487|
474|488|    #[test]
475|489|    fn test_filter_pytest_xfail_xpass() {
476|490|        let output = r#"=== test session starts ===
477|491|collected 5 items
478|492|
479|493|test_math.py ..xxX                                                 [100%]
480|494|
481|495|=== short test summary info ===
482|496|XFAIL test_math.py::test_division_by_zero - known bug in division
483|497|XFAIL test_math.py::test_float_precision - float precision issue — bug #42
484|498|XPASS test_math.py::test_unexpected_pass - this should fail but currently passes
485|499|=== 2 passed, 2 xfailed, 1 xpassed in 0.05s ==="#;
486|500|
487|501|        let result = filter_pytest_output(output);
488|502|        assert!(result.contains("xfailed"), "got: {result}");
489|503|        assert!(result.contains("xpassed"), "got: {result}");
490|504|        assert!(result.contains("XPASS"), "got: {result}");
491|505|        assert!(result.contains("float precision"), "got: {result}");
492|506|        assert!(result.contains("test_division_by_zero"), "got: {result}");
493|507|    }
494|508|
495|509|    #[test]
496|510|    fn test_filter_pytest_quiet_mode_failures() {
497|511|        // In -q mode, the final summary line has NO === wrapper
498|512|        // This was causing "No tests collected" to be reported incorrectly
499|513|        let output = r#"=== test session starts ===
500|514|platform linux -- Python 3.12.11, pytest-8.1.0
501|515|collected 1705 items
502|516|
503|517|.......F.......
504|518|
505|519|=== FAILURES ===
506|520|___ test_something ___
507|521|
508|522|E   AssertionError: expected True
509|523|
510|524|=== short test summary info ===
511|525|FAILED tests/test_foo.py::test_something - AssertionError
512|526|5 failed, 1698 passed, 2 skipped in 108.89s"#;
513|527|
514|528|        let result = filter_pytest_output(output);
515|529|        assert!(
516|530|            !result.contains("No tests collected"),
517|531|            "Should not report 'No tests collected' when tests ran. Got: {}",
518|532|            result
519|533|        );
520|534|        assert!(
521|535|            result.contains("1698") || result.contains("5 failed"),
522|536|            "Should show actual test counts. Got: {}",
523|537|            result
524|538|        );
525|539|    }
526|540|
527|541|    #[test]
528|542|    fn test_filter_pytest_only_skipped() {
529|543|        // If only skipped tests, should NOT say "No tests collected"
530|544|        let output = r#"=== test session starts ===
531|545|collected 3 items
532|546|
533|547|=== 3 skipped in 0.10s ==="#;
534|548|
535|549|        let result = filter_pytest_output(output);
536|550|        assert!(
537|551|            !result.contains("No tests collected"),
538|552|            "Should not say 'No tests collected' when tests were skipped. Got: {}",
539|553|            result
540|554|        );
541|555|    }
542|556|}
543|557|
544|1057|