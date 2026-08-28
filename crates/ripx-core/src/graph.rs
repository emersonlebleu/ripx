//! The graph ripx produces: what exists, and what depends on what.
//!
//! Containment is stored once, as each node's `parent`, and never as edges —
//! so the tree cannot contradict itself and `edges` holds only meaning.
//! [`GraphBuilder`] is the only way to construct one, which is what keeps the
//! parent links, the id index, and the edge weights in agreement.

use crate::lang::Declaration;
use serde::Serialize;
use std::collections::HashMap;

/// Bumped when the shape of the emitted JSON changes. Clients check this.
pub const FORMAT_VERSION: u32 = 1;

/// A node's identity. Opaque on purpose: the string layout is decided here and
/// nowhere else, and it is built from manifest-relative paths so it stays the
/// same no matter which directory ripx runs from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct NodeId(String);

impl NodeId {
  pub fn project(project: &str) -> Self {
    NodeId(format!("project:{project}"))
  }

  pub fn unit(project: &str, unit: &str) -> Self {
    NodeId(format!("unit:{project}:{unit}"))
  }

  pub fn file(project: &str, path: &str) -> Self {
    NodeId(format!("file:{project}:{path}"))
  }

  pub fn decl(file: &NodeId, name: &str) -> Self {
    NodeId(format!("decl:{}:{name}", file.0))
  }

  /// External units are not scoped to a project: `std` is `std` everywhere.
  pub fn external(name: &str) -> Self {
    NodeId(format!("external:{name}"))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl std::fmt::Display for NodeId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

/// What a node is, carrying whatever else is true of that kind. Serializes
/// flat: `{"kind": "decl", "decl": "function", ...}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NodeKind {
  Project {
    lang: String,
  },
  /// One analysis root: a crate, a package.
  Unit {
    path: String,
  },
  File,
  Decl {
    decl: String,
  },
  /// A unit imported but not analyzed, such as `std`.
  External,
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
  pub id: NodeId,
  pub name: String,
  /// Structural parent. `None` for projects and externals.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<NodeId>,
  #[serde(flatten)]
  pub kind: NodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
  /// A file imports a unit. Reference and call edges join this once there is a
  /// symbol table.
  Import,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edge {
  pub from: NodeId,
  pub to: NodeId,
  pub kind: EdgeKind,
  /// How many statements this edge collapses.
  pub weight: u32,
}

/// An import deliberately left as no edge, rather than guessed at.
#[derive(Debug, Clone, Serialize)]
pub struct Unresolved {
  pub file: String,
  pub path: String,
  pub reason: String,
}

/// A finished graph. Read-only: the fields are private so the index and the
/// parent links cannot drift out of step with the nodes.
#[derive(Debug, Serialize)]
pub struct Graph {
  version: u32,
  nodes: Vec<Node>,
  edges: Vec<Edge>,
  unresolved: Vec<Unresolved>,
  /// Things that went wrong while reading the tree, phrased for a human. The
  /// caller decides whether and where to show them.
  warnings: Vec<String>,

  #[serde(skip)]
  index: HashMap<NodeId, usize>,
  #[serde(skip)]
  children: HashMap<NodeId, Vec<usize>>,
}

impl Graph {
  pub fn nodes(&self) -> &[Node] {
    &self.nodes
  }

  pub fn edges(&self) -> &[Edge] {
    &self.edges
  }

  pub fn unresolved(&self) -> &[Unresolved] {
    &self.unresolved
  }

  pub fn warnings(&self) -> &[String] {
    &self.warnings
  }

  pub fn node(&self, id: &NodeId) -> Option<&Node> {
    self.index.get(id).map(|&i| &self.nodes[i])
  }

  /// Direct children, in insertion order.
  pub fn children(&self, id: &NodeId) -> impl Iterator<Item = &Node> {
    self
      .children
      .get(id)
      .map_or(&[][..], |kids| kids.as_slice())
      .iter()
      .map(|&i| &self.nodes[i])
  }

  pub fn projects(&self) -> impl Iterator<Item = &Node> {
    self
      .nodes
      .iter()
      .filter(|node| matches!(node.kind, NodeKind::Project { .. }))
  }

  pub fn imports_from(&self, id: &NodeId) -> impl Iterator<Item = &Edge> {
    self
      .edges
      .iter()
      .filter(move |edge| edge.kind == EdgeKind::Import && &edge.from == id)
  }

  pub fn to_json(&self) -> Result<String, String> {
    serde_json::to_string_pretty(self).map_err(|e| format!("could not serialize graph: {e}"))
  }
}

/// Assembles a [`Graph`], keeping its invariants so callers cannot break them.
pub struct GraphBuilder {
  graph: Graph,
  /// Where each (from, to) import edge lives, so repeats bump a weight.
  import_edges: HashMap<(NodeId, NodeId), usize>,
}

impl Default for GraphBuilder {
  fn default() -> Self {
    GraphBuilder::new()
  }
}

impl GraphBuilder {
  pub fn new() -> Self {
    GraphBuilder {
      graph: Graph {
        version: FORMAT_VERSION,
        nodes: Vec::new(),
        edges: Vec::new(),
        unresolved: Vec::new(),
        warnings: Vec::new(),
        index: HashMap::new(),
        children: HashMap::new(),
      },
      import_edges: HashMap::new(),
    }
  }

  /// Add a node and link it under its parent. Adding the same id twice is a
  /// no-op, so callers may add shared nodes (an external crate, say) whenever
  /// they need one without checking first.
  pub fn add(
    &mut self,
    id: NodeId,
    name: impl Into<String>,
    parent: Option<&NodeId>,
    kind: NodeKind,
  ) -> NodeId {
    if self.graph.index.contains_key(&id) {
      return id;
    }

    let position = self.graph.nodes.len();
    self.graph.index.insert(id.clone(), position);
    if let Some(parent) = parent {
      self
        .graph
        .children
        .entry(parent.clone())
        .or_default()
        .push(position);
    }
    self.graph.nodes.push(Node {
      id: id.clone(),
      name: name.into(),
      parent: parent.cloned(),
      kind,
    });

    id
  }

  /// Record a declaration as a child of the file that defines it.
  pub fn add_declaration(&mut self, file: &NodeId, declaration: &Declaration) {
    self.add(
      NodeId::decl(file, &declaration.name),
      declaration.name.clone(),
      Some(file),
      NodeKind::Decl {
        decl: declaration.kind.clone(),
      },
    );
  }

  /// Record one import. Repeats of the same pair collapse into a weight.
  pub fn add_import(&mut self, from: &NodeId, to: &NodeId) {
    match self.import_edges.get(&(from.clone(), to.clone())) {
      Some(&position) => self.graph.edges[position].weight += 1,
      None => {
        self
          .import_edges
          .insert((from.clone(), to.clone()), self.graph.edges.len());
        self.graph.edges.push(Edge {
          from: from.clone(),
          to: to.clone(),
          kind: EdgeKind::Import,
          weight: 1,
        });
      }
    }
  }

  pub fn add_unresolved(&mut self, file: &str, path: &str, reason: &str) {
    self.graph.unresolved.push(Unresolved {
      file: file.to_string(),
      path: path.to_string(),
      reason: reason.to_string(),
    });
  }

  pub fn warn(&mut self, message: impl Into<String>) {
    self.graph.warnings.push(message.into());
  }

  pub fn finish(self) -> Graph {
    self.graph
  }
}
