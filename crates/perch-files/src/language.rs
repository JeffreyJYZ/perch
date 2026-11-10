//! Language names for the preview's syntax colouring, from one table per source.

use std::path::Path;

/// Filenames with no revealing extension.
const BY_NAME: &[(&str, &str)] = &[
    ("CMakeLists.txt", "cmake"),
    ("Cargo.lock", "toml"),
    ("Cargo.toml", "toml"),
    ("Dockerfile", "docker"),
    ("Justfile", "just"),
    ("Makefile", "make"),
    (".gitignore", "gitignore"),
];

const BY_EXTENSION: &[(&str, &str)] = &[
    ("c", "c"),
    ("cc", "cpp"),
    ("cfg", "ini"),
    ("cmake", "cmake"),
    ("cpp", "cpp"),
    ("css", "css"),
    ("go", "go"),
    ("h", "c"),
    ("hh", "cpp"),
    ("hpp", "cpp"),
    ("html", "html"),
    ("hxx", "cpp"),
    ("ini", "ini"),
    ("js", "javascript"),
    ("json", "json"),
    ("jsx", "javascript"),
    ("lock", "toml"),
    ("md", "markdown"),
    ("py", "python"),
    ("rs", "rust"),
    ("sh", "shell"),
    ("toml", "toml"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("txt", "text"),
    ("wit", "wit"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("zsh", "shell"),
];

/// A language name for the preview header, or `None` when nothing is known.
pub fn language_for(path: &Path) -> Option<&'static str> {
    if let Some(name) = path.file_name().and_then(|name| name.to_str())
        && let Some((_, language)) = BY_NAME.iter().find(|(known, _)| *known == name)
    {
        return Some(language);
    }
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let (_, language) = BY_EXTENSION.iter().find(|(known, _)| *known == extension)?;
    Some(language)
}
