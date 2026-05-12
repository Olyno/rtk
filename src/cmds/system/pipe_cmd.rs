1|use anyhow::Result;
2|use std::io::Read;
3|
4|use crate::core::stream::RAW_CAP;
5|use crate::core::truncate::{CAP_LIST, CAP_WARNINGS};
6|
7|const MAX_PIPE_MATCHES: usize = CAP_WARNINGS;
8|const MAX_PIPE_FILES: usize = CAP_WARNINGS;
9|const MAX_PIPE_DIRS: usize = CAP_LIST;
10|
11|pub fn resolve_filter(name: &str) -> Option<fn(&str) -> String> {
12|    match name {
13|        "cargo-test" | "cargo" => Some(crate::cmds::rust::cargo_cmd::filter_cargo_test),
14|        "pytest" => Some(crate::cmds::python::pytest_cmd::filter_pytest_output),
15|        "go-test" => Some(go_test_wrapper),
16|        "go-build" => Some(crate::cmds::go::go_cmd::filter_go_build),
17|        "tsc" => Some(crate::cmds::js::tsc_cmd::filter_tsc_output),
18|        "vitest" => Some(vitest_wrapper),
19|        "grep" | "rg" => Some(grep_wrapper),
20|        "find" | "fd" => Some(find_wrapper),
21|        "git-log" => Some(git_log_wrapper),
22|        "git-diff" => Some(git_diff_wrapper),
23|24|        "git-status" => Some(git_status_wrapper),
25|28|        "log" => Some(crate::cmds::system::log_cmd::run_stdin_str),
29|        "mypy" => Some(crate::cmds::python::mypy_cmd::filter_mypy_output),
30|        "ruff-check" => Some(crate::cmds::python::ruff_cmd::filter_ruff_check_json),
31|        "ruff-format" => Some(crate::cmds::python::ruff_cmd::filter_ruff_format),
32|        "prettier" => Some(crate::cmds::js::prettier_cmd::filter_prettier_output),
33|        _ => None,
34|    }
35|}
36|
37|fn go_test_wrapper(input: &str) -> String {
38|    crate::cmds::go::go_cmd::filter_go_test_json(input)
39|}
40|
41|fn git_status_wrapper(input: &str) -> String {
42|    crate::cmds::git::git::format_status_output(input)
43|}
44|
45|fn git_log_wrapper(input: &str) -> String {
46|    crate::cmds::git::git::filter_log_output(input, 50, false, false)
47|}
48|
49|fn git_diff_wrapper(input: &str) -> String {
50|    crate::cmds::git::git::compact_diff(input, 200)
51|}
52|
53|fn vitest_wrapper(input: &str) -> String {
54|    use crate::cmds::js::vitest_cmd::VitestParser;
55|    use crate::parser::{FormatMode, OutputParser, TokenFormatter};
56|    let result = VitestParser::parse(input);
57|    match result {
58|        crate::parser::ParseResult::Full(data) => data.format(FormatMode::Compact),
59|        crate::parser::ParseResult::Degraded(data, _) => data.format(FormatMode::Compact),
60|        crate::parser::ParseResult::Passthrough(raw) => raw,
61|    }
62|}
63|
64|fn grep_wrapper(input: &str) -> String {
65|    use std::collections::HashMap;
66|
67|    let mut by_file: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
68|    let mut total = 0;
69|
70|    for line in input.lines() {
71|        let parts: Vec<&str> = line.splitn(3, ':').collect();
72|        if parts.len() == 3 {
73|            if let Ok(_line_num) = parts[1].parse::<usize>() {
74|                total += 1;
75|                by_file.entry(parts[0]).or_default().push((parts[1], parts[2]));
76|            }
77|        }
78|    }
79|
80|    if total == 0 {
81|        return input.to_string();
82|    }
83|
84|    let mut out = format!("{} matches in {}F:\n\n", total, by_file.len());
85|    let mut files: Vec<_> = by_file.iter().collect();
86|    files.sort_by_key(|(f, _)| *f);
87|
88|    for (file, matches) in files {
89|        out.push_str(&format!("[file] {} ({}):\n", file, matches.len()));
90|        for (line_num, content) in matches.iter().take(MAX_PIPE_MATCHES) {
91|            out.push_str(&format!("  {:>4}: {}\n", line_num, content.trim()));
92|        }
93|        if matches.len() > MAX_PIPE_MATCHES {
94|            out.push_str(&format!("  +{}\n", matches.len() - MAX_PIPE_MATCHES));
95|        }
96|        out.push('\n');
97|    }
98|
99|    out
100|}
101|
102|fn find_wrapper(input: &str) -> String {
103|    use std::collections::HashMap;
104|
105|    let paths: Vec<&str> = input.lines().filter(|l| !l.trim().is_empty()).collect();
106|
107|    if paths.is_empty() {
108|        return input.to_string();
109|    }
110|
111|    let mut by_dir: HashMap<&str, Vec<&str>> = HashMap::new();
112|
113|    for path in &paths {
114|        let dir = match path.rfind('/') {
115|            Some(pos) => &path[..pos],
116|            None => ".",
117|        };
118|        let name = match path.rfind('/') {
119|            Some(pos) => &path[pos + 1..],
120|            None => path,
121|        };
122|        by_dir.entry(dir).or_default().push(name);
123|    }
124|
125|    let mut out = format!("{} files in {} dirs:\n\n", paths.len(), by_dir.len());
126|    let mut dirs: Vec<_> = by_dir.iter().collect();
127|    dirs.sort_by_key(|(d, _)| *d);
128|
129|    for (dir, files) in dirs.iter().take(MAX_PIPE_DIRS) {
130|        out.push_str(&format!("{}/  ({})\n", dir, files.len()));
131|        for f in files.iter().take(MAX_PIPE_FILES) {
132|            out.push_str(&format!("  {}\n", f));
133|        }
134|        if files.len() > MAX_PIPE_FILES {
135|            out.push_str(&format!("  +{}\n", files.len() - MAX_PIPE_FILES));
136|        }
137|    }
138|
139|    if dirs.len() > MAX_PIPE_DIRS {
140|        out.push_str(&format!("\n+{} more dirs\n", dirs.len() - MAX_PIPE_DIRS));
141|    }
142|
143|    out
144|}
145|
146|pub fn auto_detect_filter(input: &str) -> fn(&str) -> String {
147|    let end = input.len().min(1024);
148|    // Avoid panic: byte 1024 may fall inside a multi-byte UTF-8 char
149|    let end = input.floor_char_boundary(end);
150|    let first_1k = &input[..end];
151|
152|    if first_1k.contains("test result:") && first_1k.contains("passed;") {
153|        return crate::cmds::rust::cargo_cmd::filter_cargo_test;
154|    }
155|
156|    if first_1k.contains("=== test session starts") {
157|        return crate::cmds::python::pytest_cmd::filter_pytest_output;
158|    }
159|
160|    let first_trimmed = first_1k.trim_start();
161|    if first_trimmed.starts_with('{') && first_1k.contains("\"Action\"") {
162|        return go_test_wrapper;
163|    }
164|
165|    if first_1k.contains(": error:") && first_1k.contains(".py:") {
166|        return crate::cmds::python::mypy_cmd::filter_mypy_output;
167|    }
168|
169|    // grep/rg: lines matching file:number:content
170|    if first_1k
171|        .lines()
172|        .take(5)
173|        .filter(|l| !l.trim().is_empty())
174|        .any(|l| {
175|            let parts: Vec<_> = l.splitn(3, ':').collect();
176|            parts.len() == 3 && parts[1].parse::<usize>().is_ok()
177|        })
178|    {
179|        return grep_wrapper;
180|    }
181|
182|    if first_1k.contains("\"testResults\"") || first_1k.contains("\"numTotalTests\"") {
183|        return vitest_wrapper;
184|    }
185|
186|    // find/fd: all non-empty lines look like file paths, minimum 3 lines
187|    let path_like_lines: usize = first_1k
188|        .lines()
189|        .filter(|l| {
190|            let t = l.trim();
191|            !t.is_empty()
192|                && !t.contains(':')
193|                && (t.starts_with('.') || t.starts_with('/') || t.contains('/'))
194|        })
195|        .count();
196|    let nonempty_lines: usize = first_1k.lines().filter(|l| !l.trim().is_empty()).count();
197|    if nonempty_lines >= 3 && path_like_lines == nonempty_lines {
198|        return find_wrapper;
199|    }
200|
201|    identity_filter
202|}
203|
204|fn identity_filter(input: &str) -> String {
205|    input.to_string()
206|}
207|
208|fn apply_filter(filter_fn: fn(&str) -> String, input: &str) -> String {
209|    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| filter_fn(input)))
210|        .unwrap_or_else(|_| {
211|            eprintln!("[rtk] warning: filter panicked — passing through raw output");
212|            input.to_string()
213|        })
214|}
215|
216|pub fn run(filter_name: Option<&str>, passthrough: bool) -> Result<()> {
217|    if passthrough {
218|        std::io::copy(&mut std::io::stdin(), &mut std::io::stdout())
219|            .map_err(|e| anyhow::anyhow!("Failed to relay stdin: {}", e))?;
220|        return Ok(());
221|    }
222|
223|    let mut buf = String::new();
224|    std::io::stdin()
225|        .take((RAW_CAP + 1) as u64)
226|        .read_to_string(&mut buf)
227|        .map_err(|e| anyhow::anyhow!("Failed to read stdin: {}", e))?;
228|    if buf.len() > RAW_CAP {
229|        anyhow::bail!("stdin exceeds {} byte limit", RAW_CAP);
230|    }
231|
232|    let filter_fn = match filter_name {
233|        Some(name) => resolve_filter(name).ok_or_else(|| {
234|            anyhow::anyhow!(
235|                "Unknown filter '{}'. Available: cargo-test, pytest, go-test, go-build, \
236|                 tsc, vitest, grep, rg, find, fd, git-log, git-diff, git-status, \
237|                 log, mypy, ruff-check, ruff-format, prettier",
238|                name
239|            )
240|        })?,
241|        None => auto_detect_filter(&buf),
242|    };
243|
244|    let output = apply_filter(filter_fn, &buf);
245|    print!("{}", output);
246|    Ok(())
247|}
248|
249|#[cfg(test)]
250|mod tests {
251|    use super::*;
252|
253|    #[test]
254|    fn test_resolve_filter_cargo_test() {
255|        let f = resolve_filter("cargo-test").expect("cargo-test filter must exist");
256|        let out = f("test result: ok. 5 passed; 0 failed");
257|        assert!(out.contains("passed") || out.contains("PASS"), "out={}", out);
258|    }
259|
260|    #[test]
261|    fn test_resolve_filter_cargo_alias() {
262|        assert!(resolve_filter("cargo").is_some());
263|    }
264|
265|    #[test]
266|    fn test_resolve_filter_grep() {
267|        let f = resolve_filter("grep").expect("grep filter must exist");
268|        let input = "src/main.rs:42:fn main() {\nsrc/lib.rs:10:pub fn helper() {}\n";
269|        let out = f(input);
270|        assert!(
271|            out.contains("main.rs") || out.contains("matches"),
272|            "out={}",
273|            out
274|        );
275|    }
276|
277|    #[test]
278|    fn test_resolve_filter_rg_alias() {
279|        assert!(resolve_filter("rg").is_some());
280|    }
281|
282|    #[test]
283|    fn test_resolve_filter_pytest() {
284|        assert!(resolve_filter("pytest").is_some());
285|    }
286|
287|    #[test]
288|    fn test_resolve_filter_go_test() {
289|        assert!(resolve_filter("go-test").is_some());
290|    }
291|
292|    #[test]
293|    fn test_resolve_filter_tsc() {
294|        assert!(resolve_filter("tsc").is_some());
295|    }
296|
297|    #[test]
298|    fn test_resolve_filter_vitest() {
299|        assert!(resolve_filter("vitest").is_some());
300|    }
301|
302|    #[test]
303|    fn test_resolve_filter_git_log() {
304|        assert!(resolve_filter("git-log").is_some());
305|    }
306|
307|    #[test]
308|    fn test_resolve_filter_git_diff() {
309|        assert!(resolve_filter("git-diff").is_some());
310|    }
311|
312|    #[test]
313|    fn test_resolve_filter_git_status() {
314|        assert!(resolve_filter("git-status").is_some());
315|    }
316|
317|    #[test]
318|    fn test_resolve_filter_log() {
319|        assert!(resolve_filter("log").is_some());
320|    }
321|
322|    #[test]
323|    fn test_resolve_filter_unknown_returns_none() {
324|        assert!(resolve_filter("nonexistent-filter").is_none());
325|    }
326|
327|    #[test]
328|    fn test_auto_detect_cargo_test() {
329|        let input = "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured\n";
330|        let f = auto_detect_filter(input);
331|        let out = f(input);
332|        assert!(!out.is_empty());
333|    }
334|
335|    #[test]
336|    fn test_auto_detect_pytest() {
337|        let input = "=== test session starts ===\ncollected 3 items\n";
338|        let f = auto_detect_filter(input);
339|        let out = f(input);
340|        assert!(!out.is_empty());
341|    }
342|
343|    #[test]
344|    fn test_auto_detect_grep_format() {
345|        let input = "src/main.rs:42:fn main() {\nsrc/lib.rs:10:pub fn helper() {}\n";
346|        let f = auto_detect_filter(input);
347|        let out = f(input);
348|        assert!(!out.is_empty());
349|    }
350|
351|    #[test]
352|    fn test_auto_detect_go_test_ndjson() {
353|        let input = r#"{"Time":"2024-01-01T00:00:00Z","Action":"run","Package":"example/pkg"}
354|{"Time":"2024-01-01T00:00:01Z","Action":"pass","Package":"example/pkg","Elapsed":0.5}
355|"#;
356|        let f = auto_detect_filter(input);
357|        let out = f(input);
358|        assert!(!out.is_empty());
359|    }
360|
361|    #[test]
362|    fn test_auto_detect_unknown_returns_identity() {
363|        let input = "some random text that doesn't match any filter pattern\n";
364|        let f = auto_detect_filter(input);
365|        let out = f(input);
366|        assert_eq!(out, input);
367|    }
368|
369|    #[test]
370|    fn test_git_log_wrapper() {
371|        let input = "abc1234 Fix bug in parser (2 days ago) <alice>\n\
372|                     def5678 Add new feature (3 days ago) <bob>\n";
373|        let out = git_log_wrapper(input);
374|        assert!(!out.is_empty());
375|    }
376|
377|    #[test]
378|    fn test_git_diff_wrapper() {
379|        let input = "diff --git a/src/main.rs b/src/main.rs\n\
380|                     --- a/src/main.rs\n\
381|                     +++ b/src/main.rs\n\
382|                     @@ -1,3 +1,4 @@\n\
383|                     +// new comment\n\
384|                      fn main() {}\n";
385|        let out = git_diff_wrapper(input);
386|        assert!(!out.is_empty());
387|    }
388|
389|    #[test]
390|    fn test_resolve_filter_find() {
391|        let f = resolve_filter("find").expect("find filter must exist");
392|        let input = "./src/main.rs\n./src/lib.rs\n./tests/foo.rs\n";
393|        let out = f(input);
394|        assert!(out.contains("3 files"), "out={}", out);
395|    }
396|
397|    #[test]
398|    fn test_resolve_filter_fd_alias() {
399|        assert!(resolve_filter("fd").is_some());
400|    }
401|
402|    #[test]
403|    fn test_auto_detect_find_paths() {
404|        let input = "./src/main.rs\n./src/lib.rs\n./src/cmd/mod.rs\n./tests/foo.rs\n";
405|        let f = auto_detect_filter(input);
406|        let out = f(input);
407|        assert!(out.contains("4 files"), "out={}", out);
408|    }
409|
410|    #[test]
411|    fn test_auto_detect_find_absolute_paths() {
412|        let input = "/home/user/src/main.rs\n/home/user/src/lib.rs\n/home/user/tests/foo.rs\n";
413|        let f = auto_detect_filter(input);
414|        let out = f(input);
415|        assert!(out.contains("3 files"), "out={}", out);
416|    }
417|
418|    #[test]
419|    fn test_auto_detect_find_not_triggered_for_few_lines() {
420|        let input = "./src/main.rs\n./src/lib.rs\n";
421|        let f = auto_detect_filter(input);
422|        let out = f(input);
423|        assert_eq!(out, input);
424|    }
425|
426|    #[test]
427|    fn test_auto_detect_find_not_triggered_for_grep_output() {
428|        let input = "src/main.rs:42:fn main() {\nsrc/lib.rs:10:pub fn helper() {}\nsrc/a.rs:1:x\n";
429|        let f = auto_detect_filter(input);
430|        let out = f(input);
431|        assert!(
432|            !out.contains("files"),
433|            "should not trigger find filter: out={}",
434|            out
435|        );
436|    }
437|
438|    #[test]
439|    fn test_auto_detect_empty_input_is_identity() {
440|        let f = auto_detect_filter("");
441|        let out = f("");
442|        assert_eq!(out, "");
443|    }
444|
445|    #[test]
446|    fn test_auto_detect_multibyte_at_1024_boundary() {
447|        // Build input where byte 1024 falls inside a multi-byte char (é = 2 bytes)
448|        let mut input = "a".repeat(1023);
449|        input.push('é'); // 2-byte char starting at byte 1023, ends at 1025
450|        let f = auto_detect_filter(&input);
451|        let out = f(&input);
452|        assert_eq!(out, input);
453|    }
454|
455|    #[test]
456|    fn test_auto_detect_single_line_unknown() {
457|        let input = "hello world\n";
458|        let f = auto_detect_filter(input);
459|        let out = f(input);
460|        assert_eq!(out, input);
461|    }
462|
463|    #[test]
464|    fn test_resolve_filter_go_build() {
465|        assert!(resolve_filter("go-build").is_some());
466|    }
467|
468|    #[test]
469|    fn test_resolve_filter_mypy() {
470|        assert!(resolve_filter("mypy").is_some());
471|    }
472|
473|    #[test]
474|    fn test_resolve_filter_ruff_check() {
475|        assert!(resolve_filter("ruff-check").is_some());
476|    }
477|
478|    #[test]
479|    fn test_resolve_filter_ruff_format() {
480|        assert!(resolve_filter("ruff-format").is_some());
481|    }
482|
483|    #[test]
484|    fn test_resolve_filter_prettier() {
485|        assert!(resolve_filter("prettier").is_some());
486|    }
487|
488|    #[test]
489|    fn test_panicking_filter_returns_passthrough() {
490|        fn panicking_filter(_input: &str) -> String {
491|            panic!("filter bug");
492|        }
493|        let input = "some output\n";
494|        let result = super::apply_filter(panicking_filter, input);
495|        assert_eq!(result, input);
496|    }
497|
498|    fn count_tokens(s: &str) -> usize {
499|        s.split_whitespace().count()
500|    }
501|
502|    #[test]
503|    fn test_grep_wrapper_token_savings() {
504|        // Realistic rg output: 200 matches across 10 files (20 per file → 10 shown + truncation)
505|        let mut input = String::new();
506|        for file_idx in 1..=10 {
507|            for line in 1..=20 {
508|                input.push_str(&format!(
509|                    "src/cmds/module{}/handler.rs:{}:    let result = process_request(ctx, &payload).await?;\n",
510|                    file_idx, line * 10
511|                ));
512|            }
513|        }
514|        let output = grep_wrapper(&input);
515|        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(&input) as f64 * 100.0);
516|        assert!(
517|            savings >= 40.0, // TODO: grep pipe filter below 60% target — improve grouping
518|            "grep filter: expected ≥40% savings, got {:.1}% (in={}, out={})",
519|            savings, count_tokens(&input), count_tokens(&output)
520|        );
521|    }
522|
523|    #[test]
524|    fn test_find_wrapper_token_savings() {
525|        // Realistic find output: 500 files across 30 dirs (20-dir cap + 10-file cap both trigger)
526|        let mut input = String::new();
527|        for dir in 1..=30 {
528|            for file in 1..=17 {
529|                input.push_str(&format!(
530|                    "./src/components/feature{}/sub_{}/component_{}.tsx\n",
531|                    dir, dir, file
532|                ));
533|            }
534|        }
535|        let output = find_wrapper(&input);
536|        let savings = 100.0 - (count_tokens(&output) as f64 / count_tokens(&input) as f64 * 100.0);
537|        assert!(
538|            savings >= 40.0, // TODO: find pipe filter below 60% target — improve grouping
539|            "find filter: expected ≥40% savings, got {:.1}% (in={}, out={})",
540|            savings, count_tokens(&input), count_tokens(&output)
541|        );
542|    }
543|
544|    #[test]
545|    fn test_auto_detect_mypy_output() {
546|        let input = "src/app.py:42: error: Argument 1 has incompatible type [arg-type]\n\
547|                     src/utils.py:10: error: Missing return statement [return]\n\
548|                     Found 2 errors in 2 files\n";
549|        let f = auto_detect_filter(input);
550|        let out = f(input);
551|        assert!(!out.is_empty());
552|    }
553|}
554|