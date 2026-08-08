use std::collections::BTreeMap;

use crate::fs::FileSystem;
use crate::resolve::relative::probe_file;
use crate::resolve::Resolved;

/// Package name → its root directory and entry file, both relative to the
/// analysis root. The root matters because deep imports (`@acme/ui/src/Button`)
/// resolve against the package directory, not against the entry file's
/// directory — those differ whenever `main` points into a subdirectory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Workspace {
    pub packages: BTreeMap<String, WorkspacePackage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePackage {
    /// Package root, relative to the analysis root.
    pub dir: String,
    /// Entry file, relative to the analysis root.
    pub entry: String,
}

/// `main` fallbacks when a workspace `package.json` names no entry. Tried in
/// order; the first that probes to a real file wins.
const DEFAULT_ENTRIES: [&str; 3] = ["src/index.ts", "index.ts", "index.js"];

/// Node.js core modules. Anything with a `node:` prefix is a builtin, and so
/// is a bare import of one of these names (with any subpath).
const NODE_BUILTINS: [&str; 41] = [
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "worker_threads",
    "zlib",
];

pub(crate) fn is_builtin(specifier: &str) -> bool {
    let bare = specifier.strip_prefix("node:").unwrap_or(specifier);
    let name = bare.split('/').next().unwrap_or(bare);
    NODE_BUILTINS.contains(&name)
}

/// Discover workspace packages at `root` from `pnpm-workspace.yaml` and
/// `package.json#workspaces`, mapping each package name to its entry file.
pub fn load_workspace(fs: &dyn FileSystem, root: &str) -> Workspace {
    let mut globs = read_pnpm_globs(fs, root);
    globs.extend(read_npm_globs(fs, root));

    let mut packages = BTreeMap::new();
    for glob in globs {
        for dir in expand_glob(fs, root, &glob) {
            if let Some((name, entry)) = read_package(fs, &dir) {
                packages.insert(name, entry);
            }
        }
    }
    Workspace { packages }
}

/// Rung three of the ladder in spec §4: workspace packages.
///
/// Returns `None` when the specifier names no known workspace package, so the
/// ladder falls through to external. A known package whose target is missing
/// is `Unresolved`.
pub(crate) fn resolve_workspace(
    fs: &dyn FileSystem,
    workspace: &Workspace,
    specifier: &str,
) -> Option<Resolved> {
    let (name, subpath) = split_package(specifier)?;
    let package = workspace.packages.get(name)?;
    if subpath.is_empty() {
        return Some(Resolved::File(package.entry.clone()));
    }
    let base = if package.dir.is_empty() {
        subpath.to_string()
    } else {
        format!("{}/{subpath}", package.dir)
    };
    match probe_file(fs, &base) {
        Some(found) => Some(Resolved::File(found)),
        None => Some(Resolved::Unresolved),
    }
}

/// Split a bare specifier into its package name and subpath. Scoped packages
/// (`@acme/ui/...`) take two segments; everything else takes one. Relative,
/// absolute, and `node:` specifiers are not packages.
fn split_package(specifier: &str) -> Option<(&str, &str)> {
    if specifier.starts_with('.') || specifier.starts_with('/') || specifier.starts_with("node:") {
        return None;
    }
    if let Some(rest) = specifier.strip_prefix('@') {
        let mut parts = rest.splitn(3, '/');
        let scope = parts.next()?;
        let name = parts.next()?;
        // `@` + scope + `/` + name.
        let package = specifier.get(..scope.len() + name.len() + 2)?;
        let subpath = parts.next().unwrap_or("");
        Some((package, subpath))
    } else {
        let mut parts = specifier.splitn(2, '/');
        let name = parts.next()?;
        if name.is_empty() {
            return None;
        }
        Some((name, parts.next().unwrap_or("")))
    }
}

/// Read the `packages:` globs from `pnpm-workspace.yaml`.
///
/// Hand-rolled, deliberately: in practice the file is a `packages:` key
/// followed by `- 'glob'` lines, and that shape is not worth a YAML
/// dependency in phase 1. Anything fancier (anchors, multi-line globs) is
/// ignored rather than misread.
fn read_pnpm_globs(fs: &dyn FileSystem, root: &str) -> Vec<String> {
    let path = join_root(root, "pnpm-workspace.yaml");
    let Some(text) = fs.read(&path) else {
        return Vec::new();
    };
    let mut globs = Vec::new();
    let mut in_packages = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("packages:") {
            in_packages = true;
            continue;
        }
        if !in_packages {
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(item) = trimmed.strip_prefix("- ") else {
            break;
        };
        let glob = item.trim().trim_matches(|c| c == '\'' || c == '"');
        if !glob.is_empty() {
            globs.push(glob.to_string());
        }
    }
    globs
}

/// Read the `workspaces` globs from the root `package.json`, in both its
/// array and `{ "packages": [...] }` shapes.
fn read_npm_globs(fs: &dyn FileSystem, root: &str) -> Vec<String> {
    let path = join_root(root, "package.json");
    let Some(text) = fs.read(&path) else {
        return Vec::new();
    };
    let Ok(value): Result<serde_json::Value, _> = serde_json::from_str(&text) else {
        return Vec::new();
    };
    let mut globs = Vec::new();
    match value.get("workspaces") {
        Some(serde_json::Value::Array(items)) => collect_strings(items, &mut globs),
        Some(serde_json::Value::Object(map)) => {
            if let Some(serde_json::Value::Array(items)) = map.get("packages") {
                collect_strings(items, &mut globs);
            }
        }
        _ => {}
    }
    globs
}

fn collect_strings(items: &[serde_json::Value], out: &mut Vec<String>) {
    for item in items {
        if let Some(glob) = item.as_str() {
            if !glob.is_empty() {
                out.push(glob.to_string());
            }
        }
    }
}

/// Expand one workspace glob to candidate package directories, root-relative.
/// Only a trailing `/*` is expanded (the shape every monorepo uses); a bare
/// directory names itself; anything fancier is ignored.
fn expand_glob(fs: &dyn FileSystem, root: &str, glob: &str) -> Vec<String> {
    if let Some(prefix) = glob.strip_suffix("/*") {
        let dir = join_root(root, prefix);
        fs.read_dir(&dir)
            .into_iter()
            .filter(|entry| entry.is_dir)
            .map(|entry| {
                if dir.is_empty() {
                    entry.name
                } else {
                    format!("{dir}/{}", entry.name)
                }
            })
            .collect()
    } else if glob.contains('*') {
        Vec::new()
    } else {
        vec![join_root(root, glob)]
    }
}

/// Read one candidate directory's `package.json`, returning its name and
/// package record. Returns `None` when the directory is not a readable
/// package with a probed entry file.
fn read_package(fs: &dyn FileSystem, dir: &str) -> Option<(String, WorkspacePackage)> {
    let manifest = if dir.is_empty() {
        "package.json".to_string()
    } else {
        format!("{dir}/package.json")
    };
    let text = fs.read(&manifest)?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let name = value.get("name")?.as_str()?.to_string();

    let mut candidates = Vec::new();
    if let Some(main) = value.get("main").and_then(|m| m.as_str()) {
        if !main.is_empty() {
            candidates.push(join_root(dir, main));
        }
    }
    // The `main` entry first, then the conventional defaults.
    for fallback in DEFAULT_ENTRIES {
        let path = if dir.is_empty() {
            fallback.to_string()
        } else {
            format!("{dir}/{fallback}")
        };
        candidates.push(path);
    }

    for candidate in candidates {
        if let Some(found) = probe_file(fs, &candidate) {
            let package = WorkspacePackage {
                dir: dir.to_string(),
                entry: found,
            };
            return Some((name, package));
        }
    }
    None
}

fn join_root(root: &str, path: &str) -> String {
    if root.is_empty() {
        path.to_string()
    } else {
        format!("{root}/{path}")
    }
}
