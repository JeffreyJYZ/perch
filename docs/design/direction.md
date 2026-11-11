# perch — visual direction

Status: proposed, not implemented.

## Why not the default

The "developer tool" rows in every design database resolve to the same thing: slate-800 background, slate-300 text, one green or blue accent, all-mono type, rounded cards. That is the look opencode, Zed and every Electron IDE share, and it is why the current window reads as a generic shell. Distinctiveness here has to come from temperature, proportion and marks — not from a novelty font.

## The idea

**An instrument, not an IDE.** A lab instrument is legible at a glance, made of rules rather than cards, and its state is always written down. Warm neutrals (a brown-black, paper-cream ink) instead of blue-grey, one signal colour that means "something is happening", and a visible tick or rule wherever the interface changes kind.

## Tokens (OKLCH, dark)

Three values are chosen; every other tone is a mix of them, computed once in
`crates/perch-ui/src/kit.rs` (`Kit::from_palette`), so no view picks a colour of its own.

| Token | Value | Use |
|---|---|---|
| `bg` | `oklch(0.165 0.002 265)` | window canvas, neutral near-black |
| `ink` | `oklch(0.94 0.004 265)` | primary text, neutral off-white |
| `signal` | `oklch(0.74 0.15 62)` | amber: in flight, needs you, focus ring |

`panel`, `sunken`, `ink-2`, `ink-3`, `rule`, `rule-strong` and `sel` are mixed from those three, so
they hold the same neutral temperature. The earlier warm brown-black (`oklch(0.185 0.012 62)`) read
as a mistake rather than a mood on a dark canvas, and was dropped; amber is now the only warm thing
in the window. The light theme is the same three anchors inverted, on near-white paper.

Contrast: `ink` on `bg` is far above 4.5:1; `ink-3` is only for non-essential labels, never for content. Colour never carries meaning alone — every state also has its word.

## Type

- **UI and labels**: the system UI face (`system-ui`), 11–13 px. Native, sharp, free, and it makes the app read as a Mac app rather than a web page in a box.
- **Code, paths, numbers, terminal**: the system monospace face (`ui-monospace`), 12–13 px, tabular figures.
- **Scale, and only this scale**: 10 (uppercase labels), 11 (secondary), 12 (code), 13 (body), 16 (nothing but the empty-state word). No 15, no 18.
- **Labels** are uppercase, 10 px, letter-spaced 0.08em, `ink-3`. They are the only uppercase text in the window.
- Weight carries hierarchy instead of size: 500 for a session name, 400 for everything else, no bold.

## Proportion

- 8 px rhythm; 4 px only inside a control.
- Sidebar 288 px fixed, dock 240 px fixed, transcript takes the rest. No resizable splits in v1.
- Row height 28 px for lists, 24 px for tree, 44 px for a card header.
- Hairlines 1 px everywhere a region changes; no shadows anywhere; radius 4 px, and only on inputs and buttons.

## Marks

- **Rack ticks**: a 1 px rule under each column header, with 6 px ticks every 48 px. This is the one decorative element in the app, and it is what makes the window read as an instrument.
- **Status underline**: a state word with a 2 px underline in its colour, 4 px below the baseline. Running pulses between `signal` and `rule` at 1.2 s; nothing else animates.
- **Tool cards** are rows, not boxes: 2 px left rule in the status colour, name in mono, arguments dimmed, output collapsed to `… 7 more lines`. No borders, no fill.
- **Composer** is a sunken single row with a `›` caret, the model shown as plain right-aligned text, and `⏎` as the only glyph hint. Beside it, the context meter: `12.3K (6%) used` in the smallest size and faint ink, from the last turn's usage against the model's own limit — a number that is always true, never a bar.
- **Dock** is a sunken well with a rule and ticks above it; tabs are words with an underline on the active one.
- **Session rows**: name at 500, worktree path and branch in mono at `ink-3`, selected row gets `sel` fill and a 2 px `rule-strong` left edge — not a rounded highlight.

## What this replaces

The current window uses `Kit::neutral`, rounded selection, boxed cards, and colour-only status dots. All four go.
