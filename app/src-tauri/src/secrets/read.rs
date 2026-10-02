//! Encrypted wallet reads and auth-session absence/error ownership.
//!
//! Older KDE Secret Service versions can negotiate a mismatched DH session key.
//! Each keyring read creates a fresh encrypted session. Retry only that typed
//! crypto failure, never access denial, unlock dismissal, absence or writes.
//! No plaintext transport fallback and no delay or unbounded retry is used.

use super::AuthSessionMaterial;
use keyring::{Entry, Error};

const MAX_READ_ATTEMPTS: usize = 16;

fn encrypted_session_read_failed(error: &Error) -> bool {
    #[cfg(target_os = "linux")]
    if let Error::PlatformFailure(source) = error {
        return matches!(
            source.downcast_ref::<dbus_secret_service::Error>(),
            Some(dbus_secret_service::Error::Crypto(_))
        );
    }
    let _ = error;
    false
}

fn read_with_retry(mut read: impl FnMut() -> Result<String, Error>) -> Result<String, Error> {
    let mut retries_left = MAX_READ_ATTEMPTS - 1;
    loop {
        match read() {
            Err(error) if retries_left > 0 && encrypted_session_read_failed(&error) => {
                retries_left -= 1;
            }
            result => return result,
        }
    }
}

pub(super) fn read_password(entry: &Entry) -> Result<String, Error> {
    read_with_retry(|| entry.get_password())
}

pub(super) fn load_auth_session(
    access: Result<Option<String>, String>,
    refresh: Result<Option<String>, String>,
    clear_partial: impl FnOnce(),
) -> Option<AuthSessionMaterial> {
    match (access, refresh) {
        (Ok(Some(access_token)), Ok(Some(refresh_token))) => Some(AuthSessionMaterial {
            access_token,
            refresh_token,
        }),
        (Ok(None), Ok(None)) | (Err(_), _) | (_, Err(_)) => None,
        _ => {
            clear_partial();
            None
        }
    }
}

#[cfg(test)]
mod tests;
