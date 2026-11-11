# A blank strip under the terminal: what the dock measured, what the grid says, what the pty was told

The report was a blank strip at the bottom of the dock, present even when the output was scrolled. It was not the scroll position and not the pty: the dock painted **only the rows that had text in them** and then stretched a column to fill the rest of the box.

## The numbers

Measured headless, `Runtime` over `perch_ui::shell::view`, window 1280×820. The dock is `DOCK = 220` with `py(12)` and `gap(6)`; the mono face is 12 pt at `lh(18)`, so one terminal line is 18 points.

| Reading | Before | After |
| --- | --- | --- |
| Grid box (`terminal-rows` bounds), y / height | 631 / **177** | 631 / **162** |
| Rows the frame reports (`frame.rows.len()`) | 9 | 9 |
| Row nodes the dock painted | **3** (only rows carrying runs) | **9** (one per frame row) |
| Height of one painted row | 18 | 18 |
| Painted height of the grid | 54 | 162 |
| **Blank strip under the last line** | **123** | **0** |
| `grid_for(box)` rows — what the pty is told | 9 | 9 |
| Last row's bottom vs the grid's bottom | n/a (no node) | 793 vs 793 |

The probe: a `20 × 9` frame whose text is `one\n\ntwo\n\n\n\n\n\nnine`. Before the fix the tree held `terminal-row-0` at y=631 and `terminal-row-8` did not exist at all; after it, `terminal-row-2` sits at 667 (= 631 + 2×18) and `terminal-row-8` at 775, ending exactly at the grid's bottom edge.

## What the grid says, and what the pty is told

`grid_for` (`crates/perch-ui/src/shell/pump.rs`) is the single answer to "how many lines does this box hold": `rows = floor(height / line_height)`, clamped to 3..200. `Shell::measure` stores it and `TerminalTab::pump` sends it to the pty on the next frame, so after one frame the pty's row count, the frame's `rows.len()` and the number of rows the dock paints are one number. The test that holds this together is `shell::dock::geometry::the_grid_paints_a_line_for_every_row_the_frame_reports`: for a given box it asserts the frame's rows, the painted rows and `grid_for(box)` are equal, and that the last line ends where the grid does.

## What was ruled out, with the evidence

- **`paint_rows` skipping trailing rows** — it does not. It walks every row of the frame (`terminal_view.rs:60`) and emits no runs for a row that is blank, which is right: there is nothing to draw. The rows were dropped one level up, in `dock_body`, which grouped runs by row and emitted a `terminal_row` only for a row that had at least one run. That is the `3 of 9` reading above, and it is the whole 123 px.
- **`grid_for` rounding down against a box that is not a whole multiple of the line height** — real, but small, and it is the 15 px that is still not painted: 177 / 18 = 9.83, so the box holds 9 lines and 162 points of them. It became visible as a strip because the grid also had `grow(1.0)`, which kept the drawn column at the full 177 while the rows inside it ended at 162. The grid's height is now `rows × line_height` exactly, so the remainder belongs to the panel behind it, not to the terminal; the next frame measures 162, which floors back to 9. A fixed point, not an oscillation.
- **`py`/`gap`/header subtracted for the grid but not for the measurement, or the reverse** — not it. `measure` is handed the **grid's own** laid-out rect (`cx.layout_size("terminal-rows")`), so the panel's 12 px padding, 6 px gap and the header row are outside the number on both sides. The grid sits at y=631 = 612 (panel content top) + 13 (header) + 6 (gap), and that is exactly where the first line was painted before and after.
- **The pty's rows lagging one resize behind** — true, and one frame wide: `measure` reads the last frame's rect and the resize goes out on the next pump. It cannot produce a strip, because the number of rows painted is now read from the same count the pty was told. At steady state the two agree, which is what the geometry test asserts.
- **A row with text is not 18 points** — it is: `terminal-row-0` measured h=18 in the same tree, so the text box and `row_height(kit)` agree, and an empty row now takes the same 18 by `h(...)` instead of collapsing to 0.

## What changed

1. `dock_body` emits one row for **every** row of the frame, including blank ones (`rows_of` in `crates/perch-ui/src/shell/dock.rs`). A blank row is a line the shell left empty, and it is a line the grid must still hold.
2. Every row is exactly one line tall, text or not (`terminal_row`, `crates/perch-ui/src/shell/dock/row.rs`). The height comes from the font's line height, never from the row's text.
3. The grid's height is `rows × line_height`, not `grow(1.0)`, so the box the dock measures and the box it paints are the same box.
4. `grid_for` is `pub(crate)`: the dock and the measurement share it instead of each computing a row count.

A side effect worth naming: `cell_at` maps a pointer to a cell with `height / rows`, which before the change was 177 / 9 = 19.67 points per row against 18 points actually painted, so a click near the bottom of a line named the line below it. It is 18 against 18 now.

## What is left, and what only a window can confirm

- The 15 px between the last line and the panel's bottom edge is `DOCK` not being a whole number of lines (220 = 24 padding + 13 header + 6 gap + 162 rows + 15). It is the panel's own background, not the terminal's, and it is invisible on the base colour. Making it zero means choosing `DOCK` as a multiple of the line height, which is a layout decision for the dock's height, not a bug in the grid.
- Headless proves the layout: bounds, row count, and the last row's bottom edge. It cannot prove what the strip *looks* like on screen — that needs the window, and it is the one thing to look at when someone next runs `cargo run -p perch-app`.
