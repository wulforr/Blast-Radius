use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::discover::discover;
use crate::fs::FileSystem;
use crate::parse::parse_imports;
use crate::resolve::{Resolved, Resolver};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicGap {
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedRef {
    pub file: String,
    pub specifier: String,
    pub line: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub files: Vec<String>,
    pub forward: Vec<BTreeSet<FileId>>,
    pub reverse: Vec<BTreeSet<FileId>>,
    pub unresolved: Vec<UnresolvedRef>,
    pub dynamic_gaps: Vec<DynamicGap>,
    pub parse_failures: Vec<String>,
}

impl Graph {
    pub fn id_of(&self, path: &str) -> Option<FileId> {
        self.files
            .iter()
            .position(|file| file == path)
            .map(|index| FileId(index as u32))
    }
}

/// Build the import graph for the tree under `root`.
///
/// Two passes. First `discover` assigns a `FileId` to every source file, so
/// the id space is stable before any edge exists. Then each file is parsed
/// and every import is resolved through the ladder: a `File` becomes an edge
/// in both directions, `Unresolved` is recorded, `External` is dropped, and a
/// computed dynamic import becomes a gap rather than a guess.
pub fn build_graph(fs: &dyn FileSystem, root: &str) -> Graph {
    let files = discover(fs, root);
    let ids: BTreeMap<String, FileId> = files
        .iter()
        .enumerate()
        .map(|(index, path)| (path.clone(), FileId(index as u32)))
        .collect();

    let mut graph = Graph {
        files,
        forward: Vec::new(),
        reverse: Vec::new(),
        unresolved: Vec::new(),
        dynamic_gaps: Vec::new(),
        parse_failures: Vec::new(),
    };
    graph.forward.resize_with(graph.files.len(), BTreeSet::new);
    graph.reverse.resize_with(graph.files.len(), BTreeSet::new);

    let resolver = Resolver::new(fs, root);

    for index in 0..graph.files.len() {
        let Some(file) = graph.files.get(index).cloned() else {
            continue;
        };
        let Some(text) = fs.read(&file) else { continue };
        let parsed = parse_imports(&text, &file);
        if parsed.parse_failed {
            graph.parse_failures.push(file.clone());
        }
        let Some(from) = ids.get(file.as_str()).copied() else {
            continue;
        };
        for import in &parsed.imports {
            // Only a computed dynamic import has no specifier; it is a gap,
            // not an error — pretending it does not exist is how a blast
            // radius becomes quietly wrong.
            let Some(specifier) = &import.specifier else {
                graph.dynamic_gaps.push(DynamicGap {
                    file: file.clone(),
                    line: import.line,
                });
                continue;
            };
            match resolver.resolve(&file, specifier) {
                Resolved::File(target) => {
                    // The target may resolve to a file outside the discovered
                    // set (a non-source extension, for example). There is no
                    // node for it, so there is no edge to record.
                    if let Some(to) = ids.get(target.as_str()).copied() {
                        insert_edge(&mut graph, from, to);
                    }
                }
                Resolved::Unresolved => graph.unresolved.push(UnresolvedRef {
                    file: file.clone(),
                    specifier: specifier.clone(),
                    line: import.line,
                }),
                Resolved::External => {}
            }
        }
    }

    graph
}

fn insert_edge(graph: &mut Graph, from: FileId, to: FileId) {
    if let Some(edges) = graph.forward.get_mut(from.0 as usize) {
        edges.insert(to);
    }
    if let Some(importers) = graph.reverse.get_mut(to.0 as usize) {
        importers.insert(from);
    }
}
