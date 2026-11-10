//! Whether a manifest is an update this machine may be offered.

use crate::{Error, Manifest, Result, Update, Version};

/// What the OS gate becomes when the running macOS version cannot be read.
///
/// Skipping the gate is the honest answer: the archive still has to match its checksum and the
/// bundle still has its own signature and identifier, so a probe that failed must not be the
/// thing that leaves a user stuck on an old build forever.
const UNKNOWN_OS: u32 = u32::MAX;

/// The update to offer, or `None` when the manifest is not newer than `current`.
///
/// An offer for a newer release that names a `min_os` above this machine's macOS major version
/// is refused with [`Error::UnsupportedOs`] rather than reported as "no update": the user has a
/// real new release to install and needs to be told why it will not run here.
pub fn check(current: &str, manifest: &str) -> Result<Option<Update>> {
    check_on(current, manifest, os_major())
}

fn check_on(current: &str, manifest: &str, os_major: u32) -> Result<Option<Update>> {
    let current = Version::parse(current).map_err(|reason| Error::Invalid {
        field: "current version",
        reason,
    })?;
    let manifest = Manifest::parse(manifest)?;
    if manifest.version <= current {
        return Ok(None);
    }
    if manifest.min_os > os_major {
        return Err(Error::UnsupportedOs {
            needed: manifest.min_os,
            running: os_major,
        });
    }
    Ok(Some(Update {
        version: manifest.version,
        url: manifest.url,
        notes: manifest.notes,
    }))
}

/// The running macOS major version, from `sw_vers`, because a build links against one SDK and
/// the machine it wakes up on is whatever the user is running.
fn os_major() -> u32 {
    let probe = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output();
    let text = match probe {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        _ => return UNKNOWN_OS,
    };
    text.trim()
        .split('.')
        .next()
        .and_then(|major| major.parse().ok())
        .unwrap_or(UNKNOWN_OS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str, min_os: u32) -> String {
        serde_json::json!({
            "version": version,
            "url": "https://release.perch.test/Perch.zip",
            "notes": "Notes.",
            "min_os": min_os,
            "sha256": "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        })
        .to_string()
    }

    #[test]
    fn a_newer_alpha_is_offered() {
        let update = check_on("0.1.0-alpha.9", &manifest("0.1.0-alpha.10", 14), 14)
            .expect("checks")
            .expect("alpha.10 follows alpha.9");
        assert_eq!(update.version.to_string(), "0.1.0-alpha.10");
        assert_eq!(update.url, "https://release.perch.test/Perch.zip");
        assert_eq!(update.notes, "Notes.");
    }

    #[test]
    fn a_stable_release_is_offered_over_its_alphas() {
        let update = check_on("0.1.0-alpha.10", &manifest("0.1.0", 14), 14)
            .expect("checks")
            .expect("0.1.0 follows 0.1.0-alpha.10");
        assert_eq!(update.version.to_string(), "0.1.0");
    }

    #[test]
    fn the_same_or_an_older_version_is_not_an_update() {
        assert!(
            check_on("0.1.0-alpha.10", &manifest("0.1.0-alpha.10", 14), 14)
                .expect("checks")
                .is_none()
        );
        assert!(
            check_on("0.1.0", &manifest("0.1.0-alpha.10", 14), 14)
                .expect("checks")
                .is_none()
        );
    }

    #[test]
    fn a_newer_release_above_this_machines_macos_is_refused() {
        let error = check_on("0.1.0-alpha.9", &manifest("0.1.0-alpha.10", 27), 14)
            .expect_err("macOS 14 cannot run a macOS 27 build");
        assert!(
            matches!(
                error,
                Error::UnsupportedOs {
                    needed: 27,
                    running: 14
                }
            ),
            "{error}"
        );
        assert_eq!(
            error.to_string(),
            "this release needs macOS 27, and this is macOS 14"
        );
    }

    #[test]
    fn a_release_at_this_machines_macos_is_offered() {
        assert!(
            check_on("0.1.0-alpha.9", &manifest("0.1.0-alpha.10", 14), 14)
                .expect("checks")
                .is_some()
        );
    }

    #[test]
    fn an_older_release_never_reaches_the_os_gate() {
        assert!(
            check_on("0.1.0", &manifest("0.1.0-alpha.9", 99), 14)
                .expect("checks")
                .is_none()
        );
    }

    #[test]
    fn a_current_version_that_does_not_parse_is_named() {
        let error = check_on("dev", &manifest("0.1.0", 14), 14).expect_err("dev is not a version");
        assert!(
            matches!(
                &error,
                Error::Invalid {
                    field: "current version",
                    ..
                }
            ),
            "{error}"
        );
    }

    #[test]
    fn check_on_the_running_machine_does_not_probe_when_nothing_is_newer() {
        assert!(
            check("0.1.0-alpha.4", &manifest("0.1.0-alpha.4", 14))
                .expect("checks")
                .is_none()
        );
    }
}
