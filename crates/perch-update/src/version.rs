//! Version parsing and precedence, including the `0.1.0-alpha.N` tags this project ships.

use std::cmp::Ordering;
use std::fmt;

/// A version: `MAJOR.MINOR.PATCH`, an optional prerelease, and an ignored build tag.
///
/// Ordering is semver's. The numeric fields decide first; then a version carrying a
/// prerelease is lower than the same version without one, so `0.1.0-alpha.9 < 0.1.0`; and
/// within a prerelease the identifiers decide one at a time, numerically when both are digits,
/// so `alpha.10 > alpha.9` and never the other way round the way a string compare would say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    numbers: [u64; 3],
    prerelease: Vec<String>,
}

impl Version {
    /// Parse `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]`.
    ///
    /// The failure is a reason, not an error type: every caller here has to name the field it
    /// was reading, and that name is the only part this parser cannot supply.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let without_build = text.split('+').next().unwrap_or_default();
        let (core, prerelease) = match without_build.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (without_build, None),
        };

        let fields: Vec<&str> = core.split('.').collect();
        if fields.len() != 3 {
            return Err(format!("`{text}` is not MAJOR.MINOR.PATCH"));
        }
        let mut numbers = [0u64; 3];
        for (index, field) in fields.iter().enumerate() {
            numbers[index] = field
                .parse()
                .map_err(|_| format!("`{text}` has a non-numeric {field:?}"))?;
        }

        let prerelease = match prerelease {
            None => Vec::new(),
            Some("") => return Err(format!("`{text}` has an empty prerelease")),
            Some(pre) => {
                let mut identifiers = Vec::new();
                for identifier in pre.split('.') {
                    if identifier.is_empty() {
                        return Err(format!("`{text}` has an empty prerelease identifier"));
                    }
                    if !identifier
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
                    {
                        return Err(format!(
                            "`{text}` has {identifier:?} in its prerelease, which is not [0-9A-Za-z-]"
                        ));
                    }
                    identifiers.push(identifier.to_string());
                }
                identifiers
            }
        };

        Ok(Self {
            numbers,
            prerelease,
        })
    }

    /// True when this version carries a prerelease tag such as `alpha.3`.
    pub fn is_prerelease(&self) -> bool {
        !self.prerelease.is_empty()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{}.{}",
            self.numbers[0], self.numbers[1], self.numbers[2]
        )?;
        if !self.prerelease.is_empty() {
            write!(formatter, "-{}", self.prerelease.join("."))?;
        }
        Ok(())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.numbers.cmp(&other.numbers) {
            Ordering::Equal => compare_prerelease(&self.prerelease, &other.prerelease),
            order => order,
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_prerelease(left: &[String], right: &[String]) -> Ordering {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => Ordering::Equal,
        // A release outranks every prerelease of the same numbers.
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            for (left, right) in left.iter().zip(right.iter()) {
                let order = compare_identifier(left, right);
                if order != Ordering::Equal {
                    return order;
                }
            }
            left.len().cmp(&right.len())
        }
    }
}

fn compare_identifier(left: &str, right: &str) -> Ordering {
    match (left.parse::<u64>(), right.parse::<u64>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        // Numeric identifiers rank below alphanumeric ones.
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => left.cmp(right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).expect("parses")
    }

    #[test]
    fn alpha_dot_ten_is_newer_than_alpha_dot_nine() {
        assert!(version("0.1.0-alpha.10") > version("0.1.0-alpha.9"));
        assert!(version("0.1.0-alpha.9") < version("0.1.0-alpha.10"));
    }

    #[test]
    fn a_stable_release_is_newer_than_any_of_its_prereleases() {
        for tag in [
            "0.1.0-alpha.1",
            "0.1.0-alpha.99",
            "0.1.0-beta.1",
            "0.1.0-rc.2",
        ] {
            assert!(version("0.1.0") > version(tag), "0.1.0 vs {tag}");
            assert!(version(tag) < version("0.1.0"), "{tag} vs 0.1.0");
        }
    }

    #[test]
    fn numbers_outrank_the_prerelease_rule() {
        assert!(version("0.1.1-alpha.1") > version("0.1.0"));
        assert!(version("0.2.0") > version("0.1.99"));
    }

    #[test]
    fn identifiers_compare_one_at_a_time() {
        assert!(version("0.1.0-alpha") < version("0.1.0-alpha.1"));
        assert!(version("0.1.0-alpha.1") < version("0.1.0-beta.1"));
        assert_eq!(version("0.1.0"), version("0.1.0"));
    }

    #[test]
    fn a_build_tag_does_not_change_the_order() {
        assert_eq!(version("0.1.0-alpha.3+7"), version("0.1.0-alpha.3"));
        assert_eq!(version("0.1.0+7").to_string(), "0.1.0");
    }

    #[test]
    fn a_malformed_version_is_a_reason_naming_the_input() {
        let reason = Version::parse("0.1").expect_err("two fields are not a version");
        assert!(reason.contains("`0.1`"), "{reason}");
        let reason = Version::parse("0.1.x").expect_err("x is not a number");
        assert!(reason.contains("\"x\""), "{reason}");
    }
}
