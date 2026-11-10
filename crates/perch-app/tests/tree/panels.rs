//! The file panel and the transcript: what they read reaches their own region.

use perch_agent::{AgentEvent, EndReason, ToolCall, Usage};
use perch_files::Workspace;
use perch_ui::Shell;

use crate::harness::{find_key, meta, says, temp_repo_with, tree_of};

#[test]
fn a_file_row_from_the_workspace_reaches_the_file_panel() {
    let repo = temp_repo_with(&[("alpha_widget.rs", "fn alpha() {}\n")]);
    let workspace = Workspace::open(repo.path()).expect("the worktree opens");
    let tree = tree_of(Shell::with_workspace(vec![meta("work")], 0, workspace));

    let files = find_key(&tree, "files").expect("the file panel is in the tree");
    assert!(
        says(files, "alpha_widget.rs"),
        "the file row names the file it was built from: {files}"
    );
}

#[test]
fn the_transcript_shows_text_and_a_tool_cards_status_word() {
    let mut shell = Shell::idle(vec![meta("work")], 0);
    {
        let transcript = shell.transcript_mut();
        transcript.push_user("why is the build red?");
        transcript.apply(&AgentEvent::TextDelta("Two jobs failed.".into()));
        transcript.apply(&AgentEvent::ToolCall(ToolCall {
            id: "call-1".to_string(),
            name: "read".to_string(),
            args: serde_json::json!({ "path": "src/main.rs" }),
        }));
        transcript.apply(&AgentEvent::ToolResult {
            id: "call-1".to_string(),
            ok: true,
            text: "fn main() {}\n".to_string(),
        });
        transcript.apply(&AgentEvent::TurnEnd {
            reason: EndReason::Done,
            usage: Usage::default(),
        });
    }

    let tree = tree_of(shell);
    let transcript = find_key(&tree, "transcript").expect("the transcript region is in the tree");
    assert!(
        says(transcript, "Two jobs failed."),
        "the assistant's text is drawn: {transcript}"
    );
    assert!(
        says(transcript, "why is the build red?"),
        "the user's turn is drawn: {transcript}"
    );
    assert!(
        says(transcript, "done"),
        "the tool card says how it ended: {transcript}"
    );
}
