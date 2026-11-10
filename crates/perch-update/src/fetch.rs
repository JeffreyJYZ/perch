//! The network: one manifest, one archive, both behind a 30 s timeout.
//!
//! The transport is a trait so the verification path can be run without a socket: the SHA-256
//! check is the part that must work, and a test that needed a server would be the part that
//! does not.

use std::fs;
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::{Error, Manifest, Result};

/// How long one request may take, connect and body together.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// One HTTP GET. The only thing this crate needs from the network.
pub trait Transport {
    fn get(&self, url: &str) -> Result<Vec<u8>>;
}

/// The real transport: one blocking client whose timeout bounds every request it makes.
pub struct Http {
    client: reqwest::blocking::Client,
}

impl Http {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .map_err(transport_error)?;
        Ok(Self { client })
    }
}

impl Transport for Http {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        let response = self.client.get(url).send().map_err(transport_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Transport {
                reason: format!("{url} answered {status}"),
            });
        }
        let bytes = response.bytes().map_err(transport_error)?;
        Ok(bytes.to_vec())
    }
}

/// Download the manifest, then the archive it names, into `dest`.
///
/// Both requests go through [`Transport`], so both are bounded by [`TIMEOUT`]. The archive is
/// written to `dest` and only then checked, because the check is over the bytes that landed on
/// disk rather than the bytes that were in memory.
pub fn download(transport: &dyn Transport, manifest_url: &str, dest: &Path) -> Result<Manifest> {
    let bytes = transport.get(manifest_url)?;
    let text = String::from_utf8(bytes).map_err(|_| Error::Shape {
        reason: format!("{manifest_url} is not UTF-8"),
    })?;
    let manifest = Manifest::parse(&text)?;
    stage(transport, &manifest, dest)?;
    Ok(manifest)
}

/// Fetch the archive a manifest names and stage it at `dest`.
///
/// A download whose SHA-256 does not match the manifest is deleted before the error returns:
/// an archive nothing vouched for must not be left where a later step might pick it up.
pub fn stage(transport: &dyn Transport, manifest: &Manifest, dest: &Path) -> Result<()> {
    let bytes = transport.get(&manifest.url)?;
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, &bytes)?;

    let actual = digest_of(&bytes);
    if actual != manifest.sha256 {
        fs::remove_file(dest)?;
        return Err(Error::Checksum {
            path: dest.to_path_buf(),
            expected: manifest.sha256.clone(),
            actual,
        });
    }
    Ok(())
}

fn digest_of(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

fn transport_error(error: reqwest::Error) -> Error {
    if error.is_timeout() {
        return Error::Timeout {
            seconds: TIMEOUT.as_secs(),
        };
    }
    Error::Transport {
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests;
