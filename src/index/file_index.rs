//! File indexing engine inspired by fff.nvim — fast file search with frecency ranking.
//!
//! Conceptual inspiration: fff.nvim by Dmitriy Kovalenko (MIT License).
//! This is an original implementation; no source code was copied.
//! Licensed under the Apache License, Version 2.0.

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

pub struct FileIndex {
    conn: Connection,
    pub base_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    #[allow(dead_code)]
    pub lines: usize,
    pub language: String,
    #[allow(dead_code)]
    pub last_access: u64,
    #[allow(dead_code)]
    pub access_count: u64,
}

impl FileIndex {
    pub fn open(base_path: &Path) -> Result<Self> {
        let data_dir = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
            .context("Cannot determine data directory")?
            .join("rtk")
            .join("index");
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join(format!("files_{}.db", sanitize_path(base_path)));

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Cannot open file index DB at {:?}", db_path))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                size INTEGER NOT NULL,
                lines INTEGER NOT NULL,
                language TEXT NOT NULL,
                last_access INTEGER NOT NULL,
                access_count INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lang ON files(language);
            ",
        )?;

        Ok(Self {
            conn,
            base_path: base_path.to_path_buf(),
        })
    }

    pub fn scan(&mut self) -> Result<usize> {
        let mut count = 0;
        let tx = self.conn.transaction()?;

        // Clear existing index for fresh scan
        tx.execute("DELETE FROM files", [])?;

        for entry in WalkDir::new(&self.base_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(should_index)
        {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let rel = path
                .strip_prefix(&self.base_path)
                .unwrap_or(path)
                .to_path_buf();
            let rel_str = rel.to_string_lossy().to_string();

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let lines = count_lines(path).unwrap_or(0);
            let language = detect_language(path);

            tx.execute(
                "INSERT INTO files (path, size, lines, language, last_access, access_count)
                 VALUES (?1, ?2, ?3, ?4, 0, 0)
                 ON CONFLICT(path) DO UPDATE SET
                   size=excluded.size,
                   lines=excluded.lines,
                   language=excluded.language",
                [&rel_str, &size.to_string(), &lines.to_string(), &language],
            )?;
            count += 1;
        }

        tx.commit()?;
        Ok(count)
    }

    #[allow(dead_code)]
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FileEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, size, lines, language, last_access, access_count
             FROM files
             WHERE path LIKE '%' || ?1 || '%'
             ORDER BY (access_count * 100) + last_access DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map([query, &limit.to_string()], |row| {
            Ok(FileEntry {
                path: row.get(0)?,
                size: row.get::<_, i64>(1)? as u64,
                lines: row.get::<_, i64>(2)? as usize,
                language: row.get(3)?,
                last_access: row.get::<_, i64>(4)? as u64,
                access_count: row.get::<_, i64>(5)? as u64,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    #[allow(dead_code)]
    pub fn fuzzy_search(&self, query: &str, limit: usize) -> Result<Vec<FileEntry>> {
        let all = self.all_files()?;
        let mut scored: Vec<(i64, FileEntry)> = all
            .into_iter()
            .map(|e| {
                let score = fuzzy_score(&e.path, query);
                (score, e)
            })
            .filter(|(s, _)| *s > 0)
            .collect();
        scored.sort_by_key(|b| std::cmp::Reverse(b.0));
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, e)| e).collect())
    }

    pub fn all_files(&self) -> Result<Vec<FileEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, size, lines, language, last_access, access_count FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok(FileEntry {
                path: row.get(0)?,
                size: row.get::<_, i64>(1)? as u64,
                lines: row.get::<_, i64>(2)? as usize,
                language: row.get(3)?,
                last_access: row.get::<_, i64>(4)? as u64,
                access_count: row.get::<_, i64>(5)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    #[allow(dead_code)]
    pub fn record_access(&mut self, path: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO files (path, size, lines, language, last_access, access_count)
             VALUES (?1, 0, 0, '', ?2, 1)
             ON CONFLICT(path) DO UPDATE SET
               last_access = ?2,
               access_count = access_count + 1",
            [path, &now.to_string()],
        )?;
        Ok(())
    }

    pub fn language_breakdown(&self) -> Result<HashMap<String, usize>> {
        let mut stmt = self
            .conn
            .prepare("SELECT language, COUNT(*) FROM files GROUP BY language")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(|e| e.into())
    }
}

fn should_index(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if name.starts_with('.')
        && name != "."
        && name != ".."
        && (name == ".git" || name == ".svn" || name == ".hg")
    {
        return false;
    }
    if name == "node_modules"
        || name == "target"
        || name == "zig-cache"
        || name == "__pycache__"
        || name == "dist"
        || name == "build"
        || name == ".venv"
        || name == "venv"
    {
        return false;
    }
    true
}

fn count_lines(path: &Path) -> Option<usize> {
    let content = std::fs::read(path).ok()?;
    Some(content.iter().filter(|&&b| b == b'\n').count() + 1)
}

pub fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("go") => "go",
        Some("zig") => "zig",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("hpp") | Some("cc") => "cpp",
        Some("rb") => "ruby",
        Some("php") => "php",
        Some("java") => "java",
        Some("kt") => "kotlin",
        Some("swift") => "swift",
        Some("md") => "markdown",
        Some("toml") => "toml",
        Some("yaml") | Some("yml") => "yaml",
        Some("json") => "json",
        Some("sh") | Some("bash") => "shell",
        Some("lua") => "lua",
        _ => "unknown",
    }
    .to_string()
}

pub fn sanitize_path(path: &Path) -> String {
    path.to_string_lossy().replace(['/', '\\', '.'], "_")
}

#[allow(dead_code)]
fn fuzzy_score(path: &str, query: &str) -> i64 {
    let path_lower = path.to_lowercase();
    let query_lower = query.to_lowercase();

    if path_lower == query_lower {
        return 1000;
    }
    if path_lower.ends_with(&query_lower) {
        return 900;
    }
    if path_lower.contains(&query_lower) {
        return 800;
    }

    let mut pi = 0;
    let mut qi = 0;
    let path_chars: Vec<char> = path_lower.chars().collect();
    let query_chars: Vec<char> = query_lower.chars().collect();

    while pi < path_chars.len() && qi < query_chars.len() {
        if path_chars[pi] == query_chars[qi] {
            qi += 1;
        }
        pi += 1;
    }

    if qi == query_chars.len() {
        return 700 - (path.len() as i64 - query.len() as i64);
    }

    0
}
