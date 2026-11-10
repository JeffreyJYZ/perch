//! File-type icons as Material Symbols names, from one extension table.

use std::path::Path;

/// Glyph for a directory that is closed, and one that is open.
pub const DIR_ICON: &str = "folder";
pub const DIR_OPEN_ICON: &str = "folder_open";
/// Glyph for a file whose type is unknown.
pub const FILE_ICON: &str = "draft";

/// The only extension-to-glyph table; every entry is a name Material Symbols actually ships.
const BY_EXTENSION: &[(&str, &str)] = &[
    ("bmp", "image"),
    ("c", "code"),
    ("cc", "code"),
    ("cpp", "code"),
    ("css", "css"),
    ("gif", "image"),
    ("go", "code"),
    ("h", "code"),
    ("hpp", "code"),
    ("html", "html"),
    ("jpeg", "image"),
    ("jpg", "image"),
    ("js", "javascript"),
    ("json", "data_object"),
    ("jsx", "javascript"),
    ("lock", "lock"),
    ("md", "description"),
    ("pdf", "picture_as_pdf"),
    ("png", "image"),
    ("py", "code"),
    ("rs", "code"),
    ("sh", "terminal"),
    ("svg", "image"),
    ("toml", "settings"),
    ("ts", "code"),
    ("tsx", "code"),
    ("txt", "description"),
    ("yaml", "data_object"),
    ("yml", "data_object"),
    ("zsh", "terminal"),
];

/// A Material Symbols glyph name for `path`, falling back to the plain file glyph.
pub fn icon_for(path: &Path) -> &'static str {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return FILE_ICON;
    };
    let extension = extension.to_ascii_lowercase();
    match BY_EXTENSION.iter().find(|(known, _)| *known == extension) {
        Some((_, glyph)) => glyph,
        None => FILE_ICON,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_rows_bind_an_extension_to_a_material_symbol() {
        assert_eq!(icon_for(Path::new("main.rs")), "code");
        assert_eq!(icon_for(Path::new("notes.md")), "description");
    }
}
