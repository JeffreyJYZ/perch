//! The chord grammar: one way to write a key combination and one way to read it back.
//!
//! A chord is its modifiers joined by `+` and then one key: `mod+,`, `ctrl+c`, `mod+shift+k`.
//! The modifiers are `mod` (⌘ on this machine), `ctrl`, `alt` and `shift`; the key is a letter,
//! a comma, or one of `enter`, `tab`, `escape`, `up`, `down` and `space`.
//!
//! One grammar, one parser and one writer, so a chord read from a file and a chord typed into a
//! field are the same string.

/// The keys a chord may name, besides a letter or a comma.
pub const NAMED: [&str; 6] = ["enter", "tab", "escape", "up", "down", "space"];

/// One key combination: the modifiers it holds and the key it ends with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chord {
    /// `mod`: ⌘ on this machine.
    pub mod_key: bool,
    /// The control key.
    pub ctrl: bool,
    /// The option key.
    pub alt: bool,
    /// The shift key.
    pub shift: bool,
    /// The key itself: a letter, a comma, or one of [`NAMED`].
    pub key: String,
}

impl Chord {
    /// Reads a chord, or says why the text is not one.
    ///
    /// The words name what is wrong with the text, so a pane can show one line that says both
    /// what to fix and which chord it was.
    pub fn parse(text: &str) -> Result<Chord, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("a chord is not empty".to_string());
        }
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (key, modifiers) = parts.split_last().expect("a split always yields one part");
        let mut chord = Chord {
            key: key_of(key)?,
            ..Chord::default()
        };
        for modifier in modifiers {
            match *modifier {
                "mod" => set_once(&mut chord.mod_key, "mod")?,
                "ctrl" => set_once(&mut chord.ctrl, "ctrl")?,
                "alt" => set_once(&mut chord.alt, "alt")?,
                "shift" => set_once(&mut chord.shift, "shift")?,
                other => {
                    return Err(format!(
                        "{other} is not a modifier: a chord holds mod, ctrl, alt and shift"
                    ));
                }
            }
        }
        if !chord.mod_key && !chord.ctrl && !chord.alt && !chord.shift {
            return Err("a chord holds a modifier: mod, ctrl, alt or shift".to_string());
        }
        Ok(chord)
    }

    /// The chord as it is written in `settings.toml`.
    pub fn write(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.mod_key {
            parts.push("mod");
        }
        if self.ctrl {
            parts.push("ctrl");
        }
        if self.alt {
            parts.push("alt");
        }
        if self.shift {
            parts.push("shift");
        }
        parts.push(&self.key);
        parts.join("+")
    }
}

/// The key a chord ends with, or why the text is not one.
fn key_of(text: &str) -> Result<String, String> {
    let lowered = text.to_lowercase();
    if NAMED.contains(&lowered.as_str()) {
        return Ok(lowered);
    }
    let mut chars = lowered.chars();
    let first = chars.next().unwrap_or_default();
    if chars.next().is_some() {
        return Err(format!(
            "{text} is not one key: a letter, a comma, or one of {}",
            NAMED.join(", ")
        ));
    }
    if first == ',' || first.is_ascii_alphabetic() {
        return Ok(first.to_string());
    }
    Err(format!(
        "{text} is not one key: a letter, a comma, or one of {}",
        NAMED.join(", ")
    ))
}

/// Sets a modifier, and refuses one that is already there.
fn set_once(flag: &mut bool, name: &str) -> Result<(), String> {
    if *flag {
        return Err(format!("{name} is in the chord twice"));
    }
    *flag = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chord_is_written_the_way_it_is_read() {
        for text in [
            "mod+,",
            "mod+k",
            "ctrl+c",
            "mod+shift+k",
            "alt+enter",
            "mod+space",
        ] {
            let chord = Chord::parse(text).expect("the chord parses");
            assert_eq!(chord.write(), text, "a chord round-trips");
        }
    }

    #[test]
    fn the_modifiers_are_read_in_any_order_but_written_in_one() {
        let chord = Chord::parse("shift+mod+k").unwrap();
        assert!(chord.mod_key && chord.shift);
        assert_eq!(chord.write(), "mod+shift+k");
    }

    #[test]
    fn a_letter_is_read_as_itself_and_a_named_key_as_its_name() {
        assert_eq!(Chord::parse("mod+K").unwrap().key, "k");
        assert_eq!(Chord::parse("mod+Enter").unwrap().key, "enter");
        assert_eq!(Chord::parse("mod+,").unwrap().key, ",");
    }

    #[test]
    fn text_that_is_not_a_chord_is_refused_in_words() {
        for (text, words) in [
            ("", "not empty"),
            ("k", "holds a modifier"),
            ("hyper+k", "not a modifier"),
            ("mod+", "not one key"),
            ("mod+shift+shift+k", "twice"),
        ] {
            let error = Chord::parse(text).expect_err("the text is not a chord");
            assert!(
                error.contains(words),
                "{text:?} is refused with {error:?}, which does not mention {words:?}"
            );
        }
    }
}
