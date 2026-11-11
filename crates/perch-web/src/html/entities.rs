//! The entities a page actually uses, decoded without a lookup table crate.

/// Decodes named and numeric character references. An unknown or unterminated one is left as it
/// was written, which is the only honest answer when the reference is not one this crate knows.
pub fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        let tail = &rest[index..];
        let Some(end) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        if end > 12 {
            out.push('&');
            rest = &tail[1..];
            continue;
        }
        let name = &tail[1..end];
        match named(name).or_else(|| numeric(name)) {
            Some(replacement) => {
                out.push_str(&replacement);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn named(name: &str) -> Option<String> {
    let replacement = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "hellip" => "\u{2026}",
        "lsquo" => "\u{2018}",
        "rsquo" => "\u{2019}",
        "ldquo" => "\u{201c}",
        "rdquo" => "\u{201d}",
        "middot" => "\u{00b7}",
        "copy" => "\u{00a9}",
        "reg" => "\u{00ae}",
        "trade" => "\u{2122}",
        "times" => "\u{00d7}",
        _ => return None,
    };
    Some(replacement.to_string())
}

fn numeric(name: &str) -> Option<String> {
    let digits = name.strip_prefix('#')?;
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse().ok()?,
    };
    Some(char::from_u32(code)?.to_string())
}
