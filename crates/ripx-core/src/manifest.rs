//! The `ripx.toml` a user points ripx at.

use serde::Deserialize;
use std::path::Path;

/// A project is one analysis tree: one language, one symbol scope. Imports
/// resolve between the `roots` of a project and never across projects, so a
/// Cargo workspace is one project with several roots while a polyglot monorepo
/// is several projects.
#[derive(Debug, Deserialize)]
pub struct Project {
  pub name: String,
  pub lang: String,
  pub roots: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
  pub version: u32,
  pub projects: Vec<Project>,
}

impl Manifest {
  /// Read and parse a manifest. The error is already phrased for a human.
  pub fn load(path: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path)
      .map_err(|e| format!("could not read {}: {e}", path.display()))?;

    toml::from_str(&text).map_err(|e| format!("could not parse {}: {e}", path.display()))
  }
}
