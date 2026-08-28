//! Finding and reading source files. Hides the traversal library, the
//! gitignore rules, and the difference between "path on this machine" and
//! "stable identity in the graph".

use ignore::WalkBuilder;
use std::path::{Component, Path, PathBuf};

pub struct SourceFile {
  /// Path as found on disk.
  pub path: PathBuf,
  pub text: String,
}

/// Every readable file under `root` with one of `extensions`, skipping
/// anything gitignored (so `target/`, `node_modules/` and friends cost
/// nothing). Unreadable entries surface as `Err` rather than being dropped
/// silently or printed by the library.
pub fn walk<'a>(
  root: &Path,
  extensions: &'a [&'a str],
) -> impl Iterator<Item = Result<SourceFile, String>> + 'a {
  WalkBuilder::new(root).build().filter_map(move |entry| {
    let entry = match entry {
      Ok(e) => e,
      Err(e) => return Some(Err(format!("could not walk: {e}"))),
    };

    let path = entry.path();
    let ext = path.extension().and_then(|s| s.to_str())?;
    if !extensions.contains(&ext) {
      return None;
    }

    Some(
      std::fs::read_to_string(path)
        .map(|text| SourceFile {
          path: path.to_path_buf(),
          text,
        })
        .map_err(|e| format!("could not read {}: {e}", path.display())),
    )
  })
}

/// A path's stable identity: relative to `base`, with `.` segments dropped and
/// `/` separators on every platform. Graph ids are built from this, so they do
/// not change when ripx is invoked from a different directory.
pub fn key(base: &Path, path: &Path) -> String {
  let relative = path.strip_prefix(base).unwrap_or(path);
  let parts: Vec<_> = relative
    .components()
    .filter(|c| !matches!(c, Component::CurDir))
    .map(|c| c.as_os_str().to_string_lossy().into_owned())
    .collect();

  if parts.is_empty() {
    ".".to_string()
  } else {
    parts.join("/")
  }
}
