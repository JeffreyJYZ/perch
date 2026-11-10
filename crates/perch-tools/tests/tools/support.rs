//! A root inside a throwaway directory, and the path `pwd -P` really prints.

use std::path::{Path, PathBuf};

use perch_tools::ToolCtx;
use tempfile::TempDir;

pub fn root_of(dir: &TempDir) -> ToolCtx {
    ToolCtx {
        root: dir.path().to_path_buf(),
    }
}

pub fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
