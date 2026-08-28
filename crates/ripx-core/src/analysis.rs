//! Turning a manifest into a graph. This is the only place that knows how the
//! manifest, the languages, the file system and the graph fit together — and
//! it knows nothing about any particular language.

use crate::graph::{Graph, GraphBuilder, NodeId, NodeKind};
use crate::lang::{self, FileFacts, ImportTarget, Language, Scope};
use crate::manifest::{Manifest, Project};
use crate::source;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Analyze the project described by the manifest at `path`.
///
/// The whole job in one call: roots resolve relative to the manifest, so the
/// graph is the same wherever ripx is run from.
pub fn analyze(path: &Path) -> Result<Graph, String> {
  let manifest = Manifest::load(path)?;
  Ok(build(&manifest, base_of(path)))
}

/// Analyze an already-loaded manifest whose roots are relative to `base`.
pub fn build(manifest: &Manifest, base: &Path) -> Graph {
  let mut builder = GraphBuilder::new();

  for project in &manifest.projects {
    let project_id = builder.add(
      NodeId::project(&project.name),
      project.name.clone(),
      None,
      NodeKind::Project {
        lang: project.lang.clone(),
      },
    );

    match lang::by_name(&project.lang) {
      Some(language) => add_project(&mut builder, project, &project_id, base, language),
      None => builder.warn(format!(
        "project {}: {} is not supported yet",
        project.name, project.lang
      )),
    }
  }

  builder.finish()
}

/// The directory the manifest sits in, which every root is relative to.
fn base_of(manifest_path: &Path) -> &Path {
  manifest_path
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
    .unwrap_or(Path::new("."))
}

/// One analysis root, named the way imports refer to it.
struct Unit {
  id: NodeId,
  name: String,
  path: PathBuf,
}

fn add_project(
  builder: &mut GraphBuilder,
  project: &Project,
  project_id: &NodeId,
  base: &Path,
  language: &dyn Language,
) {
  // Name every unit before analyzing any of them, so an import from one unit
  // into a sibling resolves regardless of the order the roots are listed in.
  let units: Vec<Unit> = project
    .roots
    .iter()
    .map(|root| {
      let path = base.join(root);
      let name = language.unit_name(&path);
      Unit {
        id: NodeId::unit(&project.name, &name),
        name,
        path,
      }
    })
    .collect();

  let unit_names: HashSet<String> = units.iter().map(|unit| unit.name.clone()).collect();

  for unit in &units {
    builder.add(
      unit.id.clone(),
      unit.name.clone(),
      Some(project_id),
      NodeKind::Unit {
        path: source::key(base, &unit.path),
      },
    );

    let (files, local_names) = read_unit(builder, unit, base, language);
    let scope = Scope {
      units: &unit_names,
      local: &local_names,
    };

    for (key, facts) in &files {
      add_file(builder, project, unit, &units, language, &scope, key, facts);
    }
  }
}

/// Parse every file in a unit, and collect the names the unit declares.
///
/// This has to finish before any import is resolved: whether `use graph::Node`
/// means a third-party crate or this unit's own `mod graph` depends on what
/// the unit as a whole declares, which is not known until the last file is
/// read.
fn read_unit(
  builder: &mut GraphBuilder,
  unit: &Unit,
  base: &Path,
  language: &dyn Language,
) -> (Vec<(String, FileFacts)>, HashSet<String>) {
  let mut files = Vec::new();
  let mut local_names = HashSet::new();

  for found in source::walk(&unit.path, language.extensions()) {
    match found {
      Ok(file) => {
        let facts = language.facts(&file.text);
        local_names.extend(language.local_names(&facts));
        files.push((source::key(base, &file.path), facts));
      }
      Err(problem) => builder.warn(problem),
    }
  }

  (files, local_names)
}

#[allow(clippy::too_many_arguments)]
fn add_file(
  builder: &mut GraphBuilder,
  project: &Project,
  unit: &Unit,
  units: &[Unit],
  language: &dyn Language,
  scope: &Scope,
  key: &str,
  facts: &FileFacts,
) {
  let file_id = builder.add(
    NodeId::file(&project.name, key),
    key,
    Some(&unit.id),
    NodeKind::File,
  );

  for declaration in &facts.declarations {
    builder.add_declaration(&file_id, declaration);
  }

  for import in &facts.imports {
    let target = match language.resolve(import, scope) {
      ImportTarget::Unresolved(reason) => {
        builder.add_unresolved(key, &import.path, reason);
        continue;
      }
      // Imports never cross projects, so only this project's units match.
      ImportTarget::Unit(name) => match units.iter().find(|unit| unit.name == name) {
        Some(unit) => unit.id.clone(),
        None => builder.add(NodeId::external(&name), name, None, NodeKind::External),
      },
      ImportTarget::External(name) => {
        builder.add(NodeId::external(&name), name, None, NodeKind::External)
      }
    };

    builder.add_import(&file_id, &target);
  }
}
