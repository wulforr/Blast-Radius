use crate::fs::FileSystem;

const SOURCE_EXTENSIONS: [&str; 6] = ["ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// Directories never worth walking. Not configurable in phase 1 — every one of
/// these is either generated output or a dependency tree, and walking them
/// turns a 30-second analysis into a several-minute one.
const IGNORED_DIRS: [&str; 10] = [
    "node_modules", "dist", "build", "out", "coverage",
    ".next", ".nuxt", ".output", ".git", "target",
];

pub fn discover(fs: &dyn FileSystem, root: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_string()];

    while let Some(dir) = stack.pop() {
        for entry in fs.read_dir(&dir) {
            let path = if dir.is_empty() { entry.name.clone() } else { format!("{dir}/{}", entry.name) };

            if entry.is_dir {
                if !IGNORED_DIRS.contains(&entry.name.as_str()) {
                    stack.push(path);
                }
                continue;
            }

            if is_analysable(&entry.name) {
                found.push(path);
            }
        }
    }

    found.sort();
    found
}

fn is_analysable(name: &str) -> bool {
    // A .d.ts declares types and contains no runtime imports worth attributing
    // a blast radius to.
    if name.ends_with(".d.ts") || name.ends_with(".d.mts") || name.ends_with(".d.cts") {
        return false;
    }
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| SOURCE_EXTENSIONS.contains(&ext))
}
