//! The result being built: lines, and a buffer that stops growing at its cap.

/// The output a program produced, clamped at the cap so one run cannot flood the conversation.
#[derive(Debug)]
pub struct Out {
    text: String,
    cap: usize,
    produced: usize,
}

impl Out {
    pub fn new(cap: usize) -> Self {
        Self {
            text: String::new(),
            cap,
            produced: 0,
        }
    }

    /// Appends one line, answering the bytes it would have taken.
    pub fn line(&mut self, line: &str) -> usize {
        self.push(line, true)
    }

    /// Appends text with no line ending, which is how a program.rs last value reads.
    pub fn value(&mut self, value: &str) -> usize {
        self.push(value, false)
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn push(&mut self, text: &str, newline: bool) -> usize {
        let bytes = text.len() + usize::from(newline as u8);
        self.produced += bytes;
        let room = self.cap.saturating_sub(self.text.len());
        if room == 0 {
            return bytes;
        }
        let take = room.min(text.len());
        let end = boundary(text, take);
        self.text.push_str(&text[..end]);
        if newline && end == text.len() && room > text.len() {
            self.text.push('\n');
        }
        bytes
    }
}

/// The largest index at or below `take` that a slice may end on.
fn boundary(text: &str, take: usize) -> usize {
    let mut end = take.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}
