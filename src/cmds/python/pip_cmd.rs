1|//! Filters pip and uv package manager output.
2|
3|use crate::core::stream::exec_capture;
4|use crate::core::tracking;
5|use crate::core::truncate::{CAP_INVENTORY, CAP_LIST};
6|use crate::core::utils::{resolved_command, tool_exists};
7|use anyhow::{Context, Result};
8|use serde::Deserialize;
9|
10|#[derive(Debug, Deserialize)]
11|struct Package {
12|    name: String,
13|    version: String,
14|    #[serde(default)]
15|    latest_version: Option<String>,
16|}
17|
18|pub fn run(args: &[String], verbose: u8) -> Result<i32> {
19|    let timer = tracking::TimedExecution::start();
20|
21|    // The user ran `pip` — run `pip` so RTK stays transparent and reports the
22|    // *same* environment the bare command would. Only fall back to `uv pip` when
23|    // `pip` genuinely isn't on PATH (uv-only environments). Auto-substituting
24|    // `uv pip` unconditionally made `pip list` show uv's discovered env instead
25|    // of the active one — often just the 2-package base interpreter.
26|    let use_uv = !tool_exists("pip") && tool_exists("uv");
27|    let base_cmd = if use_uv { "uv" } else { "pip" };
28|
29|    if verbose > 0 && use_uv {
30|        eprintln!("pip not found — falling back to `uv pip`");
31|    }
32|
33|    // Detect subcommand
34|    let subcommand = args.first().map(|s| s.as_str()).unwrap_or("");
35|
36|    let (cmd_str, filtered, exit_code) = match subcommand {
37|        "list" => run_list(base_cmd, &args[1..], verbose)?,
38|        "outdated" => run_outdated(base_cmd, &args[1..], verbose)?,
39|        "install" | "uninstall" | "show" => {
40|            // Passthrough for write operations
41|            run_passthrough(base_cmd, args, verbose)?
42|        }
43|        _ => {
44|            // Unknown subcommand: passthrough to pip/uv
45|            run_passthrough(base_cmd, args, verbose)?
46|        }
47|    };
48|
49|    timer.track(
50|        &format!("{} {}", base_cmd, args.join(" ")),
51|        &format!("rtk {} {}", base_cmd, args.join(" ")),
52|        &cmd_str,
53|        &filtered,
54|    );
55|
56|    Ok(exit_code)
57|}
58|
59|fn run_list(base_cmd: &str, args: &[String], verbose: u8) -> Result<(String, String, i32)> {
60|    let mut cmd = resolved_command(base_cmd);
61|
62|    if base_cmd == "uv" {
63|        cmd.arg("pip");
64|    }
65|
66|    cmd.arg("list").arg("--format=json");
67|
68|    for arg in args {
69|        cmd.arg(arg);
70|    }
71|
72|    if verbose > 0 {
73|        eprintln!("Running: {} pip list --format=json", base_cmd);
74|    }
75|
76|    let result = exec_capture(&mut cmd)
77|        .with_context(|| format!("Failed to run {} pip list", base_cmd))?;
78|
79|    let raw = format!("{}\n{}", result.stdout, result.stderr);
80|
81|    let filtered = filter_pip_list(&result.stdout);
82|    println!("{}", filtered);
83|
84|    Ok((raw, filtered, result.exit_code))
85|}
86|
87|fn run_outdated(base_cmd: &str, args: &[String], verbose: u8) -> Result<(String, String, i32)> {
88|    let mut cmd = resolved_command(base_cmd);
89|
90|    if base_cmd == "uv" {
91|        cmd.arg("pip");
92|    }
93|
94|    cmd.arg("list").arg("--outdated").arg("--format=json");
95|
96|    for arg in args {
97|        cmd.arg(arg);
98|    }
99|
100|    if verbose > 0 {
101|        eprintln!("Running: {} pip list --outdated --format=json", base_cmd);
102|    }
103|
104|    let result = exec_capture(&mut cmd)
105|        .with_context(|| format!("Failed to run {} pip list --outdated", base_cmd))?;
106|
107|    let raw = format!("{}\n{}", result.stdout, result.stderr);
108|
109|    let filtered = filter_pip_outdated(&result.stdout);
110|    println!("{}", filtered);
111|
112|    Ok((raw, filtered, result.exit_code))
113|}
114|
115|fn run_passthrough(base_cmd: &str, args: &[String], verbose: u8) -> Result<(String, String, i32)> {
116|    let mut cmd = resolved_command(base_cmd);
117|
118|    if base_cmd == "uv" {
119|        cmd.arg("pip");
120|    }
121|
122|    for arg in args {
123|        cmd.arg(arg);
124|    }
125|
126|    if verbose > 0 {
127|        eprintln!("Running: {} pip {}", base_cmd, args.join(" "));
128|    }
129|
130|    let result = exec_capture(&mut cmd)
131|        .with_context(|| format!("Failed to run {} pip {}", base_cmd, args.join(" ")))?;
132|
133|    let raw = format!("{}\n{}", result.stdout, result.stderr);
134|
135|    print!("{}", result.stdout);
136|    eprint!("{}", result.stderr);
137|
138|    Ok((raw.clone(), raw, result.exit_code))
139|}
140|
141|/// Filter pip list JSON output
142|fn filter_pip_list(output: &str) -> String {
143|    let packages: Vec<Package> = match serde_json::from_str(output) {
144|        Ok(p) => p,
145|        Err(e) => {
146|            return format!("pip list (JSON parse failed: {})", e);
147|        }
148|    };
149|
150|    if packages.is_empty() {
151|        return "pip list: No packages installed".to_string();
152|    }
153|
154|    let mut result = String::new();
155|    result.push_str(&format!("pip list: {} packages\n", packages.len()));
156|    result.push_str("═══════════════════════════════════════\n");
157|
158|    // Group by first letter for easier scanning
159|    let mut by_letter: std::collections::HashMap<char, Vec<&Package>> =
160|        std::collections::HashMap::new();
161|
162|    for pkg in &packages {
163|        let first_char = pkg.name.chars().next().unwrap_or('?').to_ascii_lowercase();
164|        by_letter.entry(first_char).or_default().push(pkg);
165|    }
166|
167|    let mut letters: Vec<_> = by_letter.keys().collect();
168|    letters.sort();
169|
170|    // `pip list` is an inventory query — dependency audits need every package
171|    // visible. The compression here is structural (drop the alignment padding,
172|    // group by initial); the per-group cap is just a safety bound for
173|    // pathological environments, not a normal-case truncation.
174|175|    const MAX_PER_LETTER: usize = CAP_INVENTORY;
176|179|    for letter in letters {
180|        let pkgs = by_letter.get(letter).unwrap();
181|        result.push_str(&format!("\n[{}]\n", letter.to_uppercase()));
182|
183|        for pkg in pkgs.iter().take(MAX_PER_LETTER) {
184|            result.push_str(&format!("  {} ({})\n", pkg.name, pkg.version));
185|        }
186|
187|        if pkgs.len() > MAX_PER_LETTER {
188|            result.push_str(&format!("  ... +{} more\n", pkgs.len() - MAX_PER_LETTER));
189|        }
190|    }
191|
192|    result.trim().to_string()
193|}
194|
195|/// Filter pip outdated JSON output
196|fn filter_pip_outdated(output: &str) -> String {
197|    let packages: Vec<Package> = match serde_json::from_str(output) {
198|        Ok(p) => p,
199|        Err(e) => {
200|            return format!("pip outdated (JSON parse failed: {})", e);
201|        }
202|    };
203|
204|    if packages.is_empty() {
205|        return "pip outdated: All packages up to date".to_string();
206|    }
207|
208|    let mut result = String::new();
209|    result.push_str(&format!("pip outdated: {} packages\n", packages.len()));
210|    result.push_str("═══════════════════════════════════════\n");
211|
212|    const MAX_PIP_PACKAGES: usize = CAP_LIST;
213|    for (i, pkg) in packages.iter().take(MAX_PIP_PACKAGES).enumerate() {
214|        let latest = pkg.latest_version.as_deref().unwrap_or("unknown");
215|        result.push_str(&format!(
216|            "{}. {} ({} → {})\n",
217|            i + 1,
218|            pkg.name,
219|            pkg.version,
220|            latest
221|        ));
222|    }
223|
224|    if packages.len() > MAX_PIP_PACKAGES {
225|        result.push_str(&format!(
226|            "\n... +{} more packages\n",
227|            packages.len() - MAX_PIP_PACKAGES
228|        ));
229|    }
230|
231|    result.push_str("\n[hint] Run `pip install --upgrade <package>` to update\n");
232|
233|    result.trim().to_string()
234|}
235|
236|#[cfg(test)]
237|mod tests {
238|    use super::*;
239|
240|    #[test]
241|    fn test_filter_pip_list() {
242|        let output = r#"[
243|  {"name": "requests", "version": "2.31.0"},
244|  {"name": "pytest", "version": "7.4.0"},
245|  {"name": "rich", "version": "13.0.0"}
246|]"#;
247|
248|        let result = filter_pip_list(output);
249|        assert!(result.contains("3 packages"));
250|        assert!(result.contains("requests"));
251|        assert!(result.contains("2.31.0"));
252|        assert!(result.contains("pytest"));
253|    }
254|
255|    #[test]
256|    fn test_filter_pip_list_empty() {
257|        let output = "[]";
258|        let result = filter_pip_list(output);
259|        assert!(result.contains("No packages installed"));
260|    }
261|
262|    #[test]
263|    fn test_filter_pip_outdated_none() {
264|        let output = "[]";
265|        let result = filter_pip_outdated(output);
266|        assert!(result.contains("All packages up to date"));
267|    }
268|
269|    #[test]
270|    fn test_filter_pip_outdated_some() {
271|        let output = r#"[
272|  {"name": "requests", "version": "2.31.0", "latest_version": "2.32.0"},
273|  {"name": "pytest", "version": "7.4.0", "latest_version": "8.0.0"}
274|]"#;
275|
276|        let result = filter_pip_outdated(output);
277|        assert!(result.contains("2 packages"));
278|        assert!(result.contains("requests"));
279|        assert!(result.contains("2.31.0 → 2.32.0"));
280|        assert!(result.contains("pytest"));
281|        assert!(result.contains("7.4.0 → 8.0.0"));
282|    }
283|}
284|