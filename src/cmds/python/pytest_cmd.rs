<<<<<<< HEAD
1|//! Filters pytest output to show only failures and the summary line.
2|
3|use crate::core::runner;
4|use crate::core::truncate::CAP_WARNINGS;
5|use crate::core::utils::{resolved_command, tool_exists, truncate};
6|use anyhow::Result;
7|
8|const MAX_XFAIL: usize = CAP_WARNINGS;
9|const MAX_PYTEST_FAILURES: usize = CAP_WARNINGS;
10|
11|#[derive(Debug, PartialEq)]
12|enum ParseState {
13|    Header,
14|    TestProgress,
15|    Failures,
16|    Summary,
17|}
18|
19|pub fn run(args: &[String], verbose: u8) -> Result<i32> {
20|    let mut cmd = if tool_exists("pytest") {
21|        resolved_command("pytest")
22|    } else {
23|        let mut c = resolved_command("python");
24|        c.arg("-m").arg("pytest");
25|        c
26|    };
27|
28|    let has_tb_flag = args.iter().any(|a| a.starts_with("--tb"));
29|    let has_quiet_flag = args.iter().any(|a| a == "-q" || a == "--quiet");
30|    // Only treat a short `-r…` as pytest's report flag (not `--randomly-seed` etc.)
31|    let has_report_flag = args.iter().any(|a| a.starts_with("-r") && !a.starts_with("--"));
32|
33|    if !has_tb_flag {
34|        cmd.arg("--tb=short");
35|    }
36|    if !has_quiet_flag {
37|        cmd.arg("-q");
38|    }
39|    // Surface xfailed/xpassed (and their reasons) in the short summary section
40|    // so the compact output can report expected failures and — crucially —
41|    // unexpected passes (XPASS), which signal a behavior change.
42|    if !has_report_flag {
43|        cmd.arg("-rxX");
44|    }
45|
46|    for arg in args {
47|        cmd.arg(arg);
48|    }
49|
50|    if verbose > 0 {
51|        eprintln!("Running: pytest --tb=short -q {}", args.join(" "));
52|    }
53|
54|    runner::run_filtered(
55|        cmd,
56|        "pytest",
57|        &args.join(" "),
58|        filter_pytest_output,
59|        runner::RunOptions::stdout_only().tee("pytest"),
60|    )
61|}
62|
63|pub(crate) fn filter_pytest_output(output: &str) -> String {
64|    let mut state = ParseState::Header;
65|    let mut test_files: Vec<String> = Vec::new();
66|    let mut failures: Vec<String> = Vec::new();
67|    let mut current_failure: Vec<String> = Vec::new();
68|    let mut xfail_lines: Vec<String> = Vec::new();
69|    let mut summary_line = String::new();
70|
71|    for line in output.lines() {
72|        let trimmed = line.trim();
73|
74|        // State transitions
75|        if trimmed.starts_with("===") && trimmed.contains("test session starts") {
76|            state = ParseState::Header;
77|            continue;
78|        } else if trimmed.starts_with("===") && trimmed.contains("FAILURES") {
79|            state = ParseState::Failures;
80|            continue;
81|        } else if trimmed.starts_with("===") && trimmed.contains("short test summary") {
82|            state = ParseState::Summary;
83|            // Save current failure if any
84|            if !current_failure.is_empty() {
85|                failures.push(current_failure.join("\n"));
86|                current_failure.clear();
87|            }
88|            continue;
89|        } else if trimmed.starts_with("===")
90|            && (trimmed.contains("passed")
91|                || trimmed.contains("failed")
92|                || trimmed.contains("skipped"))
93|        {
94|            summary_line = trimmed.to_string();
95|            continue;
96|        // quiet mode (-q): bare summary without === wrapper, e.g. "5 failed, 1698 passed, 2 skipped in 108.89s"
97|        } else if summary_line.is_empty()
98|            && !trimmed.starts_with("===")
99|            && !trimmed.starts_with("FAILED")
100|            && !trimmed.starts_with("ERROR")
101|            && (trimmed.contains(" passed")
102|                || trimmed.contains(" failed")
103|                || trimmed.contains(" skipped"))
104|            && trimmed.contains(" in ")
105|        {
106|            summary_line = trimmed.to_string();
107|            continue;
108|        }
109|
110|        // Process based on state
111|        match state {
112|            ParseState::Header => {
113|                if trimmed.starts_with("collected") {
114|                    state = ParseState::TestProgress;
115|                }
116|            }
117|            ParseState::TestProgress => {
118|                // Lines like "tests/test_foo.py ....  [ 40%]"
119|                if !trimmed.is_empty()
120|                    && !trimmed.starts_with("===")
121|                    && (trimmed.contains(".py") || trimmed.contains("%]"))
122|                {
123|                    test_files.push(trimmed.to_string());
124|                }
125|            }
126|            ParseState::Failures => {
127|                // Collect failure details
128|                if trimmed.starts_with("___") {
129|                    // New failure section
130|                    if !current_failure.is_empty() {
131|                        failures.push(current_failure.join("\n"));
132|                        current_failure.clear();
133|                    }
134|                    current_failure.push(trimmed.to_string());
135|                } else if !trimmed.is_empty() && !trimmed.starts_with("===") {
136|                    current_failure.push(trimmed.to_string());
137|                }
138|            }
139|            ParseState::Summary => {
140|                // FAILED test lines
141|                if trimmed.starts_with("FAILED") || trimmed.starts_with("ERROR") {
142|                    failures.push(trimmed.to_string());
143|                } else if trimmed.starts_with("XFAIL") || trimmed.starts_with("XPASS") {
144|145|148|                    xfail_lines.push(trimmed.to_string());
149|                }
150|            }
151|        }
152|    }
153|
154|    // Save last failure if any
155|    if !current_failure.is_empty() {
156|        failures.push(current_failure.join("\n"));
157|    }
158|
159|    // Build compact output
160|    build_pytest_summary(&summary_line, &test_files, &failures, &xfail_lines)
161|}
162|
163|#[derive(Default)]
164|struct PytestCounts {
165|    passed: usize,
166|    failed: usize,
167|    skipped: usize,
168|    xfailed: usize,
169|    xpassed: usize,
170|}
171|
172|fn build_pytest_summary(
173|    summary: &str,
174|    _test_files: &[String],
175|    failures: &[String],
176|    xfail_lines: &[String],
177|) -> String {
178|    let counts = parse_summary_line(summary);
179|    let PytestCounts {
180|        passed,
181|        failed,
182|        skipped,
183|        xfailed,
184|        xpassed,
185|    } = counts;
186|
187|    if passed == 0 && failed == 0 && skipped == 0 && xfailed == 0 && xpassed == 0 {
188|        return "Pytest: No tests collected".to_string();
189|    }
190|
191|    let extras_present = skipped > 0 || xfailed > 0 || xpassed > 0 || !xfail_lines.is_empty();
192|
193|    if failed == 0 && passed > 0 && !extras_present {
194|        return format!("Pytest: {} passed", passed);
195|    }
196|
197|    let mut result = String::new();
198|    result.push_str(&format!("Pytest: {} passed, {} failed", passed, failed));
199|    if skipped > 0 {
200|        result.push_str(&format!(", {} skipped", skipped));
201|    }
202|    if xfailed > 0 {
203|        result.push_str(&format!(", {} xfailed", xfailed));
204|    }
205|    if xpassed > 0 {
206|        result.push_str(&format!(", {} xpassed", xpassed));
207|    }
208|    result.push('\n');
209|    result.push_str("═══════════════════════════════════════\n");
210|
211|    // Surface xfail/xpass entries (with their reasons) — XPASS in particular
212|    // signals that something expected-to-fail now passes.
213|    if !xfail_lines.is_empty() {
214|        result.push_str("\nExpected-failure outcomes:\n");
215|216|        for line in xfail_lines.iter().take(MAX_XFAIL) {
217|            result.push_str(&format!("  {}\n", truncate(line, 120)));
218|        }
219|        if xfail_lines.len() > MAX_XFAIL {
220|            result.push_str(&format!("  … +{} more\n", xfail_lines.len() - MAX_XFAIL));
221|            let all_xfail = xfail_lines.join("\n");
222|            if let Some(hint) = crate::core::tee::force_tee_tail_hint(&all_xfail, "pytest-xfail", MAX_XFAIL + 1) {
223|                result.push_str(&format!("  {}\n", hint));
224|            }
225|232|        }
233|    }
234|
235|    if failures.is_empty() {
236|        return result.trim().to_string();
237|    }
238|
239|    // Show failures (limit to key information)
240|    result.push_str("\nFailures:\n");
241|
242|    for (i, failure) in failures.iter().take(MAX_PYTEST_FAILURES).enumerate() {
243|        // Extract test name and key error info
244|        let lines: Vec<&str> = failure.lines().collect();
245|
246|        // First line is usually test name (after ___)
247|        if let Some(first_line) = lines.first() {
248|            if first_line.starts_with("___") {
249|                // Extract test name between ___
250|                let test_name = first_line.trim_matches('_').trim();
251|                result.push_str(&format!("{}. [FAIL] {}\n", i + 1, test_name));
252|            } else if first_line.starts_with("FAILED") {
253|                // Summary format: "FAILED tests/test_foo.py::test_bar - AssertionError"
254|                let parts: Vec<&str> = first_line.split(" - ").collect();
255|                if let Some(test_path) = parts.first() {
256|                    let test_name = test_path.trim_start_matches("FAILED ");
257|                    result.push_str(&format!("{}. [FAIL] {}\n", i + 1, test_name));
258|                }
259|                if parts.len() > 1 {
260|                    result.push_str(&format!("     {}\n", truncate(parts[1], 100)));
261|                }
262|                continue;
263|            }
264|        }
265|
266|        // Show relevant error lines (assertions, errors, file locations)
267|        let mut relevant_lines = 0;
268|        for line in &lines[1..] {
269|            let line_lower = line.to_lowercase();
270|            let is_relevant = line.trim().starts_with('>')
271|                || line.trim().starts_with('E')
272|                || line_lower.contains("assert")
273|                || line_lower.contains("error")
274|                || line.contains(".py:");
275|
276|            if is_relevant && relevant_lines < 3 {
277|                result.push_str(&format!("     {}\n", truncate(line, 100)));
278|                relevant_lines += 1;
279|            }
280|        }
281|
282|        if i < failures.len() - 1 {
283|            result.push('\n');
284|        }
285|    }
286|
287|    if failures.len() > MAX_PYTEST_FAILURES {
288|        result.push_str(&format!(
289|            "\n… +{} more failures\n",
290|            failures.len() - MAX_PYTEST_FAILURES
291|        ));
292|        let all_failures = failures.join("\n\n");
293|        if let Some(hint) = crate::core::tee::force_tee_hint(&all_failures, "pytest-failures") {
294|            result.push_str(&format!("  {}\n", hint));
295|        }
296|    }
297|
298|    result.trim().to_string()
299|}
300|
301|fn parse_summary_line(summary: &str) -> PytestCounts {
302|    let mut counts = PytestCounts::default();
303|
304|    // Parse lines like "=== 4 passed, 1 failed, 2 xfailed, 1 xpassed in 0.50s ==="
305|    for part in summary.split(',') {
306|        let words: Vec<&str> = part.split_whitespace().collect();
307|        for (i, word) in words.iter().enumerate() {
308|            if i == 0 {
309|                continue;
310|            }
311|            let Ok(n) = words[i - 1].parse::<usize>() else {
312|                continue;
313|            };
314|            // Order matters: "xpassed"/"xfailed" contain "passed"/"failed".
315|            if word.contains("xpassed") {
316|                counts.xpassed = n;
317|            } else if word.contains("xfailed") {
318|                counts.xfailed = n;
319|            } else if word.contains("passed") {
320|                counts.passed = n;
321|            } else if word.contains("failed") {
322|                counts.failed = n;
323|            } else if word.contains("skipped") {
324|                counts.skipped = n;
325|            }
326|        }
327|    }
328|
329|    counts
330|}
331|
332|#[cfg(test)]
333|mod tests {
334|    use super::*;
335|
336|    #[test]
337|    fn test_filter_pytest_all_pass() {
338|        let output = r#"=== test session starts ===
339|platform darwin -- Python 3.11.0
340|collected 5 items
341|
342|tests/test_foo.py .....                                            [100%]
343|
344|=== 5 passed in 0.50s ==="#;
345|
346|        let result = filter_pytest_output(output);
347|        assert!(result.contains("Pytest"));
348|        assert!(result.contains("5 passed"));
349|    }
350|
351|    #[test]
352|    fn test_filter_pytest_with_failures() {
353|        let output = r#"=== test session starts ===
354|collected 5 items
355|
356|tests/test_foo.py ..F..                                            [100%]
357|
358|=== FAILURES ===
359|___ test_something ___
360|
361|    def test_something():
362|>       assert False
363|E       assert False
364|
365|tests/test_foo.py:10: AssertionError
366|
367|=== short test summary info ===
368|FAILED tests/test_foo.py::test_something - assert False
369|=== 4 passed, 1 failed in 0.50s ==="#;
370|
371|        let result = filter_pytest_output(output);
372|        assert!(result.contains("4 passed, 1 failed"));
373|        assert!(result.contains("test_something"));
374|        assert!(result.contains("assert False"));
375|    }
376|
377|    #[test]
378|    fn test_filter_pytest_multiple_failures() {
379|        let output = r#"=== test session starts ===
380|collected 3 items
381|
382|tests/test_foo.py FFF                                              [100%]
383|
384|=== FAILURES ===
385|___ test_one ___
386|E   AssertionError: expected 5
387|
388|___ test_two ___
389|E   ValueError: invalid value
390|
391|=== short test summary info ===
392|FAILED tests/test_foo.py::test_one - AssertionError: expected 5
393|FAILED tests/test_foo.py::test_two - ValueError: invalid value
394|FAILED tests/test_foo.py::test_three - KeyError
395|=== 3 failed in 0.20s ==="#;
396|
397|        let result = filter_pytest_output(output);
398|        assert!(result.contains("3 failed"));
399|        assert!(result.contains("test_one"));
400|        assert!(result.contains("test_two"));
401|        assert!(result.contains("expected 5"));
402|    }
403|
404|    #[test]
405|    fn test_filter_pytest_no_tests() {
406|        let output = r#"=== test session starts ===
407|collected 0 items
408|
409|=== no tests ran in 0.00s ==="#;
410|
411|        let result = filter_pytest_output(output);
412|        assert!(result.contains("No tests collected"));
413|    }
414|
415|    #[test]
416|    fn test_parse_summary_line() {
417|        let c = parse_summary_line("=== 5 passed in 0.50s ===");
418|        assert_eq!((c.passed, c.failed, c.skipped), (5, 0, 0));
419|
420|        let c = parse_summary_line("=== 4 passed, 1 failed in 0.50s ===");
421|        assert_eq!((c.passed, c.failed, c.skipped), (4, 1, 0));
422|
423|        let c = parse_summary_line("=== 3 passed, 1 failed, 2 skipped in 1.0s ===");
424|        assert_eq!((c.passed, c.failed, c.skipped), (3, 1, 2));
425|
426|        let c = parse_summary_line("=== 2 passed, 1 failed, 2 xfailed, 1 xpassed in 1.0s ===");
427|        assert_eq!(
428|            (c.passed, c.failed, c.xfailed, c.xpassed),
429|            (2, 1, 2, 1)
430|431|        );
432|    }
433|
434|    #[test]
435|    fn test_filter_pytest_xfail_caps_and_tee_hint() {
436|        let mut lines = String::from("=== test session starts ===\ncollected 30 items\n\n");
437|        lines.push_str("test_x.py ");
438|        for _ in 0..15 {
439|            lines.push('x');
440|        }
441|        lines.push_str("\n\n=== short test summary info ===\n");
442|        for i in 0..15 {
443|            lines.push_str(&format!(
444|                "XFAIL test_x.py::test_case_{i} - known issue #{i}\n"
445|            ));
446|        }
447|        lines.push_str("=== 0 passed, 15 xfailed in 0.05s ===\n");
448|
449|        let result = filter_pytest_output(&lines);
450|        let xfail_in_section = result
451|            .split("Expected-failure outcomes:")
452|            .nth(1)
453|            .unwrap_or("");
454|        let listed = xfail_in_section
455|            .lines()
456|            .filter(|l| l.trim().starts_with("XFAIL"))
457|            .count();
458|        assert!(
459|            listed <= 10,
460|            "MAX_XFAIL cap not enforced: listed {listed}"
461|463|        );
464|        assert!(result.contains("… +5 more"), "missing '+N more': {result}");
465|    }
466|
467|    #[test]
468|    fn test_filter_pytest_xfail_xpass() {
469|        let output = r#"=== test session starts ===
470|collected 5 items
471|
472|test_math.py ..xxX                                                 [100%]
473|
474|=== short test summary info ===
475|XFAIL test_math.py::test_division_by_zero - known bug in division
476|XFAIL test_math.py::test_float_precision - float precision issue — bug #42
477|XPASS test_math.py::test_unexpected_pass - this should fail but currently passes
478|=== 2 passed, 2 xfailed, 1 xpassed in 0.05s ==="#;
479|
480|        let result = filter_pytest_output(output);
481|        assert!(result.contains("xfailed"), "got: {result}");
482|        assert!(result.contains("xpassed"), "got: {result}");
483|        assert!(result.contains("XPASS"), "got: {result}");
484|        assert!(result.contains("float precision"), "got: {result}");
485|        assert!(result.contains("test_division_by_zero"), "got: {result}");
486|    }
487|
488|    #[test]
489|    fn test_filter_pytest_xfail_xpass() {
490|        let output = r#"=== test session starts ===
491|collected 5 items
492|
493|test_math.py ..xxX                                                 [100%]
494|
495|=== short test summary info ===
496|XFAIL test_math.py::test_division_by_zero - known bug in division
497|XFAIL test_math.py::test_float_precision - float precision issue — bug #42
498|XPASS test_math.py::test_unexpected_pass - this should fail but currently passes
499|=== 2 passed, 2 xfailed, 1 xpassed in 0.05s ==="#;
500|
501|        let result = filter_pytest_output(output);
502|        assert!(result.contains("xfailed"), "got: {result}");
503|        assert!(result.contains("xpassed"), "got: {result}");
504|        assert!(result.contains("XPASS"), "got: {result}");
505|        assert!(result.contains("float precision"), "got: {result}");
506|        assert!(result.contains("test_division_by_zero"), "got: {result}");
507|    }
508|
509|    #[test]
510|    fn test_filter_pytest_quiet_mode_failures() {
511|        // In -q mode, the final summary line has NO === wrapper
512|        // This was causing "No tests collected" to be reported incorrectly
513|        let output = r#"=== test session starts ===
514|platform linux -- Python 3.12.11, pytest-8.1.0
515|collected 1705 items
516|
517|.......F.......
518|
519|=== FAILURES ===
520|___ test_something ___
521|
522|E   AssertionError: expected True
523|
524|=== short test summary info ===
525|FAILED tests/test_foo.py::test_something - AssertionError
526|5 failed, 1698 passed, 2 skipped in 108.89s"#;
527|
528|        let result = filter_pytest_output(output);
529|        assert!(
530|            !result.contains("No tests collected"),
531|            "Should not report 'No tests collected' when tests ran. Got: {}",
532|            result
533|        );
534|        assert!(
535|            result.contains("1698") || result.contains("5 failed"),
536|            "Should show actual test counts. Got: {}",
537|            result
538|        );
539|    }
540|
541|    #[test]
542|    fn test_filter_pytest_only_skipped() {
543|        // If only skipped tests, should NOT say "No tests collected"
544|        let output = r#"=== test session starts ===
545|collected 3 items
546|
547|=== 3 skipped in 0.10s ==="#;
548|
549|        let result = filter_pytest_output(output);
550|        assert!(
551|            !result.contains("No tests collected"),
552|            "Should not say 'No tests collected' when tests were skipped. Got: {}",
553|            result
554|        );
555|    }
556|}
557|
=======
//! Filters pytest output to show only failures and the summary line.

use crate::core::runner;
use crate::core::utils::{resolved_command, tool_exists, truncate};
use anyhow::Result;

const MAX_XFAIL: usize = 10;

#[derive(Debug, PartialEq)]
enum ParseState {
    Header,
    TestProgress,
    Failures,
    Summary,
}

pub fn run(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = if tool_exists("pytest") {
        resolved_command("pytest")
    } else {
        let mut c = resolved_command("python");
        c.arg("-m").arg("pytest");
        c
    };

    let has_tb_flag = args.iter().any(|a| a.starts_with("--tb"));
    let has_quiet_flag = args.iter().any(|a| a == "-q" || a == "--quiet");
    // Only treat a short `-r…` as pytest's report flag (not `--randomly-seed` etc.)
    let has_report_flag = args.iter().any(|a| a.starts_with("-r") && !a.starts_with("--"));

    if !has_tb_flag {
        cmd.arg("--tb=short");
    }
    if !has_quiet_flag {
        cmd.arg("-q");
    }
    // Surface xfailed/xpassed (and their reasons) in the short summary section
    // so the compact output can report expected failures and — crucially —
    // unexpected passes (XPASS), which signal a behavior change.
    if !has_report_flag {
        cmd.arg("-rxX");
    }

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: pytest --tb=short -q {}", args.join(" "));
    }

    runner::run_filtered(
        cmd,
        "pytest",
        &args.join(" "),
        filter_pytest_output,
        runner::RunOptions::stdout_only().tee("pytest"),
    )
}

pub(crate) fn filter_pytest_output(output: &str) -> String {
    let mut state = ParseState::Header;
    let mut test_files: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    let mut current_failure: Vec<String> = Vec::new();
    let mut xfail_lines: Vec<String> = Vec::new();
    let mut summary_line = String::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // State transitions
        if trimmed.starts_with("===") && trimmed.contains("test session starts") {
            state = ParseState::Header;
            continue;
        } else if trimmed.starts_with("===") && trimmed.contains("FAILURES") {
            state = ParseState::Failures;
            continue;
        } else if trimmed.starts_with("===") && trimmed.contains("short test summary") {
            state = ParseState::Summary;
            // Save current failure if any
            if !current_failure.is_empty() {
                failures.push(current_failure.join("\n"));
                current_failure.clear();
            }
            continue;
        } else if trimmed.starts_with("===")
            && (trimmed.contains("passed")
                || trimmed.contains("failed")
                || trimmed.contains("skipped"))
        {
            summary_line = trimmed.to_string();
            continue;
        // quiet mode (-q): bare summary without === wrapper, e.g. "5 failed, 1698 passed, 2 skipped in 108.89s"
        } else if summary_line.is_empty()
            && !trimmed.starts_with("===")
            && !trimmed.starts_with("FAILED")
            && !trimmed.starts_with("ERROR")
            && (trimmed.contains(" passed")
                || trimmed.contains(" failed")
                || trimmed.contains(" skipped"))
            && trimmed.contains(" in ")
        {
            summary_line = trimmed.to_string();
            continue;
        }

        // Process based on state
        match state {
            ParseState::Header => {
                if trimmed.starts_with("collected") {
                    state = ParseState::TestProgress;
                }
            }
            ParseState::TestProgress => {
                // Lines like "tests/test_foo.py ....  [ 40%]"
                if !trimmed.is_empty()
                    && !trimmed.starts_with("===")
                    && (trimmed.contains(".py") || trimmed.contains("%]"))
                {
                    test_files.push(trimmed.to_string());
                }
            }
            ParseState::Failures => {
                // Collect failure details
                if trimmed.starts_with("___") {
                    // New failure section
                    if !current_failure.is_empty() {
                        failures.push(current_failure.join("\n"));
                        current_failure.clear();
                    }
                    current_failure.push(trimmed.to_string());
                } else if !trimmed.is_empty() && !trimmed.starts_with("===") {
                    current_failure.push(trimmed.to_string());
                }
            }
            ParseState::Summary => {
                // FAILED test lines
                if trimmed.starts_with("FAILED") || trimmed.starts_with("ERROR") {
                    failures.push(trimmed.to_string());
                } else if trimmed.starts_with("XFAIL") || trimmed.starts_with("XPASS") {
                    xfail_lines.push(trimmed.to_string());
                }
            }
        }
    }

    // Save last failure if any
    if !current_failure.is_empty() {
        failures.push(current_failure.join("\n"));
    }

    // Build compact output
    build_pytest_summary(&summary_line, &test_files, &failures, &xfail_lines)
}

#[derive(Default)]
struct PytestCounts {
    passed: usize,
    failed: usize,
    skipped: usize,
    xfailed: usize,
    xpassed: usize,
}

fn build_pytest_summary(
    summary: &str,
    _test_files: &[String],
    failures: &[String],
    xfail_lines: &[String],
) -> String {
    let counts = parse_summary_line(summary);
    let PytestCounts {
        passed,
        failed,
        skipped,
        xfailed,
        xpassed,
    } = counts;

    if passed == 0 && failed == 0 && skipped == 0 && xfailed == 0 && xpassed == 0 {
        return "Pytest: No tests collected".to_string();
    }

    let extras_present = skipped > 0 || xfailed > 0 || xpassed > 0 || !xfail_lines.is_empty();

    if failed == 0 && passed > 0 && !extras_present {
        return format!("Pytest: {} passed", passed);
    }

    let mut result = String::new();
    result.push_str(&format!("Pytest: {} passed, {} failed", passed, failed));
    if skipped > 0 {
        result.push_str(&format!(", {} skipped", skipped));
    }
    if xfailed > 0 {
        result.push_str(&format!(", {} xfailed", xfailed));
    }
    if xpassed > 0 {
        result.push_str(&format!(", {} xpassed", xpassed));
    }
    result.push('\n');
    result.push_str("═══════════════════════════════════════\n");

    // Surface xfail/xpass entries (with their reasons) — XPASS in particular
    // signals that something expected-to-fail now passes.
    if !xfail_lines.is_empty() {
        result.push_str("\nExpected-failure outcomes:\n");
        for line in xfail_lines.iter().take(MAX_XFAIL) {
            result.push_str(&format!("  {}\n", truncate(line, 120)));
        }
        if xfail_lines.len() > MAX_XFAIL {
            result.push_str(&format!("  … +{} more\n", xfail_lines.len() - MAX_XFAIL));
            let all_xfail = xfail_lines.join("\n");
            if let Some(hint) = crate::core::tee::force_tee_tail_hint(&all_xfail, "pytest-xfail", MAX_XFAIL + 1) {
                result.push_str(&format!("  {}\n", hint));
            }
        }
    }

    if failures.is_empty() {
        return result.trim().to_string();
    }

    // Show failures (limit to key information)
    result.push_str("\nFailures:\n");

    for (i, failure) in failures.iter().take(5).enumerate() {
        // Extract test name and key error info
        let lines: Vec<&str> = failure.lines().collect();

        // First line is usually test name (after ___)
        if let Some(first_line) = lines.first() {
            if first_line.starts_with("___") {
                // Extract test name between ___
                let test_name = first_line.trim_matches('_').trim();
                result.push_str(&format!("{}. [FAIL] {}\n", i + 1, test_name));
            } else if first_line.starts_with("FAILED") {
                // Summary format: "FAILED tests/test_foo.py::test_bar - AssertionError"
                let parts: Vec<&str> = first_line.split(" - ").collect();
                if let Some(test_path) = parts.first() {
                    let test_name = test_path.trim_start_matches("FAILED ");
                    result.push_str(&format!("{}. [FAIL] {}\n", i + 1, test_name));
                }
                if parts.len() > 1 {
                    result.push_str(&format!("     {}\n", truncate(parts[1], 100)));
                }
                continue;
            }
        }

        // Show relevant error lines (assertions, errors, file locations)
        let mut relevant_lines = 0;
        for line in &lines[1..] {
            let line_lower = line.to_lowercase();
            let is_relevant = line.trim().starts_with('>')
                || line.trim().starts_with('E')
                || line_lower.contains("assert")
                || line_lower.contains("error")
                || line.contains(".py:");

            if is_relevant && relevant_lines < 3 {
                result.push_str(&format!("     {}\n", truncate(line, 100)));
                relevant_lines += 1;
            }
        }

        if i < failures.len() - 1 {
            result.push('\n');
        }
    }

    if failures.len() > 5 {
        result.push_str(&format!("\n… +{} more failures\n", failures.len() - 5));
    }

    result.trim().to_string()
}

fn parse_summary_line(summary: &str) -> PytestCounts {
    let mut counts = PytestCounts::default();

    // Parse lines like "=== 4 passed, 1 failed, 2 xfailed, 1 xpassed in 0.50s ==="
    for part in summary.split(',') {
        let words: Vec<&str> = part.split_whitespace().collect();
        for (i, word) in words.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let Ok(n) = words[i - 1].parse::<usize>() else {
                continue;
            };
            // Order matters: "xpassed"/"xfailed" contain "passed"/"failed".
            if word.contains("xpassed") {
                counts.xpassed = n;
            } else if word.contains("xfailed") {
                counts.xfailed = n;
            } else if word.contains("passed") {
                counts.passed = n;
            } else if word.contains("failed") {
                counts.failed = n;
            } else if word.contains("skipped") {
                counts.skipped = n;
            }
        }
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_pytest_all_pass() {
        let output = r#"=== test session starts ===
platform darwin -- Python 3.11.0
collected 5 items

tests/test_foo.py .....                                            [100%]

=== 5 passed in 0.50s ==="#;

        let result = filter_pytest_output(output);
        assert!(result.contains("Pytest"));
        assert!(result.contains("5 passed"));
    }

    #[test]
    fn test_filter_pytest_with_failures() {
        let output = r#"=== test session starts ===
collected 5 items

tests/test_foo.py ..F..                                            [100%]

=== FAILURES ===
___ test_something ___

    def test_something():
>       assert False
E       assert False

tests/test_foo.py:10: AssertionError

=== short test summary info ===
FAILED tests/test_foo.py::test_something - assert False
=== 4 passed, 1 failed in 0.50s ==="#;

        let result = filter_pytest_output(output);
        assert!(result.contains("4 passed, 1 failed"));
        assert!(result.contains("test_something"));
        assert!(result.contains("assert False"));
    }

    #[test]
    fn test_filter_pytest_multiple_failures() {
        let output = r#"=== test session starts ===
collected 3 items

tests/test_foo.py FFF                                              [100%]

=== FAILURES ===
___ test_one ___
E   AssertionError: expected 5

___ test_two ___
E   ValueError: invalid value

=== short test summary info ===
FAILED tests/test_foo.py::test_one - AssertionError: expected 5
FAILED tests/test_foo.py::test_two - ValueError: invalid value
FAILED tests/test_foo.py::test_three - KeyError
=== 3 failed in 0.20s ==="#;

        let result = filter_pytest_output(output);
        assert!(result.contains("3 failed"));
        assert!(result.contains("test_one"));
        assert!(result.contains("test_two"));
        assert!(result.contains("expected 5"));
    }

    #[test]
    fn test_filter_pytest_no_tests() {
        let output = r#"=== test session starts ===
collected 0 items

=== no tests ran in 0.00s ==="#;

        let result = filter_pytest_output(output);
        assert!(result.contains("No tests collected"));
    }

    #[test]
    fn test_parse_summary_line() {
        let c = parse_summary_line("=== 5 passed in 0.50s ===");
        assert_eq!((c.passed, c.failed, c.skipped), (5, 0, 0));

        let c = parse_summary_line("=== 4 passed, 1 failed in 0.50s ===");
        assert_eq!((c.passed, c.failed, c.skipped), (4, 1, 0));

        let c = parse_summary_line("=== 3 passed, 1 failed, 2 skipped in 1.0s ===");
        assert_eq!((c.passed, c.failed, c.skipped), (3, 1, 2));

        let c = parse_summary_line("=== 2 passed, 1 failed, 2 xfailed, 1 xpassed in 1.0s ===");
        assert_eq!(
            (c.passed, c.failed, c.xfailed, c.xpassed),
            (2, 1, 2, 1)
        );
    }

    #[test]
    fn test_filter_pytest_xfail_caps_and_tee_hint() {
        let mut lines = String::from("=== test session starts ===\ncollected 30 items\n\n");
        lines.push_str("test_x.py ");
        for _ in 0..15 {
            lines.push('x');
        }
        lines.push_str("\n\n=== short test summary info ===\n");
        for i in 0..15 {
            lines.push_str(&format!(
                "XFAIL test_x.py::test_case_{i} - known issue #{i}\n"
            ));
        }
        lines.push_str("=== 0 passed, 15 xfailed in 0.05s ===\n");

        let result = filter_pytest_output(&lines);
        let xfail_in_section = result
            .split("Expected-failure outcomes:")
            .nth(1)
            .unwrap_or("");
        let listed = xfail_in_section
            .lines()
            .filter(|l| l.trim().starts_with("XFAIL"))
            .count();
        assert!(
            listed <= 10,
            "MAX_XFAIL cap not enforced: listed {listed}"
        );
        assert!(result.contains("… +5 more"), "missing '+N more': {result}");
    }

    #[test]
    fn test_filter_pytest_xfail_xpass() {
        let output = r#"=== test session starts ===
collected 5 items

test_math.py ..xxX                                                 [100%]

=== short test summary info ===
XFAIL test_math.py::test_division_by_zero - known bug in division
XFAIL test_math.py::test_float_precision - float precision issue — bug #42
XPASS test_math.py::test_unexpected_pass - this should fail but currently passes
=== 2 passed, 2 xfailed, 1 xpassed in 0.05s ==="#;

        let result = filter_pytest_output(output);
        assert!(result.contains("xfailed"), "got: {result}");
        assert!(result.contains("xpassed"), "got: {result}");
        assert!(result.contains("XPASS"), "got: {result}");
        assert!(result.contains("float precision"), "got: {result}");
        assert!(result.contains("test_division_by_zero"), "got: {result}");
    }

    #[test]
    fn test_filter_pytest_quiet_mode_failures() {
        // In -q mode, the final summary line has NO === wrapper
        // This was causing "No tests collected" to be reported incorrectly
        let output = r#"=== test session starts ===
platform linux -- Python 3.12.11, pytest-8.1.0
collected 1705 items

.......F.......

=== FAILURES ===
___ test_something ___

E   AssertionError: expected True

=== short test summary info ===
FAILED tests/test_foo.py::test_something - AssertionError
5 failed, 1698 passed, 2 skipped in 108.89s"#;

        let result = filter_pytest_output(output);
        assert!(
            !result.contains("No tests collected"),
            "Should not report 'No tests collected' when tests ran. Got: {}",
            result
        );
        assert!(
            result.contains("1698") || result.contains("5 failed"),
            "Should show actual test counts. Got: {}",
            result
        );
    }

    #[test]
    fn test_filter_pytest_only_skipped() {
        // If only skipped tests, should NOT say "No tests collected"
        let output = r#"=== test session starts ===
collected 3 items

=== 3 skipped in 0.10s ==="#;

        let result = filter_pytest_output(output);
        assert!(
            !result.contains("No tests collected"),
            "Should not say 'No tests collected' when tests were skipped. Got: {}",
            result
        );
    }
}
>>>>>>> f21b864 (fix(filters): split docker ps/-a paths, cap ruff violations at 50)
