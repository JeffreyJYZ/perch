//! `glob`: paths under the worktree that match a pattern, ignore-aware and sorted by path.

use std::path::Path;

use globset::GlobBuilder;
use ignore::WalkBuilder;
use serde_json::{Value, json};

use super::{LIST_MAX_ENTRIES, object_schema, string_arg};
use crate::{Error, Result, Tool, ToolCtx, ToolOutput, ToolSpec, confine};

pub struct Glob;

impl Tool for Glob {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "glob".into(),
            description: format!(
                "Find files under the worktree whose path matches a glob pattern, honouring \
                 .gitignore. `*` and `?` stay inside one path segment and `**` crosses \
                 directories, so `**/*.rs` finds every Rust file. Results are sorted by path \
                 and capped at {LIST_MAX_ENTRIES} entries. A pattern that escapes the worktree \
                 is refused."
            ),
            parameters: object_schema(
                json!({
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern relative to the worktree root, such as `**/*.rs` or `src/**/mod.rs`."
                    }
                }),
                &["pattern"],
                "Find paths by glob pattern.",
            ),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let pattern = string_arg(args, "pattern")?;
        if pattern.trim().is_empty() {
            return Err(Error::Message("`pattern` must not be empty".into()));
        }
        let pattern = pattern.strip_prefix("./").unwrap_or(&pattern).to_string();
        // A pattern is still path-shaped: the same confinement that refuses `../x` has to refuse
        // `../**/*.rs`, and its resolved path is used for nothing else.
        confine(&ctx.root, Path::new(&pattern))?;

        let matcher = GlobBuilder::new(&pattern)
            // `*` must not swallow a directory separator, or `*.rs` would mean `**/*.rs`.
            .literal_separator(true)
            .build()
            .map_err(|error| Error::Message(format!("invalid glob `{pattern}`: {error}")))?
            .compile_matcher();

        let root = ctx.root.canonicalize()?;
        let mut builder = WalkBuilder::new(&root);
        builder
            .hidden(true)
            .git_ignore(true)
            .git_exclude(true)
            // This machine's global excludes must not change what a worktree holds.
            .git_global(false)
            .parents(false)
            // Honour a `.gitignore` in a directory that is not a repository yet.
            .require_git(false)
            .follow_links(false);

        let mut found = Vec::new();
        for entry in builder.build().flatten() {
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            if entry.file_type().is_some_and(|kind| kind.is_dir()) {
                continue;
            }
            let name = relative.to_string_lossy();
            if matcher.is_match(name.as_ref()) {
                found.push(name.into_owned());
            }
        }

        if found.is_empty() {
            return Ok(ToolOutput::text(format!("no paths match `{pattern}`")));
        }
        found.sort();
        let hidden = found.len().saturating_sub(LIST_MAX_ENTRIES);
        found.truncate(LIST_MAX_ENTRIES);
        let mut text = found.join("\n");
        text.push('\n');
        if hidden > 0 {
            text.push_str(&format!("[truncated {hidden} entries]\n"));
        }
        Ok(ToolOutput::text(text))
    }
}
