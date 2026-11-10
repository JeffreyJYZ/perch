//! Path confinement: every tool resolves its target against one root and refuses escapes.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Resolves `candidate` against `root` and refuses anything whose real path leaves it.
///
/// Both sides are canonicalised, so a symlink pointing outside is caught as surely as `../`.
/// A target that does not exist yet is resolved through its deepest existing ancestor, which
/// is what lets `write` create a new file without letting it create one outside the root.
pub fn confine(root: &Path, candidate: &Path) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let resolved = resolve(&joined)?;
    if resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(Error::OutsideRoot {
            path: candidate.to_path_buf(),
        })
    }
}

/// Canonicalises the deepest ancestor of `path` that exists, then re-appends the missing tail.
fn resolve(path: &Path) -> Result<PathBuf> {
    let mut missing = Vec::new();
    let mut cursor = path;
    loop {
        match cursor.canonicalize() {
            Ok(base) => {
                let mut resolved = base;
                for part in missing.iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match (cursor.parent(), cursor.file_name()) {
                    (Some(parent), Some(name)) if parent != cursor => {
                        missing.push(name.to_os_string());
                        cursor = parent;
                    }
                    _ => {
                        return Err(Error::OutsideRoot {
                            path: path.to_path_buf(),
                        });
                    }
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}
