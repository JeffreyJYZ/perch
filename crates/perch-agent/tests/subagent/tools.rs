//! What a child may reach: no tool that asks a person, and no `task` of its own.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use perch_agent::{TASK, Task};

use crate::harness::{self, Memory, Named};

#[test]
fn the_child_is_given_only_the_tools_that_need_no_approval() {
    let (task, memory) = harness::tool(&harness::parent(), Memory::ok("done"));
    let child = memory.child();

    for gone in ["write", "bash", "edit", "patch", TASK] {
        assert!(
            !harness::holds(&child, gone),
            "a child was handed `{gone}`, which would change something while the parent waits: \
             {child:?}"
        );
    }
    for kept in ["read", "grep", "glob", "list"] {
        assert!(
            harness::holds(&child, kept),
            "a child lost `{kept}`, which asks nobody anything: {child:?}"
        );
    }

    // The seam the app builds its child around is this same set, not a second copy of it.
    let handed: Vec<String> = task
        .tools()
        .specs()
        .iter()
        .map(|spec| spec.name.clone())
        .collect();
    assert_eq!(
        handed, child,
        "the tool and the delegate disagree on the child's tools"
    );
}

#[test]
fn a_registry_that_already_holds_task_is_refused_and_no_delegate_is_built() {
    let mut registry = harness::parent();
    registry.register(Arc::new(Named(TASK)));
    let built = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&built);

    let refused = Task::new(&registry, move |_tools| {
        seen.store(true, Ordering::SeqCst);
        Arc::new(Memory::ok("a grandchild would have run"))
    });

    let why = harness::refused(refused);
    assert!(
        why.contains("`task`"),
        "the refusal does not name the tool: {why}"
    );
    assert!(
        why.contains(TASK),
        "the refusal does not name what a child could do: {why}"
    );
    assert!(
        !built.load(Ordering::SeqCst),
        "a delegate was built for a registry that could recurse"
    );
}
