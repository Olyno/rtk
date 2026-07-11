//! rtk index — unified file and code indexing engine.
//!
//! Combines fff-inspired file indexing (frecency, fuzzy search)
//! with codedb-inspired code indexing (outlines, symbols, lightweight search).
//!
//! See NOTICE for attribution details.

pub mod code_index;
pub mod file_index;

#[cfg(test)]
mod tests;

use anyhow::Result;
use std::path::Path;

/// Unified project index holding both file and code indices.
pub struct ProjectIndex {
    pub files: file_index::FileIndex,
    pub code: code_index::CodeIndex,
}

impl ProjectIndex {
    pub fn open(base_path: &Path) -> Result<Self> {
        Ok(Self {
            files: file_index::FileIndex::open(base_path)?,
            code: code_index::CodeIndex::open(base_path)?,
        })
    }

    /// Full project scan: index files then extract code symbols from source files only.
    pub fn scan(&mut self) -> Result<(usize, usize)> {
        let file_count = self.files.scan()?;
        let mut symbol_count = 0;

        let files = self.files.all_files()?;
        for entry in files {
            // Skip non-source files to avoid parsing binaries, images, lockfiles, etc.
            if entry.language == "unknown"
                || entry.language == "json"
                || entry.language == "markdown"
            {
                continue;
            }
            let rel = &entry.path;
            let full = self.files.base_path.join(rel);
            // Skip files that are too large to parse efficiently (>500KB)
            if entry.size > 500_000 {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&full) {
                if let Ok(n) = self.code.scan_file(rel, &content) {
                    symbol_count += n;
                }
            }
        }

        Ok((file_count, symbol_count))
    }
}
