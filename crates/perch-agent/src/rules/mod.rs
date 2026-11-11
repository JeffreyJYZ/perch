//! Saved permission rules: an approval that outlives the session that gave it.
//!
//! The dispatcher asks about a call once, and "for the session" dies with it. A decision written
//! to a file in the data directory is read back at the start of the next session instead.
//!
//! The file is one JSON object per line, so a rule is one diffable line a person can edit:
//!
//! ```text
//! {"tool":"bash","pattern":"cargo test","decision":"allow"}
//! {"tool":"write","pattern":"src/","decision":"allow"}
//! {"tool":"bash","decision":"deny"}
//! ```
//!
//! A saved allow never widens. A rule with a pattern applies only to that pattern, and a path
//! pattern is resolved through `perch-tools`' own confinement, so `write` allowed for `src/` says
//! nothing about `~/.ssh` and nothing outside the worktree. A rule with no pattern is the
//! tool-wide allow a person deliberately chose.
//!
//! A corrupt file is an error the caller can see, never an empty rule set: an unreadable allow
//! list that silently means "allow everything" is the one failure this module must not have.

mod pattern;

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dispatch::Decision;
use crate::{Error, Result};
use pattern::{command, path, pattern_of, under};

/// A decision that was written down, as opposed to one that lasts a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    Allow,
    Deny,
}

impl Permission {
    /// "For the session" is a session-scoped `Allow`, so there is nothing wider to remember.
    fn saved(decision: Decision) -> Self {
        match decision {
            Decision::Allow | Decision::AllowForSession => Self::Allow,
            Decision::Deny => Self::Deny,
        }
    }
}

/// One saved rule: a tool, optionally narrowed to one argument, and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub tool: String,
    /// `None` for a tool-wide rule, else the exact command or a path whose contents are covered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub decision: Permission,
}

impl Rule {
    /// Whether this rule governs `tool` called with `args` inside `root`.
    fn matches(&self, tool: &str, args: &Value, root: &Path) -> bool {
        if self.tool != tool {
            return false;
        }
        let Some(saved) = self.pattern.as_deref() else {
            return true;
        };
        match (command(args), path(args)) {
            // A command is matched exactly: `cargo test` is not `cargo test --all`.
            (Some(command), _) => command == saved,
            (None, Some(path)) => under(root, &path, saved),
            // A narrowed rule cannot match a call with no argument to narrow on.
            (None, None) => false,
        }
    }
}

/// The rules file and the rules in it.
#[derive(Debug, Clone)]
pub struct Rules {
    path: PathBuf,
    rules: Vec<Rule>,
}

impl Rules {
    /// No rules at all, but the file they will be written to: the state a corrupt file starts
    /// from. Every call that needs permission asks, and a `remember` still has somewhere to go.
    pub fn none(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            rules: Vec::new(),
        }
    }

    /// Reads `path`, which is simply absent until a person saves something.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };

        let mut rules = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let rule: Rule = serde_json::from_str(line).map_err(|error| {
                Error::Config(format!(
                    "{} line {}: {error}; fix or delete that line, no rule from it applies",
                    path.display(),
                    index + 1
                ))
            })?;
            rules.push(rule);
        }
        Ok(Self { path, rules })
    }

    /// Where these rules are stored.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every rule, in file order, for a settings view.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The decision already saved for this call, if any. The last matching rule wins, so a later
    /// line can overrule an earlier one.
    pub fn permit(&self, tool: &str, args: &Value, root: &Path) -> Option<Permission> {
        self.rules
            .iter()
            .rev()
            .find(|rule| rule.matches(tool, args, root))
            .map(|rule| rule.decision)
    }

    /// Writes down a decision for this call, replacing a rule for the same tool and argument.
    pub fn remember(&mut self, tool: &str, args: &Value, decision: Decision) -> Result<()> {
        let rule = Rule {
            tool: tool.to_string(),
            pattern: pattern_of(args),
            decision: Permission::saved(decision),
        };
        self.rules
            .retain(|existing| !(existing.tool == rule.tool && existing.pattern == rule.pattern));
        self.rules.push(rule);
        self.persist()
    }

    /// Drops every rule for one tool, so a mistake is fixable from settings later.
    pub fn forget(&mut self, tool: &str) -> Result<()> {
        self.rules.retain(|rule| rule.tool != tool);
        self.persist()
    }

    /// Rewrites the file readably, 0600, and waits for the disk before returning.
    fn persist(&self) -> Result<()> {
        if let Some(dir) = self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }

        let mut text = String::new();
        for rule in &self.rules {
            text.push_str(&serde_json::to_string(rule)?);
            text.push('\n');
        }

        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&self.path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // `mode` above only applies to a created file, so one left 0644 by hand is narrowed.
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }
}
