use super::GitError;
use std::{collections::BTreeSet, ffi::OsString};

/// `config --show-scope --null --name-only` emits scope NUL key NUL pairs.
/// Includes retain the scope of their source. Never parse (possibly binary) values.
pub(super) fn overrides(output: &[u8]) -> Result<Vec<OsString>, GitError> {
    let invalid = || GitError::InvalidCommand("malformed scoped filter configuration".into());
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let mut fields = output
        .strip_suffix(&[0])
        .ok_or_else(invalid)?
        .split(|b| *b == 0);
    let mut drivers = BTreeSet::new();
    while let Some(scope) = fields.next() {
        let key = fields.next().ok_or_else(invalid)?;
        if matches!(scope, b"system" | b"global" | b"command") {
            continue;
        }
        let key = key.strip_prefix(b"filter.").ok_or_else(invalid)?;
        let end = key.iter().rposition(|b| *b == b'.').ok_or_else(invalid)?;
        drivers.insert(key[..end].to_vec());
    }
    let mut result = Vec::new();
    for driver in drivers {
        for setting in [
            b".clean=".as_slice(),
            b".smudge=",
            b".process=",
            b".required=false",
        ] {
            let key = [b"filter.", driver.as_slice(), setting].concat();
            #[cfg(unix)]
            let key = {
                use std::os::unix::ffi::OsStringExt;
                OsString::from_vec(key)
            };
            #[cfg(not(unix))]
            let key = OsString::from(String::from_utf8(key).map_err(|_| invalid())?);
            result.extend(["-c".into(), key]);
        }
    }
    Ok(result)
}
