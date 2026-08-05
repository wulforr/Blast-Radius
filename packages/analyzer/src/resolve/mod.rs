pub mod relative;

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
