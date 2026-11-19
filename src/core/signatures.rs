//! Signature extraction for `rtk read --mode signatures|map`.
//!
//! Extracts function, method, class, struct, interface, and trait signatures
//! from source files using lightweight regex patterns. Modeled after lean-ctx's
//! tree-sitter AST-based signatures but uses regex for zero-dependency extraction
//! across 10+ languages.
//!
//! Modes:
//! - `signatures` — only function/method/class signatures, no bodies
//! - `map` — signatures + imports + module declarations

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|_| panic!("BUG: invalid regex: {pattern}"))
}

/// Signature patterns per file extension, compiled once.
static SIG_PATTERNS: LazyLock<HashMap<&'static str, Vec<Regex>>> = LazyLock::new(|| {
    HashMap::from([
        (
            "rs",
            vec![
                regex(r"^(pub\s+)?(async\s+)?fn\s+\w+"),
                regex(r"^(pub\s+)?struct\s+\w+"),
                regex(r"^(pub\s+)?enum\s+\w+"),
                regex(r"^(pub\s+)?trait\s+\w+"),
                regex(r"^(pub\s+)?impl\b"),
                regex(r"^(pub\s+)?type\s+\w+"),
                regex(r"^(pub\s+)?mod\s+\w+"),
            ],
        ),
        (
            "py",
            vec![
                regex(r"^\s*(async\s+)?def\s+\w+"),
                regex(r"^\s*class\s+\w+"),
            ],
        ),
        (
            "ts",
            vec![
                regex(r"^\s*(export\s+)?(async\s+)?function\s+\w+"),
                regex(r"^\s*(export\s+)?(abstract\s+)?class\s+\w+"),
                regex(r"^\s*(export\s+)?interface\s+\w+"),
                regex(r"^\s*(export\s+)?type\s+\w+"),
                regex(r"^\s*(export\s+)?enum\s+\w+"),
                regex(r"^\s*(public\s+|private\s+|protected\s+)?(async\s+)?\w+\s*\([^)]*\)\s*:"),
            ],
        ),
        (
            "js",
            vec![
                regex(r"^\s*(async\s+)?function\s+\w+"),
                regex(r"^\s*class\s+\w+"),
                regex(r"^\s*(async\s+)?\w+\s*=\s*(async\s+)?\([^)]*\)\s*=>"),
            ],
        ),
        (
            "go",
            vec![
                regex(r"^func\s+(\(\w+\s+\*?\w+\)\s+)?\w+"),
                regex(r"^type\s+\w+\s+(struct|interface)"),
            ],
        ),
        (
            "java",
            vec![
                regex(
                    r"^\s*(public\s+|private\s+|protected\s+)?(static\s+)?(class|interface|enum)\s+\w+",
                ),
                regex(r"^\s*(public\s+|private\s+|protected\s+)?(static\s+)?\w+\s+\w+\s*\([^)]*\)"),
            ],
        ),
        (
            "rb",
            vec![
                regex(r"^\s*def\s+\w+"),
                regex(r"^\s*class\s+\w+"),
                regex(r"^\s*module\s+\w+"),
            ],
        ),
        (
            "c",
            vec![
                regex(r"^\s*\w+\s+\w+\s*\([^)]*\)\s*;"),
                regex(r"^\s*struct\s+\w+"),
                regex(r"^\s*enum\s+\w+"),
            ],
        ),
        (
            "cpp",
            vec![
                regex(
                    r"^\s*(virtual\s+)?\w+\s+\w+\s*\([^)]*\)\s*(const\s*)?(override\s*)?(;\s*)?$",
                ),
                regex(r"^\s*(class|struct|enum)\s+\w+"),
            ],
        ),
    ])
});

fn sig_patterns_for(ext: &str) -> &'static [Regex] {
    match ext {
        "tsx" => &SIG_PATTERNS["ts"],
        "jsx" | "mjs" | "cjs" => &SIG_PATTERNS["js"],
        "kt" | "kts" => &SIG_PATTERNS["java"],
        "h" => &SIG_PATTERNS["c"],
        "cc" | "cxx" | "hpp" | "hh" => &SIG_PATTERNS["cpp"],
        other => SIG_PATTERNS.get(other).map(Vec::as_slice).unwrap_or(&[]),
    }
}

/// Import/dependency patterns per file extension, compiled once.
static IMPORT_PATTERNS: LazyLock<HashMap<&'static str, Vec<Regex>>> = LazyLock::new(|| {
    let includes = vec![regex(r"^\s*#include\s+")];
    let js_imports = vec![
        regex(r"^\s*import\s+"),
        regex(r"^\s*const\s+\w+\s*=\s*require\("),
    ];
    let mut map = HashMap::from([
        ("rs", vec![regex(r"^use\s+")]),
        ("py", vec![regex(r"^\s*(from\s+\S+\s+)?import\s+")]),
        ("ts", js_imports.clone()),
        ("js", js_imports),
        ("go", vec![Regex::new(r#"^\s*""#).unwrap()]), // import block lines
        ("java", vec![regex(r"^\s*import\s+")]),
        ("rb", vec![regex(r"^\s*require\s+")]),
        ("c", includes.clone()),
    ]);
    // Extension aliases share one pattern set.
    map.insert("tsx", map["ts"].clone());
    map.insert("jsx", map["ts"].clone());
    map.insert("mjs", map["ts"].clone());
    map.insert("cjs", map["ts"].clone());
    map.insert("kt", map["java"].clone());
    map.insert("kts", map["java"].clone());
    map.insert("h", map["c"].clone());
    for alias in ["cc", "cxx", "hpp", "hh"] {
        map.insert(alias, includes.clone());
    }
    map
});

/// Extract signatures from source code, returning one line per declaration.
pub fn extract_signatures(content: &str, ext: &str) -> String {
    let patterns: &[Regex] = sig_patterns_for(ext);
    let mut results: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }
        for pat in patterns {
            if pat.is_match(trimmed) {
                results.push(trimmed.to_string());
                break;
            }
        }
    }

    if results.is_empty() {
        "(no signatures found)".to_string()
    } else {
        results.join("\n")
    }
}

/// Extract signatures + imports for map mode.
pub fn extract_map(content: &str, ext: &str) -> String {
    let sigs = extract_signatures(content, ext);
    let imports = extract_imports(content, ext);

    if imports.is_empty() {
        sigs
    } else {
        format!("{imports}\n\n{sigs}")
    }
}

/// Extract import/dependency lines.
fn extract_imports(content: &str, ext: &str) -> String {
    let import_patterns: &[Regex] = IMPORT_PATTERNS.get(ext).map(Vec::as_slice).unwrap_or(&[]);
    let mut results: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        for pat in import_patterns {
            if pat.is_match(trimmed) {
                results.push(trimmed.to_string());
                break;
            }
        }
    }

    results.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_signatures() {
        let code = "// comment\npub async fn main() {\n    println!(\"hi\");\n}\n\npub struct Config {\n    debug: bool,\n}";
        let result = extract_signatures(code, "rs");
        assert!(result.contains("pub async fn main()"));
        assert!(result.contains("pub struct Config"));
        assert!(!result.contains("println"));
    }

    #[test]
    fn test_python_signatures() {
        let code = "def foo(x):\n    return x\n\nclass Bar:\n    def method(self):\n        pass";
        let result = extract_signatures(code, "py");
        assert!(result.contains("def foo(x):"));
        assert!(result.contains("class Bar:"));
        assert!(result.contains("def method(self):"));
    }

    #[test]
    fn test_typescript_signatures() {
        let code = "import { z } from 'zod';\n\nexport function hello(name: string): string {\n  return name;\n}\n\nexport interface User {\n  id: number;\n}";
        let result = extract_signatures(code, "ts");
        assert!(result.contains("export function hello"));
        assert!(result.contains("export interface User"));
    }

    #[test]
    fn test_map_mode_includes_imports() {
        let code = "use std::collections::HashMap;\n\npub fn get_map() -> HashMap<String, u32> {\n  HashMap::new()\n}";
        let result = extract_map(code, "rs");
        assert!(result.contains("use std::collections::HashMap"));
        assert!(result.contains("pub fn get_map"));
    }

    #[test]
    fn test_unknown_extension() {
        let result = extract_signatures("some text", "xyz");
        assert_eq!(result, "(no signatures found)");
    }
}
