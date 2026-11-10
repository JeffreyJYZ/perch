//! The worktree walk and its tree.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::status::{self, FileStatus};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub path: PathBuf,
    pub name: String,
    pub kind: NodeKind,
    pub status: Option<FileStatus>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Dir,
    File,
}

/// One session's worktree: its canonical root, its git statuses and its walker settings.
#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
    statuses: HashMap<PathBuf, FileStatus>,
}

impl Workspace {
    /// Opens `root` and reads its git state once; a plain directory opens too, with no statuses.
    pub fn open(root: &Path) -> Result<Workspace> {
        let root = std::fs::canonicalize(root)
            .map_err(|e| Error::Message(format!("cannot open {}: {e}", root.display())))?;
        if !root.is_dir() {
            return Err(Error::Message(format!(
                "{} is not a directory",
                root.display()
            )));
        }
        let statuses = status::status_map(&root);
        Ok(Workspace { root, statuses })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Re-reads git state; the caller decides when a refresh is worth a subprocess.
    pub fn refresh(&mut self) {
        self.statuses = status::status_map(&self.root);
    }

    /// The canonical form of `candidate`, refused when it leaves the root.
    ///
    /// Canonicalising both sides is the point: a symlink inside the root that resolves
    /// outside it is an escape, and `starts_with` is component-wise so a sibling directory
    /// whose name merely shares the prefix cannot pass.
    pub fn confine(&self, candidate: &Path) -> Result<PathBuf> {
        let joined = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.root.join(candidate)
        };
        let canonical = std::fs::canonicalize(&joined)
            .map_err(|e| Error::Message(format!("cannot resolve {}: {e}", joined.display())))?;
        if !canonical.starts_with(&self.root) {
            return Err(Error::OutsideRoot {
                path: candidate.to_path_buf(),
            });
        }
        Ok(canonical)
    }

    /// Directories first, then alphabetically; ignored and hidden paths are absent.
    pub fn tree(&self) -> Vec<Node> {
        let mut root = Node {
            path: self.root.clone(),
            name: String::new(),
            kind: NodeKind::Dir,
            status: None,
            children: Vec::new(),
        };

        let mut builder = WalkBuilder::new(&self.root);
        builder
            .hidden(true)
            .git_ignore(true)
            .git_exclude(true)
            // This machine's global excludes must not change what a worktree contains.
            .git_global(false)
            .parents(false)
            // Honour a `.gitignore` even in a directory that is not a repository yet.
            .require_git(false)
            .follow_links(false);

        for entry in builder.build().flatten() {
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(&self.root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            let kind = match entry.file_type() {
                Some(file_type) if file_type.is_dir() => NodeKind::Dir,
                _ => NodeKind::File,
            };
            let status = match kind {
                NodeKind::File => self.statuses.get(relative).copied(),
                NodeKind::Dir => None,
            };
            let parts: Vec<String> = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            insert(&mut root, &parts, kind, status);
        }

        sort_children(&mut root.children);
        root.children
    }
}

fn insert(parent: &mut Node, parts: &[String], kind: NodeKind, status: Option<FileStatus>) {
    let Some((head, rest)) = parts.split_first() else {
        return;
    };
    let index = match parent.children.iter().position(|child| child.name == *head) {
        Some(index) => index,
        None => {
            let node = Node {
                path: parent.path.join(head),
                name: head.clone(),
                kind: if rest.is_empty() { kind } else { NodeKind::Dir },
                status: if rest.is_empty() { status } else { None },
                children: Vec::new(),
            };
            parent.children.push(node);
            parent.children.len() - 1
        }
    };
    if !rest.is_empty() {
        insert(&mut parent.children[index], rest, kind, status);
    }
}

fn sort_children(children: &mut [Node]) {
    children.sort_by(|a, b| match (a.kind, b.kind) {
        (NodeKind::Dir, NodeKind::File) => std::cmp::Ordering::Less,
        (NodeKind::File, NodeKind::Dir) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
    for child in children.iter_mut() {
        sort_children(&mut child.children);
    }
}
