//! The CHAP credentials spit logs in with: both `--chap-user` and `--chap-secret-file`, or neither;
//! the secret is read from a file only its owner may read, never from the command line. A login that
//! may need them goes without security first and uses CHAP only when the target answers with an
//! authentication failure.

use super::login::login;
use super::path::Path;
use super::session::Session;
use crate::{cli::Config, Result};
use std::path::Path as FsPath;

/// The CHAP credentials from the command line: both the user and the secret file, or neither.
pub fn chap_from_config(config: &Config) -> Result<Option<(String, Vec<u8>)>> {
    match (&config.chap_user, &config.chap_secret_file) {
        (Some(user), Some(file)) => Ok(Some((user.clone(), read_secret(file)?))),
        (None, None) => Ok(None),
        _ => Err("CHAP needs both --chap-user and --chap-secret-file".into()),
    }
}

/// The CHAP secret, read from a file that only its owner may read.
pub fn read_secret(path: &FsPath) -> Result<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!("{} must be mode 0600 or stricter", path.display()));
        }
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let secret = text.trim_end_matches(['\n', '\r']).as_bytes().to_vec();
    if secret.is_empty() {
        return Err(format!("{} is empty", path.display()));
    }
    Ok(secret)
}

/// Logs in without security first. Only a target that answers with an authentication failure gets the
/// CHAP credentials, so a no-auth target never sees CHAP.
pub fn login_with_fallback(
    path: &Path,
    initiator: &str,
    chap: Option<(&str, &[u8])>,
    digest: &str,
) -> Result<Session> {
    match login(&path.portal, &path.target, initiator, None, digest) {
        Ok(session) => Ok(session.for_lun(path)),
        Err(e) if chap.is_some() && e.contains("authentication failure") => {
            login(&path.portal, &path.target, initiator, chap, digest).map(|s| s.for_lun(path))
        }
        Err(e) => Err(e),
    }
}
