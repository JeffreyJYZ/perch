//! Replay recorded VT bytes and assert the frame the UI would paint.

use perch_term::{Color, replay};

#[test]
fn sgr_colours_and_cursor_land_in_the_frame() {
    let frame = replay(b"hi \x1b[31mred\x1b[0m\r\nsecond", 20, 4);
    assert_eq!(
        frame.rows[0]
            .cells
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>()
            .trim_end(),
        "hi red"
    );
    assert_eq!(frame.rows[0].cells[3].fg, Color::Indexed(1));
    assert_eq!(frame.cursor.row, 1);
    assert_eq!(frame.cursor.col, 6);
}

#[test]
fn wide_character_takes_two_columns() {
    let frame = replay("日本".as_bytes(), 10, 2);
    assert_eq!(frame.rows[0].cells[0].text, "日");
    assert_eq!(frame.rows[0].cells[1].text, "");
}
