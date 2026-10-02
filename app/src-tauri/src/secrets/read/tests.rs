use super::*;
use std::cell::Cell;

#[cfg(target_os = "linux")]
fn crypto_error() -> Error {
    Error::PlatformFailure(Box::new(dbus_secret_service::Error::Crypto(Box::new(
        std::io::Error::other("synthetic invalid session padding"),
    ))))
}

#[test]
fn reads_real_entry_api_and_does_not_retry_success_or_absence() {
    let entry = Entry::new_with_credential(Box::new(keyring::mock::MockCredential::default()));
    assert!(matches!(read_password(&entry), Err(Error::NoEntry)));
    entry.set_password("synthetic-credential").unwrap();
    assert_eq!(read_password(&entry).unwrap(), "synthetic-credential");
    let calls = Cell::new(0);
    assert!(matches!(
        read_with_retry(|| {
            calls.set(calls.get() + 1);
            Err(Error::NoEntry)
        }),
        Err(Error::NoEntry)
    ));
    assert_eq!(calls.get(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn fresh_encrypted_read_recovers_from_session_failure_and_stops_at_success() {
    let mut calls = 0;
    let recovered = read_with_retry(|| {
        calls += 1;
        if calls < 3 {
            Err(crypto_error())
        } else {
            Ok("synthetic-saved-key".into())
        }
    });
    assert_eq!(recovered.unwrap(), "synthetic-saved-key");
    assert_eq!(calls, 3);
}

#[cfg(target_os = "linux")]
#[test]
fn encrypted_read_failure_has_a_strict_limit_and_remains_an_error() {
    let mut calls = 0;
    let result = read_with_retry(|| {
        calls += 1;
        Err(crypto_error())
    });
    assert!(matches!(result, Err(Error::PlatformFailure(_))));
    assert_eq!(calls, MAX_READ_ATTEMPTS);
}

#[test]
fn never_retry_denial_lock_prompt_ambiguous_or_unknown_errors() {
    let errors = vec![
        Error::NoStorageAccess(Box::new(std::io::Error::other("denied"))),
        Error::PlatformFailure(Box::new(std::io::Error::other("unknown"))),
        Error::BadEncoding(vec![0xff]),
        Error::Ambiguous(vec![]),
        Error::Invalid("attribute".into(), "invalid".into()),
    ];
    for error in errors {
        let mut error = Some(error);
        let mut calls = 0;
        assert!(read_with_retry(|| {
            calls += 1;
            Err(error.take().expect("must not retry"))
        })
        .is_err());
        assert_eq!(calls, 1);
    }
    #[cfg(target_os = "linux")]
    for backend in [
        dbus_secret_service::Error::Locked,
        dbus_secret_service::Error::Prompt,
        dbus_secret_service::Error::Unavailable,
    ] {
        assert!(!encrypted_session_read_failed(&Error::PlatformFailure(
            Box::new(backend)
        )));
    }
}

#[test]
fn complete_and_absent_auth_sessions_never_clear_credentials() {
    let session = load_auth_session(
        Ok(Some("synthetic-access".into())),
        Ok(Some("synthetic-refresh".into())),
        || panic!("complete session must remain"),
    );
    assert_eq!(
        session,
        Some(AuthSessionMaterial {
            access_token: "synthetic-access".into(),
            refresh_token: "synthetic-refresh".into()
        })
    );
    assert_eq!(
        load_auth_session(Ok(None), Ok(None), || panic!(
            "absent session needs no deletion"
        )),
        None
    );
}

#[test]
fn unreadable_auth_material_never_deletes_either_token() {
    for (access, refresh) in [
        (
            Err("wallet failure".into()),
            Ok(Some("synthetic-refresh".into())),
        ),
        (
            Ok(Some("synthetic-access".into())),
            Err("wallet failure".into()),
        ),
        (Err("wallet failure".into()), Ok(None)),
        (Ok(None), Err("wallet failure".into())),
        (Err("wallet failure".into()), Err("wallet failure".into())),
    ] {
        assert_eq!(
            load_auth_session(access, refresh, || panic!("unreadable is not missing")),
            None
        );
    }
}

#[test]
fn genuinely_partial_auth_session_clears_once() {
    for (access, refresh) in [
        (Ok(Some("synthetic-access".into())), Ok(None)),
        (Ok(None), Ok(Some("synthetic-refresh".into()))),
    ] {
        let cleared = Cell::new(0);
        assert_eq!(
            load_auth_session(access, refresh, || cleared.set(cleared.get() + 1)),
            None
        );
        assert_eq!(cleared.get(), 1);
    }
}
