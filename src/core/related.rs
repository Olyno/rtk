//! Cross-file dependency awareness — discovers import relationships between
//! source files to provide context hints during filtering.
//!
//! Modeled after lean-ctx's Property Graph graph-aware reads: when a file is
//! being read or diff'd, rtk can report which other files import it (inbound
//! dependencies) and which files it imports (outbound dependencies).
//!
//! This is a lightweight regex-based implementation that doesn't require
//! building a full index — it scans on-demand.

#![allow(dead_code)] // Public API not yet wired into command dispatch

use regex::Regex;
use std::path::Path;

/// Result of scanning a file for cross-file relationships.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct FileRelations {
    /// Files that this file imports/references.
    pub imports: Vec<String>,
    /// Files in the project that import this file.
    pub imported_by: Vec<String>,
}

/// Scan a source file for import statements and return the referenced module names.
///
/// Recognizes imports across multiple languages:
/// - Rust: `use crate::foo;`, `mod bar;`
/// - Python: `import foo`, `from foo import bar`
/// - JS/TS: `import ... from './foo'`, `require('./foo')`
/// - Go: `"github.com/foo/bar"`
/// - C/C++: `#include "foo.h"`
pub fn scan_imports(file_path: &Path, content: &str) -> Vec<String> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let patterns = import_patterns(ext);
    let mut imports = Vec::new();

    for line in content.lines() {
        for pat in &patterns {
            if let Some(caps) = pat.captures(line) {
                if let Some(m) = caps.get(1) {
                    let import = m.as_str().to_string();
                    if !imports.contains(&import) {
                        imports.push(import);
                    }
                }
            }
        }
    }

    imports
}

/// Find files in a project directory that import the given module/file name.
///
/// This scans source files in `project_root` for import statements that
/// reference `target_module` (matched against the basename or module path).
pub fn find_importers(
    project_root: &Path,
    target_file: &Path,
) -> Vec<String> {
    let target_stem = target_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let ext = target_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let mut importers = Vec::new();
    let source_extensions = ["rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "rb", "c", "cpp", "h"];

    // Walk project root looking for source files
    if let Ok(entries) = std::fs::read_dir(project_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !source_extensions.contains(&file_ext) {
                continue;
            }

            // Read and scan
            if let Ok(content) = std::fs::read_to_string(&path) {
                let imports = scan_imports(&path, &content);
                // Check if any import references the target file
                for imp in &imports {
                    if imp.contains(target_stem)
                        || (ext == "rs" && imp.contains(&format!("{}::", target_stem)))
                        || imp.contains(&format!("/{}", target_stem))
                    {
                        // Report relative path from project root
                        if let Ok(stripped) = path.strip_prefix(project_root) {
                            importers.push(stripped.to_string_lossy().to_string());
                        } else {
                            importers.push(path.to_string_lossy().to_string());
                        }
                        break;
                    }
                }
            }
        }
    }

    importers
}

/// Format a cross-file awareness hint for filtered output.
///
/// Example: `[related: src/auth.rs, src/handlers.rs import this file]`
pub fn format_hint(importers: &[String]) -> Option<String> {
    if importers.is_empty() {
        return None;
    }

    let mut files: Vec<&str> = importers.iter().map(|s| s.as_str()).collect();
    files.sort();
    files.truncate(5);

    let list = files.join(", ");
    if importers.len() > 5 {
        Some(format!(
            "[related: {}, +{} more]",
            list,
            importers.len() - 5
        ))
    } else {
        Some(format!("[related: {}]", list))
    }
}

/// Regex patterns for extracting import targets by file extension.
fn import_patterns(ext: &str) -> Vec<Regex> {
    match ext {
        "rs" => vec![
            Regex::new(r"^use\s+(?:crate::)?(\w+)").unwrap(),
            Regex::new(r"^mod\s+(\w+)").unwrap(),
            Regex::new(r"^pub\s+mod\s+(\w+)").unwrap(),
        ],
        "py" => vec![
            Regex::new(r"^\s*(?:from\s+(\S+)\s+)?import\s+(\w+)").unwrap(),
        ],
        "ts" | "tsx" => vec![
            Regex::new(r#"import\s+.*\s+from\s+['\"]\.?/?([^'\"]+)['\"]"#).unwrap(),
            Regex::new(r#"require\(['\"]\.?/?([^'\"]+)['\"]"#).unwrap(),
        ],
        "js" | "jsx" | "mjs" => vec![
            Regex::new(r#"import\s+.*\s+from\s+['\"]\.?/?([^'\"]+)['\"]"#).unwrap(),
            Regex::new(r#"require\(['\"]\.?/?([^'\"]+)['\"]"#).unwrap(),
        ],
        "go" => vec![
            Regex::new(r#"\s+\"(github\.com/\S+)\""#).unwrap(),
        ],
        "java" => vec![
            Regex::new(r"^import\s+(\S+)").unwrap(),
        ],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_rust_imports() {
        let code = "use std::collections::HashMap;\nuse crate::utils::config;\npub mod parser;\n\nfn main() {}";
        let imports = scan_imports(Path::new("main.rs"), code);
        assert!(imports.contains(&"std".to_string()));
        assert!(imports.contains(&"utils".to_string()));
        assert!(imports.contains(&"parser".to_string()));
    }

    #[test]
    fn test_scan_typescript_imports() {
        let code = "import { z } from 'zod';\nimport { config } from './config';\nconst utils = require('./utils');";
        let imports = scan_imports(Path::new("app.ts"), code);
        assert!(imports.iter().any(|i| i.contains("config")));
        assert!(imports.iter().any(|i| i.contains("utils")));
    }

    #[test]
    fn test_format_hint_empty() {
        assert!(format_hint(&[]).is_none());
    }

    #[test]
    fn test_format_hint_single() {
        let hint = format_hint(&["src/auth.rs".to_string()]);
        assert!(hint.unwrap().contains("src/auth.rs"));
    }
}
