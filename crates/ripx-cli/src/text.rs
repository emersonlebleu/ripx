//! The human-readable rendering of a graph.

use ripx_core::{Graph, Node, NodeKind, lang};
use std::fmt::Write;

pub fn render(graph: &Graph) -> String {
  let mut out = String::new();

  for project in graph.projects() {
    let NodeKind::Project { lang } = &project.kind else {
      continue;
    };
    let _ = writeln!(out, "\n=== project: {} ({lang}) ===", project.name);

    for unit in graph.children(&project.id) {
      let _ = writeln!(out, "\n  {} {}", lang::unit_label(lang), unit.name);

      for file in graph.children(&unit.id) {
        render_file(&mut out, graph, file);
      }
    }
  }

  render_summary(&mut out, graph);
  out
}

fn render_file(out: &mut String, graph: &Graph, file: &Node) {
  let _ = writeln!(out, "    {}", file.name);

  for declaration in graph.children(&file.id) {
    if let NodeKind::Decl { decl } = &declaration.kind {
      let _ = writeln!(out, "      {decl} {}", declaration.name);
    }
  }

  for edge in graph.imports_from(&file.id) {
    let target = graph.node(&edge.to);
    let name = target.map_or(edge.to.as_str(), |node| node.name.as_str());
    let origin = match target.map(|node| &node.kind) {
      Some(NodeKind::External) => "external",
      _ => "internal",
    };
    let _ = writeln!(out, "      -> {name} ({origin}, x{})", edge.weight);
  }
}

fn render_summary(out: &mut String, graph: &Graph) {
  let _ = writeln!(
    out,
    "\n{} nodes, {} import edges, {} unresolved",
    graph.nodes().len(),
    graph.edges().len(),
    graph.unresolved().len()
  );

  for unresolved in graph.unresolved() {
    let _ = writeln!(
      out,
      "  unresolved {} in {}: {}",
      unresolved.path, unresolved.file, unresolved.reason
    );
  }
}
