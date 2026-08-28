//! The one seam where language-specific knowledge lives.
//!
//! Everything a language knows — its file extensions, its grammar, how a
//! directory gets the name that imports refer to it by, which import paths
//! point outside the file — sits behind [`Language`]. The graph builder never
//! mentions Rust, so adding TypeScript means adding a file here and nothing
//! else.

mod rust;

use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// A top-level thing a file defines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Declaration {
  /// Language-specific: "function", "struct", "interface", ...
  pub kind: String,
  pub name: String,
}

/// One import statement, as written. No resolution has happened: this is text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Import {
  /// What was imported, verbatim: `ripx_core::{Graph, analyze}`.
  pub path: String,
  /// Names bound into the file's scope: `["Graph", "analyze"]`. Not used for
  /// import edges, which are unit-level; this is the raw material for the
  /// reference edges that come once there is a symbol table.
  pub items: Vec<String>,
}

/// What one file says about itself, before anything is cross-referenced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFacts {
  pub declarations: Vec<Declaration>,
  pub imports: Vec<Import>,
}

/// What names an import could be referring to. An import head means different
/// things depending on what surrounds it: `use graph::Node` is a third-party
/// crate in one unit and a local module in another.
pub struct Scope<'a> {
  /// Names of the units in the importing unit's project.
  pub units: &'a HashSet<String>,
  /// Names declared inside the importing unit, from [`Language::local_names`].
  pub local: &'a HashSet<String>,
}

/// What an import points at, once the language has weighed it against scope.
pub enum ImportTarget {
  /// A unit of this project, by name.
  Unit(String),
  /// A unit outside the analysis, such as `std`.
  External(String),
  /// Cannot be tied to a unit. Carries a reason so the graph can report it
  /// instead of guessing.
  Unresolved(&'static str),
}

pub trait Language: Sync {
  /// Matches `lang` in `ripx.toml`.
  fn name(&self) -> &'static str;

  /// What this language calls one analysis root: "crate", "package".
  fn unit_label(&self) -> &'static str;

  /// File extensions, without the dot.
  fn extensions(&self) -> &'static [&'static str];

  /// The name imports in sibling units use to refer to the unit at `root`.
  fn unit_name(&self, root: &Path) -> String;

  /// Extract declarations and imports from one file. Must not fail: a file
  /// that will not parse yields whatever was recoverable.
  fn facts(&self, source: &str) -> FileFacts;

  /// Names a unit declares that an import head could mean instead of another
  /// unit — Rust's top-level `mod` declarations. Collected across a unit's
  /// files and handed back as [`Scope::local`].
  fn local_names(&self, facts: &FileFacts) -> Vec<String>;

  /// Classify an import against what is in scope. Anything needing a symbol
  /// table must come back `Unresolved` rather than being approximated.
  fn resolve(&self, import: &Import, scope: &Scope) -> ImportTarget;
}

static RUST: rust::Rust = rust::Rust;

/// The language registered under `lang` in the manifest, if any.
pub fn by_name(name: &str) -> Option<&'static dyn Language> {
  match name {
    "rust" => Some(&RUST),
    _ => None,
  }
}

/// What a language calls one analysis root, even when ripx cannot analyze it.
pub fn unit_label(lang: &str) -> &'static str {
  by_name(lang).map_or("unit", |l| l.unit_label())
}
