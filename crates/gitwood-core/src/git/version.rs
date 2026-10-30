use std::fmt;

use super::GitError;

/// 2.35.2 adds the ownership / safe.directory boundary; all used options predate it.
pub const MIN_GIT_VERSION: GitVersion = GitVersion(2, 35, 2);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GitVersion(pub u32, pub u32, pub u32);

impl GitVersion {
    pub fn parse(bytes: &[u8]) -> Result<Self, GitError> {
        let text = String::from_utf8_lossy(bytes);
        let invalid = || GitError::InvalidVersion(text.trim().to_owned());
        let version = text
            .trim()
            .strip_prefix("git version ")
            .ok_or_else(invalid)?;
        let mut parts = version.split('.');
        let major = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(invalid)?;
        let minor = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(invalid)?;
        let patch: String = parts
            .next()
            .ok_or_else(invalid)?
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        Ok(Self(major, minor, patch.parse().map_err(|_| invalid())?))
    }

    pub(super) fn require_supported(self) -> Result<Self, GitError> {
        if self < MIN_GIT_VERSION {
            return Err(GitError::VersionTooOld {
                found: self.to_string(),
                minimum: MIN_GIT_VERSION.to_string(),
            });
        }
        Ok(self)
    }
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}
