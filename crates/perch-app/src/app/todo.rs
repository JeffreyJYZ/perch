//! The plan the window draws: the active session's worktree, read from the table the tools write.
//!
//! The list is not copied into the app when a turn starts, because the turn is what changes it: a
//! `todowrite` in the middle of a run must show on the next frame, and the only thing that is
//! always current is the table itself. Reading it is one lock and at most [`perch_tools::MAX_TODOS`]
//! clones, so a frame can afford it.

use perch_tools::Todo;

use crate::AppState;

impl AppState {
    /// This session's todo list: the one `todowrite` writes in the active session's worktree.
    ///
    /// A window with no session open reads the data directory, which no turn ever writes, so it
    /// shows no plan rather than another session's.
    pub fn todos(&self) -> Vec<Todo> {
        let root = self
            .active_session()
            .map_or_else(|| self.dir.clone(), |session| session.meta.worktree.clone());
        perch_tools::todos(&root)
    }
}

#[cfg(test)]
mod tests {
    use perch_store::secret::MemorySecrets;
    use serde_json::json;

    use crate::{AppState, ProviderDefaults};

    fn app(data: &tempfile::TempDir) -> AppState {
        AppState::open_with_secrets(
            data.path(),
            ProviderDefaults::default(),
            Box::new(MemorySecrets::new()),
        )
        .expect("a data directory with no sessions opens")
    }

    fn write(app: &AppState, content: &str) {
        app.call_tool(
            "todowrite",
            &json!({"todos": [{"content": content, "status": "pending"}]}),
        )
        .expect("the write is allowed");
    }

    #[test]
    fn the_window_reads_its_sessions_plan_from_the_table_the_tool_writes() {
        let data = tempfile::tempdir().unwrap();
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();
        let mut app = app(&data);
        let first = app.new_session(one.path()).unwrap();
        let second = app.new_session(two.path()).unwrap();

        app.switch(first).unwrap();
        write(&app, "only in the first worktree");
        let plan = app.todos();
        assert_eq!(plan.len(), 1, "{plan:?}");
        assert_eq!(plan[0].content, "only in the first worktree");

        // A second session is a second worktree: its plan is its own.
        app.switch(second).unwrap();
        assert!(app.todos().is_empty(), "a session saw another's plan");

        app.switch(first).unwrap();
        assert_eq!(app.todos().len(), 1, "the plan did not survive the switch");
    }
}
