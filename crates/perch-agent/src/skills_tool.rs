//! The `skill` tool: one call that loads a skill's body into the conversation.
//!
//! A name that matches nothing answers the names that do exist, so a model that guessed wrong
//! corrects itself in one call instead of spending a second one asking what is installed. The
//! tool is a read: it runs nothing and changes nothing, so it asks for no approval.

use std::path::{Path, PathBuf};

use perch_tools::{Result, Tool, ToolCtx, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::skills::{self, Found, Skill};

/// What the model is told about the tool, in the words it reads.
const DESCRIPTION: &str = "A skill is a directory holding a SKILL.md: instructions for a kind of \
     task, such as how this repository's Rust is written or how a commit message is made here. \
     Loading one returns its body, and that body is instructions, never code — calling this runs \
     nothing — so read it and follow it for the rest of the task. Give the name discovery \
     printed; a name that does not exist answers the names that do, so a guess costs one call.";

/// The `skill` tool: a name in, the body out.
pub struct SkillTool {
    /// The home the machine's own skills are read from, when a test states one instead of letting
    /// the environment answer.
    home: Option<PathBuf>,
}

impl SkillTool {
    /// The tool a session is offered: the machine's skills from `PERCH_HOME` or the real home.
    #[must_use]
    pub fn new() -> SkillTool {
        SkillTool { home: None }
    }

    /// The tool with the home stated, which is what a test uses so it never reads the machine.
    #[must_use]
    pub fn in_home(home: impl Into<PathBuf>) -> SkillTool {
        SkillTool {
            home: Some(home.into()),
        }
    }

    /// Every skill this call can see, read fresh from the worktree it was given.
    fn found(&self, root: &Path) -> Found {
        let home = self.home.clone().or_else(skills::home_dir);
        skills::discover_in(Some(root), home.as_deref())
    }
}

impl Default for SkillTool {
    fn default() -> Self {
        SkillTool::new()
    }
}

impl Tool for SkillTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "skill".to_string(),
            description: DESCRIPTION.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The skill's name, as the available list prints it."
                    }
                },
                "required": ["name"],
                "additionalProperties": false
            }),
        }
    }

    fn needs_approval(&self) -> bool {
        false
    }

    fn call(&self, args: &Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let wanted = args
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let found = self.found(&ctx.root);
        match found.skills.iter().find(|skill| skill.name == wanted) {
            Some(skill) => Ok(ToolOutput::text(loaded(skill))),
            None => Ok(ToolOutput::text(missing(wanted, &found.skills))),
        }
    }
}

/// The answer for a name that exists: one line naming the skill and its file, then the body.
fn loaded(skill: &Skill) -> String {
    format!(
        "skill {} — {}\n\n{}",
        skill.name,
        skill.path.display(),
        skill.body
    )
}

/// The answer for a name that does not, listing what does so the next call can be right.
fn missing(wanted: &str, skills: &[Skill]) -> String {
    if skills.is_empty() {
        return format!(
            "no skill named {wanted}, and none are installed: a skill is a directory with a \
             SKILL.md in it, under .perch/skills beside the work or ~/.config/perch/skills"
        );
    }
    let names: Vec<&str> = skills.iter().map(|skill| skill.name.as_str()).collect();
    format!("no skill named {wanted}. available: {}", names.join(", "))
}
