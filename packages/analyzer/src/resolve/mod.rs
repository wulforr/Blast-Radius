pub mod relative;
pub mod tsconfig;
pub mod workspace;

use crate::fs::FileSystem;
use crate::resolve::tsconfig::TsConfig;
use crate::resolve::workspace::Workspace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    File(String),
    /// A package, a node: builtin, or anything outside the analysis root.
    /// Recorded as a leaf, never traversed.
    External,
    /// This rung owns the specifier and could not find a target.
    Unresolved,
}

pub const EXTENSIONS: [&str; 6] = ["ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// The whole ladder from spec §4, in order: relative → tsconfig paths →
/// workspace packages → external. This is the only entry point the graph
/// builder uses.
pub struct Resolver<'f> {
    fs: &'f dyn FileSystem,
    root: String,
    tsconfig: Option<TsConfig>,
    workspace: Workspace,
}

impl<'f> Resolver<'f> {
    pub fn new(fs: &'f dyn FileSystem, root: &str) -> Self {
        let tsconfig = tsconfig::load_tsconfig(fs, root);
        let workspace = workspace::load_workspace(fs, root);
        Self {
            fs,
            root: root.to_string(),
            tsconfig,
            workspace,
        }
    }

    pub fn resolve(&self, importer: &str, specifier: &str) -> Resolved {
        if workspace::is_builtin(specifier) {
            return Resolved::External;
        }
        if let Some(found) = relative::resolve_relative(self.fs, importer, specifier) {
            return found;
        }
        if let Some(config) = &self.tsconfig {
            if let Some(found) = tsconfig::resolve_paths(self.fs, config, &self.root, specifier) {
                return found;
            }
        }
        if let Some(found) = workspace::resolve_workspace(self.fs, &self.workspace, specifier) {
            return found;
        }
        Resolved::External
    }
}
