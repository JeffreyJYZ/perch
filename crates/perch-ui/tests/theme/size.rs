//! The type scale: one ratio, two bounds, and nothing changed when nothing is picked.

use perch_ui::kit;

#[test]
fn a_picked_size_scales_every_type_size_by_one_ratio() {
    let kit = kit::from_anchors(&kit::builtin(true), Some(14.0));
    let ratio = 14.0 / kit::BASE;

    assert!(
        (kit.ty.body.size - 14.0).abs() < 1e-4,
        "the body is the size that was picked, drawn at {}",
        kit.ty.body.size
    );
    let sizes = [
        (kit.ty.small.size, 10.0),
        (kit.ty.small_medium.size, 10.0),
        (kit.ty.text_small.size, 12.0),
        (kit.ty.mono.size, 12.0),
        (kit.ty.body.size, 13.0),
        (kit.ty.ui.size, 13.0),
        (kit.ty.text_xsmall.size, 12.0),
    ];
    for (drawn, today) in sizes {
        assert!(
            (drawn / today - ratio).abs() < 1e-5,
            "{drawn} from {today} is not {ratio}× it"
        );
    }
    assert!(
        (kit.ty.small.line_height / 13.0 - ratio).abs() < 1e-5,
        "line heights keep the ratio too, so the scale does not close up"
    );
}

#[test]
fn a_size_under_the_floor_is_drawn_at_ten_and_says_so() {
    let (size, note) = kit::clamp_size(8.0);
    assert_eq!(size, 10.0);
    assert_eq!(note.unwrap(), "font size 8 is under 10; drawn at 10");

    let kit = kit::from_anchors(&kit::builtin(true), Some(8.0));
    assert!(
        (kit.ty.body.size - 10.0).abs() < 1e-4,
        "the drawn size is the floor, not {}",
        kit.ty.body.size
    );
    let small = kit.ty.small.size;
    assert!(
        (small - 10.0 * 10.0 / 13.0).abs() < 1e-4,
        "the 10pt label follows the body down, drawn at {small}"
    );
}

#[test]
fn a_size_over_the_ceiling_is_drawn_at_twenty_and_says_so() {
    let (size, note) = kit::clamp_size(25.0);
    assert_eq!(size, 20.0);
    assert_eq!(note.unwrap(), "font size 25 is over 20; drawn at 20");

    let kit = kit::from_anchors(&kit::builtin(true), Some(25.0));
    assert!(
        (kit.ty.body.size - 20.0).abs() < 1e-4,
        "the drawn size is the ceiling, not {}",
        kit.ty.body.size
    );
}

#[test]
fn a_size_inside_the_bounds_is_taken_as_written() {
    assert_eq!(kit::clamp_size(10.0), (10.0, None));
    assert_eq!(kit::clamp_size(20.0), (20.0, None));
    assert_eq!(kit::clamp_size(13.5), (13.5, None));
}

#[test]
fn no_size_leaves_todays_scale_byte_for_byte() {
    let kit = kit::perch(true);
    assert_eq!(kit.ty.small.size, 10.0);
    assert_eq!(kit.ty.small.line_height, 13.0);
    assert_eq!(kit.ty.small_medium.size, 10.0);
    assert_eq!(kit.ty.body.size, 13.0);
    assert_eq!(kit.ty.body.line_height, 19.0);
    assert_eq!(kit.ty.body_medium.size, 13.0);
    assert_eq!(kit.ty.text_small.size, 12.0);
    assert_eq!(kit.ty.text_small.line_height, 17.0);
    assert_eq!(kit.ty.mono.size, 12.0);
    assert_eq!(kit.ty.mono.line_height, 18.0);
    assert_eq!(
        kit.ty.ui.size, 13.0,
        "a size nobody picked leaves the library's own scale alone"
    );
    assert_eq!(kit.ty.ui.line_height, 20.0, "and its line height with it");
}
