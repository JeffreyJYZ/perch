//! The fixture every rules test opens: the store at the standard file name.

use std::path::Path;

use perch_agent::Rules;

pub fn at(dir: &Path) -> Rules {
    Rules::load(dir.join("permissions.jsonl")).unwrap()
}
