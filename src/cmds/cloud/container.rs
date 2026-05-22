1|1|1|//! Filters Docker and kubectl output into compact summaries.
2|2|2|
3|3|3|use crate::core::runner::{self, RunOptions};
4|4|4|use crate::core::stream::exec_capture;
5|5|5|use crate::core::tracking;
6|6|6|use crate::core::truncate::{CAP_INVENTORY, CAP_LIST, CAP_WARNINGS};
7|7|7|use crate::core::utils::resolved_command;
8|8|8|use anyhow::{Context, Result};
9|9|9|use serde_json::Value;
10|10|10|use std::ffi::OsString;
11|11|11|use std::process::Command;
12|12|12|
13|13|13|#[derive(Debug, Clone, Copy)]
14|14|14|pub enum ContainerCmd {
15|15|15|    DockerPs,
16|16|16|    DockerPsAll,
17|17|17|    DockerImages,
18|18|18|    DockerLogs,
19|19|19|    KubectlPods,
20|20|20|    KubectlServices,
21|21|21|    KubectlLogs,
22|22|22|}
23|23|23|
24|24|24|pub fn run(cmd: ContainerCmd, args: &[String], verbose: u8) -> Result<i32> {
25|25|25|    match cmd {
26|26|26|        ContainerCmd::DockerPs => docker_ps(verbose),
27|27|27|        ContainerCmd::DockerPsAll => docker_ps_all(verbose),
28|28|28|        ContainerCmd::DockerImages => docker_images(verbose),
29|29|29|        ContainerCmd::DockerLogs => docker_logs(args, verbose),
30|30|30|        ContainerCmd::KubectlPods => kubectl_pods(args, verbose),
31|31|31|        ContainerCmd::KubectlServices => kubectl_services(args, verbose),
32|32|32|        ContainerCmd::KubectlLogs => kubectl_logs(args, verbose),
33|33|33|    }
34|34|34|}
35|35|35|
36|36|36|fn run_kubectl_json<F>(cmd: Command, label: &str, filter_fn: F) -> Result<i32>
37|37|37|where
38|38|38|    F: Fn(&Value) -> String,
39|39|39|{
40|40|40|    runner::run_filtered(
41|41|41|        cmd,
42|42|42|        "kubectl",
43|43|43|        label,
44|44|44|        |stdout| match serde_json::from_str::<Value>(stdout) {
45|45|45|            Ok(json) => filter_fn(&json),
46|46|46|            Err(e) => {
47|47|47|                eprintln!("[rtk] kubectl: JSON parse failed: {}", e);
48|48|48|                stdout.to_string()
49|49|49|            }
50|50|50|        },
51|51|51|        RunOptions::stdout_only()
52|52|52|            .early_exit_on_failure()
53|53|53|            .no_trailing_newline(),
54|54|54|    )
55|55|55|}
56|56|56|
57|57|57|fn docker_ps(_verbose: u8) -> Result<i32> {
58|58|58|    let timer = tracking::TimedExecution::start();
59|59|59|
60|60|60|    // Baseline the LLM would otherwise see.
61|61|61|    let raw = exec_capture(resolved_command("docker").args(["ps"]))
62|62|62|        .map(|r| r.stdout)
63|63|63|        .unwrap_or_default();
64|64|64|
65|65|65|    // One structured call over *all* containers (`-a`) — splitting on the State
66|66|66|    // field lets us list crashed/exited ones too, which plain `docker ps` hides.
67|67|67|    let result = exec_capture(resolved_command("docker").args([
68|68|68|        "ps",
69|69|69|        "-a",
70|70|70|        "--format",
71|71|71|        "{{.State}}\t{{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}",
72|72|72|    ]))
73|73|73|    .context("Failed to run docker ps")?;
74|74|74|
75|75|75|    if !result.success() {
76|76|76|        eprint!("{}", result.stderr);
77|77|77|        timer.track("docker ps", "rtk docker ps", &raw, &raw);
78|78|78|        return Ok(result.exit_code);
79|79|79|    }
80|80|80|
81|81|81|82|    let stdout = result.stdout;
82|82|83|    let mut rtk = String::new();
83|83|84|
84|84|85|    if stdout.trim().is_empty() {
85|85|86|        rtk.push_str("[docker] 0 containers");
86|86|87|        println!("{}", rtk);
87|87|88|        timer.track("docker ps", "rtk docker ps", &raw, &rtk);
88|88|89|        return Ok(0);
89|89|90|    }
90|90|91|
91|91|92|    const MAX_CONTAINERS: usize = CAP_LIST;
92|92|93|    let lines: Vec<String> = stdout
93|93|94|        .lines()
94|94|95|        .filter(|l| !l.trim().is_empty())
95|95|96|        .filter_map(|line| format_container_line(line, true))
96|96|97|        .collect();
97|97|98|
98|98|99|    rtk.push_str(&format!("[docker] {} containers:\n", lines.len()));
99|99|100|    for entry in lines.iter().take(MAX_CONTAINERS) {
100|100|101|        rtk.push_str(entry);
101|101|102|    }
102|102|103|    if lines.len() > MAX_CONTAINERS {
103|103|104|        rtk.push_str(&format!("  … +{} more\n", lines.len() - MAX_CONTAINERS));
104|104|105|        let full: String = lines.concat();
105|105|106|        if let Some(hint) = crate::core::tee::force_tee_hint(&full, "docker-ps") {
106|106|107|            rtk.push_str(&format!("{}\n", hint));
107|107|108|169|        }
108|108|170|    }
109|109|171|
110|110|172|    print!("{}", rtk);
111|111|173|    timer.track("docker ps", "rtk docker ps", &raw, &rtk);
112|112|174|    Ok(0)
113|113|175|}
114|114|176|
115|115|177|fn docker_ps_all(_verbose: u8) -> Result<i32> {
116|116|178|    let timer = tracking::TimedExecution::start();
117|117|179|
118|118|180|    let raw = exec_capture(resolved_command("docker").args(["ps", "-a"]))
119|119|181|        .map(|r| r.stdout)
120|120|182|        .unwrap_or_default();
121|121|183|
122|122|184|    let result = exec_capture(resolved_command("docker").args([
123|123|185|        "ps",
124|124|186|        "-a",
125|125|187|        "--format",
126|126|188|        "{{.State}}\t{{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}",
127|127|189|    ]))
128|128|190|    .context("Failed to run docker ps -a")?;
129|129|191|
130|130|192|    if !result.success() {
131|131|193|        eprint!("{}", result.stderr);
132|132|194|        timer.track("docker ps -a", "rtk docker ps -a", &raw, &raw);
133|133|195|        return Ok(result.exit_code);
134|134|196|    }
135|135|197|
136|136|198|    let mut running_lines: Vec<String> = Vec::new();
137|137|199|    let mut stopped_lines: Vec<String> = Vec::new();
138|138|200|    for line in result.stdout.lines().filter(|l| !l.trim().is_empty()) {
139|139|201|        let parts: Vec<&str> = line.split('\t').collect();
140|140|202|        let state = parts.first().copied().unwrap_or("");
141|141|203|        let is_running = matches!(state, "running" | "restarting");
142|142|204|        if let Some(entry) = format_container_line_from_parts(&parts[1..], is_running) {
143|143|205|            if is_running {
144|144|206|                running_lines.push(entry);
145|145|207|            } else {
146|146|208|                stopped_lines.push(entry);
147|147|209|            }
148|148|210|        }
149|149|211|    }
150|150|212|
151|151|213|    const MAX_CONTAINERS: usize = 20;
152|152|214|    let truncated = running_lines.len() > MAX_CONTAINERS || stopped_lines.len() > MAX_CONTAINERS;
153|153|215|
154|154|216|    let mut rtk = String::new();
155|155|217|    rtk.push_str(&format!("[docker] {} running:\n", running_lines.len()));
156|156|218|    for l in running_lines.iter().take(MAX_CONTAINERS) {
157|157|219|        rtk.push_str(l);
158|158|220|    }
159|159|221|    if running_lines.len() > MAX_CONTAINERS {
160|160|222|        rtk.push_str(&format!(
161|161|223|            "  … +{} more\n",
162|162|224|            running_lines.len() - MAX_CONTAINERS
163|163|225|        ));
164|164|226|    }
165|165|227|    if !stopped_lines.is_empty() {
166|166|228|        rtk.push_str(&format!(
167|167|229|            "[docker] {} stopped/exited:\n",
168|168|230|            stopped_lines.len()
169|169|231|        ));
170|170|232|        for l in stopped_lines.iter().take(MAX_CONTAINERS) {
171|171|233|            rtk.push_str(l);
172|172|234|        }
173|173|235|        if stopped_lines.len() > MAX_CONTAINERS {
174|174|236|            rtk.push_str(&format!(
175|175|237|                "  … +{} more\n",
176|176|238|                stopped_lines.len() - MAX_CONTAINERS
177|177|239|            ));
178|178|240|        }
179|179|241|    }
180|180|242|    if truncated {
181|181|243|        let full: String = running_lines.iter().chain(stopped_lines.iter()).cloned().collect();
182|182|244|        if let Some(hint) = crate::core::tee::force_tee_hint(&full, "docker-ps-a") {
183|183|245|            rtk.push_str(&format!("{}\n", hint));
184|184|246|        }
185|185|247|    }
186|186|248|
187|187|249|    print!("{}", rtk);
188|188|250|    timer.track("docker ps -a", "rtk docker ps -a", &raw, &rtk);
189|189|251|    Ok(0)
190|190|252|}
191|191|253|
192|192|254|fn format_container_line(line: &str, with_ports: bool) -> Option<String> {
193|193|255|    let parts: Vec<&str> = line.split('\t').collect();
194|194|256|    format_container_line_from_parts(&parts, with_ports)
195|195|257|}
196|196|258|
197|197|259|fn format_container_line_from_parts(parts: &[&str], with_ports: bool) -> Option<String> {
198|198|260|    if parts.len() < 4 {
199|199|261|        return None;
200|200|262|    }
201|201|263|    let id = &parts[0][..12.min(parts[0].len())];
202|202|264|    let name = parts[1];
203|203|265|    let status = parts[2].trim();
204|204|266|    let short_image = parts[3].split('/').next_back().unwrap_or("");
205|205|267|    let port_suffix = if with_ports {
206|206|268|        let ports = compact_ports(parts.get(4).unwrap_or(&""));
207|207|269|        if ports == "-" {
208|208|270|            String::new()
209|209|271|        } else {
210|210|272|            format!(" [{}]", ports)
211|211|273|        }
212|212|274|    } else {
213|213|275|        String::new()
214|214|276|    };
215|215|277|    Some(format!(
216|216|278|        "  {} {} ({}) {}{}\n",
217|217|279|        id, name, short_image, status, port_suffix
218|218|280|    ))
219|219|281|}
220|220|282|
221|221|283|fn docker_images(_verbose: u8) -> Result<i32> {
222|222|284|    let timer = tracking::TimedExecution::start();
223|223|285|
224|224|286|    let raw = exec_capture(resolved_command("docker").args(["images"]))
225|225|287|        .map(|r| r.stdout)
226|226|288|        .unwrap_or_default();
227|227|289|
228|228|290|    let result = exec_capture(resolved_command("docker").args([
229|229|291|        "images",
230|230|292|        "--format",
231|231|293|        "{{.Repository}}:{{.Tag}}\t{{.Size}}",
232|232|294|    ]))
233|233|295|    .context("Failed to run docker images")?;
234|234|296|
235|235|297|    if !result.success() {
236|236|298|        eprint!("{}", result.stderr);
237|237|299|        timer.track("docker images", "rtk docker images", &raw, &raw);
238|238|300|        return Ok(result.exit_code);
239|239|301|    }
240|240|302|
241|241|303|    let stdout = result.stdout;
242|242|304|    let lines: Vec<&str> = stdout.lines().collect();
243|243|305|    let mut rtk = String::new();
244|244|306|
245|245|307|    if lines.is_empty() {
246|246|308|        rtk.push_str("[docker] 0 images");
247|247|309|        println!("{}", rtk);
248|248|310|        timer.track("docker images", "rtk docker images", &raw, &rtk);
249|249|311|        return Ok(0);
250|250|312|    }
251|251|313|
252|252|314|    let mut total_size_mb: f64 = 0.0;
253|253|315|    for line in &lines {
254|254|316|        let parts: Vec<&str> = line.split('\t').collect();
255|255|317|        if let Some(size_str) = parts.get(1) {
256|256|318|            if size_str.contains("GB") {
257|257|319|                if let Ok(n) = size_str.replace("GB", "").trim().parse::<f64>() {
258|258|320|                    total_size_mb += n * 1024.0;
259|259|321|                }
260|260|322|            } else if size_str.contains("MB") {
261|261|323|                if let Ok(n) = size_str.replace("MB", "").trim().parse::<f64>() {
262|262|324|                    total_size_mb += n;
263|263|325|                }
264|264|326|            }
265|265|327|        }
266|266|328|    }
267|267|329|
268|268|330|    let total_display = if total_size_mb > 1024.0 {
269|269|331|        format!("{:.1}GB", total_size_mb / 1024.0)
270|270|332|    } else {
271|271|333|        format!("{:.0}MB", total_size_mb)
272|272|334|    };
273|273|335|    rtk.push_str(&format!(
274|274|336|        "[docker] {} images ({})\n",
275|275|337|        lines.len(),
276|276|338|        total_display
277|277|339|    ));
278|278|340|
279|279|341|342|    // a full image list is an inventory query, like pip list.
280|280|343|    const MAX_IMAGES: usize = CAP_INVENTORY;
281|281|344|    let image_lines: Vec<String> = lines
282|282|345|        .iter()
283|283|346|        .map(|line| {
284|284|347|            let parts: Vec<&str> = line.split('\t').collect();
285|285|348|            let image = parts.first().copied().unwrap_or("");
286|286|349|            let size = parts.get(1).copied().unwrap_or("");
287|287|350|            format!("  {} [{}]\n", image, size)
288|288|351|        })
289|289|352|        .collect();
290|290|353|
291|291|354|    let mut full_rtk = rtk.clone();
292|292|355|    for l in &image_lines {
293|293|356|        full_rtk.push_str(l);
294|294|357|    }
295|295|358|
296|296|359|    for l in image_lines.iter().take(MAX_IMAGES) {
297|297|360|        rtk.push_str(l);
298|298|361|    }
299|299|362|    if image_lines.len() > MAX_IMAGES {
300|300|363|        rtk.push_str(&format!("  … +{} more\n", image_lines.len() - MAX_IMAGES));
301|301|364|        if let Some(hint) = crate::core::tee::force_tee_tail_hint(&full_rtk, "docker-images", MAX_IMAGES + 2) {
302|302|365|            rtk.push_str(&format!("{}\n", hint));
303|303|366|        }
304|304|367|385|    }
305|305|386|
306|306|387|    print!("{}", rtk);
307|307|388|    timer.track("docker images", "rtk docker images", &raw, &rtk);
308|308|389|    Ok(0)
309|309|390|}
310|310|391|
311|311|392|fn docker_logs(args: &[String], _verbose: u8) -> Result<i32> {
312|312|393|    let container = args.first().map(|s| s.as_str()).unwrap_or("");
313|313|394|    if container.is_empty() {
314|314|395|        println!("Usage: rtk docker logs <container>");
315|315|396|        return Ok(0);
316|316|397|    }
317|317|398|
318|318|399|    let mut cmd = resolved_command("docker");
319|319|400|    cmd.args(["logs", "--tail", "100", container]);
320|320|401|
321|321|402|    let label = format!("logs {}", container);
322|322|403|    runner::run_filtered(
323|323|404|        cmd,
324|324|405|        "docker",
325|325|406|        &label,
326|326|407|        |raw| {
327|327|408|            format!(
328|328|409|                "[docker] Logs for {}:\n{}",
329|329|410|                container,
330|330|411|                crate::log_cmd::run_stdin_str(raw)
331|331|412|            )
332|332|413|        },
333|333|414|        RunOptions::default().early_exit_on_failure(),
334|334|415|    )
335|335|416|}
336|336|417|
337|337|418|fn kubectl_pods(args: &[String], _verbose: u8) -> Result<i32> {
338|338|419|    let mut cmd = resolved_command("kubectl");
339|339|420|    cmd.args(["get", "pods", "-o", "json"]);
340|340|421|    for arg in args {
341|341|422|        cmd.arg(arg);
342|342|423|    }
343|343|424|    run_kubectl_json(cmd, "get pods", format_kubectl_pods)
344|344|425|}
345|345|426|
346|346|427|fn format_kubectl_pods(json: &Value) -> String {
347|347|428|    let Some(pods) = json["items"].as_array().filter(|a| !a.is_empty()) else {
348|348|429|        return "No pods found\n".to_string();
349|349|430|    };
350|350|431|    let (mut running, mut pending, mut failed, mut restarts_total) = (0, 0, 0, 0i64);
351|351|432|    let mut issues: Vec<String> = Vec::new();
352|352|433|
353|353|434|    for pod in pods {
354|354|435|        let ns = pod["metadata"]["namespace"].as_str().unwrap_or("-");
355|355|436|        let name = pod["metadata"]["name"].as_str().unwrap_or("-");
356|356|437|        let phase = pod["status"]["phase"].as_str().unwrap_or("Unknown");
357|357|438|
358|358|439|        if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
359|359|440|            for c in containers {
360|360|441|                restarts_total += c["restartCount"].as_i64().unwrap_or(0);
361|361|442|            }
362|362|443|        }
363|363|444|
364|364|445|        match phase {
365|365|446|            "Running" => running += 1,
366|366|447|            "Pending" => {
367|367|448|                pending += 1;
368|368|449|                issues.push(format!("{}/{} Pending", ns, name));
369|369|450|            }
370|370|451|            "Failed" | "Error" => {
371|371|452|                failed += 1;
372|372|453|                issues.push(format!("{}/{} {}", ns, name, phase));
373|373|454|            }
374|374|455|            _ => {
375|375|456|                if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
376|376|457|                    for c in containers {
377|377|458|                        if let Some(w) = c["state"]["waiting"]["reason"].as_str() {
378|378|459|                            if w.contains("CrashLoop") || w.contains("Error") {
379|379|460|                                failed += 1;
380|380|461|                                issues.push(format!("{}/{} {}", ns, name, w));
381|381|462|                            }
382|382|463|                        }
383|383|464|                    }
384|384|465|                }
385|385|466|            }
386|386|467|        }
387|387|468|    }
388|388|469|
389|389|470|    let mut parts = Vec::new();
390|390|471|    if running > 0 {
391|391|472|        parts.push(format!("{}", running));
392|392|473|    }
393|393|474|    if pending > 0 {
394|394|475|        parts.push(format!("{} pending", pending));
395|395|476|    }
396|396|477|    if failed > 0 {
397|397|478|        parts.push(format!("{} [x]", failed));
398|398|479|    }
399|399|480|    if restarts_total > 0 {
400|400|481|        parts.push(format!("{} restarts", restarts_total));
401|401|482|    }
402|402|483|
403|403|484|    let mut out = format!("{} pods: {}\n", pods.len(), parts.join(", "));
404|404|485|    if !issues.is_empty() {
405|405|486|        const MAX_PODS_ISSUES: usize = CAP_WARNINGS;
406|406|487|        out.push_str("[warn] Issues:\n");
407|407|488|        for issue in issues.iter().take(MAX_PODS_ISSUES) {
408|408|489|            out.push_str(&format!("  {}\n", issue));
409|409|490|        }
410|410|491|        if issues.len() > MAX_PODS_ISSUES {
411|411|492|            out.push_str(&format!("  … +{} more", issues.len() - MAX_PODS_ISSUES));
412|412|493|            let all_issues = issues.join("\n");
413|413|494|            if let Some(hint) =
414|414|495|                crate::core::tee::force_tee_tail_hint(&all_issues, "kubectl-pods", MAX_PODS_ISSUES + 1)
415|415|496|            {
416|416|497|                out.push_str(&format!(" {}", hint));
417|417|498|            }
418|418|499|        }
419|419|500|    }
420|420|501|    out
421|421|502|}
422|422|503|
423|423|504|fn kubectl_services(args: &[String], _verbose: u8) -> Result<i32> {
424|424|505|    let mut cmd = resolved_command("kubectl");
425|425|506|    cmd.args(["get", "services", "-o", "json"]);
426|426|507|    for arg in args {
427|427|508|        cmd.arg(arg);
428|428|509|    }
429|429|510|    run_kubectl_json(cmd, "get services", format_kubectl_services)
430|430|511|}
431|431|512|
432|432|513|fn format_kubectl_services(json: &Value) -> String {
433|433|514|    let Some(services) = json["items"].as_array().filter(|a| !a.is_empty()) else {
434|434|515|        return "No services found\n".to_string();
435|435|516|    };
436|436|517|    let mut out = format!("{} services:\n", services.len());
437|437|518|
438|438|519|    let all_lines: Vec<String> = services
439|439|520|        .iter()
440|440|521|        .map(|svc| {
441|441|522|            let ns = svc["metadata"]["namespace"].as_str().unwrap_or("-");
442|442|523|            let name = svc["metadata"]["name"].as_str().unwrap_or("-");
443|443|524|            let svc_type = svc["spec"]["type"].as_str().unwrap_or("-");
444|444|525|            let ports: Vec<String> = svc["spec"]["ports"]
445|445|526|                .as_array()
446|446|527|                .map(|arr| {
447|447|528|                    arr.iter()
448|448|529|                        .map(|p| {
449|449|530|                            let port = p["port"].as_i64().unwrap_or(0);
450|450|531|                            let target = p["targetPort"]
451|451|532|                                .as_i64()
452|452|533|                                .or_else(|| p["targetPort"].as_str().and_then(|s| s.parse().ok()))
453|453|534|                                .unwrap_or(port);
454|454|535|                            if port == target {
455|455|536|                                format!("{}", port)
456|456|537|                            } else {
457|457|538|                                format!("{}→{}", port, target)
458|458|539|                            }
459|459|540|                        })
460|460|541|                        .collect()
461|461|542|                })
462|462|543|                .unwrap_or_default();
463|463|544|            format!("  {}/{} {} [{}]", ns, name, svc_type, ports.join(","))
464|464|545|        })
465|465|546|        .collect();
466|466|547|
467|467|548|    const MAX_KUBECTL_SERVICES: usize = CAP_LIST;
468|468|549|    for line in all_lines.iter().take(MAX_KUBECTL_SERVICES) {
469|469|550|        out.push_str(&format!("{}\n", line));
470|470|551|    }
471|471|552|    if all_lines.len() > MAX_KUBECTL_SERVICES {
472|472|553|        out.push_str(&format!("  … +{} more", all_lines.len() - MAX_KUBECTL_SERVICES));
473|473|554|        let all_text = all_lines.join("\n");
474|474|555|        if let Some(hint) =
475|475|556|            crate::core::tee::force_tee_tail_hint(&all_text, "kubectl-services", MAX_KUBECTL_SERVICES + 1)
476|476|557|        {
477|477|558|            out.push_str(&format!(" {}", hint));
478|478|559|        }
479|479|560|        out.push('\n');
480|480|561|    }
481|481|562|    out
482|482|563|}
483|483|564|
484|484|565|fn kubectl_logs(args: &[String], _verbose: u8) -> Result<i32> {
485|485|566|    let pod = args.first().map(|s| s.as_str()).unwrap_or("");
486|486|567|    if pod.is_empty() {
487|487|568|        println!("Usage: rtk kubectl logs <pod>");
488|488|569|        return Ok(0);
489|489|570|    }
490|490|571|
491|491|572|    let mut cmd = resolved_command("kubectl");
492|492|573|    cmd.args(["logs", "--tail", "100", pod]);
493|493|574|    for arg in args.iter().skip(1) {
494|494|575|        cmd.arg(arg);
495|495|576|    }
496|496|577|
497|497|578|    let label = format!("logs {}", pod);
498|498|579|    runner::run_filtered(
499|499|580|        cmd,
500|500|581|        "kubectl",
501|501|582|        &label,
502|502|583|        |stdout| {
503|503|584|            format!(
504|504|585|                "Logs for {}:\n{}",
505|505|586|                pod,
506|506|587|                crate::log_cmd::run_stdin_str(stdout)
507|507|588|            )
508|508|589|        },
509|509|590|        RunOptions::stdout_only().early_exit_on_failure(),
510|510|591|    )
511|511|592|}
512|512|593|
513|513|594|/// Format `docker compose ps --format` output into compact form.
514|514|595|/// Expects tab-separated lines: Name\tImage\tStatus\tPorts
515|515|596|/// (no header row — `--format` output is headerless)
516|516|597|pub fn format_compose_ps(raw: &str) -> String {
517|517|598|    const MAX_COMPOSE_SERVICES: usize = CAP_LIST;
518|518|599|    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
519|519|600|
520|520|601|    if lines.is_empty() {
521|521|602|        return "[compose] 0 services".to_string();
522|522|603|    }
523|523|604|
524|524|605|    let mut result = format!("[compose] {} services:\n", lines.len());
525|525|606|
526|526|607|    // Pre-build all formatted lines so the tee file matches what the agent sees.
527|527|608|    let all_formatted: Vec<String> = lines
528|528|609|        .iter()
529|529|610|        .filter_map(|line| {
530|530|611|            let parts: Vec<&str> = line.split('\t').collect();
531|531|612|            if parts.len() < 4 {
532|532|613|                return None;
533|533|614|            }
534|534|615|            let name = parts[0];
535|535|616|            let image = parts[1];
536|536|617|            let status = parts[2];
537|537|618|            let ports = parts[3];
538|538|619|            let short_image = image.split('/').next_back().unwrap_or(image);
539|539|620|            let port_str = if ports.trim().is_empty() {
540|540|621|                String::new()
541|541|622|            } 
542|542|
543|543|... [OUTPUT TRUNCATED - 5332 chars omitted out of 55332 total] ...
544|544|
545|545|er ps", "rtk docker ps", &raw, &rtk);
546|546|        return Ok(0);
547|547|    }
548|548|
549|549|<<<<<<< HEAD
550|550|    let count = stdout.lines().count();
551|551|    rtk.push_str(&format!("[docker] {} containers:\n", count));
552|552|
553|553|    for line in stdout.lines().take(15) {
554|554|        let parts: Vec<&str> = line.split('\t').collect();
555|555|        if parts.len() >= 4 {
556|556|            let id = &parts[0][..12.min(parts[0].len())];
557|557|            let name = parts[1];
558|558|            let short_image = parts
559|559|                .get(3)
560|560|                .unwrap_or(&"")
561|561|                .split('/')
562|562|                .next_back()
563|563|                .unwrap_or("");
564|564|            let ports = compact_ports(parts.get(4).unwrap_or(&""));
565|565|            if ports == "-" {
566|566|                rtk.push_str(&format!("  {} {} ({})\n", id, name, short_image));
567|567|            } else {
568|568|                rtk.push_str(&format!(
569|569|                    "  {} {} ({}) [{}]\n",
570|570|                    id, name, short_image, ports
571|571|                ));
572|572|            }
573|573|        }
574|574|    }
575|575|    if count > 15 {
576|576|        rtk.push_str(&format!("  ... +{} more", count - 15));
577|577|    }
578|578|
579|579|    print!("{}", rtk);
580|580|    timer.track("docker ps", "rtk docker ps", &raw, &rtk);
581|581|    Ok(0)
582|582|}
583|583|
584|584|fn docker_ps_all(_verbose: u8) -> Result<i32> {
585|585|    let timer = tracking::TimedExecution::start();
586|586|
587|587|    let raw = exec_capture(resolved_command("docker").args(["ps", "-a"]))
588|588|        .map(|r| r.stdout)
589|589|        .unwrap_or_default();
590|590|
591|591|    let result = exec_capture(resolved_command("docker").args([
592|592|        "ps",
593|593|        "-a",
594|594|        "--format",
595|595|        "{{.State}}\t{{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}",
596|596|    ]))
597|597|    .context("Failed to run docker ps -a")?;
598|598|
599|599|    if !result.success() {
600|600|        eprint!("{}", result.stderr);
601|601|        timer.track("docker ps -a", "rtk docker ps -a", &raw, &raw);
602|602|        return Ok(result.exit_code);
603|603|    }
604|604|
605|605|    let mut running_lines: Vec<String> = Vec::new();
606|606|    let mut stopped_lines: Vec<String> = Vec::new();
607|607|    for line in result.stdout.lines().filter(|l| !l.trim().is_empty()) {
608|608|        let parts: Vec<&str> = line.split('\t').collect();
609|609|        let state = parts.first().copied().unwrap_or("");
610|610|        let is_running = matches!(state, "running" | "restarting");
611|611|        if let Some(entry) = format_container_line_from_parts(&parts[1..], is_running) {
612|612|            if is_running {
613|613|                running_lines.push(entry);
614|614|            } else {
615|615|                stopped_lines.push(entry);
616|616|            }
617|617|        }
618|618|    }
619|619|
620|620|    const MAX_CONTAINERS: usize = 20;
621|621|    let truncated = running_lines.len() > MAX_CONTAINERS || stopped_lines.len() > MAX_CONTAINERS;
622|622|
623|623|    let mut rtk = String::new();
624|624|    rtk.push_str(&format!("[docker] {} running:\n", running_lines.len()));
625|625|    for l in running_lines.iter().take(MAX_CONTAINERS) {
626|626|        rtk.push_str(l);
627|627|    }
628|628|    if running_lines.len() > MAX_CONTAINERS {
629|629|        rtk.push_str(&format!(
630|630|            "  … +{} more\n",
631|631|            running_lines.len() - MAX_CONTAINERS
632|632|        ));
633|633|    }
634|634|    if !stopped_lines.is_empty() {
635|635|        rtk.push_str(&format!(
636|636|            "[docker] {} stopped/exited:\n",
637|637|            stopped_lines.len()
638|638|        ));
639|639|        for l in stopped_lines.iter().take(MAX_CONTAINERS) {
640|640|            rtk.push_str(l);
641|641|        }
642|642|        if stopped_lines.len() > MAX_CONTAINERS {
643|643|            rtk.push_str(&format!(
644|644|                "  … +{} more\n",
645|645|                stopped_lines.len() - MAX_CONTAINERS
646|646|            ));
647|647|        }
648|648|    }
649|649|    if truncated {
650|650|        let full: String = running_lines.iter().chain(stopped_lines.iter()).cloned().collect();
651|651|        if let Some(hint) = crate::core::tee::force_tee_hint(&full, "docker-ps-a") {
652|652|            rtk.push_str(&format!("{}\n", hint));
653|653|        }
654|654|    }
655|655|
656|656|    print!("{}", rtk);
657|657|    timer.track("docker ps -a", "rtk docker ps -a", &raw, &rtk);
658|658|    Ok(0)
659|659|}
660|660|
661|661|fn format_container_line(line: &str, with_ports: bool) -> Option<String> {
662|662|    let parts: Vec<&str> = line.split('\t').collect();
663|663|    format_container_line_from_parts(&parts, with_ports)
664|664|}
665|665|
666|666|fn format_container_line_from_parts(parts: &[&str], with_ports: bool) -> Option<String> {
667|667|    if parts.len() < 4 {
668|668|        return None;
669|669|    }
670|670|    let id = &parts[0][..12.min(parts[0].len())];
671|671|    let name = parts[1];
672|672|    let status = parts[2].trim();
673|673|    let short_image = parts[3].split('/').next_back().unwrap_or("");
674|674|    let port_suffix = if with_ports {
675|675|        let ports = compact_ports(parts.get(4).unwrap_or(&""));
676|676|        if ports == "-" {
677|677|            String::new()
678|678|        } else {
679|679|            format!(" [{}]", ports)
680|680|        }
681|681|    } else {
682|682|        String::new()
683|683|    };
684|684|    Some(format!(
685|685|        "  {} {} ({}) {}{}\n",
686|686|        id, name, short_image, status, port_suffix
687|687|    ))
688|688|}
689|689|
690|690|fn docker_images(_verbose: u8) -> Result<i32> {
691|691|    let timer = tracking::TimedExecution::start();
692|692|
693|693|    let raw = exec_capture(resolved_command("docker").args(["images"]))
694|694|        .map(|r| r.stdout)
695|695|        .unwrap_or_default();
696|696|
697|697|    let result = exec_capture(resolved_command("docker").args([
698|698|        "images",
699|699|        "--format",
700|700|        "{{.Repository}}:{{.Tag}}\t{{.Size}}",
701|701|    ]))
702|702|    .context("Failed to run docker images")?;
703|703|
704|704|    if !result.success() {
705|705|        eprint!("{}", result.stderr);
706|706|        timer.track("docker images", "rtk docker images", &raw, &raw);
707|707|        return Ok(result.exit_code);
708|708|    }
709|709|
710|710|    let stdout = result.stdout;
711|711|    let lines: Vec<&str> = stdout.lines().collect();
712|712|    let mut rtk = String::new();
713|713|
714|714|    if lines.is_empty() {
715|715|        rtk.push_str("[docker] 0 images");
716|716|        println!("{}", rtk);
717|717|        timer.track("docker images", "rtk docker images", &raw, &rtk);
718|718|        return Ok(0);
719|719|    }
720|720|
721|721|    let mut total_size_mb: f64 = 0.0;
722|722|    for line in &lines {
723|723|        let parts: Vec<&str> = line.split('\t').collect();
724|724|        if let Some(size_str) = parts.get(1) {
725|725|            if size_str.contains("GB") {
726|726|                if let Ok(n) = size_str.replace("GB", "").trim().parse::<f64>() {
727|727|                    total_size_mb += n * 1024.0;
728|728|                }
729|729|            } else if size_str.contains("MB") {
730|730|                if let Ok(n) = size_str.replace("MB", "").trim().parse::<f64>() {
731|731|                    total_size_mb += n;
732|732|                }
733|733|            }
734|734|        }
735|735|    }
736|736|
737|737|    let total_display = if total_size_mb > 1024.0 {
738|738|        format!("{:.1}GB", total_size_mb / 1024.0)
739|739|    } else {
740|740|        format!("{:.0}MB", total_size_mb)
741|741|    };
742|742|    rtk.push_str(&format!(
743|743|        "[docker] {} images ({})\n",
744|744|        lines.len(),
745|745|        total_display
746|746|    ));
747|747|
748|748|    for line in lines.iter().take(15) {
749|749|        let parts: Vec<&str> = line.split('\t').collect();
750|750|        if !parts.is_empty() {
751|751|            let image = parts[0];
752|752|            let size = parts.get(1).unwrap_or(&"");
753|753|            let short = if image.len() > 40 {
754|754|                format!("...{}", &image[image.len() - 37..])
755|755|            } else {
756|756|                image.to_string()
757|757|            };
758|758|            rtk.push_str(&format!("  {} [{}]\n", short, size));
759|759|        }
760|760|    }
761|761|    if lines.len() > 15 {
762|762|        rtk.push_str(&format!("  ... +{} more", lines.len() - 15));
763|763|    }
764|764|
765|765|    print!("{}", rtk);
766|766|    timer.track("docker images", "rtk docker images", &raw, &rtk);
767|767|    Ok(0)
768|768|}
769|769|
770|770|fn docker_logs(args: &[String], _verbose: u8) -> Result<i32> {
771|771|    let container = args.first().map(|s| s.as_str()).unwrap_or("");
772|772|    if container.is_empty() {
773|773|        println!("Usage: rtk docker logs <container>");
774|774|        return Ok(0);
775|775|    }
776|776|
777|777|    let mut cmd = resolved_command("docker");
778|778|    cmd.args(["logs", "--tail", "100", container]);
779|779|
780|780|    let label = format!("logs {}", container);
781|781|    runner::run_filtered(
782|782|        cmd,
783|783|        "docker",
784|784|        &label,
785|785|        |raw| {
786|786|            format!(
787|787|                "[docker] Logs for {}:\n{}",
788|788|                container,
789|789|                crate::log_cmd::run_stdin_str(raw)
790|790|            )
791|791|        },
792|792|        RunOptions::default().early_exit_on_failure(),
793|793|    )
794|794|}
795|795|
796|796|fn kubectl_pods(args: &[String], _verbose: u8) -> Result<i32> {
797|797|    let mut cmd = resolved_command("kubectl");
798|798|    cmd.args(["get", "pods", "-o", "json"]);
799|799|    for arg in args {
800|800|        cmd.arg(arg);
801|801|    }
802|802|    run_kubectl_json(cmd, "get pods", format_kubectl_pods)
803|803|}
804|804|
805|805|fn format_kubectl_pods(json: &Value) -> String {
806|806|    let Some(pods) = json["items"].as_array().filter(|a| !a.is_empty()) else {
807|807|        return "No pods found\n".to_string();
808|808|    };
809|809|    let (mut running, mut pending, mut failed, mut restarts_total) = (0, 0, 0, 0i64);
810|810|    let mut issues: Vec<String> = Vec::new();
811|811|
812|812|    for pod in pods {
813|813|        let ns = pod["metadata"]["namespace"].as_str().unwrap_or("-");
814|814|        let name = pod["metadata"]["name"].as_str().unwrap_or("-");
815|815|        let phase = pod["status"]["phase"].as_str().unwrap_or("Unknown");
816|816|
817|817|        if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
818|818|            for c in containers {
819|819|                restarts_total += c["restartCount"].as_i64().unwrap_or(0);
820|820|            }
821|821|        }
822|822|
823|823|        match phase {
824|824|            "Running" => running += 1,
825|825|            "Pending" => {
826|826|                pending += 1;
827|827|                issues.push(format!("{}/{} Pending", ns, name));
828|828|            }
829|829|            "Failed" | "Error" => {
830|830|                failed += 1;
831|831|                issues.push(format!("{}/{} {}", ns, name, phase));
832|832|            }
833|833|            _ => {
834|834|                if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
835|835|                    for c in containers {
836|836|                        if let Some(w) = c["state"]["waiting"]["reason"].as_str() {
837|837|                            if w.contains("CrashLoop") || w.contains("Error") {
838|838|                                failed += 1;
839|839|                                issues.push(format!("{}/{} {}", ns, name, w));
840|840|                            }
841|841|                        }
842|842|                    }
843|843|                }
844|844|            }
845|845|        }
846|846|    }
847|847|
848|848|    let mut parts = Vec::new();
849|849|    if running > 0 {
850|850|        parts.push(format!("{}", running));
851|851|    }
852|852|    if pending > 0 {
853|853|        parts.push(format!("{} pending", pending));
854|854|    }
855|855|    if failed > 0 {
856|856|        parts.push(format!("{} [x]", failed));
857|857|    }
858|858|    if restarts_total > 0 {
859|859|        parts.push(format!("{} restarts", restarts_total));
860|860|    }
861|861|
862|862|    let mut out = format!("{} pods: {}\n", pods.len(), parts.join(", "));
863|863|    if !issues.is_empty() {
864|864|        out.push_str("[warn] Issues:\n");
865|865|        for issue in issues.iter().take(10) {
866|866|            out.push_str(&format!("  {}\n", issue));
867|867|        }
868|868|        if issues.len() > 10 {
869|869|            out.push_str(&format!("  ... +{} more", issues.len() - 10));
870|870|        }
871|871|    }
872|872|    out
873|873|}
874|874|
875|875|fn kubectl_services(args: &[String], _verbose: u8) -> Result<i32> {
876|876|    let mut cmd = resolved_command("kubectl");
877|877|    cmd.args(["get", "services", "-o", "json"]);
878|878|    for arg in args {
879|879|        cmd.arg(arg);
880|880|    }
881|881|    run_kubectl_json(cmd, "get services", format_kubectl_services)
882|882|}
883|883|
884|884|fn format_kubectl_services(json: &Value) -> String {
885|885|    let Some(services) = json["items"].as_array().filter(|a| !a.is_empty()) else {
886|886|        return "No services found\n".to_string();
887|887|    };
888|888|    let mut out = format!("{} services:\n", services.len());
889|889|
890|890|    for svc in services.iter().take(15) {
891|891|        let ns = svc["metadata"]["namespace"].as_str().unwrap_or("-");
892|892|        let name = svc["metadata"]["name"].as_str().unwrap_or("-");
893|893|        let svc_type = svc["spec"]["type"].as_str().unwrap_or("-");
894|894|        let ports: Vec<String> = svc["spec"]["ports"]
895|895|            .as_array()
896|896|            .map(|arr| {
897|897|                arr.iter()
898|898|                    .map(|p| {
899|899|                        let port = p["port"].as_i64().unwrap_or(0);
900|900|                        let target = p["targetPort"]
901|901|                            .as_i64()
902|902|                            .or_else(|| p["targetPort"].as_str().and_then(|s| s.parse().ok()))
903|903|                            .unwrap_or(port);
904|904|                        if port == target {
905|905|                            format!("{}", port)
906|906|                        } else {
907|907|                            format!("{}→{}", port, target)
908|908|                        }
909|909|                    })
910|910|                    .collect()
911|911|            })
912|912|            .unwrap_or_default();
913|913|        out.push_str(&format!(
914|914|            "  {}/{} {} [{}]\n",
915|915|            ns,
916|916|            name,
917|917|            svc_type,
918|918|            ports.join(",")
919|919|        ));
920|920|    }
921|921|    if services.len() > 15 {
922|922|        out.push_str(&format!("  ... +{} more", services.len() - 15));
923|923|    }
924|924|    out
925|925|}
926|926|
927|927|fn kubectl_logs(args: &[String], _verbose: u8) -> Result<i32> {
928|928|    let pod = args.first().map(|s| s.as_str()).unwrap_or("");
929|929|    if pod.is_empty() {
930|930|        println!("Usage: rtk kubectl logs <pod>");
931|931|        return Ok(0);
932|932|    }
933|933|
934|934|    let mut cmd = resolved_command("kubectl");
935|935|    cmd.args(["logs", "--tail", "100", pod]);
936|936|    for arg in args.iter().skip(1) {
937|937|        cmd.arg(arg);
938|938|    }
939|939|
940|940|    let label = format!("logs {}", pod);
941|941|    runner::run_filtered(
942|942|        cmd,
943|943|        "kubectl",
944|944|        &label,
945|945|        |stdout| {
946|946|            format!(
947|947|                "Logs for {}:\n{}",
948|948|                pod,
949|949|                crate::log_cmd::run_stdin_str(stdout)
950|950|            )
951|951|        },
952|952|        RunOptions::stdout_only().early_exit_on_failure(),
953|953|    )
954|954|}
955|955|
956|956|/// Format `docker compose ps --format` output into compact form.
957|957|/// Expects tab-separated lines: Name\tImage\tStatus\tPorts
958|958|/// (no header row — `--format` output is headerless)
959|959|pub fn format_compose_ps(raw: &str) -> String {
960|960|    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
961|961|
962|962|    if lines.is_empty() {
963|963|        return "[compose] 0 services".to_string();
964|964|    }
965|965|
966|966|    let mut result = format!("[compose] {} services:\n", lines.len());
967|967|
968|968|    for line in lines.iter().take(20) {
969|969|        let parts: Vec<&str> = line.split('\t').collect();
970|970|        if parts.len() >= 4 {
971|971|            let name = parts[0];
972|972|            let image = parts[1];
973|973|            let status = parts[2];
974|974|            let ports = parts[3];
975|975|
976|976|            let short_image = image.split('/').next_back().unwrap_or(image);
977|977|
978|978|            let port_str = if ports.trim().is_empty() {
979|979|                String::new()
980|980|            } else {
981|981|                let compact = compact_ports(ports.trim());
982|982|                if compact == "-" {
983|983|                    String::new()
984|984|                } else {
985|985|                    format!(" [{}]", compact)
986|986|                }
987|987|            };
988|988|
989|989|            result.push_str(&format!(
990|990|                "  {} ({}) {}{}\n",
991|991|                name, short_image, status, port_str
992|992|            ));
993|993|        }
994|994|    }
995|995|    if lines.len() > 20 {
996|996|        result.push_str(&format!("  ... +{} more\n", lines.len() - 20));
997|997|    }
998|998|
999|999|    result.trim_end().to_string()
1000|1000|}
1001|1001|
1002|1002|/// Format `docker compose logs` output into compact form
1003|1003|pub fn format_compose_logs(raw: &str) -> String {
1004|1004|    if raw.trim().is_empty() {
1005|1005|        return "[compose] No logs".to_string();
1006|1006|    }
1007|1007|
1008|1008|    // docker compose logs prefixes each line with "service-N  | "
1009|1009|    // Use the existing log deduplication engine
1010|1010|    let analyzed = crate::log_cmd::run_stdin_str(raw);
1011|1011|    format!("[compose] Logs:\n{}", analyzed)
1012|1012|}
1013|1013|
1014|1014|/// Format `docker compose build` output into compact summary
1015|1015|pub fn format_compose_build(raw: &str) -> String {
1016|1016|    if raw.trim().is_empty() {
1017|1017|        return "[compose] Build: no output".to_string();
1018|1018|    }
1019|1019|
1020|1020|    let mut result = String::new();
1021|1021|
1022|1022|    // Extract the summary line: "[+] Building 12.3s (8/8) FINISHED"
1023|1023|    for line in raw.lines() {
1024|1024|        if line.contains("Building") && line.contains("FINISHED") {
1025|1025|            result.push_str(&format!("[compose] {}\n", line.trim()));
1026|1026|            break;
1027|1027|        }
1028|1028|    }
1029|1029|
1030|1030|    if result.is_empty() {
1031|1031|        // No FINISHED line found — might still be building or errored
1032|1032|        if let Some(line) = raw.lines().find(|l| l.contains("Building")) {
1033|1033|            result.push_str(&format!("[compose] {}\n", line.trim()));
1034|1034|        } else {
1035|1035|            result.push_str("[compose] Build:\n");
1036|1036|        }
1037|1037|    }
1038|1038|
1039|1039|    // Collect unique service names from build steps like "[web 1/4]"
1040|1040|    let mut services: Vec<String> = Vec::new();
1041|1041|    // find('[') returns byte offset — use byte slicing throughout
1042|1042|    // '[' and ']' are single-byte ASCII, so byte arithmetic is safe
1043|1043|    for line in raw.lines() {
1044|1044|        if let Some(start) = line.find('[') {
1045|1045|            if let Some(end) = line[start + 1..].find(']') {
1046|1046|                let bracket = &line[start + 1..start + 1 + end];
1047|1047|                let svc = bracket.split_whitespace().next().unwrap_or("");
1048|1048|                if !svc.is_empty() && svc != "+" && !services.contains(&svc.to_string()) {
1049|1049|                    services.push(svc.to_string());
1050|1050|                }
1051|1051|            }
1052|1052|        }
1053|1053|    }
1054|1054|
1055|1055|    if !services.is_empty() {
1056|1056|        result.push_str(&format!("  Services: {}\n", services.join(", ")));
1057|1057|    }
1058|1058|
1059|1059|    // Count build steps (lines starting with " => ")
1060|1060|    let step_count = raw
1061|1061|        .lines()
1062|1062|        .filter(|l| l.trim_start().starts_with("=> "))
1063|1063|        .count();
1064|1064|    if step_count > 0 {
1065|1065|        result.push_str(&format!("  Steps: {}", step_count));
1066|1066|    }
1067|1067|
1068|1068|    result.trim_end().to_string()
1069|1069|}
1070|1070|
1071|1071|fn compact_ports(ports: &str) -> String {
1072|1072|    if ports.is_empty() {
1073|1073|        return "-".to_string();
1074|1074|    }
1075|1075|
1076|1076|    // Extract just the port numbers
1077|1077|    let port_nums: Vec<&str> = ports
1078|1078|        .split(',')
1079|1079|        .filter_map(|p| p.split("->").next().and_then(|s| s.split(':').next_back()))
1080|1080|        .collect();
1081|1081|
1082|1082|    if port_nums.len() <= 3 {
1083|1083|        port_nums.join(", ")
1084|1084|    } else {
1085|1085|        format!(
1086|1086|            "{}, ... +{}",
1087|1087|            port_nums[..2].join(", "),
1088|1088|            port_nums.len() - 2
1089|1089|        )
1090|1090|    }
1091|1091|}
1092|1092|
1093|1093|pub fn run_docker_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
1094|1094|    crate::core::runner::run_passthrough("docker", args, verbose)
1095|1095|}
1096|1096|
1097|1097|/// Run `docker compose ps` (or `docker compose ps -a`) with compact output
1098|1098|pub fn run_compose_ps(all: bool, verbose: u8) -> Result<i32> {
1099|1099|    let timer = tracking::TimedExecution::start();
1100|1100|
1101|1101|    // Raw output for token tracking
1102|1102|    let raw_result = exec_capture(resolved_command("docker").args(["compose", "ps"]))
1103|1103|        .context("Failed to run docker compose ps")?;
1104|1104|
1105|1105|    if !raw_result.success() {
1106|1106|        eprintln!("{}", raw_result.stderr);
1107|1107|        return Ok(raw_result.exit_code);
1108|1108|    }
1109|1109|    let raw = raw_result.stdout;
1110|1110|
1111|1111|    // Structured output for parsing (same pattern as docker_ps)
1112|1112|    let result = exec_capture(resolved_command("docker").args([
1113|1113|        "compose",
1114|1114|        "ps",
1115|1115|        "--format",
1116|1116|        "{{.Name}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}",
1117|1117|    ]))
1118|1118|    .context("Failed to run docker compose ps --format")?;
1119|1119|
1120|1120|    if !result.success() {
1121|1121|        eprintln!("{}", result.stderr);
1122|1122|        return Ok(result.exit_code);
1123|1123|    }
1124|1124|    let structured = result.stdout;
1125|1125|
1126|1126|    if verbose > 0 {
1127|1127|        eprintln!("raw docker compose ps:\n{}", raw);
1128|1128|    }
1129|1129|
1130|1130|    let rtk = format_compose_ps(&structured);
1131|1131|    println!("{}", rtk);
1132|1132|    let label = if all { "docker compose ps -a" } else { "docker compose ps" };
1133|1133|    let rtk_label = if all { "rtk docker compose ps -a" } else { "rtk docker compose ps" };
1134|1134|    timer.track(label, rtk_label, &raw, &rtk);
1135|1135|    Ok(0)
1136|1136|}
1137|1137|
1138|1138|pub fn run_compose_logs(service: Option<&str>, tail: u32, verbose: u8) -> Result<i32> {
1139|1139|    let mut cmd = resolved_command("docker");
1140|1140|    let tail_str = tail.to_string();
1141|1141|    cmd.args(["compose", "logs", "--tail", &tail_str]);
1142|1142|    if let Some(svc) = service {
1143|1143|        cmd.arg(svc);
1144|1144|    }
1145|1145|
1146|1146|    let svc_label = service.unwrap_or("all");
1147|1147|    runner::run_filtered(
1148|1148|        cmd,
1149|1149|        "docker",
1150|1150|        &format!("compose logs {}", svc_label),
1151|1151|        |raw| {
1152|1152|            if verbose > 0 {
1153|1153|                eprintln!("raw docker compose logs:\n{}", raw);
1154|1154|            }
1155|1155|            format_compose_logs(raw)
1156|1156|        },
1157|1157|        RunOptions::default().early_exit_on_failure(),
1158|1158|    )
1159|1159|}
1160|1160|
1161|1161|pub fn run_compose_build(service: Option<&str>, verbose: u8) -> Result<i32> {
1162|1162|    let mut cmd = resolved_command("docker");
1163|1163|    cmd.args(["compose", "build"]);
1164|1164|    if let Some(svc) = service {
1165|1165|        cmd.arg(svc);
1166|1166|    }
1167|1167|
1168|1168|    let svc_label = service.unwrap_or("all");
1169|1169|    runner::run_filtered(
1170|1170|        cmd,
1171|1171|        "docker",
1172|1172|        &format!("compose build {}", svc_label),
1173|1173|        |raw| {
1174|1174|            if verbose > 0 {
1175|1175|                eprintln!("raw docker compose build:\n{}", raw);
1176|1176|            }
1177|1177|            format_compose_build(raw)
1178|1178|        },
1179|1179|        RunOptions::default().early_exit_on_failure(),
1180|1180|    )
1181|1181|}
1182|1182|
1183|1183|pub fn run_compose_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
1184|1184|    let mut combined = vec![OsString::from("compose")];
1185|1185|    combined.extend_from_slice(args);
1186|1186|    crate::core::runner::run_passthrough("docker", &combined, verbose)
1187|1187|}
1188|1188|
1189|1189|pub fn run_kubectl_get(args: &[String], verbose: u8) -> Result<i32> {
1190|1190|    match kubectl_get_target(args) {
1191|1191|        Some(("pods", rest)) => run(ContainerCmd::KubectlPods, rest, verbose),
1192|1192|        Some(("services", rest)) => run(ContainerCmd::KubectlServices, rest, verbose),
1193|1193|        _ => run_kubectl_get_passthrough(args, verbose),
1194|1194|    }
1195|1195|}
1196|1196|
1197|1197|fn kubectl_get_target(args: &[String]) -> Option<(&'static str, &[String])> {
1198|1198|    let resource = args.first()?.as_str();
1199|1199|    let rest = &args[1..];
1200|1200|    if kubectl_get_requests_raw_output(rest) {
1201|1201|        return None;
1202|1202|    }
1203|1203|
1204|1204|    match resource {
1205|1205|        "po" | "pod" | "pods" => Some(("pods", rest)),
1206|1206|        "svc" | "service" | "services" => Some(("services", rest)),
1207|1207|        _ => None,
1208|1208|    }
1209|1209|}
1210|1210|
1211|1211|fn kubectl_get_requests_raw_output(args: &[String]) -> bool {
1212|1212|    args.iter().any(|arg| {
1213|1213|        matches!(
1214|1214|            arg.as_str(),
1215|1215|            "-o" | "--output" | "-w" | "--watch" | "--show-labels" | "--show-kind"
1216|1216|        ) || arg.starts_with("-o")
1217|1217|            || arg.starts_with("--output=")
1218|1218|    })
1219|1219|}
1220|1220|
1221|1221|fn run_kubectl_get_passthrough(args: &[String], verbose: u8) -> Result<i32> {
1222|1222|    let passthrough_args: Vec<OsString> = std::iter::once(OsString::from("get"))
1223|1223|        .chain(args.iter().map(|arg| OsString::from(arg.as_str())))
1224|1224|        .collect();
1225|1225|    run_kubectl_passthrough(&passthrough_args, verbose)
1226|1226|}
1227|1227|
1228|1228|pub fn run_kubectl_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
1229|1229|    crate::core::runner::run_passthrough("kubectl", args, verbose)
1230|1230|}
1231|1231|
1232|1232|#[cfg(test)]
1233|1233|mod tests {
1234|1234|    use super::*;
1235|1235|
1236|1236|    // ── format_compose_ps ──────────────────────────────────
1237|1237|
1238|1238|    #[test]
1239|1239|    fn test_format_compose_ps_basic() {
1240|1240|        // Tab-separated --format output: Name\tImage\tStatus\tPorts
1241|1241|        let raw = "web-1\tnginx:latest\tUp 2 hours\t0.0.0.0:80->80/tcp\n\
1242|1242|                   api-1\tnode:20\tUp 2 hours\t0.0.0.0:3000->3000/tcp\n\
1243|1243|                   db-1\tpostgres:16\tUp 2 hours\t0.0.0.0:5432->5432/tcp";
1244|1244|        let out = format_compose_ps(raw);
1245|1245|        assert!(out.contains("3"), "should show container count");
1246|1246|        assert!(out.contains("web"), "should show service name");
1247|1247|        assert!(out.contains("api"), "should show service name");
1248|1248|        assert!(out.contains("db"), "should show service name");
1249|1249|        assert!(out.contains("Up 2 hours"), "should show status");
1250|1250|        assert!(out.len() < raw.len(), "output should be shorter than raw");
1251|1251|    }
1252|1252|
1253|1253|    #[test]
1254|1254|    fn test_format_compose_ps_empty() {
1255|1255|        let out = format_compose_ps("");
1256|1256|        assert!(out.contains("0"), "should show zero containers");
1257|1257|    }
1258|1258|
1259|1259|    #[test]
1260|1260|    fn test_format_compose_ps_whitespace_only() {
1261|1261|        let out = format_compose_ps("   \n  \n");
1262|1262|        assert!(out.contains("0"), "should show zero containers");
1263|1263|    }
1264|1264|
1265|1265|    #[test]
1266|1266|    fn test_format_compose_ps_exited_service() {
1267|1267|        // Tab-separated --format output
1268|1268|        let raw = "worker-1\tpython:3.12\tExited (1) 2 minutes ago\t";
1269|1269|        let out = format_compose_ps(raw);
1270|1270|        assert!(out.contains("worker"), "should show service name");
1271|1271|        assert!(out.contains("Exited"), "should show exited status");
1272|1272|    }
1273|1273|
1274|1274|    #[test]
1275|1275|    fn test_format_compose_ps_no_ports() {
1276|1276|        let raw = "redis-1\tredis:7\tUp 5 hours\t";
1277|1277|        let out = format_compose_ps(raw);
1278|1278|        assert!(out.contains("redis"), "should show service name");
1279|1279|        // Should not show port info when no ports (but [compose] prefix is OK)
1280|1280|        let lines: Vec<&str> = out.lines().collect();
1281|1281|        let redis_line = lines.iter().find(|l| l.contains("redis")).unwrap();
1282|1282|        assert!(
1283|1283|            !redis_line.contains("] ["),
1284|1284|            "should not show port brackets when empty"
1285|1285|        );
1286|1286|    }
1287|1287|
1288|1288|    #[test]
1289|1289|    fn test_format_compose_ps_long_image_path() {
1290|1290|        let raw = "app-1\tghcr.io/myorg/myapp:latest\tUp 1 hour\t0.0.0.0:8080->8080/tcp";
1291|1291|        let out = format_compose_ps(raw);
1292|1292|        assert!(
1293|1293|            out.contains("myapp:latest"),
1294|1294|            "should shorten image to last segment"
1295|1295|        );
1296|1296|        assert!(
1297|1297|            !out.contains("ghcr.io"),
1298|1298|            "should not show full registry path"
1299|1299|        );
1300|1300|    }
1301|1301|
1302|1302|    // ── format_compose_logs ────────────────────────────────
1303|1303|
1304|1304|    #[test]
1305|1305|    fn test_format_compose_logs_basic() {
1306|1306|        let raw = "\
1307|1307|web-1  | 192.168.1.1 - GET / 200
1308|1308|web-1  | 192.168.1.1 - GET /favicon.ico 404
1309|1309|api-1  | Server listening on port 3000
1310|1310|api-1  | Connected to database";
1311|1311|        let out = format_compose_logs(raw);
1312|1312|        assert!(out.contains("Logs"), "should have compose logs header");
1313|1313|    }
1314|1314|
1315|1315|    #[test]
1316|1316|    fn test_format_compose_logs_empty() {
1317|1317|        let out = format_compose_logs("");
1318|1318|        assert!(out.contains("No logs"), "should indicate no logs");
1319|1319|    }
1320|1320|
1321|1321|    // ── format_compose_build ───────────────────────────────
1322|1322|
1323|1323|    #[test]
1324|1324|    fn test_format_compose_build_basic() {
1325|1325|        let raw = "\
1326|1326|[+] Building 12.3s (8/8) FINISHED
1327|1327| => [web internal] load build definition from Dockerfile           0.0s
1328|1328| => [web internal] load metadata for docker.io/library/node:20     1.2s
1329|1329| => [web 1/4] FROM docker.io/library/node:20@sha256:abc123         0.0s
1330|1330| => [web 2/4] WORKDIR /app                                         0.1s
1331|1331| => [web 3/4] COPY package*.json ./                                0.1s
1332|1332| => [web 4/4] RUN npm install                                      8.5s
1333|1333| => [web] exporting to image                                       2.3s
1334|1334| => => naming to docker.io/library/myapp-web                       0.0s";
1335|1335|        let out = format_compose_build(raw);
1336|1336|        assert!(out.contains("12.3s"), "should show total build time");
1337|1337|        assert!(out.contains("web"), "should show service name");
1338|1338|        assert!(out.len() < raw.len(), "should be shorter than raw");
1339|1339|    }
1340|1340|
1341|1341|    #[test]
1342|1342|    fn test_format_compose_build_empty() {
1343|1343|        let out = format_compose_build("");
1344|1344|        assert!(
1345|1345|            !out.is_empty(),
1346|1346|            "should produce output even for empty input"
1347|1347|        );
1348|1348|    }
1349|1349|
1350|1350|    // ── compact_ports (existing, previously untested) ──────
1351|1351|
1352|1352|    #[test]
1353|1353|    fn test_compact_ports_empty() {
1354|1354|        assert_eq!(compact_ports(""), "-");
1355|1355|    }
1356|1356|
1357|1357|    #[test]
1358|1358|    fn test_compact_ports_single() {
1359|1359|        let result = compact_ports("0.0.0.0:8080->80/tcp");
1360|1360|        assert!(result.contains("8080"));
1361|1361|    }
1362|1362|
1363|1363|    #[test]
1364|1364|    fn test_compact_ports_many() {
1365|1365|        let result = compact_ports("0.0.0.0:80->80/tcp, 0.0.0.0:443->443/tcp, 0.0.0.0:8080->8080/tcp, 0.0.0.0:9090->9090/tcp");
1366|1366|        assert!(result.contains("..."), "should truncate for >3 ports");
1367|1367|    }
1368|1368|
1369|1369|    #[test]
1370|1370|    fn test_kubectl_get_target_pods_aliases() {
1371|1371|        for resource in ["po", "pod", "pods"] {
1372|1372|            let args = vec![resource.to_string(), "-n".to_string(), "default".to_string()];
1373|1373|
1374|1374|            assert_eq!(
1375|1375|                kubectl_get_target(&args),
1376|1376|                Some(("pods", &args[1..])),
1377|1377|                "failed for {resource}"
1378|1378|            );
1379|1379|        }
1380|1380|    }
1381|1381|
1382|1382|    #[test]
1383|1383|    fn test_kubectl_get_target_services_aliases() {
1384|1384|        for resource in ["svc", "service", "services"] {
1385|1385|            let args = vec![resource.to_string(), "-A".to_string()];
1386|1386|
1387|1387|            assert_eq!(
1388|1388|                kubectl_get_target(&args),
1389|1389|                Some(("services", &args[1..])),
1390|1390|                "failed for {resource}"
1391|1391|            );
1392|1392|        }
1393|1393|    }
1394|1394|
1395|1395|    #[test]
1396|1396|    fn test_kubectl_get_target_unsupported_resource() {
1397|1397|        let args = vec!["deployments".to_string()];
1398|1398|
1399|1399|        assert_eq!(kubectl_get_target(&args), None);
1400|1400|    }
1401|1401|
1402|1402|    #[test]
1403|1403|    fn test_kubectl_get_target_respects_output_flags() {
1404|1404|        for output_flag in ["-o", "-owide", "--output", "--output=json"] {
1405|1405|            let args = vec![
1406|1406|                "pods".to_string(),
1407|1407|                output_flag.to_string(),
1408|1408|                "wide".to_string(),
1409|1409|            ];
1410|1410|
1411|1411|            assert_eq!(
1412|1412|                kubectl_get_target(&args),
1413|1413|                None,
1414|1414|                "should pass through {output_flag}"
1415|1415|            );
1416|1416|        }
1417|1417|    }
1418|1418|}
1419|1419|>>>>>>> b8172e5 (fix(kubectl): compact get pods and services aliases)
1420|1420|