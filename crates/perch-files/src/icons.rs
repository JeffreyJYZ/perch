//! What a row is, as one table of kinds — not a glyph name.
//!
//! The kind is answered from [`language_for`], the crate's only extension table: a second list
//! here would drift from the preview's colouring the first time one of them learned a new
//! suffix. A directory has no extension to read, so its kind comes from the node that named it.

use std::path::Path;

use crate::language_for;

/// The kinds a row can be drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A directory, always drawn with a caret of its own.
    Folder,
    /// Rust sources.
    Rust,
    /// C sources and headers.
    C,
    /// C++ sources and headers.
    Cpp,
    /// TOML, and the lock files it describes.
    Toml,
    /// JSON.
    Json,
    /// Markdown.
    Markdown,
    /// Everything else, extension or none.
    File,
}

impl Kind {
    /// Every kind, so a test can prove each one is drawn and each icon is its own.
    pub const ALL: [Kind; 8] = [
        Kind::Folder,
        Kind::Rust,
        Kind::C,
        Kind::Cpp,
        Kind::Toml,
        Kind::Json,
        Kind::Markdown,
        Kind::File,
    ];

    /// The kind a language name from the preview's table is drawn as.
    fn of_language(language: &str) -> Kind {
        match language {
            "rust" => Kind::Rust,
            "c" => Kind::C,
            "cpp" => Kind::Cpp,
            "toml" => Kind::Toml,
            "json" => Kind::Json,
            "markdown" => Kind::Markdown,
            _ => Kind::File,
        }
    }
}

/// The kind `path` is drawn as, read from the one extension table this crate has.
///
/// Only a node knows it is a directory: two names without an extension are the same path here,
/// so [`Kind::Folder`] never comes back from this function.
pub fn icon_for(path: &Path) -> Kind {
    language_for(path).map_or(Kind::File, Kind::of_language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_the_tree_draws_is_answered_from_the_language_table() {
        let cases = [
            ("main.rs", Kind::Rust),
            ("main.c", Kind::C),
            ("main.h", Kind::C),
            ("main.cc", Kind::Cpp),
            ("main.cpp", Kind::Cpp),
            ("main.cxx", Kind::Cpp),
            ("main.hpp", Kind::Cpp),
            ("main.hh", Kind::Cpp),
            ("main.hxx", Kind::Cpp),
            ("Cargo.toml", Kind::Toml),
            ("Cargo.lock", Kind::Toml),
            ("package.json", Kind::Json),
            ("README.md", Kind::Markdown),
            ("notes.txt", Kind::File),
            ("Makefile", Kind::File),
            ("LICENSE", Kind::File),
        ];
        for (name, expected) in cases {
            assert_eq!(icon_for(Path::new(name)), expected, "{name}");
        }
    }

    #[test]
    fn an_uppercase_extension_is_the_same_kind() {
        assert_eq!(icon_for(Path::new("MAIN.RS")), Kind::Rust);
        assert_eq!(icon_for(Path::new("NOTES.MD")), Kind::Markdown);
    }

    #[test]
    fn a_path_never_answers_folder() {
        assert!(Kind::ALL.contains(&Kind::Folder));
        assert_eq!(icon_for(Path::new("src")), Kind::File);
        assert_eq!(icon_for(Path::new("src/")), Kind::File);
    }
}
