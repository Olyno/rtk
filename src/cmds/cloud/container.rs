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
622|            } else {
623|                let compact = compact_ports(ports.trim());
624|                if compact == "-" {
625|                    String::new()
626|                } else {
627|                    format!(" [{}]", compact)
628|                }
629|            };
630|            Some(format!("  {} ({}) {}{}", name, short_image, status, port_str))
631|        })
632|        .collect();
633|
634|    for line in all_formatted.iter().take(MAX_COMPOSE_SERVICES) {
635|        result.push_str(line);
636|        result.push('\n');
637|    }
638|    if all_formatted.len() > MAX_COMPOSE_SERVICES {
639|        result.push_str(&format!("  … +{} more\n", all_formatted.len() - MAX_COMPOSE_SERVICES));
640|        let all_text = all_formatted.join("\n");
641|        if let Some(hint) = crate::core::tee::force_tee_tail_hint(&all_text, "compose-ps", MAX_COMPOSE_SERVICES + 1) {
642|            result.push_str(&format!("  {}\n", hint));
643|        }
644|    }
645|
646|    result.trim_end().to_string()
647|}
648|
649|/// Format `docker compose logs` output into compact form
650|pub fn format_compose_logs(raw: &str) -> String {
651|    if raw.trim().is_empty() {
652|        return "[compose] No logs".to_string();
653|    }
654|
655|    // docker compose logs prefixes each line with "service-N  | "
656|    // Use the existing log deduplication engine
657|    let analyzed = crate::log_cmd::run_stdin_str(raw);
658|    format!("[compose] Logs:\n{}", analyzed)
659|}
660|
661|/// Format `docker compose build` output into compact summary
662|pub fn format_compose_build(raw: &str) -> String {
663|    if raw.trim().is_empty() {
664|        return "[compose] Build: no output".to_string();
665|    }
666|
667|    let mut result = String::new();
668|
669|    // Extract the summary line: "[+] Building 12.3s (8/8) FINISHED"
670|    for line in raw.lines() {
671|        if line.contains("Building") && line.contains("FINISHED") {
672|            result.push_str(&format!("[compose] {}\n", line.trim()));
673|            break;
674|        }
675|    }
676|
677|    if result.is_empty() {
678|        // No FINISHED line found — might still be building or errored
679|        if let Some(line) = raw.lines().find(|l| l.contains("Building")) {
680|            result.push_str(&format!("[compose] {}\n", line.trim()));
681|        } else {
682|            result.push_str("[compose] Build:\n");
683|        }
684|    }
685|
686|    // Collect unique service names from build steps like "[web 1/4]"
687|    let mut services: Vec<String> = Vec::new();
688|    // find('[') returns byte offset — use byte slicing throughout
689|    // '[' and ']' are single-byte ASCII, so byte arithmetic is safe
690|    for line in raw.lines() {
691|        if let Some(start) = line.find('[') {
692|            if let Some(end) = line[start + 1..].find(']') {
693|                let bracket = &line[start + 1..start + 1 + end];
694|                let svc = bracket.split_whitespace().next().unwrap_or("");
695|                if !svc.is_empty() && svc != "+" && !services.contains(&svc.to_string()) {
696|                    services.push(svc.to_string());
697|                }
698|            }
699|        }
700|    }
701|
702|    if !services.is_empty() {
703|        result.push_str(&format!("  Services: {}\n", services.join(", ")));
704|    }
705|
706|    // Count build steps (lines starting with " => ")
707|    let step_count = raw
708|        .lines()
709|        .filter(|l| l.trim_start().starts_with("=> "))
710|        .count();
711|    if step_count > 0 {
712|        result.push_str(&format!("  Steps: {}", step_count));
713|    }
714|
715|    result.trim_end().to_string()
716|}
717|
718|fn compact_ports(ports: &str) -> String {
719|    if ports.is_empty() {
720|        return "-".to_string();
721|    }
722|
723|    // Extract just the port numbers
724|    let port_nums: Vec<&str> = ports
725|        .split(',')
726|        .filter_map(|p| p.split("->").next().and_then(|s| s.split(':').next_back()))
727|        .collect();
728|
729|    if port_nums.len() <= 3 {
730|        port_nums.join(", ")
731|    } else {
732|        format!(
733|            "{}, … +{}",
734|            port_nums[..2].join(", "),
735|            port_nums.len() - 2
736|        )
737|    }
738|}
739|
740|pub fn run_docker_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
741|    crate::core::runner::run_passthrough("docker", args, verbose)
742|}
743|
744|/// Run `docker compose ps` (or `docker compose ps -a`) with compact output
745|pub fn run_compose_ps(all: bool, verbose: u8) -> Result<i32> {
746|    let timer = tracking::TimedExecution::start();
747|
748|749|    let mut raw_args: Vec<&str> = vec!["compose", "ps"];
750|    if all {
751|        raw_args.push("-a");
752|    }
753|    let raw_result = exec_capture(resolved_command("docker").args(&raw_args))
754|759|        .context("Failed to run docker compose ps")?;
760|
761|    if !raw_result.success() {
762|        eprintln!("{}", raw_result.stderr);
763|        return Ok(raw_result.exit_code);
764|    }
765|    let raw = raw_result.stdout;
766|
767|768|    let mut format_args: Vec<&str> = vec!["compose", "ps"];
769|    if all {
770|        format_args.push("-a");
771|    }
772|    format_args.extend(["--format", "{{.Name}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}"]);
773|    let result = exec_capture(resolved_command("docker").args(&format_args))
774|        .context("Failed to run docker compose ps --format")?;
775|786|
787|    if !result.success() {
788|        eprintln!("{}", result.stderr);
789|        return Ok(result.exit_code);
790|    }
791|    let structured = result.stdout;
792|
793|    if verbose > 0 {
794|        eprintln!("raw docker compose ps:\n{}", raw);
795|    }
796|
797|    let rtk = format_compose_ps(&structured);
798|    println!("{}", rtk);
799|    let label = if all { "docker compose ps -a" } else { "docker compose ps" };
800|    let rtk_label = if all { "rtk docker compose ps -a" } else { "rtk docker compose ps" };
801|    timer.track(label, rtk_label, &raw, &rtk);
802|    Ok(0)
803|}
804|
805|pub fn run_compose_logs(service: Option<&str>, tail: u32, verbose: u8) -> Result<i32> {
806|    let mut cmd = resolved_command("docker");
807|    let tail_str = tail.to_string();
808|    cmd.args(["compose", "logs", "--tail", &tail_str]);
809|    if let Some(svc) = service {
810|        cmd.arg(svc);
811|    }
812|
813|    let svc_label = service.unwrap_or("all");
814|    runner::run_filtered(
815|        cmd,
816|        "docker",
817|        &format!("compose logs {}", svc_label),
818|        |raw| {
819|            if verbose > 0 {
820|                eprintln!("raw docker compose logs:\n{}", raw);
821|            }
822|            format_compose_logs(raw)
823|        },
824|        RunOptions::default().early_exit_on_failure(),
825|    )
826|}
827|
828|pub fn run_compose_build(service: Option<&str>, verbose: u8) -> Result<i32> {
829|    let mut cmd = resolved_command("docker");
830|    cmd.args(["compose", "build"]);
831|    if let Some(svc) = service {
832|        cmd.arg(svc);
833|    }
834|
835|    let svc_label = service.unwrap_or("all");
836|    runner::run_filtered(
837|        cmd,
838|        "docker",
839|        &format!("compose build {}", svc_label),
840|        |raw| {
841|            if verbose > 0 {
842|                eprintln!("raw docker compose build:\n{}", raw);
843|            }
844|            format_compose_build(raw)
845|        },
846|        RunOptions::default().early_exit_on_failure(),
847|    )
848|}
849|
850|pub fn run_compose_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
851|    let mut combined = vec![OsString::from("compose")];
852|    combined.extend_from_slice(args);
853|    crate::core::runner::run_passthrough("docker", &combined, verbose)
854|}
855|
856|pub fn run_kubectl_get(args: &[String], verbose: u8) -> Result<i32> {
857|    match kubectl_get_target(args) {
858|        Some(("pods", rest)) => run(ContainerCmd::KubectlPods, rest, verbose),
859|        Some(("services", rest)) => run(ContainerCmd::KubectlServices, rest, verbose),
860|        _ => run_kubectl_get_passthrough(args, verbose),
861|    }
862|}
863|
864|fn kubectl_get_target(args: &[String]) -> Option<(&'static str, &[String])> {
865|    let resource = args.first()?.as_str();
866|    let rest = &args[1..];
867|    if kubectl_get_requests_raw_output(rest) {
868|        return None;
869|    }
870|
871|    match resource {
872|        "po" | "pod" | "pods" => Some(("pods", rest)),
873|        "svc" | "service" | "services" => Some(("services", rest)),
874|        _ => None,
875|    }
876|}
877|
878|fn kubectl_get_requests_raw_output(args: &[String]) -> bool {
879|    args.iter().any(|arg| {
880|        matches!(
881|            arg.as_str(),
882|            "-o" | "--output" | "-w" | "--watch" | "--show-labels" | "--show-kind"
883|        ) || arg.starts_with("-o")
884|            || arg.starts_with("--output=")
885|    })
886|}
887|
888|fn run_kubectl_get_passthrough(args: &[String], verbose: u8) -> Result<i32> {
889|    let passthrough_args: Vec<OsString> = std::iter::once(OsString::from("get"))
890|        .chain(args.iter().map(|arg| OsString::from(arg.as_str())))
891|        .collect();
892|    run_kubectl_passthrough(&passthrough_args, verbose)
893|}
894|
895|pub fn run_kubectl_passthrough(args: &[OsString], verbose: u8) -> Result<i32> {
896|    crate::core::runner::run_passthrough("kubectl", args, verbose)
897|}
898|
899|#[cfg(test)]
900|mod tests {
901|    use super::*;
902|
903|    // ── format_compose_ps ──────────────────────────────────
904|
905|    #[test]
906|    fn test_format_compose_ps_basic() {
907|        // Tab-separated --format output: Name\tImage\tStatus\tPorts
908|        let raw = "web-1\tnginx:latest\tUp 2 hours\t0.0.0.0:80->80/tcp\n\
909|                   api-1\tnode:20\tUp 2 hours\t0.0.0.0:3000->3000/tcp\n\
910|                   db-1\tpostgres:16\tUp 2 hours\t0.0.0.0:5432->5432/tcp";
911|        let out = format_compose_ps(raw);
912|        assert!(out.contains("3"), "should show container count");
913|        assert!(out.contains("web"), "should show service name");
914|        assert!(out.contains("api"), "should show service name");
915|        assert!(out.contains("db"), "should show service name");
916|        assert!(out.contains("Up 2 hours"), "should show status");
917|        assert!(out.len() < raw.len(), "output should be shorter than raw");
918|    }
919|
920|    #[test]
921|    fn test_format_compose_ps_empty() {
922|        let out = format_compose_ps("");
923|        assert!(out.contains("0"), "should show zero containers");
924|    }
925|
926|    #[test]
927|    fn test_format_compose_ps_whitespace_only() {
928|        let out = format_compose_ps("   \n  \n");
929|        assert!(out.contains("0"), "should show zero containers");
930|    }
931|
932|    #[test]
933|    fn test_format_compose_ps_exited_service() {
934|        // Tab-separated --format output
935|        let raw = "worker-1\tpython:3.12\tExited (1) 2 minutes ago\t";
936|        let out = format_compose_ps(raw);
937|        assert!(out.contains("worker"), "should show service name");
938|        assert!(out.contains("Exited"), "should show exited status");
939|    }
940|
941|    #[test]
942|    fn test_format_compose_ps_no_ports() {
943|        let raw = "redis-1\tredis:7\tUp 5 hours\t";
944|        let out = format_compose_ps(raw);
945|        assert!(out.contains("redis"), "should show service name");
946|        // Should not show port info when no ports (but [compose] prefix is OK)
947|        let lines: Vec<&str> = out.lines().collect();
948|        let redis_line = lines.iter().find(|l| l.contains("redis")).unwrap();
949|        assert!(
950|            !redis_line.contains("] ["),
951|            "should not show port brackets when empty"
952|        );
953|    }
954|
955|    #[test]
956|    fn test_format_compose_ps_long_image_path() {
957|        let raw = "app-1\tghcr.io/myorg/myapp:latest\tUp 1 hour\t0.0.0.0:8080->8080/tcp";
958|        let out = format_compose_ps(raw);
959|        assert!(
960|            out.contains("myapp:latest"),
961|            "should shorten image to last segment"
962|        );
963|        assert!(
964|            !out.contains("ghcr.io"),
965|            "should not show full registry path"
966|        );
967|    }
968|
969|    // ── format_compose_logs ────────────────────────────────
970|
971|    #[test]
972|    fn test_format_compose_logs_basic() {
973|        let raw = "\
974|web-1  | 192.168.1.1 - GET / 200
975|web-1  | 192.168.1.1 - GET /favicon.ico 404
976|api-1  | Server listening on port 3000
977|api-1  | Connected to database";
978|        let out = format_compose_logs(raw);
979|        assert!(out.contains("Logs"), "should have compose logs header");
980|    }
981|
982|    #[test]
983|    fn test_format_compose_logs_empty() {
984|        let out = format_compose_logs("");
985|        assert!(out.contains("No logs"), "should indicate no logs");
986|    }
987|
988|    // ── format_compose_build ───────────────────────────────
989|
990|    #[test]
991|    fn test_format_compose_build_basic() {
992|        let raw = "\
993|[+] Building 12.3s (8/8) FINISHED
994| => [web internal] load build definition from Dockerfile           0.0s
995| => [web internal] load metadata for docker.io/library/node:20     1.2s
996| => [web 1/4] FROM docker.io/library/node:20@sha256:abc123         0.0s
997| => [web 2/4] WORKDIR /app                                         0.1s
998| => [web 3/4] COPY package*.json ./                                0.1s
999| => [web 4/4] RUN npm install                                      8.5s
1000| => [web] exporting to image                                       2.3s
1001| => => naming to docker.io/library/myapp-web                       0.0s";
1002|        let out = format_compose_build(raw);
1003|        assert!(out.contains("12.3s"), "should show total build time");
1004|        assert!(out.contains("web"), "should show service name");
1005|        assert!(out.len() < raw.len(), "should be shorter than raw");
1006|    }
1007|
1008|    #[test]
1009|    fn test_format_compose_build_empty() {
1010|        let out = format_compose_build("");
1011|        assert!(
1012|            !out.is_empty(),
1013|            "should produce output even for empty input"
1014|        );
1015|    }
1016|
1017|    // ── compact_ports (existing, previously untested) ──────
1018|
1019|    #[test]
1020|    fn test_compact_ports_empty() {
1021|        assert_eq!(compact_ports(""), "-");
1022|    }
1023|
1024|    #[test]
1025|    fn test_compact_ports_single() {
1026|        let result = compact_ports("0.0.0.0:8080->80/tcp");
1027|        assert!(result.contains("8080"));
1028|    }
1029|
1030|    #[test]
1031|    fn test_compact_ports_many() {
1032|        let result = compact_ports("0.0.0.0:80->80/tcp, 0.0.0.0:443->443/tcp, 0.0.0.0:8080->8080/tcp, 0.0.0.0:9090->9090/tcp");
1033|        assert!(result.contains("…"), "should truncate for >3 ports");
1034|    }
1035|
1036|    #[test]
1037|    fn test_kubectl_get_target_pods_aliases() {
1038|        for resource in ["po", "pod", "pods"] {
1039|            let args = vec![resource.to_string(), "-n".to_string(), "default".to_string()];
1040|
1041|            assert_eq!(
1042|                kubectl_get_target(&args),
1043|                Some(("pods", &args[1..])),
1044|                "failed for {resource}"
1045|            );
1046|        }
1047|    }
1048|
1049|    #[test]
1050|    fn test_kubectl_get_target_services_aliases() {
1051|        for resource in ["svc", "service", "services"] {
1052|            let args = vec![resource.to_string(), "-A".to_string()];
1053|
1054|            assert_eq!(
1055|                kubectl_get_target(&args),
1056|                Some(("services", &args[1..])),
1057|                "failed for {resource}"
1058|            );
1059|        }
1060|    }
1061|
1062|    #[test]
1063|    fn test_kubectl_get_target_unsupported_resource() {
1064|        let args = vec!["deployments".to_string()];
1065|
1066|        assert_eq!(kubectl_get_target(&args), None);
1067|    }
1068|
1069|    #[test]
1070|    fn test_kubectl_get_target_respects_output_flags() {
1071|        for output_flag in ["-o", "-owide", "--output", "--output=json"] {
1072|            let args = vec![
1073|                "pods".to_string(),
1074|                output_flag.to_string(),
1075|                "wide".to_string(),
1076|            ];
1077|
1078|            assert_eq!(
1079|                kubectl_get_target(&args),
1080|                None,
1081|                "should pass through {output_flag}"
1082|            );
1083|        }
1084|    }
1085|}
1086|