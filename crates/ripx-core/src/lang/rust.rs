//! Rust: the tree-sitter grammar, `Cargo.toml`, and what `crate::` means.

use super::{Declaration, FileFacts, Import, ImportTarget, Language, Scope};
use serde::Deserialize;
use std::path::Path;
use tree_sitter::{Node, Parser};

pub struct Rust;

impl Language for Rust {
  fn name(&self) -> &'static str {
    "rust"
  }

  fn unit_label(&self) -> &'static str {
    "crate"
  }

  fn extensions(&self) -> &'static [&'static str] {
    &["rs"]
  }

  /// The crate name, which is what `use` statements spell. Read from
  /// `Cargo.toml` when there is one so this is fact rather than guess; the
  /// directory name is the fallback, normalized the way Cargo normalizes it.
  fn unit_name(&self, root: &Path) -> String {
    std::fs::read_to_string(root.join("Cargo.toml"))
      .ok()
      .and_then(|text| toml::from_str::<CargoToml>(&text).ok())
      .and_then(|cargo| cargo.package)
      .map(|package| package.name)
      .unwrap_or_else(|| {
        root
          .file_name()
          .and_then(|name| name.to_str())
          .unwrap_or("unknown")
          .to_string()
      })
      .replace('-', "_")
  }

  fn facts(&self, source: &str) -> FileFacts {
    let Some(tree) = parse(source) else {
      return FileFacts::default();
    };
    let root = tree.root_node();

    FileFacts {
      declarations: declarations(root, source),
      imports: imports(root, source),
    }
  }

  /// Only `mod` declarations can be the head of a `use` path.
  fn local_names(&self, facts: &FileFacts) -> Vec<String> {
    facts
      .declarations
      .iter()
      .filter(|declaration| declaration.kind == "module")
      .map(|declaration| declaration.name.clone())
      .collect()
  }

  fn resolve(&self, import: &Import, scope: &Scope) -> ImportTarget {
    const INTRA_CRATE: &str = "intra-crate path, needs module resolution";

    match head(&import.path) {
      "" => ImportTarget::Unresolved("no leading path segment"),
      // Explicitly inside the importing crate.
      "crate" | "self" | "super" => ImportTarget::Unresolved(INTRA_CRATE),
      name if scope.units.contains(name) => ImportTarget::Unit(name.to_string()),
      // Under uniform paths a bare head is a crate *or* a top-level module of
      // this crate, and a module wins. Naming the target file needs module
      // resolution, so say so rather than inventing an external crate.
      name if scope.local.contains(name) => ImportTarget::Unresolved(INTRA_CRATE),
      name => ImportTarget::External(name.to_string()),
    }
  }
}

#[derive(Deserialize)]
struct CargoToml {
  package: Option<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
  name: String,
}

fn parse(source: &str) -> Option<tree_sitter::Tree> {
  let mut parser = Parser::new();
  parser
    .set_language(&tree_sitter_rust::LANGUAGE.into())
    .expect("tree-sitter-rust grammar is incompatible with the linked tree-sitter");

  parser.parse(source, None)
}

/// NOTE: top level only. Methods inside `impl` blocks and items inside `mod`
/// blocks are not reported yet.
fn declarations(root: Node, source: &str) -> Vec<Declaration> {
  let mut out = Vec::new();
  let mut cursor = root.walk();

  for child in root.children(&mut cursor) {
    let kind = match child.kind() {
      "function_item" => "function",
      "struct_item" => "struct",
      "enum_item" => "enum",
      "mod_item" => "module",
      _ => continue,
    };

    if let Some(name) = child.child_by_field_name("name") {
      out.push(Declaration {
        kind: kind.to_string(),
        name: source[name.byte_range()].to_string(),
      });
    }
  }

  out
}

/// Every `use` in the file, including ones nested inside `mod` blocks.
fn imports(root: Node, source: &str) -> Vec<Import> {
  let mut out = Vec::new();
  collect_imports(root, source, &mut out);
  out
}

fn collect_imports(node: Node, source: &str, out: &mut Vec<Import>) {
  if node.kind() == "use_declaration" {
    if let Some(argument) = node.child_by_field_name("argument") {
      let mut items = Vec::new();
      collect_bindings(argument, source, &mut items);
      out.push(Import {
        path: source[argument.byte_range()].to_string(),
        items,
      });
    }
    return;
  }

  let mut cursor = node.walk();
  for child in node.children(&mut cursor) {
    collect_imports(child, source, out);
  }
}

/// First segment of a use path. `""` when there is none, as in `use {a, b};`.
fn head(path: &str) -> &str {
  path
    .trim()
    .trim_start_matches("::")
    .split("::")
    .next()
    .unwrap_or("")
    // `use foo as bar;` has no `::` to split on.
    .split_whitespace()
    .next()
    .unwrap_or("")
}

/// The names a use statement binds. `a::b::{c, d as e}` binds `c` and `e`.
fn collect_bindings(node: Node, source: &str, out: &mut Vec<String>) {
  match node.kind() {
    "identifier" | "type_identifier" | "crate" | "self" | "super" => {
      out.push(source[node.byte_range()].to_string());
    }
    "scoped_identifier" => {
      if let Some(name) = node.child_by_field_name("name") {
        out.push(source[name.byte_range()].to_string());
      }
    }
    "use_as_clause" => {
      if let Some(alias) = node.child_by_field_name("alias") {
        out.push(source[alias.byte_range()].to_string());
      }
    }
    "use_wildcard" => out.push("*".to_string()),
    // Only the list side of `a::b::{..}` binds names; the path side does not.
    "scoped_use_list" => {
      if let Some(list) = node.child_by_field_name("list") {
        collect_bindings(list, source, out);
      }
    }
    "use_list" => {
      let mut cursor = node.walk();
      for child in node.named_children(&mut cursor) {
        collect_bindings(child, source, out);
      }
    }
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn facts(source: &str) -> FileFacts {
    Rust.facts(source)
  }

  /// Resolve every import in `source`, with `units` as the project's units.
  fn resolved(source: &str, units: &[&str]) -> Vec<String> {
    let facts = facts(source);
    let units = units.iter().map(|u| u.to_string()).collect();
    let local = Rust.local_names(&facts).into_iter().collect();
    let scope = Scope {
      units: &units,
      local: &local,
    };

    facts
      .imports
      .iter()
      .map(|import| match Rust.resolve(import, &scope) {
        ImportTarget::Unit(name) => format!("unit: {name}"),
        ImportTarget::External(name) => format!("external: {name}"),
        ImportTarget::Unresolved(reason) => format!("unresolved: {reason}"),
      })
      .collect()
  }

  #[test]
  fn finds_top_level_declarations() {
    let facts = facts("struct S; enum E {} fn f() {} mod m {} const C: u8 = 0;");
    let found: Vec<_> = facts
      .declarations
      .iter()
      .map(|d| (d.kind.as_str(), d.name.as_str()))
      .collect();

    assert_eq!(
      found,
      [
        ("struct", "S"),
        ("enum", "E"),
        ("function", "f"),
        ("module", "m"),
      ]
    );
  }

  #[test]
  fn binds_names_through_lists_aliases_and_wildcards() {
    let facts = facts(
      "use std::collections::{HashMap, BTreeMap as Sorted};
       use serde::*;
       use ::regex::Regex;
       pub use ripx_core::Graph;",
    );
    let bound: Vec<_> = facts.imports.iter().map(|i| i.items.clone()).collect();

    assert_eq!(
      bound,
      [
        vec!["HashMap", "Sorted"],
        vec!["*"],
        vec!["Regex"],
        vec!["Graph"],
      ]
    );
  }

  #[test]
  fn sees_imports_nested_in_modules() {
    let facts = facts("mod inner { use std::io::Write; }");
    assert_eq!(facts.imports.len(), 1);
    assert_eq!(facts.imports[0].path, "std::io::Write");
  }

  #[test]
  fn separates_sibling_units_from_third_parties() {
    assert_eq!(
      resolved("use ripx_core::Graph; use ::regex::Regex;", &["ripx_core"]),
      ["unit: ripx_core", "external: regex"]
    );
  }

  #[test]
  fn refuses_to_guess_intra_crate_paths() {
    assert_eq!(
      resolved(
        "use crate::helpers::thing;
         use self::helpers::other;
         use super::sibling;",
        &[]
      ),
      ["unresolved: intra-crate path, needs module resolution"; 3]
    );
  }

  #[test]
  fn a_local_module_is_not_mistaken_for_an_external_crate() {
    // `mod graph;` in the same unit means `use graph::Node` is local, even
    // though the head looks exactly like a crate name.
    assert_eq!(
      resolved("mod graph; use graph::Node; use serde::Serialize;", &[]),
      [
        "unresolved: intra-crate path, needs module resolution",
        "external: serde",
      ]
    );
  }

  #[test]
  fn unparsable_source_yields_what_it_can_rather_than_panicking() {
    let facts = facts("fn ok() {} fn ((( ");
    assert_eq!(facts.declarations[0].name, "ok");
  }
}
