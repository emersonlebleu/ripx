use ripx_core::{analyze_dir, load_manifest};
use std::path::Path;

fn main() {
  // TODO: we will maybe want to allow our tool to walk up the tree looking for our ripx.toml
  let manifest_path = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "ripx.toml".to_string());

  let manifest = match load_manifest(Path::new(&manifest_path)) {
    Ok(m) => m,
    Err(e) => {
      eprintln!("sorry, {e}");
      std::process::exit(1);
    }
  };

  for project in &manifest.projects {
    if project.lang != "rust" {
      println!(
        "skipping {} ({}): not yet supported",
        project.name, project.lang
      );
      continue;
    }

    println!("\n=== project: {} ===", project.name);
    for root in &project.roots {
      analyze_dir(Path::new(root));
    }
  }
}
