use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// Every module that touches files goes through this. Nothing calls `std::fs`
/// directly, which is what lets the resolver suite run entirely in memory.
pub trait FileSystem {
    fn read(&self, path: &str) -> Option<String>;
    fn exists(&self, path: &str) -> bool;
    fn read_dir(&self, path: &str) -> Vec<DirEntry>;
}

pub struct RealFs {
    root: PathBuf,
}

impl RealFs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn full(&self, path: &str) -> PathBuf {
        if path.is_empty() { self.root.clone() } else { self.root.join(path) }
    }
}

impl FileSystem for RealFs {
    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(self.full(path)).ok()
    }

    fn exists(&self, path: &str) -> bool {
        self.full(path).exists()
    }

    fn read_dir(&self, path: &str) -> Vec<DirEntry> {
        let Ok(entries) = std::fs::read_dir(self.full(path)) else {
            return Vec::new();
        };
        let mut out: Vec<DirEntry> = entries
            .flatten()
            .map(|entry| DirEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: entry.path().is_dir(),
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

#[derive(Default)]
pub struct MemFs {
    files: BTreeMap<String, String>,
}

impl MemFs {
    pub fn from<const N: usize>(entries: [(&str, &str); N]) -> Self {
        Self {
            files: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    pub fn insert(&mut self, path: &str, contents: &str) {
        self.files.insert(path.to_string(), contents.to_string());
    }
}

impl FileSystem for MemFs {
    fn read(&self, path: &str) -> Option<String> {
        self.files.get(path).cloned()
    }

    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
            || self.files.keys().any(|k| k.starts_with(&format!("{path}/")))
    }

    fn read_dir(&self, path: &str) -> Vec<DirEntry> {
        let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
        let mut names: BTreeMap<String, bool> = BTreeMap::new();
        for key in self.files.keys() {
            let Some(rest) = key.strip_prefix(&prefix) else { continue };
            if rest.is_empty() { continue }
            match rest.split_once('/') {
                Some((head, _)) => { names.insert(head.to_string(), true); }
                None => { names.insert(rest.to_string(), false); }
            }
        }
        names.into_iter().map(|(name, is_dir)| DirEntry { name, is_dir }).collect()
    }
}

/// Normalise a path for storage in the graph: forward slashes, no leading `./`.
pub fn normalise(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").unwrap_or(&text).to_string()
}
