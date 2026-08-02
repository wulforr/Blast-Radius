use std::path::Path;

const FIXTURES: [&str; 4] = ["plain", "paths", "monorepo", "cyclic"];

#[test]
fn every_fixture_exists_and_has_a_package_json() {
    for name in FIXTURES {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        assert!(root.is_dir(), "missing fixture: {name}");
        assert!(root.join("package.json").is_file(), "fixture {name} has no package.json");
    }
}

#[test]
fn fixture_json_parses() {
    for name in FIXTURES {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        for entry in walk_json(&root) {
            let text = std::fs::read_to_string(&entry).expect("readable");
            serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or_else(|e| panic!("{} is not valid json: {e}", entry.display()));
        }
    }
}

fn walk_json(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).expect("readable dir").flatten() {
            let path = entry.path();
            if path.is_dir() { stack.push(path); }
            else if path.extension().is_some_and(|e| e == "json") { out.push(path); }
        }
    }
    out
}
