//! Index-aware command helpers — lazy init, stale detection, and query routing.
//!
//! Commands that benefit from the project index (grep, find, read) call
//! `with_index()` to get a lazy-built, auto-refreshed index reference.

use crate::index::ProjectIndex;
use anyhow::Result;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

thread_local! {
    /// Per-thread singleton index — built once, reused across commands.
    static INDEX: RefCell<Option<IndexState>> = const { RefCell::new(None) };
}

struct IndexState {
    idx: ProjectIndex,
    base_path: PathBuf,
    last_scan: SystemTime,
}

/// Build the index if needed (lazy + stale detection) and run a query closure.
/// The closure receives a reference to the fresh index.
/// When `verbose` is true, index build progress is emitted to stderr.
pub fn with_index<F, T>(base_path: &Path, verbose: bool, f: F) -> Result<Option<T>>
where
    F: FnOnce(&ProjectIndex) -> Option<T>,
{
    // Check if we already have a fresh index
    let needs_build = INDEX.with(|cell| {
        let state = cell.borrow();
        !matches!(
            state.as_ref(),
            Some(s) if s.base_path == base_path && !is_stale(base_path, s.last_scan)
        )
    });

    if needs_build {
        let now = SystemTime::now();
        let mut idx = ProjectIndex::open(base_path)?;
        if verbose {
            eprintln!("rtk: building project index (one-time) ...");
        }
        let (files, symbols) = idx.scan()?;
        if verbose {
            eprintln!(
                "rtk: indexed {} files, {} symbols in {:?}",
                files, symbols, base_path
            );
        }

        INDEX.with(|cell| {
            *cell.borrow_mut() = Some(IndexState {
                idx,
                base_path: base_path.to_path_buf(),
                last_scan: now,
            });
        });
    }

    INDEX.with(|cell| {
        let state = cell.borrow();
        match state.as_ref() {
            Some(s) if s.base_path == base_path => Ok(f(&s.idx)),
            _ => Ok(None),
        }
    })
}

/// Check if the index is stale by comparing against git HEAD timestamp.
fn is_stale(base_path: &Path, last_scan: SystemTime) -> bool {
    if let Ok(output) = std::process::Command::new("git")
        .args(["-C"])
        .arg(base_path)
        .args(["log", "-1", "--format=%ct", "HEAD"])
        .output()
    {
        if let Ok(ts_str) = String::from_utf8(output.stdout) {
            if let Ok(ts) = ts_str.trim().parse::<u64>() {
                let head_time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(ts);
                if head_time > last_scan {
                    return true;
                }
            }
        }
    }

    // Fallback: check if any file in src/ is newer than last scan
    if let Ok(entries) = std::fs::read_dir(base_path.join("src")) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if let Ok(mod_time) = meta.modified() {
                    if mod_time > last_scan {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Try to find symbols matching a query in the code index.
/// Returns None if the index isn't suitable for this query (regex, too broad).
pub fn query_code_index(idx: &ProjectIndex, pattern: &str) -> Option<Vec<String>> {
    // Only route simple word queries through the index — no regex, no paths
    if pattern.contains('*')
        || pattern.contains('.')
        || pattern.contains('/')
        || pattern.contains('\\')
        || pattern.contains('[')
        || pattern.contains('(')
        || pattern.len() < 2
    {
        return None;
    }

    let symbols = idx.code.search_symbols(pattern, 50).ok()?;
    if symbols.is_empty() {
        return None;
    }

    let mut lines: Vec<String> = Vec::with_capacity(symbols.len());
    for sym in symbols.iter() {
        lines.push(format!(
            "{}:{}:{} {} {}",
            sym.file, sym.line, sym.kind.as_str(), sym.name, sym.signature
        ));
    }
    Some(lines)
}

/// Try to find files matching a pattern in the file index.
#[allow(dead_code)]
pub fn query_file_index(idx: &ProjectIndex, pattern: &str) -> Option<Vec<String>> {
    let files = idx.files.search(pattern, 30).ok()?;
    if files.is_empty() {
        return None;
    }

    let mut lines: Vec<String> = Vec::with_capacity(files.len().min(30));
    for f in files.iter().take(30) {
        lines.push(format!("{} ({}, {})", f.path, f.language, f.size));
    }
    Some(lines)
}
