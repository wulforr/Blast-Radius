use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::fs::FileSystem;
use crate::resolve::relative::probe_file;
use crate::resolve::Resolved;

/// Maximum `extends` hops followed before giving up. Real chains are one or
/// two deep; anything beyond this is a cycle or a mistake, and both must
/// terminate rather than hang someone's CI.
const MAX_EXTENDS_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsConfig {
    pub base_url: Option<String>,
    pub paths: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
struct RawConfig {
    #[serde(default, deserialize_with = "string_or_vec")]
    extends: Vec<String>,
    #[serde(default, rename = "compilerOptions")]
    compiler_options: RawOptions,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawOptions {
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    paths: Option<BTreeMap<String, Vec<String>>>,
}

/// `extends` accepts a single path or an array of paths in real configs.
fn string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> serde::de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a string or an array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Vec<String>, E>
        where
            E: serde::de::Error,
        {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Vec<String>, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut out = Vec::new();
            while let Some(item) = seq.next_element()? {
                out.push(item);
            }
            Ok(out)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

/// Load `tsconfig.json` (falling back to `jsconfig.json`) at `root` and follow
/// its `extends` chain. Returns `None` when no config file exists or the root
/// file does not parse — an unreadable ancestor further down the chain is
/// skipped rather than fatal, so one bad link cannot veto the whole file.
pub fn load_tsconfig(fs: &dyn FileSystem, root: &str) -> Option<TsConfig> {
    let candidate = config_path(root, "tsconfig.json");
    let candidate = if fs.read(&candidate).is_some() {
        candidate
    } else {
        let fallback = config_path(root, "jsconfig.json");
        if fs.read(&fallback).is_some() {
            fallback
        } else {
            return None;
        }
    };

    let mut chain = Vec::new();
    collect_chain(fs, &candidate, &mut BTreeSet::new(), &mut chain, 0);
    if chain.is_empty() {
        // The root file exists but does not parse. Report no config rather
        // than a half-empty one.
        return None;
    }

    // Parents were pushed before their children, so a later entry overriding
    // an earlier one is exactly tsc's shallow merge of `compilerOptions`: a
    // child that defines `paths` replaces the parent's entirely.
    let mut base_url = None;
    let mut paths = None;
    for raw in &chain {
        if raw.compiler_options.base_url.is_some() {
            base_url = raw.compiler_options.base_url.clone();
        }
        if raw.compiler_options.paths.is_some() {
            paths = raw.compiler_options.paths.clone();
        }
    }

    Some(TsConfig {
        base_url,
        paths: paths.unwrap_or_default(),
    })
}

fn collect_chain(
    fs: &dyn FileSystem,
    path: &str,
    visited: &mut BTreeSet<String>,
    out: &mut Vec<RawConfig>,
    depth: usize,
) {
    if depth > MAX_EXTENDS_DEPTH || !visited.insert(path.to_string()) {
        return;
    }
    let Some(text) = fs.read(path) else { return };
    let Ok(raw): Result<RawConfig, _> = serde_json::from_str(&strip_jsonc(&text)) else {
        return;
    };
    let dir = dir_of(path);
    for parent in &raw.extends {
        collect_chain(fs, &join_under(&dir, parent), visited, out, depth + 1);
    }
    out.push(raw);
}

/// Rung two of the ladder in spec §4: `compilerOptions.paths` mappings.
///
/// Longest matching pattern wins, matching tsc. Returns `None` when no
/// pattern matches ("not my rung"); a pattern that matches but whose targets
/// are all missing is `Unresolved` ("mine, and it is broken").
pub fn resolve_paths(
    fs: &dyn FileSystem,
    config: &TsConfig,
    root: &str,
    specifier: &str,
) -> Option<Resolved> {
    let mut best: Option<(&Vec<String>, String, usize)> = None;
    for (pattern, targets) in &config.paths {
        if let Some((captured, prefix_len)) = match_pattern(pattern, specifier) {
            let wins = match &best {
                Some((_, _, best_len)) => prefix_len > *best_len,
                None => true,
            };
            if wins {
                best = Some((targets, captured, prefix_len));
            }
        }
    }
    let (targets, captured, _) = best?;

    // Targets resolve against `baseUrl` when set, else against the root.
    let anchor = match &config.base_url {
        Some(base) => join_under(root, base),
        None => root.to_string(),
    };
    for target in targets {
        let substituted = target.replace('*', captured.as_str());
        let candidate = if anchor.is_empty() {
            collapse_path(&substituted)
        } else {
            join_under(&anchor, &substituted)
        };
        if let Some(found) = probe_file(fs, &candidate) {
            return Some(Resolved::File(found));
        }
    }

    Some(Resolved::Unresolved)
}

/// Match a specifier against one paths pattern. Returns the captured `*`
/// text and the length of the matched prefix, which is what longest-wins
/// compares. Patterns carry at most one `*`, like tsc.
fn match_pattern(pattern: &str, specifier: &str) -> Option<(String, usize)> {
    match pattern.find('*') {
        None => {
            if pattern == specifier {
                Some((String::new(), pattern.len()))
            } else {
                None
            }
        }
        Some(star) => {
            let prefix = pattern.get(..star)?;
            let suffix = pattern.get(star + 1..)?;
            let rest = specifier.strip_prefix(prefix)?;
            if suffix.is_empty() {
                Some((rest.to_string(), prefix.len()))
            } else {
                let captured = rest.strip_suffix(suffix)?;
                Some((captured.to_string(), prefix.len()))
            }
        }
    }
}

fn config_path(root: &str, name: &str) -> String {
    if root.is_empty() {
        name.to_string()
    } else {
        format!("{root}/{name}")
    }
}

fn dir_of(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

/// Join a directory with a relative path, collapsing `.` and `..` without
/// touching the real filesystem, so this works on `MemFs`.
fn join_under(dir: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for segment in rel.split('/') {
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

/// Collapse `.` and `..` segments in an already-joined path.
fn collapse_path(path: &str) -> String {
    join_under("", path)
}

/// Strip `//` and `/* */` comments, then trailing commas, from JSONC text.
/// Both passes are string-aware with escape handling, so a `//` inside a
/// string value survives.
fn strip_jsonc(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    // 0 = normal, 1 = in string, 2 = line comment, 3 = block comment
    let mut state = 0;
    while i < chars.len() {
        let Some(&c) = chars.get(i) else { break };
        match state {
            0 => {
                if c == '"' {
                    state = 1;
                    out.push(c);
                } else if c == '/' {
                    match chars.get(i + 1) {
                        Some('/') => state = 2,
                        Some('*') => {
                            state = 3;
                            i += 1;
                        }
                        _ => out.push(c),
                    }
                } else {
                    out.push(c);
                }
            }
            1 => {
                out.push(c);
                if c == '\\' {
                    if let Some(&next) = chars.get(i + 1) {
                        out.push(next);
                        i += 1;
                    }
                } else if c == '"' {
                    state = 0;
                }
            }
            2 => {
                if c == '\n' {
                    state = 0;
                    out.push(c);
                }
            }
            _ => {
                if c == '*' && chars.get(i + 1) == Some(&'/') {
                    state = 0;
                    i += 1;
                }
            }
        }
        i += 1;
    }
    strip_trailing_commas(&out)
}

fn strip_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let Some(&c) = chars.get(i) else { break };
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(&next) = chars.get(i + 1) {
                    out.push(next);
                    i += 1;
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ',' {
            let mut j = i + 1;
            while let Some(&next) = chars.get(j) {
                if next == ' ' || next == '\t' || next == '\n' || next == '\r' {
                    j += 1;
                } else {
                    break;
                }
            }
            match chars.get(j) {
                Some('}' | ']') => {}
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}
