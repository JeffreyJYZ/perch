//! The language a filename implies, which is the only thing a preview asks about it.

use std::path::Path;

use perch_files::language_for;

#[test]
fn language_is_inferred_from_the_extension() {
    assert_eq!(language_for(Path::new("a.rs")), Some("rust"));
    assert_eq!(language_for(Path::new("CMakeLists.txt")), Some("cmake"));
    assert_eq!(language_for(Path::new("README")), None);
}
