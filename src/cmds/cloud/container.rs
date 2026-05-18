1|//! Filters Docker and kubectl output into compact summaries.
2|
3|use crate::core::runner::{self, RunOptions};
4|use crate::core::stream::exec_capture;
5|use crate::core::tracking;
6|use crate::core::truncate::{CAP_INVENTORY, CAP_LIST, CAP_WARNINGS};
7|use crate::core::utils::resolved_command;
8|use anyhow::{Context, Result};
9|use serde_json::Value;
10|use std::ffi::OsString;
11|use std::process::Command;
12|
13|#[derive(Debug, Clone, Copy)]
14|pub enum ContainerCmd {
15|    DockerPs,
16|    DockerPsAll,
17|    DockerImages,
18|    DockerLogs,
19|    KubectlPods,
20|    KubectlServices,
21|    KubectlLogs,
22|}
23|
24|pub fn run(cmd: ContainerCmd, args: &[String], verbose: u8) -> Result<i32> {
25|    match cmd {
26|        ContainerCmd::DockerPs => docker_ps(verbose),
27|        ContainerCmd::DockerPsAll => docker_ps_all(verbose),
28|        ContainerCmd::DockerImages => docker_images(verbose),
29|        ContainerCmd::DockerLogs => docker_logs(args, verbose),
30|        ContainerCmd::KubectlPods => kubectl_pods(args, verbose),
31|        ContainerCmd::KubectlServices => kubectl_services(args, verbose),
32|        ContainerCmd::KubectlLogs => kubectl_logs(args, verbose),
33|    }
34|}
35|
36|fn run_kubectl_json<F>(cmd: Command, label: &str, filter_fn: F) -> Result<i32>
37|where
38|    F: Fn(&Value) -> String,
39|{
40|    runner::run_filtered(
41|        cmd,
42|        "kubectl",
43|        label,
44|        |stdout| match serde_json::from_str::<Value>(stdout) {
45|            Ok(json) => filter_fn(&json),
46|            Err(e) => {
47|                eprintln!("[rtk] kubectl: JSON parse failed: {}", e);
48|                stdout.to_string()
49|            }
50|        },
51|        RunOptions::stdout_only()
52|            .early_exit_on_failure()
53|            .no_trailing_newline(),
54|    )
55|}
56|
57|fn docker_ps(_verbose: u8) -> Result<i32> {
58|    let timer = tracking::TimedExecution::start();
59|
60|    // Baseline the LLM would otherwise see.
61|    let raw = exec_capture(resolved_command("docker").args(["ps"]))
62|        .map(|r| r.stdout)
63|        .unwrap_or_default();
64|
65|    // One structured call over *all* containers (`-a`) — splitting on the State
66|    // field lets us list crashed/exited ones too, which plain `docker ps` hides.
67|    let result = exec_capture(resolved_command("docker").args([
68|        "ps",
69|        "-a",
70|        "--format",
71|        "{{.State}}\t{{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}",
72|    ]))
73|    .context("Failed to run docker ps")?;
74|
75|    if !result.success() {
76|        eprint!("{}", result.stderr);
77|        timer.track("docker ps", "rtk docker ps", &raw, &raw);
78|        return Ok(result.exit_code);
79|    }
80|
81|82|    let stdout = result.stdout;
83|    let mut rtk = String::new();
84|
85|    if stdout.trim().is_empty() {
86|        rtk.push_str("[docker] 0 containers");
87|        println!("{}", rtk);
88|        timer.track("docker ps", "rtk docker ps", &raw, &rtk);
89|        return Ok(0);
90|    }
91|
92|    const MAX_CONTAINERS: usize = CAP_LIST;
93|    let lines: Vec<String> = stdout
94|        .lines()
95|        .filter(|l| !l.trim().is_empty())
96|        .filter_map(|line| format_container_line(line, true))
97|        .collect();
98|
99|    rtk.push_str(&format!("[docker] {} containers:\n", lines.len()));
100|    for entry in lines.iter().take(MAX_CONTAINERS) {
101|        rtk.push_str(entry);
102|    }
103|    if lines.len() > MAX_CONTAINERS {
104|        rtk.push_str(&format!("  … +{} more\n", lines.len() - MAX_CONTAINERS));
105|        let full: String = lines.concat();
106|        if let Some(hint) = crate::core::tee::force_tee_hint(&full, "docker-ps") {
107|            rtk.push_str(&format!("{}\n", hint));
108|169|        }
170|    }
171|
172|    print!("{}", rtk);
173|    timer.track("docker ps", "rtk docker ps", &raw, &rtk);
174|    Ok(0)
175|}
176|
177|fn docker_ps_all(_verbose: u8) -> Result<i32> {
178|    let timer = tracking::TimedExecution::start();
179|
180|    let raw = exec_capture(resolved_command("docker").args(["ps", "-a"]))
181|        .map(|r| r.stdout)
182|        .unwrap_or_default();
183|
184|    let result = exec_capture(resolved_command("docker").args([
185|        "ps",
186|        "-a",
187|        "--format",
188|        "{{.State}}\t{{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}",
189|    ]))
190|    .context("Failed to run docker ps -a")?;
191|
192|    if !result.success() {
193|        eprint!("{}", result.stderr);
194|        timer.track("docker ps -a", "rtk docker ps -a", &raw, &raw);
195|        return Ok(result.exit_code);
196|    }
197|
198|    let mut running_lines: Vec<String> = Vec::new();
199|    let mut stopped_lines: Vec<String> = Vec::new();
200|    for line in result.stdout.lines().filter(|l| !l.trim().is_empty()) {
201|        let parts: Vec<&str> = line.split('\t').collect();
202|        let state = parts.first().copied().unwrap_or("");
203|        let is_running = matches!(state, "running" | "restarting");
204|        if let Some(entry) = format_container_line_from_parts(&parts[1..], is_running) {
205|            if is_running {
206|                running_lines.push(entry);
207|            } else {
208|                stopped_lines.push(entry);
209|            }
210|        }
211|    }
212|
213|    const MAX_CONTAINERS: usize = 20;
214|    let truncated = running_lines.len() > MAX_CONTAINERS || stopped_lines.len() > MAX_CONTAINERS;
215|
216|    let mut rtk = String::new();
217|    rtk.push_str(&format!("[docker] {} running:\n", running_lines.len()));
218|    for l in running_lines.iter().take(MAX_CONTAINERS) {
219|        rtk.push_str(l);
220|    }
221|    if running_lines.len() > MAX_CONTAINERS {
222|        rtk.push_str(&format!(
223|            "  … +{} more\n",
224|            running_lines.len() - MAX_CONTAINERS
225|        ));
226|    }
227|    if !stopped_lines.is_empty() {
228|        rtk.push_str(&format!(
229|            "[docker] {} stopped/exited:\n",
230|            stopped_lines.len()
231|        ));
232|        for l in stopped_lines.iter().take(MAX_CONTAINERS) {
233|            rtk.push_str(l);
234|        }
235|        if stopped_lines.len() > MAX_CONTAINERS {
236|            rtk.push_str(&format!(
237|                "  … +{} more\n",
238|                stopped_lines.len() - MAX_CONTAINERS
239|            ));
240|        }
241|    }
242|    if truncated {
243|        let full: String = running_lines.iter().chain(stopped_lines.iter()).cloned().collect();
244|        if let Some(hint) = crate::core::tee::force_tee_hint(&full, "docker-ps-a") {
245|            rtk.push_str(&format!("{}\n", hint));
246|        }
247|    }
248|
249|    print!("{}", rtk);
250|    timer.track("docker ps -a", "rtk docker ps -a", &raw, &rtk);
251|    Ok(0)
252|}
253|
254|fn format_container_line(line: &str, with_ports: bool) -> Option<String> {
255|    let parts: Vec<&str> = line.split('\t').collect();
256|    format_container_line_from_parts(&parts, with_ports)
257|}
258|
259|fn format_container_line_from_parts(parts: &[&str], with_ports: bool) -> Option<String> {
260|    if parts.len() < 4 {
261|        return None;
262|    }
263|    let id = &parts[0][..12.min(parts[0].len())];
264|    let name = parts[1];
265|    let status = parts[2].trim();
266|    let short_image = parts[3].split('/').next_back().unwrap_or("");
267|    let port_suffix = if with_ports {
268|        let ports = compact_ports(parts.get(4).unwrap_or(&""));
269|        if ports == "-" {
270|            String::new()
271|        } else {
272|            format!(" [{}]", ports)
273|        }
274|    } else {
275|        String::new()
276|    };
277|    Some(format!(
278|        "  {} {} ({}) {}{}\n",
279|        id, name, short_image, status, port_suffix
280|    ))
281|}
282|
283|fn docker_images(_verbose: u8) -> Result<i32> {
284|    let timer = tracking::TimedExecution::start();
285|
286|    let raw = exec_capture(resolved_command("docker").args(["images"]))
287|        .map(|r| r.stdout)
288|        .unwrap_or_default();
289|
290|    let result = exec_capture(resolved_command("docker").args([
291|        "images",
292|        "--format",
293|        "{{.Repository}}:{{.Tag}}\t{{.Size}}",
294|    ]))
295|    .context("Failed to run docker images")?;
296|
297|    if !result.success() {
298|        eprint!("{}", result.stderr);
299|        timer.track("docker images", "rtk docker images", &raw, &raw);
300|        return Ok(result.exit_code);
301|    }
302|
303|    let stdout = result.stdout;
304|    let lines: Vec<&str> = stdout.lines().collect();
305|    let mut rtk = String::new();
306|
307|    if lines.is_empty() {
308|        rtk.push_str("[docker] 0 images");
309|        println!("{}", rtk);
310|        timer.track("docker images", "rtk docker images", &raw, &rtk);
311|        return Ok(0);
312|    }
313|
314|    let mut total_size_mb: f64 = 0.0;
315|    for line in &lines {
316|        let parts: Vec<&str> = line.split('\t').collect();
317|        if let Some(size_str) = parts.get(1) {
318|            if size_str.contains("GB") {
319|                if let Ok(n) = size_str.replace("GB", "").trim().parse::<f64>() {
320|                    total_size_mb += n * 1024.0;
321|                }
322|            } else if size_str.contains("MB") {
323|                if let Ok(n) = size_str.replace("MB", "").trim().parse::<f64>() {
324|                    total_size_mb += n;
325|                }
326|            }
327|        }
328|    }
329|
330|    let total_display = if total_size_mb > 1024.0 {
331|        format!("{:.1}GB", total_size_mb / 1024.0)
332|    } else {
333|        format!("{:.0}MB", total_size_mb)
334|    };
335|    rtk.push_str(&format!(
336|        "[docker] {} images ({})\n",
337|        lines.len(),
338|        total_display
339|    ));
340|
341|342|    // a full image list is an inventory query, like pip list.
343|    const MAX_IMAGES: usize = CAP_INVENTORY;
344|    let image_lines: Vec<String> = lines
345|        .iter()
346|        .map(|line| {
347|            let parts: Vec<&str> = line.split('\t').collect();
348|            let image = parts.first().copied().unwrap_or("");
349|            let size = parts.get(1).copied().unwrap_or("");
350|            format!("  {} [{}]\n", image, size)
351|        })
352|        .collect();
353|
354|    let mut full_rtk = rtk.clone();
355|    for l in &image_lines {
356|        full_rtk.push_str(l);
357|    }
358|
359|    for l in image_lines.iter().take(MAX_IMAGES) {
360|        rtk.push_str(l);
361|    }
362|    if image_lines.len() > MAX_IMAGES {
363|        rtk.push_str(&format!("  … +{} more\n", image_lines.len() - MAX_IMAGES));
364|        if let Some(hint) = crate::core::tee::force_tee_tail_hint(&full_rtk, "docker-images", MAX_IMAGES + 2) {
365|            rtk.push_str(&format!("{}\n", hint));
366|        }
367|385|    }
386|
387|    print!("{}", rtk);
388|    timer.track("docker images", "rtk docker images", &raw, &rtk);
389|    Ok(0)
390|}
391|
392|fn docker_logs(args: &[String], _verbose: u8) -> Result<i32> {
393|    let container = args.first().map(|s| s.as_str()).unwrap_or("");
394|    if container.is_empty() {
395|        println!("Usage: rtk docker logs <container>");
396|        return Ok(0);
397|    }
398|
399|    let mut cmd = resolved_command("docker");
400|    cmd.args(["logs", "--tail", "100", container]);
401|
402|    let label = format!("logs {}", container);
403|    runner::run_filtered(
404|        cmd,
405|        "docker",
406|        &label,
407|        |raw| {
408|            format!(
409|                "[docker] Logs for {}:\n{}",
410|                container,
411|                crate::log_cmd::run_stdin_str(raw)
412|            )
413|        },
414|        RunOptions::default().early_exit_on_failure(),
415|    )
416|}
417|
418|fn kubectl_pods(args: &[String], _verbose: u8) -> Result<i32> {
419|    let mut cmd = resolved_command("kubectl");
420|    cmd.args(["get", "pods", "-o", "json"]);
421|    for arg in args {
422|        cmd.arg(arg);
423|    }
424|    run_kubectl_json(cmd, "get pods", format_kubectl_pods)
425|}
426|
427|fn format_kubectl_pods(json: &Value) -> String {
428|    let Some(pods) = json["items"].as_array().filter(|a| !a.is_empty()) else {
429|        return "No pods found\n".to_string();
430|    };
431|    let (mut running, mut pending, mut failed, mut restarts_total) = (0, 0, 0, 0i64);
432|    let mut issues: Vec<String> = Vec::new();
433|
434|    for pod in pods {
435|        let ns = pod["metadata"]["namespace"].as_str().unwrap_or("-");
436|        let name = pod["metadata"]["name"].as_str().unwrap_or("-");
437|        let phase = pod["status"]["phase"].as_str().unwrap_or("Unknown");
438|
439|        if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
440|            for c in containers {
441|                restarts_total += c["restartCount"].as_i64().unwrap_or(0);
442|            }
443|        }
444|
445|        match phase {
446|            "Running" => running += 1,
447|            "Pending" => {
448|                pending += 1;
449|                issues.push(format!("{}/{} Pending", ns, name));
450|            }
451|            "Failed" | "Error" => {
452|                failed += 1;
453|                issues.push(format!("{}/{} {}", ns, name, phase));
454|            }
455|            _ => {
456|                if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
457|                    for c in containers {
458|                        if let Some(w) = c["state"]["waiting"]["reason"].as_str() {
459|                            if w.contains("CrashLoop") || w.contains("Error") {
460|                                failed += 1;
461|                                issues.push(format!("{}/{} {}", ns, name, w));
462|                            }
463|                        }
464|                    }
465|                }
466|            }
467|        }
468|    }
469|
470|    let mut parts = Vec::new();
471|    if running > 0 {
472|        parts.push(format!("{}", running));
473|    }
474|    if pending > 0 {
475|        parts.push(format!("{} pending", pending));
476|    }
477|    if failed > 0 {
478|        parts.push(format!("{} [x]", failed));
479|    }
480|    if restarts_total > 0 {
481|        parts.push(format!("{} restarts", restarts_total));
482|    }
483|
484|    let mut out = format!("{} pods: {}\n", pods.len(), parts.join(", "));
485|    if !issues.is_empty() {
486|        const MAX_PODS_ISSUES: usize = CAP_WARNINGS;
487|        out.push_str("[warn] Issues:\n");
488|        for issue in issues.iter().take(MAX_PODS_ISSUES) {
489|            out.push_str(&format!("  {}\n", issue));
490|        }
491|        if issues.len() > MAX_PODS_ISSUES {
492|            out.push_str(&format!("  … +{} more", issues.len() - MAX_PODS_ISSUES));
493|            let all_issues = issues.join("\n");
494|            if let Some(hint) =
495|                crate::core::tee::force_tee_tail_hint(&all_issues, "kubectl-pods", MAX_PODS_ISSUES + 1)
496|            {
497|                out.push_str(&format!(" {}", hint));
498|            }
499|        }
500|    }
501|    out
502|}
503|
504|fn kubectl_services(args: &[String], _verbose: u8) -> Result<i32> {
505|    let mut cmd = resolved_command("kubectl");
506|    cmd.args(["get", "services", "-o", "json"]);
507|    for arg in args {
508|        cmd.arg(arg);
509|    }
510|    run_kubectl_json(cmd, "get services", format_kubectl_services)
511|}
512|
513|fn format_kubectl_services(json: &Value) -> String {
514|    let Some(services) = json["items"].as_array().filter(|a| !a.is_empty()) else {
515|        return "No services found\n".to_string();
516|    };
517|    let mut out = format!("{} services:\n", services.len());
518|
519|    let all_lines: Vec<String> = services
520|        .iter()
521|        .map(|svc| {
522|            let ns = svc["metadata"]["namespace"].as_str().unwrap_or("-");
523|            let name = svc["metadata"]["name"].as_str().unwrap_or("-");
524|            let svc_type = svc["spec"]["type"].as_str().unwrap_or("-");
525|            let ports: Vec<String> = svc["spec"]["ports"]
526|                .as_array()
527|                .map(|arr| {
528|                    arr.iter()
529|                        .map(|p| {
530|                            let port = p["port"].as_i64().unwrap_or(0);
531|                            let target = p["targetPort"]
532|                                .as_i64()
533|                                .or_else(|| p["targetPort"].as_str().and_then(|s| s.parse().ok()))
534|                                .unwrap_or(port);
535|                            if port == target {
536|                                format!("{}", port)
537|                            } else {
538|                                format!("{}→{}", port, target)
539|                            }
540|                        })
541|                        .collect()
542|                })
543|                .unwrap_or_default();
544|            format!("  {}/{} {} [{}]", ns, name, svc_type, ports.join(","))
545|        })
546|        .collect();
547|
548|    const MAX_KUBECTL_SERVICES: usize = CAP_LIST;
549|    for line in all_lines.iter().take(MAX_KUBECTL_SERVICES) {
550|        out.push_str(&format!("{}\n", line));
551|    }
552|    if all_lines.len() > MAX_KUBECTL_SERVICES {
553|        out.push_str(&format!("  … +{} more", all_lines.len() - MAX_KUBECTL_SERVICES));
554|        let all_text = all_lines.join("\n");
555|        if let Some(hint) =
556|            crate::core::tee::force_tee_tail_hint(&all_text, "kubectl-services", MAX_KUBECTL_SERVICES + 1)
557|        {
558|            out.push_str(&format!(" {}", hint));
559|        }
560|        out.push('\n');
561|    }
562|    out
563|}
564|
565|fn kubectl_logs(args: &[String], _verbose: u8) -> Result<i32> {
566|    let pod = args.first().map(|s| s.as_str()).unwrap_or("");
567|    if pod.is_empty() {
568|        println!("Usage: rtk kubectl logs <pod>");
569|        return Ok(0);
570|    }
571|
572|    let mut cmd = resolved_command("kubectl");
573|    cmd.args(["logs", "--tail", "100", pod]);
574|    for arg in args.iter().skip(1) {
575|        cmd.arg(arg);
576|    }
577|
578|    let label = format!("logs {}", pod);
579|    runner::run_filtered(
580|        cmd,
581|        "kubectl",
582|        &label,
583|        |stdout| {
584|            format!(
585|                "Logs for {}:\n{}",
586|                pod,
587|                crate::log_cmd::run_stdin_str(stdout)
588|            )
589|        },
590|        RunOptions::stdout_only().early_exit_on_failure(),
591|    )
592|}
593|
594|/// Format `docker compose ps --format` output into compact form.
595|/// Expects tab-separated lines: Name\tImage\tStatus\tPorts
596|/// (no header row — `--format` output is headerless)
597|pub fn format_compose_ps(raw: &str) -> String {
598|    const MAX_COMPOSE_SERVICES: usize = CAP_LIST;
599|    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
600|
601|    if lines.is_empty() {
602|        return "[compose] 0 services".to_string();
603|    }
604|
605|    let mut result = format!("[compose] {} services:\n", lines.len());
606|
607|    // Pre-build all formatted lines so the tee file matches what the agent sees.
608|    let all_formatted: Vec<String> = lines
609|        .iter()
610|        .filter_map(|line| {
611|            let parts: Vec<&str> = line.split('\t').collect();
612|            if parts.len() < 4 {
613|                return None;
614|            }
615|            let name = parts[0];
616|            let image = parts[1];
617|            let status = parts[2];
618|            let ports = parts[3];
619|            let short_image = image.split('/').next_back().unwrap_or(image);
620|            let port_str = if ports.trim().is_empty() {
621|                String::new()
622|            } 

... [OUTPUT TRUNCATED - 5332 chars omitted out of 55332 total] ...

er ps", "rtk docker ps", &raw, &rtk);
        return Ok(0);
    }

<<<<<<< HEAD
    let count = stdout.lines().count();
    rtk.push_str(&format!("[docker] {} containers:\n", count));

    for line in stdout.lines().take(15) {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            let id = &parts[0][..12.min(parts[0].len())];
            let name = parts[1];
            let short_image = parts
                .get(3)
                .unwrap_or(&"")
                .split('/')
                .next_back()
                .unwrap_or("");
            let ports = compact_ports(parts.get(4).unwrap_or(&""));
            if ports == "-" {
                rtk.push_str(&format!("  {} {} ({})\n", id, name, short_image));
            } else {
                rtk.push_str(&format!(
                    "  {} {} ({}) [{}]\n",
                    id, name, short_image, ports
                ));
            }
        }
    }
    if count > 15 {
        rtk.push_str(&format!("  ... +{} more", count - 15));
    }

    print!("{}", rtk);
    timer.track("docker ps", "rtk docker ps", &raw, &rtk);
    Ok(0)
}

fn docker_ps_all(_verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    let raw = exec_capture(resolved_command("docker").args(["ps", "-a"]))
        .map(|r| r.stdout)
        .unwrap_or_default();

    let result = exec_capture(resolved_command("docker").args([
        "ps",
        "-a",
        "--format",
        "{{.State}}\t{{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Image}}\t{{.Ports}}",
    ]))
    .context("Failed to run docker ps -a")?;

    if !result.success() {
        eprint!("{}", result.stderr);
        timer.track("docker ps -a", "rtk docker ps -a", &raw, &raw);
        return Ok(result.exit_code);
    }

    let mut running_lines: Vec<String> = Vec::new();
    let mut stopped_lines: Vec<String> = Vec::new();
    for line in result.stdout.lines().filter(|l| !l.trim().is_empty()) {
        let parts: Vec<&str> = line.split('\t').collect();
        let state = parts.first().copied().unwrap_or("");
        let is_running = matches!(state, "running" | "restarting");
        if let Some(entry) = format_container_line_from_parts(&parts[1..], is_running) {
            if is_running {
                running_lines.push(entry);
            } else {
                stopped_lines.push(entry);
            }
        }
    }

    const MAX_CONTAINERS: usize = 20;
    let truncated = running_lines.len() > MAX_CONTAINERS || stopped_lines.len() > MAX_CONTAINERS;

    let mut rtk = String::new();
    rtk.push_str(&format!("[docker] {} running:\n", running_lines.len()));
    for l in running_lines.iter().take(MAX_CONTAINERS) {
        rtk.push_str(l);
    }
    if running_lines.len() > MAX_CONTAINERS {
        rtk.push_str(&format!(
            "  … +{} more\n",
            running_lines.len() - MAX_CONTAINERS
        ));
    }
    if !stopped_lines.is_empty() {
        rtk.push_str(&format!(
            "[docker] {} stopped/exited:\n",
            stopped_lines.len()
        ));
        for l in stopped_lines.iter().take(MAX_CONTAINERS) {
            rtk.push_str(l);
        }
        if stopped_lines.len() > MAX_CONTAINERS {
            rtk.push_str(&format!(
                "  … +{} more\n",
                stopped_lines.len() - MAX_CONTAINERS
            ));
        }
    }
    if truncated {
        let full: String = running_lines.iter().chain(stopped_lines.iter()).cloned().collect();
        if let Some(hint) = crate::core::tee::force_tee_hint(&full, "docker-ps-a") {
            rtk.push_str(&format!("{}\n", hint));
        }
    }

    print!("{}", rtk);
    timer.track("docker ps -a", "rtk docker ps -a", &raw, &rtk);
    Ok(0)
}

fn format_container_line(line: &str, with_ports: bool) -> Option<String> {
    let parts: Vec<&str> = line.split('\t').collect();
    format_container_line_from_parts(&parts, with_ports)
}

fn format_container_line_from_parts(parts: &[&str], with_ports: bool) -> Option<String> {
    if parts.len() < 4 {
        return None;
    }
    let id = &parts[0][..12.min(parts[0].len())];
    let name = parts[1];
    let status = parts[2].trim();
    let short_image = parts[3].split('/').next_back().unwrap_or("");
    let port_suffix = if with_ports {
        let ports = compact_ports(parts.get(4).unwrap_or(&""));
        if ports == "-" {
            String::new()
        } else {
            format!(" [{}]", ports)
        }
    } else {
        String::new()
    };
    Some(format!(
        "  {} {} ({}) {}{}\n",
        id, name, short_image, status, port_suffix
    ))
}

fn docker_images(_verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    let raw = exec_capture(resolved_command("docker").args(["images"]))
        .map(|r| r.stdout)
        .unwrap_or_default();

    let result = exec_capture(resolved_command("docker").args([
        "images",
        "--format",
        "{{.Repository}}:{{.Tag}}\t{{.Size}}",
    ]))
    .context("Failed to run docker images")?;

    if !result.success() {
        eprint!("{}", result.stderr);
        timer.track("docker images", "rtk docker images", &raw, &raw);
        return Ok(result.exit_code);
    }

    let stdout = result.stdout;
    let lines: Vec<&str> = stdout.lines().collect();
    let mut rtk = String::new();

    if lines.is_empty() {
        rtk.push_str("[docker] 0 images");
        println!("{}", rtk);
        timer.track("docker images", "rtk docker images", &raw, &rtk);
        return Ok(0);
    }

    let mut total_size_mb: f64 = 0.0;
    for line in &lines {
        let parts: Vec<&str> = line.split('\t').collect();
        if let Some(size_str) = parts.get(1) {
            if size_str.contains("GB") {
                if let Ok(n) = size_str.replace("GB", "").trim().parse::<f64>() {
                    total_size_mb += n * 1024.0;
                }
            } else if size_str.contains("MB") {
                if let Ok(n) = size_str.replace("MB", "").trim().parse::<f64>() {
                    total_size_mb += n;
                }
            }
        }
    }

    let total_display = if total_size_mb > 1024.0 {
        format!("{:.1}GB", total_size_mb / 1024.0)
    } else {
        format!("{:.0}MB", total_size_mb)
    };
    rtk.push_str(&format!(
        "[docker] {} images ({})\n",
        lines.len(),
        total_display
    ));

    for line in lines.iter().take(15) {
        let parts: Vec<&str> = line.split('\t').collect();
        if !parts.is_empty() {
            let image = parts[0];
            let size = parts.get(1).unwrap_or(&"");
            let short = if image.len() > 40 {
                format!("...{}", &image[image.len() - 37..])
            } else {
                image.to_string()
            };
            rtk.push_str(&format!("  {} [{}]\n", short, size));
        }
    }
    if lines.len() > 15 {
        rtk.push_str(&format!("  ... +{} more", lines.len() - 15));
    }

    print!("{}", rtk);
    timer.track("docker images", "rtk docker images", &raw, &rtk);
    Ok(0)
}

fn docker_logs(args: &[String], _verbose: u8) -> Result<i32> {
    let container = args.first().map(|s| s.as_str()).unwrap_or("");
    if container.is_empty() {
        println!("Usage: rtk docker logs <container>");
        return Ok(0);
    }

    let mut cmd = resolved_command("docker");
    cmd.args(["logs", "--tail", "100", container]);

    let label = format!("logs {}", container);
    runner::run_filtered(
        cmd,
        "docker",
        &label,
        |raw| {
            format!(
                "[docker] Logs for {}:\n{}",
                container,
                crate::log_cmd::run_stdin_str(raw)
            )
        },
        RunOptions::default().early_exit_on_failure(),
    )
}

fn kubectl_pods(args: &[String], _verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("kubectl");
    cmd.args(["get", "pods", "-o", "json"]);
    for arg in args {
        cmd.arg(arg);
    }
    run_kubectl_json(cmd, "get pods", format_kubectl_pods)
}

fn format_kubectl_pods(json: &Value) -> String {
    let Some(pods) = json["items"].as_array().filter(|a| !a.is_empty()) else {
        return "No pods found\n".to_string();
    };
    let (mut running, mut pending, mut failed, mut restarts_total) = (0, 0, 0, 0i64);
    let mut issues: Vec<String> = Vec::new();

    for pod in pods {
        let ns = pod["metadata"]["namespace"].as_str().unwrap_or("-");
        let name = pod["metadata"]["name"].as_str().unwrap_or("-");
        let phase = pod["status"]["phase"].as_str().unwrap_or("Unknown");

        if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
            for c in containers {
                restarts_total += c["restartCount"].as_i64().unwrap_or(0);
            }
        }

        match phase {
            "Running" => running += 1,
            "Pending" => {
                pending += 1;
                issues.push(format!("{}/{} Pending", ns, name));
            }
            "Failed" | "Error" => {
                failed += 1;
                issues.push(format!("{}/{} {}", ns, name, phase));
            }
            _ => {
                if let Some(containers) = pod["status"]["containerStatuses"].as_array() {
                    for c in containers {
                        if let Some(w) = c["state"]["waiting"]["reason"].as_str() {
                            if w.contains("CrashLoop") || w.contains("Error") {
                                failed += 1;
                                issues.push(format!("{}/{} {}", ns, name, w));
                            }
                        }
                    }
                }
            }
        }
    }

    let mut parts = Vec::new();
    if running > 0 {
        parts.push(format!("{}", running));
    }
    if pending > 0 {
        parts.push(format!("{} pending", pending));
    }
    if failed > 0 {
        parts.push(format!("{} [x]", failed));
    }
    if restarts_total > 0 {
        parts.push(format!("{} restarts", restarts_total));
    }

    let mut out = format!("{} pods: {}\n", pods.len(), parts.join(", "));
    if !issues.is_empty() {
        out.push_str("[warn] Issues:\n");
        for issue in issues.iter().take(10) {
            out.push_str(&format!("  {}\n", issue));
        }
        if issues.len() > 10 {
            out.push_str(&format!("  ... +{} more", issues.len() - 10));
        }
    }
    out
}

fn kubectl_services(args: &[String], _verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("kubectl");
    cmd.args(["get", "services", "-o", "json"]);
    for arg in args {
        cmd.arg(arg);
    }
    run_kubectl_json(cmd, "get services", format_kubectl_services)
}

fn format_kubectl_services(json: &Value) -> String {
    let Some(services) = json["items"].as_array().filter(|a| !a.is_empty()) else {
        return "No services found\n".to_string();
    };
    let mut out = format!("{} services:\n", services.len());

    for svc in services.iter().take(15) {
        let ns = svc["metadata"]["namespace"].as_str().unwrap_or("-");
        let name = svc["metadata"]["name"].as_str().unwrap_or("-");
        let svc_type = svc["spec"]["type"].as_str().unwrap_or("-");
        let ports: Vec<String> = svc["spec"]["ports"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|p| {
                        let port = p["port"].as_i64().unwrap_or(0);
                        let target = p["targetPort"]
                            .as_i64()
                            .or_else(|| p["targetPort"].as_str().and_then(|s| s.parse().ok()))
                            .unwrap_or(port);
                        if port == target {
                            format!("{}", port)
                        } else {
                            format!("{}→{}", port, target)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "  {}/{} {} [{}]\n",
            ns,
            name,
            svc_type,
            ports.join(",")
        ));
    }
    if services.len() > 15 {
        out.push_str(&format!("  ... +{} more", services.len() - 15));
    }
    out
}

fn kubectl_logs(args: &[String], _verbose: u8) -> Result<i32> {
    let pod = args.first().map(|s| s.as_str()).unwrap_or("");
    if pod.is_empty() {
        println!("Usage: rtk kubectl logs <pod>");
        return Ok(0);
    }

    let mut cmd = resolved_command("kubectl");
    cmd.args(["logs", "--tail", "100", pod]);
    for arg in args.iter().skip(1) {
        cmd.arg(arg);
    }

    let label = format!("logs {}", pod);
    runner::run_filtered(
        cmd,
        "kubectl",
        &label,
        |stdout| {
            format!(
                "Logs for {}:\n{}",
                pod,
                crate::log_cmd::run_stdin_str(stdout)
            )
        },
        RunOptions::stdout_only().early_exit_on_failure(),
    )
}

/// Format `docker compose ps --format` output into compact form.
/// Expects tab-separated lines: Name\tImage\tStatus\tPorts
/// (no header row — `--format` output is headerless)
pub fn format_compose_ps(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();

    if lines.is_empty() {
        return "[compose] 0 services".to_string();
    }

    let mut result = format!("[compose] {} services:\n", lines.len());

    for line in lines.iter().take(20) {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            let name = parts[0];
            let image = parts[1];
            let status = parts[2];
            let ports = parts[3];

            let short_image = image.split('/').next_back().unwrap_or(image);

            let port_str = if ports.trim().is_empty() {
                String::new()
            } else {
                let compact = compact_ports(ports.trim());
                if compact == "-" {
                    String::new()
                } else {
                    format!(" [{}]", compact)
                }
            };

            result.push_str(&format!(
                "  {} ({}) {}{}\n",
                name, short_image, status, port_str
            ));
        }
    }
    if lines.len() > 20 {
        result.push_str(&format!("  ... +{} more\n", lines.len() - 20));
    }

    result.trim_end().to_string()
}

/// Format `docker compose logs` output into compact form
pub fn format_compose_logs(raw: &str) -> String {
    if raw.trim().is_empty() {
        return "[compose] No logs".to_string();
    }

    // docker compose logs prefixes each line with "service-N  | "
    // Use the existing log deduplication engine
    let analyzed = crate::log_cmd::run_stdin_str(raw);
    format!("[compose] Logs:\n{}", analyzed)
}

/// Format `docker compose build` output into compact summary
pub fn format_compose_build(raw: &str) -> String {
    if raw.trim().is_empty() {
        return "[compose] Build: no output".to_string();
    }

    let mut result = String::new();

    // Extract the summary line: "[+] Building 12.3s (8/8) FINISHED"
    for line in raw.lines() {
        if line.contains("Building") && line.contains("FINISHED") {
            result.push_str(&format!("[compose] {}\n", line.trim()));
            break;
        }
    }

    if result.is_empty() {
        // No FINISHED line found — might still be building or errored
        if let Some(line) = raw.lines().find(|l| l.contains("Building")) {
            result.push_str(&format!("[compose] {}\n", line.trim()));
        } else {
            result.push_str("[compose] Build:\n");
        }
    }

    // Collect unique service names from build steps like "[web 1/4]"
    let mut services: Vec<String> = Vec::new();
    // find('[') returns byte offset — use byte slicing throughout
    // '[' and ']' are single-byte ASCII, so byte arithmetic is safe
    for line in raw.lines() {
        if let Some(start) = line.find('[') {
            if let Some(end) = line[start + 1..].find(']') {
                let bracket = &line[start + 1..start + 1 + end];
                let svc = bracket.split_whitespace().next().unwrap_or("");
                if !svc.is_empty() && svc != "+" && !services.contains(&svc.to_string()) {
                    services.push(svc.to_string());
                }
            }
        }
    }

    if !services.is_empty() {
        result.push_str(&format!("  Services: {}\n", services.join(", ")));
    }

    // Count build steps (lines starting with " => ")
    let step_count = raw
        .lines()
        .filter(|l| l.trim_start().starts_with("=> "))
        .count();
    if step_count > 0 {
        result.push_str(&format!("  Steps: {}", step_count));
    }

    result.trim_end().to_string()
}

fn compact_ports(ports: &str) -> String {
    if ports.is_empty() {
        return "-".to_string();
    }

    // Extract just the port numbers
    let port_nums: Vec<&str> = ports
        .split(',')
        .filter_map(|p| p.split("->").next().and_then(|s| s.split(':').next_back()))
        .collect();

    if port_nums.len() <= 3 {
        port_nums.join(", ")
    } else {
        format!(
            "{}, ... +{}",
            port_nums[..2].join(", "),
            port_nums.len() - 2
        )
    }
}

pub fn run_docker_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
    crate::core::runner::run_passthrough("docker", args, verbose)
}

/// Run `docker compose ps` (or `docker compose ps -a`) with compact output
pub fn run_compose_ps(all: bool, verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();

    // Raw output for token tracking
    let raw_result = exec_capture(resolved_command("docker").args(["compose", "ps"]))
        .context("Failed to run docker compose ps")?;

    if !raw_result.success() {
        eprintln!("{}", raw_result.stderr);
        return Ok(raw_result.exit_code);
    }
    let raw = raw_result.stdout;

    // Structured output for parsing (same pattern as docker_ps)
    let result = exec_capture(resolved_command("docker").args([
        "compose",
        "ps",
        "--format",
        "{{.Name}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}",
    ]))
    .context("Failed to run docker compose ps --format")?;

    if !result.success() {
        eprintln!("{}", result.stderr);
        return Ok(result.exit_code);
    }
    let structured = result.stdout;

    if verbose > 0 {
        eprintln!("raw docker compose ps:\n{}", raw);
    }

    let rtk = format_compose_ps(&structured);
    println!("{}", rtk);
    let label = if all { "docker compose ps -a" } else { "docker compose ps" };
    let rtk_label = if all { "rtk docker compose ps -a" } else { "rtk docker compose ps" };
    timer.track(label, rtk_label, &raw, &rtk);
    Ok(0)
}

pub fn run_compose_logs(service: Option<&str>, tail: u32, verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("docker");
    let tail_str = tail.to_string();
    cmd.args(["compose", "logs", "--tail", &tail_str]);
    if let Some(svc) = service {
        cmd.arg(svc);
    }

    let svc_label = service.unwrap_or("all");
    runner::run_filtered(
        cmd,
        "docker",
        &format!("compose logs {}", svc_label),
        |raw| {
            if verbose > 0 {
                eprintln!("raw docker compose logs:\n{}", raw);
            }
            format_compose_logs(raw)
        },
        RunOptions::default().early_exit_on_failure(),
    )
}

pub fn run_compose_build(service: Option<&str>, verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("docker");
    cmd.args(["compose", "build"]);
    if let Some(svc) = service {
        cmd.arg(svc);
    }

    let svc_label = service.unwrap_or("all");
    runner::run_filtered(
        cmd,
        "docker",
        &format!("compose build {}", svc_label),
        |raw| {
            if verbose > 0 {
                eprintln!("raw docker compose build:\n{}", raw);
            }
            format_compose_build(raw)
        },
        RunOptions::default().early_exit_on_failure(),
    )
}

pub fn run_compose_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
    let mut combined = vec![OsString::from("compose")];
    combined.extend_from_slice(args);
    crate::core::runner::run_passthrough("docker", &combined, verbose)
}

pub fn run_kubectl_get(args: &[String], verbose: u8) -> Result<i32> {
    match kubectl_get_target(args) {
        Some(("pods", rest)) => run(ContainerCmd::KubectlPods, rest, verbose),
        Some(("services", rest)) => run(ContainerCmd::KubectlServices, rest, verbose),
        _ => run_kubectl_get_passthrough(args, verbose),
    }
}

fn kubectl_get_target(args: &[String]) -> Option<(&'static str, &[String])> {
    let resource = args.first()?.as_str();
    let rest = &args[1..];
    if kubectl_get_requests_raw_output(rest) {
        return None;
    }

    match resource {
        "po" | "pod" | "pods" => Some(("pods", rest)),
        "svc" | "service" | "services" => Some(("services", rest)),
        _ => None,
    }
}

fn kubectl_get_requests_raw_output(args: &[String]) -> bool {
    args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "-o" | "--output" | "-w" | "--watch" | "--show-labels" | "--show-kind"
        ) || arg.starts_with("-o")
            || arg.starts_with("--output=")
    })
}

fn run_kubectl_get_passthrough(args: &[String], verbose: u8) -> Result<i32> {
    let passthrough_args: Vec<OsString> = std::iter::once(OsString::from("get"))
        .chain(args.iter().map(|arg| OsString::from(arg.as_str())))
        .collect();
    run_kubectl_passthrough(&passthrough_args, verbose)
}

pub fn run_kubectl_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
    crate::core::runner::run_passthrough("kubectl", args, verbose)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_compose_ps ──────────────────────────────────

    #[test]
    fn test_format_compose_ps_basic() {
        // Tab-separated --format output: Name\tImage\tStatus\tPorts
        let raw = "web-1\tnginx:latest\tUp 2 hours\t0.0.0.0:80->80/tcp\n\
                   api-1\tnode:20\tUp 2 hours\t0.0.0.0:3000->3000/tcp\n\
                   db-1\tpostgres:16\tUp 2 hours\t0.0.0.0:5432->5432/tcp";
        let out = format_compose_ps(raw);
        assert!(out.contains("3"), "should show container count");
        assert!(out.contains("web"), "should show service name");
        assert!(out.contains("api"), "should show service name");
        assert!(out.contains("db"), "should show service name");
        assert!(out.contains("Up 2 hours"), "should show status");
        assert!(out.len() < raw.len(), "output should be shorter than raw");
    }

    #[test]
    fn test_format_compose_ps_empty() {
        let out = format_compose_ps("");
        assert!(out.contains("0"), "should show zero containers");
    }

    #[test]
    fn test_format_compose_ps_whitespace_only() {
        let out = format_compose_ps("   \n  \n");
        assert!(out.contains("0"), "should show zero containers");
    }

    #[test]
    fn test_format_compose_ps_exited_service() {
        // Tab-separated --format output
        let raw = "worker-1\tpython:3.12\tExited (1) 2 minutes ago\t";
        let out = format_compose_ps(raw);
        assert!(out.contains("worker"), "should show service name");
        assert!(out.contains("Exited"), "should show exited status");
    }

    #[test]
    fn test_format_compose_ps_no_ports() {
        let raw = "redis-1\tredis:7\tUp 5 hours\t";
        let out = format_compose_ps(raw);
        assert!(out.contains("redis"), "should show service name");
        // Should not show port info when no ports (but [compose] prefix is OK)
        let lines: Vec<&str> = out.lines().collect();
        let redis_line = lines.iter().find(|l| l.contains("redis")).unwrap();
        assert!(
            !redis_line.contains("] ["),
            "should not show port brackets when empty"
        );
    }

    #[test]
    fn test_format_compose_ps_long_image_path() {
        let raw = "app-1\tghcr.io/myorg/myapp:latest\tUp 1 hour\t0.0.0.0:8080->8080/tcp";
        let out = format_compose_ps(raw);
        assert!(
            out.contains("myapp:latest"),
            "should shorten image to last segment"
        );
        assert!(
            !out.contains("ghcr.io"),
            "should not show full registry path"
        );
    }

    // ── format_compose_logs ────────────────────────────────

    #[test]
    fn test_format_compose_logs_basic() {
        let raw = "\
web-1  | 192.168.1.1 - GET / 200
web-1  | 192.168.1.1 - GET /favicon.ico 404
api-1  | Server listening on port 3000
api-1  | Connected to database";
        let out = format_compose_logs(raw);
        assert!(out.contains("Logs"), "should have compose logs header");
    }

    #[test]
    fn test_format_compose_logs_empty() {
        let out = format_compose_logs("");
        assert!(out.contains("No logs"), "should indicate no logs");
    }

    // ── format_compose_build ───────────────────────────────

    #[test]
    fn test_format_compose_build_basic() {
        let raw = "\
[+] Building 12.3s (8/8) FINISHED
 => [web internal] load build definition from Dockerfile           0.0s
 => [web internal] load metadata for docker.io/library/node:20     1.2s
 => [web 1/4] FROM docker.io/library/node:20@sha256:abc123         0.0s
 => [web 2/4] WORKDIR /app                                         0.1s
 => [web 3/4] COPY package*.json ./                                0.1s
 => [web 4/4] RUN npm install                                      8.5s
 => [web] exporting to image                                       2.3s
 => => naming to docker.io/library/myapp-web                       0.0s";
        let out = format_compose_build(raw);
        assert!(out.contains("12.3s"), "should show total build time");
        assert!(out.contains("web"), "should show service name");
        assert!(out.len() < raw.len(), "should be shorter than raw");
    }

    #[test]
    fn test_format_compose_build_empty() {
        let out = format_compose_build("");
        assert!(
            !out.is_empty(),
            "should produce output even for empty input"
        );
    }

    // ── compact_ports (existing, previously untested) ──────

    #[test]
    fn test_compact_ports_empty() {
        assert_eq!(compact_ports(""), "-");
    }

    #[test]
    fn test_compact_ports_single() {
        let result = compact_ports("0.0.0.0:8080->80/tcp");
        assert!(result.contains("8080"));
    }

    #[test]
    fn test_compact_ports_many() {
        let result = compact_ports("0.0.0.0:80->80/tcp, 0.0.0.0:443->443/tcp, 0.0.0.0:8080->8080/tcp, 0.0.0.0:9090->9090/tcp");
        assert!(result.contains("..."), "should truncate for >3 ports");
    }

    #[test]
    fn test_kubectl_get_target_pods_aliases() {
        for resource in ["po", "pod", "pods"] {
            let args = vec![resource.to_string(), "-n".to_string(), "default".to_string()];

            assert_eq!(
                kubectl_get_target(&args),
                Some(("pods", &args[1..])),
                "failed for {resource}"
            );
        }
    }

    #[test]
    fn test_kubectl_get_target_services_aliases() {
        for resource in ["svc", "service", "services"] {
            let args = vec![resource.to_string(), "-A".to_string()];

            assert_eq!(
                kubectl_get_target(&args),
                Some(("services", &args[1..])),
                "failed for {resource}"
            );
        }
    }

    #[test]
    fn test_kubectl_get_target_unsupported_resource() {
        let args = vec!["deployments".to_string()];

        assert_eq!(kubectl_get_target(&args), None);
    }

    #[test]
    fn test_kubectl_get_target_respects_output_flags() {
        for output_flag in ["-o", "-owide", "--output", "--output=json"] {
            let args = vec![
                "pods".to_string(),
                output_flag.to_string(),
                "wide".to_string(),
            ];

            assert_eq!(
                kubectl_get_target(&args),
                None,
                "should pass through {output_flag}"
            );
        }
    }
}
>>>>>>> b8172e5 (fix(kubectl): compact get pods and services aliases)
