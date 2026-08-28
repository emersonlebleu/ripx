//! Command line parsing, kept apart so `main` reads as what the tool does.

use std::path::PathBuf;

pub const USAGE: &str = "usage: ripx [ripx.toml] [--format=text|json]";

pub enum Format {
  Text,
  Json,
}

pub struct Options {
  pub manifest: PathBuf,
  pub format: Format,
}

impl Options {
  pub fn from_args(args: impl Iterator<Item = String>) -> Result<Options, String> {
    // TODO: walk up the tree looking for ripx.toml, the way cargo finds Cargo.toml.
    let mut manifest = PathBuf::from("ripx.toml");
    let mut format = Format::Text;

    for arg in args {
      match arg.as_str() {
        "--format=text" => format = Format::Text,
        "--format=json" => format = Format::Json,
        "-h" | "--help" => return Err(USAGE.to_string()),
        flag if flag.starts_with('-') => return Err(format!("unknown flag {flag}\n{USAGE}")),
        path => manifest = PathBuf::from(path),
      }
    }

    Ok(Options { manifest, format })
  }
}
