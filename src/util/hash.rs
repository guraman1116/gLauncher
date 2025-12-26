//! Hash verification utilities
//!
//! SHA1 hash verification for downloaded files.

use sha1::{Digest, Sha1};
use std::path::Path;

/// Calculate SHA1 hash of a file
pub fn sha1_file(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    let hash = Sha1::digest(&bytes);
    Ok(format!("{:x}", hash))
}

/// Verify file hash matches expected
pub fn verify_sha1(path: &Path, expected: &str) -> anyhow::Result<bool> {
    let actual = sha1_file(path)?;
    Ok(actual == expected)
}
