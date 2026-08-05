use crate::fs::FileSystem;
use crate::resolve::{Resolved, EXTENSIONS};

/// JavaScript-output extensions that TypeScript accepts as aliases for its own
/// sources. A specifier ending in one of these is tried literally first, then
/// with the suffix stripped, so `./round.js` finds `round.ts` — but a genuine
/// `.js` file still wins because the exact path is tried before stripping.
const TS_OUTPUT_EXTENSIONS: [&str; 4] = ["js", "jsx", "mjs", "cjs"];

/// Rung one of the ladder in spec §4: relative specifiers.
///
/// Returns `None` when the specifier is not relative — that means "not my
/// rung, try the next one" — and `Some` either way once ownership is taken:
/// a found file, or `Unresolved` when the target is missing.
pub fn resolve_relative(fs: &dyn FileSystem, importer: &str, specifier: &str) -> Option<Resolved> {
    if !is_relative(specifier) {
        return None;
    }

    let base = join_importer_dir(importer, specifier);
    let stem = strip_ts_output_extension(&base);

    if is_file(fs, &base) {
        return Some(Resolved::File(base));
    }
    for candidate in extension_candidates(&base) {
        if is_file(fs, &candidate) {
            return Some(Resolved::File(candidate));
        }
    }
    // The stripped stem only differs when the specifier named a js-family
    // extension; probing its index covers `./dir.js` pointing at a directory.
    if stem != base {
        for candidate in extension_candidates(&stem) {
            if is_file(fs, &candidate) {
                return Some(Resolved::File(candidate));
            }
        }
    }
    for candidate in index_candidates(&base) {
        if is_file(fs, &candidate) {
            return Some(Resolved::File(candidate));
        }
    }

    Some(Resolved::Unresolved)
}

fn is_relative(specifier: &str) -> bool {
    specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier == "."
        || specifier == ".."
}

/// `base`, then `base` + each extension in ladder order, then
/// `base/index` + each extension.
fn extension_candidates(base: &str) -> Vec<String> {
    EXTENSIONS
        .iter()
        .map(|ext| format!("{base}.{ext}"))
        .collect()
}

fn index_candidates(base: &str) -> Vec<String> {
    EXTENSIONS
        .iter()
        .map(|ext| format!("{base}/index.{ext}"))
        .collect()
}

fn strip_ts_output_extension(base: &str) -> String {
    match base.rsplit_once('.') {
        Some((stem, ext)) if TS_OUTPUT_EXTENSIONS.contains(&ext) => stem.to_string(),
        _ => base.to_string(),
    }
}

/// The `FileSystem` trait has no `is_file`, so readability through `read` is
/// the file check: directories are not readable as files on either backend.
fn is_file(fs: &dyn FileSystem, path: &str) -> bool {
    fs.read(path).is_some()
}

/// Join the importer's directory with the specifier and collapse `.` and `..`
/// segments without touching the real filesystem, so this works on `MemFs`.
fn join_importer_dir(importer: &str, specifier: &str) -> String {
    let mut parts: Vec<&str> = match importer.rsplit_once('/') {
        Some((dir, _)) => dir.split('/').filter(|s| !s.is_empty()).collect(),
        None => Vec::new(),
    };

    for segment in specifier.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(segment),
        }
    }

    parts.join("/")
}
