#[cfg(test)]
mod index_tests {
    use super::super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_file_index_scan_and_search() {
        let dir = TempDir::new().unwrap();
        let mut file = std::fs::File::create(dir.path().join("main.rs")).unwrap();
        writeln!(file, "fn main() {{}}").unwrap();

        let mut idx = file_index::FileIndex::open(dir.path()).unwrap();
        let count = idx.scan().unwrap();
        assert_eq!(count, 1);

        let results = idx.search("main", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "main.rs");
        assert_eq!(results[0].language, "rust");
    }

    #[test]
    fn test_file_index_fuzzy_search() {
        let dir = TempDir::new().unwrap();
        std::fs::File::create(dir.path().join("hello_world.rs")).unwrap();
        std::fs::File::create(dir.path().join("goodbye.rs")).unwrap();

        let mut idx = file_index::FileIndex::open(dir.path()).unwrap();
        idx.scan().unwrap();

        let results = idx.fuzzy_search("hw", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_code_index_rust_symbols() {
        let dir = TempDir::new().unwrap();
        let mut idx = code_index::CodeIndex::open(dir.path()).unwrap();

        let content = r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

struct Point {
    x: f64,
    y: f64,
}
"#;
        let count = idx.scan_file("src/math.rs", content).unwrap();
        assert_eq!(count, 2);

        let symbols = idx.find_symbol("add").unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, code_index::SymbolKind::Function);
        assert_eq!(symbols[0].line, 1);

        let outline = idx.outline("src/math.rs").unwrap();
        assert_eq!(outline.len(), 2);
    }

    #[test]
    fn test_code_index_python_symbols() {
        let dir = TempDir::new().unwrap();
        let mut idx = code_index::CodeIndex::open(dir.path()).unwrap();

        let content = r#"class User:
    def __init__(self, name):
        self.name = name

def greet(user):
    return f"Hello {user.name}"
"#;
        let count = idx.scan_file("app.py", content).unwrap();
        assert_eq!(count, 3);

        let symbols = idx.find_symbol("greet").unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, code_index::SymbolKind::Function);
    }

    #[test]
    fn test_project_index_scan() {
        let dir = TempDir::new().unwrap();
        let mut f1 = std::fs::File::create(dir.path().join("lib.rs")).unwrap();
        writeln!(f1, "pub fn helper() {{}}").unwrap();
        let mut f2 = std::fs::File::create(dir.path().join("main.rs")).unwrap();
        writeln!(f2, "fn main() {{ helper(); }}").unwrap();

        let mut idx = ProjectIndex::open(dir.path()).unwrap();
        let (files, symbols) = idx.scan().unwrap();
        assert_eq!(files, 2);
        assert!(symbols >= 2);
    }
}
