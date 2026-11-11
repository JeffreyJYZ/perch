//! The tree's icons, drawn: one 16-point stroke glyph per [`Kind`].
//!
//! Every glyph is a page (or a folder) with a short mark inside it, drawn in the row's one ink on
//! a 16 × 16 grid and painted at 16 points, so the stroke lands on a whole device pixel at scale
//! 1. Nothing here names a font: a tree of a thousand rows draws eight shapes.

use perch_files::Kind;
use rune::Icon;

/// The points every icon is drawn at, which is also its grid: one unit is one point.
pub(super) const ICON: f32 = 16.0;

/// A directory.
static FOLDER: Icon = Icon {
    name: "perch-folder",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M2.1 4.9A1.4 1.4 0 0 1 3.5 3.5h3l1.5 2.1h4.5a1.4 1.4 0 0 1 1.4 1.4v4.6a1.4 1.4 0 0 1-1.4 1.4h-9a1.4 1.4 0 0 1-1.4-1.4z"/></g>"#,
};

/// Rust: a page with an `R`.
static RUST: Icon = Icon {
    name: "perch-rust",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M3.9 2.1h5l3.2 3.3v8.5H3.9z"/><path d="M8.9 2.1v3.3h3.2"/><path d="M5.8 12.2V7.4h1.9a1.5 1.5 0 0 1 0 3H5.8"/><path d="M7.7 10.4l2 1.8"/></g>"#,
};

/// C: a page with a `C`.
static C: Icon = Icon {
    name: "perch-c",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M3.9 2.1h5l3.2 3.3v8.5H3.9z"/><path d="M8.9 2.1v3.3h3.2"/><path d="M9.9 8.2a2.2 2.2 0 1 0 0 3.2"/></g>"#,
};

/// C++: a page with a `C` and a `+`.
static CPP: Icon = Icon {
    name: "perch-cpp",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M3.9 2.1h5l3.2 3.3v8.5H3.9z"/><path d="M8.9 2.1v3.3h3.2"/><path d="M8.1 8.5a2 2 0 1 0 0 2.9"/><path d="M10.8 9.7v1.8"/><path d="M9.9 10.6h1.8"/></g>"#,
};

/// TOML: a page with a `#`, the comment it is written with.
static TOML: Icon = Icon {
    name: "perch-toml",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M3.9 2.1h5l3.2 3.3v8.5H3.9z"/><path d="M8.9 2.1v3.3h3.2"/><path d="M7.2 7.7v4.5"/><path d="M9.2 7.7v4.5"/><path d="M6.4 9.1h4.6"/><path d="M6.4 10.9h4.6"/></g>"#,
};

/// JSON: a page with a brace pair.
static JSON: Icon = Icon {
    name: "perch-json",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M3.9 2.1h5l3.2 3.3v8.5H3.9z"/><path d="M8.9 2.1v3.3h3.2"/><path d="M7.5 7.6c-.9 0-.9.5-.9 1v.5c0 .5-.1.9-.8 1.1.7.2.8.6.8 1.1v.5c0 .5 0 1 .9 1"/><path d="M8.7 7.6c.9 0 .9.5.9 1v.5c0 .5.1.9.8 1.1-.7.2-.8.6-.8 1.1v.5c0 .5 0 1-.9 1"/></g>"#,
};

/// Markdown: a page with an `M`.
static MARKDOWN: Icon = Icon {
    name: "perch-markdown",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M3.9 2.1h5l3.2 3.3v8.5H3.9z"/><path d="M8.9 2.1v3.3h3.2"/><path d="M5.6 12.2V7.6l2.4 2.9 2.4-2.9v4.6"/></g>"#,
};

/// Everything else: a page with no mark.
static FILE: Icon = Icon {
    name: "perch-file",
    view: 16.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M3.9 2.1h5l3.2 3.3v8.5H3.9z"/><path d="M8.9 2.1v3.3h3.2"/></g>"#,
};

/// The drawing for `kind`: one shape per kind, and no kind shares another's.
pub(super) fn drawn(kind: Kind) -> &'static Icon {
    match kind {
        Kind::Folder => &FOLDER,
        Kind::Rust => &RUST,
        Kind::C => &C,
        Kind::Cpp => &CPP,
        Kind::Toml => &TOML,
        Kind::Json => &JSON,
        Kind::Markdown => &MARKDOWN,
        Kind::File => &FILE,
    }
}

#[cfg(test)]
mod tests {
    use rune::text::icon_coverage;

    use super::*;

    /// The 16 points each icon is rasterised at, matching [`ICON`].
    const PX: u32 = 16;

    #[test]
    fn every_kind_draws_its_own_glyph_inside_the_box() {
        let mut names: Vec<&str> = Vec::new();
        let mut rasters: Vec<Vec<u8>> = Vec::new();
        for kind in Kind::ALL {
            let icon = drawn(kind);
            assert_eq!(icon.view, ICON, "{} is drawn on the icon grid", icon.name);
            assert!(
                !names.contains(&icon.name),
                "two kinds answer {}",
                icon.name
            );
            names.push(icon.name);
            let coverage = icon_coverage(icon, PX);
            assert!(
                coverage.iter().any(|&ink| ink != 0),
                "{} draws nothing",
                icon.name
            );
            // A border texel is ink that ran off the grid: the box is 16 × 16 and holds it all.
            for i in 0..PX as usize {
                let border = [
                    coverage[i],
                    coverage[(PX as usize - 1) * PX as usize + i],
                    coverage[i * PX as usize],
                    coverage[i * PX as usize + PX as usize - 1],
                ];
                assert_eq!(border, [0; 4], "{} is clipped by its box", icon.name);
            }
            assert!(
                !rasters.contains(&coverage),
                "{} repeats another kind's drawing",
                icon.name
            );
            rasters.push(coverage);
        }
        assert_eq!(names.len(), Kind::ALL.len());
    }
}
