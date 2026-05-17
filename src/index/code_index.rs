//! Code indexing engine inspired by codedb — structural outlines, symbol search,
//! and lightweight dependency tracking without external parsers.
//!
//! Conceptual inspiration: codedb by Rach Pradhan (BSD-3-Clause License).
//! This is an original implementation; no source code was copied.
//! Licensed under the Apache License, Version 2.0.

use anyhow::{Context, Result};
use regex::Regex;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

pub struct CodeIndex {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Function,
    Struct,
    Class,
    Interface,
    Enum,
    Trait,
    Impl,
    Module,
    Import,
    Const,
    Static,
    Type,
    Unknown,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Struct => "struct",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Module => "mod",
            SymbolKind::Import => "import",
            SymbolKind::Const => "const",
            SymbolKind::Static => "static",
            SymbolKind::Type => "type",
            SymbolKind::Unknown => "?",
        }
    }
}

impl CodeIndex {
    pub fn open(base_path: &Path) -> Result<Self> {
        let data_dir = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
            .context("Cannot determine data directory")?
            .join("rtk")
            .join("index");
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join(format!(
            "code_{}.db",
            super::file_index::sanitize_path(base_path)
        ));

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Cannot open code index DB at {:?}", db_path))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS symbols (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                file TEXT NOT NULL,
                line INTEGER NOT NULL,
                signature TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_name ON symbols(name);
            CREATE INDEX IF NOT EXISTS idx_file ON symbols(file);
            CREATE INDEX IF NOT EXISTS idx_kind ON symbols(kind);

            CREATE TABLE IF NOT EXISTS file_content (
                file TEXT PRIMARY KEY,
                content_preview TEXT NOT NULL
            );
            ",
        )?;

        Ok(Self { conn })
    }

    pub fn scan_file(&mut self, rel_path: &str, content: &str) -> Result<usize> {
        let symbols = extract_symbols(rel_path, content);
        let tx = self.conn.transaction()?;

        // Remove existing symbols for this file
        tx.execute("DELETE FROM symbols WHERE file = ?1", [rel_path])?;
        tx.execute(
            "INSERT OR REPLACE INTO file_content (file, content_preview)
             VALUES (?1, ?2)",
            [rel_path, &content.chars().take(2000).collect::<String>()],
        )?;

        for sym in &symbols {
            tx.execute(
                "INSERT INTO symbols (name, kind, file, line, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                [
                    &sym.name,
                    sym.kind.as_str(),
                    &sym.file,
                    &sym.line.to_string(),
                    &sym.signature,
                ],
            )?;
        }

        tx.commit()?;
        Ok(symbols.len())
    }

    #[allow(dead_code)]
    pub fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, file, line, signature FROM symbols WHERE name = ?1 ORDER BY file, line"
        )?;
        let rows = stmt.query_map([name], |row| {
            Ok(Symbol {
                name: row.get(0)?,
                kind: parse_kind(&row.get::<_, String>(1)?),
                file: row.get(2)?,
                line: row.get::<_, i64>(3)? as usize,
                signature: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    #[allow(dead_code)]
    pub fn search_symbols(&self, query: &str, limit: usize) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, file, line, signature FROM symbols
             WHERE name LIKE '%' || ?1 || '%'
             ORDER BY file, line
             LIMIT ?2",
        )?;
        let rows = stmt.query_map([query, &limit.to_string()], |row| {
            Ok(Symbol {
                name: row.get(0)?,
                kind: parse_kind(&row.get::<_, String>(1)?),
                file: row.get(2)?,
                line: row.get::<_, i64>(3)? as usize,
                signature: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    #[allow(dead_code)]
    pub fn outline(&self, file: &str) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, file, line, signature FROM symbols
             WHERE file = ?1
             ORDER BY line",
        )?;
        let rows = stmt.query_map([file], |row| {
            Ok(Symbol {
                name: row.get(0)?,
                kind: parse_kind(&row.get::<_, String>(1)?),
                file: row.get(2)?,
                line: row.get::<_, i64>(3)? as usize,
                signature: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    #[allow(dead_code)]
    pub fn all_files_with_symbols(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT file FROM symbols ORDER BY file")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn stats(&self) -> Result<HashMap<String, usize>> {
        let mut stmt = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM symbols GROUP BY kind")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(|e| e.into())
    }
}

fn parse_kind(s: &str) -> SymbolKind {
    match s {
        "fn" => SymbolKind::Function,
        "struct" => SymbolKind::Struct,
        "class" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "enum" => SymbolKind::Enum,
        "trait" => SymbolKind::Trait,
        "impl" => SymbolKind::Impl,
        "mod" => SymbolKind::Module,
        "import" => SymbolKind::Import,
        "const" => SymbolKind::Const,
        "static" => SymbolKind::Static,
        "type" => SymbolKind::Type,
        _ => SymbolKind::Unknown,
    }
}

fn extract_symbols(file: &str, content: &str) -> Vec<Symbol> {
    let lang = detect_language_from_path(file);
    let lines: Vec<&str> = content.lines().collect();
    let mut symbols = Vec::new();

    match lang.as_str() {
        "rust" => extract_rust(&lines, file, &mut symbols),
        "python" => extract_python(&lines, file, &mut symbols),
        "javascript" | "typescript" => extract_js_ts(&lines, file, &mut symbols),
        "go" => extract_go(&lines, file, &mut symbols),
        "c" | "cpp" => extract_c_cpp(&lines, file, &mut symbols),
        _ => {}
    }

    symbols
}

fn detect_language_from_path(file: &str) -> String {
    let path = Path::new(file);
    super::file_index::detect_language(path)
}

fn extract_rust(lines: &[&str], file: &str, out: &mut Vec<Symbol>) {
    lazy_static::lazy_static! {
        static ref RE_FN: Regex = Regex::new(r"^\s*(pub\s+)?(async\s+)?(unsafe\s+)?fn\s+(\w+)").unwrap();
        static ref RE_STRUCT: Regex = Regex::new(r"^\s*(pub\s+)?struct\s+(\w+)").unwrap();
        static ref RE_TRAIT: Regex = Regex::new(r"^\s*(pub\s+)?trait\s+(\w+)").unwrap();
        static ref RE_ENUM: Regex = Regex::new(r"^\s*(pub\s+)?enum\s+(\w+)").unwrap();
        static ref RE_IMPL: Regex = Regex::new(r"^\s*impl(?:\s+<[^>]+>)?\s+(\w+)").unwrap();
        static ref RE_MOD: Regex = Regex::new(r"^\s*(pub\s+)?mod\s+(\w+)").unwrap();
        static ref RE_USE: Regex = Regex::new(r"^\s*use\s+([^;]+);").unwrap();
        static ref RE_CONST: Regex = Regex::new(r"^\s*(pub\s+)?const\s+(\w+)").unwrap();
        static ref RE_TYPE: Regex = Regex::new(r"^\s*(pub\s+)?type\s+(\w+)").unwrap();
    }

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = RE_FN.captures(line) {
            out.push(Symbol {
                name: cap[4].to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_STRUCT.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Struct,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_TRAIT.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Trait,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_ENUM.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Enum,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_IMPL.captures(line) {
            out.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Impl,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_MOD.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Module,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_USE.captures(line) {
            out.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Import,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_CONST.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Const,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_TYPE.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Type,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        }
    }
}

fn extract_python(lines: &[&str], file: &str, out: &mut Vec<Symbol>) {
    lazy_static::lazy_static! {
        static ref RE_DEF: Regex = Regex::new(r"^(\s*)def\s+(\w+)").unwrap();
        static ref RE_CLASS: Regex = Regex::new(r"^(\s*)class\s+(\w+)").unwrap();
        static ref RE_IMPORT: Regex = Regex::new(r"^\s*(import|from)\s+([^;]+)").unwrap();
    }

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = RE_DEF.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_CLASS.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Class,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_IMPORT.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Import,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        }
    }
}

fn extract_js_ts(lines: &[&str], file: &str, out: &mut Vec<Symbol>) {
    lazy_static::lazy_static! {
        static ref RE_FN: Regex = Regex::new(r"^\s*(export\s+)?(async\s+)?function\s+(\w+)").unwrap();
        static ref RE_ARROW: Regex = Regex::new(r"^\s*(export\s+)?const\s+(\w+)\s*=").unwrap();
        static ref RE_CLASS: Regex = Regex::new(r"^\s*(export\s+)?class\s+(\w+)").unwrap();
        static ref RE_INTERFACE: Regex = Regex::new(r"^\s*(export\s+)?interface\s+(\w+)").unwrap();
        static ref RE_IMPORT: Regex = Regex::new(r"^\s*import\s+[^;]+").unwrap();
    }

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = RE_FN.captures(line) {
            out.push(Symbol {
                name: cap[3].to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_CLASS.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Class,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_INTERFACE.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Interface,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_ARROW.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if RE_IMPORT.is_match(line) {
            out.push(Symbol {
                name: line.trim().to_string(),
                kind: SymbolKind::Import,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        }
    }
}

fn extract_go(lines: &[&str], file: &str, out: &mut Vec<Symbol>) {
    lazy_static::lazy_static! {
        static ref RE_FN: Regex = Regex::new(r"^\s*func\s+(?:\([^)]+\)\s+)?(\w+)").unwrap();
        static ref RE_TYPE: Regex = Regex::new(r"^\s*type\s+(\w+)").unwrap();
        static ref RE_IMPORT: Regex = Regex::new(r"^\s*import\s+").unwrap();
    }

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = RE_FN.captures(line) {
            out.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_TYPE.captures(line) {
            out.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Type,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if RE_IMPORT.is_match(line) {
            out.push(Symbol {
                name: line.trim().to_string(),
                kind: SymbolKind::Import,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        }
    }
}

fn extract_c_cpp(lines: &[&str], file: &str, out: &mut Vec<Symbol>) {
    lazy_static::lazy_static! {
        static ref RE_FN: Regex = Regex::new(r"^\s*(?:[\w*]+\s+)+(\w+)\s*\([^)]*\)\s*\{").unwrap();
        static ref RE_STRUCT: Regex = Regex::new(r"^\s*(typedef\s+)?struct\s+(\w+)").unwrap();
        static ref RE_INCLUDE: Regex = Regex::new(r"^\s*#include\s+.*").unwrap();
    }

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if let Some(cap) = RE_FN.captures(line) {
            out.push(Symbol {
                name: cap[1].to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if let Some(cap) = RE_STRUCT.captures(line) {
            out.push(Symbol {
                name: cap[2].to_string(),
                kind: SymbolKind::Struct,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        } else if RE_INCLUDE.is_match(line) {
            out.push(Symbol {
                name: line.trim().to_string(),
                kind: SymbolKind::Import,
                file: file.to_string(),
                line: line_num,
                signature: line.trim().to_string(),
            });
        }
    }
}
