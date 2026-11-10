//! Reading a recorded fixture into the responses it holds.

use std::time::Duration;

/// A fixture cannot ask the server to wait longer than this between events.
const MAX_PACE: u64 = 1_000;

/// One recorded fixture: the responses it holds and how it wants to be served.
pub struct Recording {
    /// How long to wait between two events of a response, when the fixture says.
    pub pause: Option<Duration>,
    /// One entry per request: the SSE events of that response.
    pub responses: Vec<Vec<String>>,
    /// Whether the last response answers every further request, for a turn that never settles.
    pub repeat: bool,
}

/// Read a recording into its responses.
pub fn read_fixture(path: &str) -> Recording {
    let recorded = std::fs::read_to_string(path).unwrap();
    let mut pause = None;
    let mut repeat = false;
    let mut bodies: Vec<String> = vec![String::new()];
    for line in recorded.lines() {
        let Some(note) = line.trim_start().strip_prefix(": ") else {
            if let Some(body) = bodies.last_mut() {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        };
        let note = note.trim();
        if let Some(millis) = note.strip_prefix("pace_ms ") {
            let millis = millis.trim().parse::<u64>().unwrap_or(0).min(MAX_PACE);
            pause = Some(Duration::from_millis(millis));
        } else if note == "next request" {
            bodies.push(String::new());
        } else if note == "repeat" {
            repeat = true;
        }
        // Any other `: ...` line is a note for whoever recorded it, not something to send.
    }
    Recording {
        pause,
        responses: bodies.iter().map(|body| events_of(body)).collect(),
        repeat,
    }
}

/// The SSE events of one response, each kept with its own `data:` lines.
fn events_of(body: &str) -> Vec<String> {
    body.split("\n\n")
        .filter(|event| !event.trim().is_empty())
        .map(str::to_string)
        .collect()
}
