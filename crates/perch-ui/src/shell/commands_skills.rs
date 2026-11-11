//! `/skills` and `/skill <name>`: the instruction sets a person can hand to the model.
//!
//! The list is read from disk on every call, so a skill added during a session is offered by the
//! next command without a restart. `/skills` writes into the transcript and `/skill` writes into
//! the composer, where the body is a prompt the person edits before sending it.

use std::path::PathBuf;

use perch_agent::skills::{self, Found, Skill};

use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// `/skills`: every skill as `name — description`, or the words that there are none.
    pub(super) fn command_skills(&mut self) -> Result<Option<String>, String> {
        let found = self.skill_found();
        if found.skills.is_empty() {
            return Ok(Some(empty(found.skipped)));
        }
        for skill in &found.skills {
            self.say(&row(skill));
        }
        Ok(None)
    }

    /// `/skill <name>`: the body in the composer, where the person edits it before sending.
    pub(super) fn command_skill(&mut self, arg: &str) -> Result<Option<String>, String> {
        let name = arg.trim();
        if name.is_empty() {
            return Err("skill needs a name: /skill <name> — /skills lists them".to_string());
        }
        let found = self.skill_found();
        let Some(skill) = found.skills.into_iter().find(|skill| skill.name == name) else {
            return Err(format!("no skill named {name}; /skills lists them"));
        };
        self.set_prompt(skill.body);
        Ok(Some(format!(
            "skill: {} — edit the prompt, then send it",
            skill.name
        )))
    }

    /// The skills this window can see: the active session's worktree, and the machine's own.
    fn skill_found(&self) -> Found {
        skills::discover_in(self.worktree().as_deref(), skills::home_dir().as_deref())
    }

    /// The worktree the active session is bound to, when there is a session at all.
    fn worktree(&self) -> Option<PathBuf> {
        self.sessions
            .get(self.active)
            .map(|session| session.worktree.clone())
    }
}

/// One line of `/skills`: the name, then what the skill is for.
fn row(skill: &Skill) -> String {
    let about = skill.description.as_deref().unwrap_or("no description");
    format!("{} — {about}", skill.name)
}

/// The sentence `/skills` writes when nothing was found, naming where a skill would go.
fn empty(skipped: usize) -> String {
    let mut words = String::from(
        "no skills: a skill is a folder with a SKILL.md in it, under this project's .perch/skills \
         or the machine's .config/perch/skills",
    );
    if skipped > 0 {
        words.push_str(&format!(
            " ({skipped} SKILL.md file(s) were passed over for having no front matter)"
        ));
    }
    words
}
