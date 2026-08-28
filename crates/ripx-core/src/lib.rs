//! ripx — what a codebase contains, and what depends on what.
//!
//! The engine. Every client (CLI, editor extension, future `serve` mode) calls
//! [`analyze`] and renders the [`Graph`] it returns; no client should contain
//! analysis of its own.

mod analysis;
pub mod graph;
pub mod lang;
pub mod manifest;
mod source;

pub use analysis::{analyze, build};
pub use graph::{Edge, EdgeKind, FORMAT_VERSION, Graph, Node, NodeId, NodeKind, Unresolved};
pub use lang::{Declaration, Import, Language};
pub use manifest::{Manifest, Project};
