1|/// Token-efficient formatting trait for canonical types
2|use super::types::*;
3|use crate::core::truncate::CAP_INVENTORY;
4|
5|const MAX_DEPS_LISTING: usize = CAP_INVENTORY;
6|
7|/// Output formatting modes
8|#[derive(Debug, Clone, Copy, PartialEq, Eq)]
9|pub enum FormatMode {
10|    /// Ultra-compact: Summary only (default)
11|    Compact,
12|    /// Verbose: Include details
13|    Verbose,
14|    /// Ultra-compressed: Symbols and abbreviations
15|    Ultra,
16|}
17|
18|impl FormatMode {
19|    pub fn from_verbosity(verbosity: u8) -> Self {
20|        match verbosity {
21|            0 => FormatMode::Compact,
22|            1 => FormatMode::Verbose,
23|            _ => FormatMode::Ultra,
24|        }
25|    }
26|}
27|
28|/// Trait for formatting canonical types into token-efficient strings
29|pub trait TokenFormatter {
30|    /// Format as compact summary (default)
31|    fn format_compact(&self) -> String;
32|
33|    /// Format with details (verbose mode)
34|    fn format_verbose(&self) -> String;
35|
36|    /// Format with symbols (ultra-compressed mode)
37|    fn format_ultra(&self) -> String;
38|
39|    /// Format according to mode
40|    fn format(&self, mode: FormatMode) -> String {
41|        match mode {
42|            FormatMode::Compact => self.format_compact(),
43|            FormatMode::Verbose => self.format_verbose(),
44|            FormatMode::Ultra => self.format_ultra(),
45|        }
46|    }
47|}
48|
49|impl TokenFormatter for TestResult {
50|    fn format_compact(&self) -> String {
51|52|        // All-green: ultra-compact single line
53|        if self.failed == 0 {
54|            let dur = self
55|                .duration_ms
56|                .map(|ms| format!(" ({:.1}s)", ms as f64 / 1000.0))
57|                .unwrap_or_default();
58|            let mut summary = format!("✓ {} passed{}", self.passed, dur);
59|            // Always surface skipped/pending tests — hiding them lets coverage gaps
60|            // (test.skip / it.skip / xfail) accumulate invisibly.
61|            if self.skipped > 0 {
62|                summary.push_str(&format!(" skipped ({})", self.skipped));
63|            }
64|            return summary;
65|        }
66|70|        let mut summary = format!("PASS ({}) FAIL ({})", self.passed, self.failed);
71|        if self.skipped > 0 {
72|            summary.push_str(&format!(" skipped ({})", self.skipped));
73|        }
74|        let mut lines = vec![summary];
75|
76|        if !self.failures.is_empty() {
77|            lines.push(String::new());
78|            for (idx, failure) in self.failures.iter().enumerate().take(5) {
79|                lines.push(format!("{}. {}", idx + 1, failure.test_name));
80|                for line in failure.error_message.lines() {
81|                    lines.push(format!("   {}", line));
82|                }
83|            }
84|
85|            if self.failures.len() > 5 {
86|                lines.push(format!("\n... +{} more failures", self.failures.len() - 5));
87|            }
88|        }
89|
90|        if let Some(duration) = self.duration_ms {
91|            lines.push(format!("\nTime: {}ms", duration));
92|        }
93|
94|        lines.join("\n")
95|    }
96|
97|    fn format_verbose(&self) -> String {
98|        let mut lines = vec![format!(
99|            "Tests: {} passed, {} failed, {} skipped (total: {})",
100|            self.passed, self.failed, self.skipped, self.total
101|        )];
102|
103|        if !self.failures.is_empty() {
104|            lines.push("\nFailures:".to_string());
105|            for (idx, failure) in self.failures.iter().enumerate() {
106|                lines.push(format!(
107|                    "\n{}. {} ({})",
108|                    idx + 1,
109|                    failure.test_name,
110|                    failure.file_path
111|                ));
112|                lines.push(format!("   {}", failure.error_message));
113|                if let Some(stack) = &failure.stack_trace {
114|                    let stack_preview: String =
115|                        stack.lines().take(3).collect::<Vec<_>>().join("\n   ");
116|                    lines.push(format!("   {}", stack_preview));
117|                }
118|            }
119|        }
120|
121|        if let Some(duration) = self.duration_ms {
122|            lines.push(format!("\nDuration: {}ms", duration));
123|        }
124|
125|        lines.join("\n")
126|    }
127|
128|    fn format_ultra(&self) -> String {
129|        format!(
130|            "[ok]{} [x]{} [skip]{} ({}ms)",
131|            self.passed,
132|            self.failed,
133|            self.skipped,
134|            self.duration_ms.unwrap_or(0)
135|        )
136|    }
137|}
138|
139|impl TokenFormatter for DependencyState {
140|    fn format_compact(&self) -> String {
141|        // A plain package listing (`pnpm list` / `npm ls`) carries no upgrade
142|        // info — every dep has `latest_version == None`. Reporting "All packages
143|        // up-to-date" there is a false positive that hides the entire list, so
144|        // we render the actual packages instead.
145|        let is_listing = self.outdated_count == 0
146|            && !self.dependencies.is_empty()
147|            && self.dependencies.iter().all(|d| d.latest_version.is_none());
148|        if is_listing {
149|            let total = self.total_packages.max(self.dependencies.len());
150|            let mut lines = vec![format!("{} packages", total)];
151|152|            for dep in self.dependencies.iter().take(MAX_DEPS_LISTING) {
153|                let dev = if dep.dev_dependency { " (dev)" } else { "" };
154|                lines.push(format!("  {} {}{}", dep.name, dep.current_version, dev));
155|            }
156|            if self.dependencies.len() > MAX_DEPS_LISTING {
157|                lines.push(format!(
158|                    "  ... +{} more",
159|                    self.dependencies.len() - MAX_DEPS_LISTING
160|                ));
161|169|            }
170|            return lines.join("\n");
171|        }
172|
173|        if self.outdated_count == 0 {
174|            return "All packages up-to-date".to_string();
175|        }
176|
177|        let mut lines = vec![format!(
178|            "{} outdated packages (of {})",
179|            self.outdated_count, self.total_packages
180|        )];
181|
182|        for dep in self.dependencies.iter().take(10) {
183|            if let Some(latest) = &dep.latest_version {
184|                if &dep.current_version != latest {
185|                    lines.push(format!(
186|                        "{}: {} → {}",
187|                        dep.name, dep.current_version, latest
188|                    ));
189|                }
190|            }
191|        }
192|
193|        if self.outdated_count > 10 {
194|            lines.push(format!("\n... +{} more", self.outdated_count - 10));
195|        }
196|
197|        lines.join("\n")
198|    }
199|
200|    fn format_verbose(&self) -> String {
201|        let mut lines = vec![format!(
202|            "Total packages: {} ({} outdated)",
203|            self.total_packages, self.outdated_count
204|        )];
205|
206|        if self.outdated_count > 0 {
207|            lines.push("\nOutdated packages:".to_string());
208|            for dep in &self.dependencies {
209|                if let Some(latest) = &dep.latest_version {
210|                    if &dep.current_version != latest {
211|                        let dev_marker = if dep.dev_dependency { " (dev)" } else { "" };
212|                        lines.push(format!(
213|                            "  {}: {} → {}{}",
214|                            dep.name, dep.current_version, latest, dev_marker
215|                        ));
216|                        if let Some(wanted) = &dep.wanted_version {
217|                            if wanted != latest {
218|                                lines.push(format!("    (wanted: {})", wanted));
219|                            }
220|                        }
221|                    }
222|                }
223|            }
224|        }
225|
226|        lines.join("\n")
227|    }
228|
229|    fn format_ultra(&self) -> String {
230|        format!("pkg:{} ^{}", self.total_packages, self.outdated_count)
231|    }
232|}
233|
234|#[cfg(test)]
235|mod tests {
236|    use super::*;
237|    use crate::parser::types::{TestFailure, TestResult};
238|
239|    fn make_failure(name: &str, error: &str) -> TestFailure {
240|        TestFailure {
241|            test_name: name.to_string(),
242|            file_path: "tests/e2e.spec.ts".to_string(),
243|            error_message: error.to_string(),
244|            stack_trace: None,
245|        }
246|    }
247|
248|    fn make_result(passed: usize, failures: Vec<TestFailure>) -> TestResult {
249|        TestResult {
250|            total: passed + failures.len(),
251|            passed,
252|            failed: failures.len(),
253|            skipped: 0,
254|            duration_ms: Some(1500),
255|            failures,
256|        }
257|    }
258|
259|    fn make_dep(name: &str, version: &str, latest: Option<&str>) -> Dependency {
260|        Dependency {
261|            name: name.to_string(),
262|            current_version: version.to_string(),
263|            latest_version: latest.map(str::to_string),
264|            wanted_version: None,
265|            dev_dependency: false,
266|        }
267|    }
268|
269|    #[test]
270|    fn test_dependency_state_plain_listing_shows_packages() {
271|        let state = DependencyState {
272|            total_packages: 2,
273|            outdated_count: 0,
274|            dependencies: vec![
275|                make_dep("react", "18.0.0", None),
276|                make_dep("typescript", "5.0.0", None),
277|            ],
278|        };
279|        let out = state.format_compact();
280|        assert!(out.contains("react"), "package name missing");
281|        assert!(out.contains("typescript"), "package name missing");
282|        assert!(
283|            !out.contains("up-to-date"),
284|            "false positive: plain listing should not say up-to-date"
285|        );
286|    }
287|
288|    // RED: format_compact must show the full error message, not just 2 lines.
289|    // Playwright errors contain the expected/received diff and call log starting
290|    // at line 3+. Truncating to 2 lines leaves the agent with no debug info.
291|    #[test]
292|    fn test_compact_shows_full_error_message() {
293|        let error = "Error: expect(locator).toHaveText(expected)\n\nExpected: 'Submit'\nReceived: 'Loading'\n\nCall log:\n  - waiting for getByRole('button', { name: 'Submit' })";
294|        let result = make_result(5, vec![make_failure("should click submit", error)]);
295|
296|        let output = result.format_compact();
297|
298|        assert!(
299|            output.contains("Expected: 'Submit'"),
300|            "format_compact must preserve expected/received diff\nGot:\n{output}"
301|        );
302|        assert!(
303|            output.contains("Received: 'Loading'"),
304|            "format_compact must preserve received value\nGot:\n{output}"
305|        );
306|        assert!(
307|            output.contains("Call log:"),
308|            "format_compact must preserve call log\nGot:\n{output}"
309|        );
310|    }
311|
312|    // RED: summary line stays compact regardless of failure detail
313|    #[test]
314|    fn test_compact_summary_line_is_concise() {
315|        let result = make_result(28, vec![make_failure("test", "some error")]);
316|        let output = result.format_compact();
317|        let first_line = output.lines().next().unwrap_or("");
318|        assert!(
319|            first_line.contains("28") && first_line.contains("1"),
320|            "First line must show pass/fail counts, got: {first_line}"
321|        );
322|    }
323|
324|    // RED: all-pass output stays compact (no failure detail bloat)
325|    #[test]
326|    fn test_compact_all_pass_is_one_line() {
327|        let result = make_result(10, vec![]);
328|        let output = result.format_compact();
329|        assert!(
330|            output.lines().count() <= 3,
331|            "All-pass output should be compact, got {} lines:\n{output}",
332|            output.lines().count()
333|        );
334|    }
335|
336|    // RED: error_message with only 1 line still works (no trailing noise)
337|    #[test]
338|    fn test_compact_single_line_error_no_trailing_noise() {
339|        let result = make_result(0, vec![make_failure("should work", "Timeout exceeded")]);
340|        let output = result.format_compact();
341|        assert!(
342|            output.contains("Timeout exceeded"),
343|            "Single-line error must appear\nGot:\n{output}"
344|        );
345|    }
346|}
347|