use std::collections::{BTreeMap, VecDeque};

use crate::graph::Graph;

#[derive(Debug, Default)]
pub struct Reached {
    pub depths: BTreeMap<String, u32>,
}

/// Breadth-first over reverse edges. Iterative with a visited set, so an
/// import cycle terminates rather than overflowing the stack — see
/// docs/SPEC.md section 4 and the `cyclic` fixture.
pub fn reverse_reach(graph: &Graph, changed: &[String]) -> Reached {
    let mut depths: BTreeMap<u32, u32> = BTreeMap::new();
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();

    for path in changed {
        // A changed path with no node — a deleted file, a markdown edit — is
        // simply not a seed. It is not an error.
        let Some(id) = graph.id_of(path) else {
            continue;
        };
        if depths.insert(id.0, 0).is_none() {
            queue.push_back((id.0, 0));
        }
    }

    while let Some((id, depth)) = queue.pop_front() {
        let Some(importers) = graph.reverse.get(id as usize) else {
            continue;
        };
        for importer in importers {
            // BFS visits in non-decreasing depth order, so the first time a
            // node is seen is its shortest depth.
            if depths.contains_key(&importer.0) {
                continue;
            }
            depths.insert(importer.0, depth + 1);
            queue.push_back((importer.0, depth + 1));
        }
    }

    Reached {
        depths: depths
            .into_iter()
            .filter_map(|(id, depth)| graph.files.get(id as usize).map(|p| (p.clone(), depth)))
            .collect(),
    }
}
