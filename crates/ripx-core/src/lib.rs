use ignore::WalkBuilder;
use serde::Deserialize;
use std::path::Path;
use tree_sitter::Parser;

#[derive(Debug, Deserialize)]
pub struct Manifest {
  pub version: u32,
  pub projects: Vec<Project>,
}

#[derive(Debug, Deserialize)]
pub struct Project {
  pub name: String,
  pub lang: String,
  pub roots: Vec<String>,
}

pub fn load_manifest(path: &Path) -> Result<Manifest, String> {
  let text =
    std::fs::read_to_string(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;

  // Implicit return notice no ; at the end
  toml::from_str(&text).map_err(|e| format!("could not parse {}: {e}", path.display()))
}

pub struct Declaration {
  pub kind: String,
  pub name: String,
}

pub fn declarations(source: &str) -> Vec<Declaration> {
  let mut parser = Parser::new();
  parser
    .set_language(&tree_sitter_rust::LANGUAGE.into())
    .expect("Error loading Rust grammar");
  let tree = parser.parse(source, None).unwrap();

  let root = tree.root_node();
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
    if let Some(name_node) = child.child_by_field_name("name") {
      let name = source[name_node.byte_range()].to_string();
      out.push(Declaration {
        kind: kind.to_string(),
        name,
      });
    }
  }
  out
}

pub fn analyze_dir(root: &Path) {
  for result in WalkBuilder::new(root).build() {
    let entry = match result {
      Ok(e) => e,
      Err(e) => {
        eprintln!("skip: {e}");
        continue;
      }
    };
    let path = entry.path();

    if path.extension().and_then(|s| s.to_str()) != Some("rs") {
      continue;
    }

    let source = match std::fs::read_to_string(path) {
      Ok(s) => s,
      Err(e) => {
        eprintln!("skipping {}: {e}", path.display());
        continue;
      }
    };

    let decls = declarations(&source);
    if !decls.is_empty() {
      println!("\n{}", path.display());
      for d in decls {
        println!("  {}: {}", d.kind, d.name);
      }
    }
  }
}
