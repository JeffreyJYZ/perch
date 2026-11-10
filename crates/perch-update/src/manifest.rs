//! The manifest a release server publishes at one stable URL.

use serde_json::{Map, Value};

use crate::{Error, Result, Version};

/// What one release says about itself.
///
/// Every field is required, because the manifest is written by a release script and a script
/// that forgot the checksum produces an archive nothing can vouch for. Keys this version does
/// not know are ignored, so a server may add one before the app that reads it ships.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub version: Version,
    pub url: String,
    pub notes: String,
    /// The lowest macOS major version the archive runs on.
    pub min_os: u32,
    /// Lowercase hex SHA-256 of the archive at `url`.
    pub sha256: String,
}

impl Manifest {
    /// Parse the document, naming the field when one is missing or does not hold what it must.
    pub fn parse(text: &str) -> Result<Self> {
        let document: Value = serde_json::from_str(text).map_err(|error| Error::Shape {
            reason: format!("not JSON: {error}"),
        })?;
        let object = document.as_object().ok_or_else(|| Error::Shape {
            reason: format!("expected a JSON object, got {}", kind_of(&document)),
        })?;

        let text = required_text(object, "version")?;
        let version = Version::parse(&text).map_err(|reason| Error::Invalid {
            field: "version",
            reason,
        })?;
        Ok(Self {
            version,
            url: required_text(object, "url")?,
            notes: required_text(object, "notes")?,
            min_os: required_major(object, "min_os")?,
            sha256: required_digest(object, "sha256")?,
        })
    }
}

/// A string field. A JSON `null` is a missing field: it carries no value either way.
fn required_text(object: &Map<String, Value>, field: &'static str) -> Result<String> {
    match object.get(field) {
        None | Some(Value::Null) => Err(Error::MissingField { field }),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(other) => Err(Error::Invalid {
            field,
            reason: format!("expected a string, got {}", kind_of(other)),
        }),
    }
}

/// A whole-number field. `"14"` is refused rather than coerced, so a script that quoted the
/// version is told so instead of shipping something the parser guessed at.
fn required_major(object: &Map<String, Value>, field: &'static str) -> Result<u32> {
    match object.get(field) {
        None | Some(Value::Null) => Err(Error::MissingField { field }),
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| Error::Invalid {
                field,
                reason: format!("expected a positive integer, got {number}"),
            }),
        Some(other) => Err(Error::Invalid {
            field,
            reason: format!("expected a positive integer, got {}", kind_of(other)),
        }),
    }
}

/// The archive checksum, normalised to lowercase so a comparison is a string comparison.
fn required_digest(object: &Map<String, Value>, field: &'static str) -> Result<String> {
    let text = required_text(object, field)?;
    let digest = text.to_ascii_lowercase();
    if digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(Error::Invalid {
            field,
            reason: format!("expected 64 hex digits, got {text:?}"),
        });
    }
    Ok(digest)
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests;
