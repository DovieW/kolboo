//! Real Wry storage wiring with an isolated, synthetic credential store.
use super::*;
use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi, CredentialPersistence};
use std::{any::Any, collections::HashMap, sync::Arc};

#[derive(Debug, Default)]
struct MemoryBuilder(
    Arc<Mutex<HashMap<String, Arc<keyring::mock::MockCredential>>>>,
    Arc<Mutex<HashMap<String, usize>>>,
);

#[derive(Debug)]
struct SharedCredential(
    Arc<keyring::mock::MockCredential>,
    String,
    Arc<Mutex<HashMap<String, usize>>>,
);

impl CredentialApi for SharedCredential {
    fn set_password(&self, value: &str) -> keyring::Result<()> {
        let mut failures = self.2.lock().unwrap();
        if let Some(remaining) = failures.get_mut(&self.1) {
            *remaining -= 1;
            if *remaining == 0 {
                failures.remove(&self.1);
                return Err(keyring::Error::NoStorageAccess(Box::new(
                    std::io::Error::other("synthetic failed wallet write"),
                )));
            }
        }
        self.0.set_password(value)
    }
    fn get_password(&self) -> keyring::Result<String> {
        self.0.get_password()
    }
    fn set_secret(&self, value: &[u8]) -> keyring::Result<()> {
        self.0.set_secret(value)
    }
    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        self.0.get_secret()
    }
    fn delete_credential(&self) -> keyring::Result<()> {
        self.0.delete_credential()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn debug_fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("synthetic credential; values hidden")
    }
}

impl CredentialBuilderApi for MemoryBuilder {
    fn build(
        &self,
        _: Option<&str>,
        service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        assert_eq!(service, "kolboo");
        let credential = self
            .0
            .lock()
            .unwrap()
            .entry(user.into())
            .or_default()
            .clone();
        Ok(Box::new(SharedCredential(
            credential,
            user.into(),
            self.1.clone(),
        )))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::UntilDelete
    }
}

pub(crate) fn native_wallet_reads(app: &AppHandle) {
    assert_eq!(
        std::env::var("KOLBOO_NATIVE_WINDOW_TEST").as_deref(),
        Ok("1")
    );
    let builder = MemoryBuilder::default();
    let credentials = builder.0.clone();
    let failed_writes = builder.1.clone();
    keyring::set_default_credential_builder(Box::new(builder));
    // Only this opted-in process uses private D-Bus and temporary application data.
    let credential = |name: &str| credentials.lock().unwrap().get(name).unwrap().clone();
    assert!(load_desktop_input_token().is_none());
    save_desktop_input_token(Some("synthetic-desktop-grant")).unwrap();
    assert_eq!(
        load_desktop_input_token().as_deref(),
        Some("synthetic-desktop-grant")
    );
    save_desktop_input_token(None).unwrap();
    assert!(load_desktop_input_token().is_none());
    assert!(get_secret_result(app, "invalid-key").is_err());
    assert_eq!(
        get_secret_result(app, AUTH_SESSION_ACCESS_TOKEN_KEY).unwrap(),
        None
    );
    entry_for_key(AUTH_SESSION_ACCESS_TOKEN_KEY)
        .unwrap()
        .set_password("   ")
        .unwrap();
    assert_eq!(
        get_secret_result(app, AUTH_SESSION_ACCESS_TOKEN_KEY).unwrap(),
        None
    );
    set_secret(app, AUTH_SESSION_ACCESS_TOKEN_KEY, "synthetic-access").unwrap();
    set_secret(app, AUTH_SESSION_REFRESH_TOKEN_KEY, "synthetic-refresh").unwrap();
    assert_eq!(
        get_secret(app, AUTH_SESSION_ACCESS_TOKEN_KEY).as_deref(),
        Some("synthetic-access")
    );
    assert!(load_auth_session_material(app).is_some());
    credential(AUTH_SESSION_REFRESH_TOKEN_KEY).set_error(keyring::Error::NoStorageAccess(
        Box::new(std::io::Error::other("synthetic locked wallet")),
    ));
    assert!(load_auth_session_material(app).is_none());
    assert_eq!(
        get_secret(app, AUTH_SESSION_ACCESS_TOKEN_KEY).as_deref(),
        Some("synthetic-access")
    );
    assert_eq!(
        get_secret(app, AUTH_SESSION_REFRESH_TOKEN_KEY).as_deref(),
        Some("synthetic-refresh")
    );
    clear_secret(app, AUTH_SESSION_REFRESH_TOKEN_KEY).unwrap();
    assert!(load_auth_session_material(app).is_none());
    assert!(get_secret(app, AUTH_SESSION_ACCESS_TOKEN_KEY).is_none());
    assert!(get_secret(app, AUTH_SESSION_REFRESH_TOKEN_KEY).is_none());
    assert!(load_auth_session_material(app).is_none());

    assert!(get_api_key(app, "groq_api_key").is_none());
    assert!(!has_api_key_checked(app, "groq_api_key").unwrap());
    let store = app.store("settings.json").unwrap();
    store.set("groq_api_key", serde_json::json!("synthetic-groq-key"));
    assert!(has_api_key_checked(app, "groq_api_key").unwrap());
    migrate_api_keys_from_store(app).unwrap();
    assert_eq!(
        get_api_key(app, "groq_api_key").as_deref(),
        Some("synthetic-groq-key")
    );
    assert!(store.get("groq_api_key").is_none());
    assert!(has_api_key_checked(app, "groq_api_key").unwrap());
    credential("groq_api_key").set_error(keyring::Error::PlatformFailure(Box::new(
        dbus_secret_service::Error::Crypto(Box::new(std::io::Error::other(
            "synthetic session mismatch",
        ))),
    )));
    assert_eq!(
        get_api_key(app, "groq_api_key").as_deref(),
        Some("synthetic-groq-key")
    );
    credential("groq_api_key").set_error(keyring::Error::NoStorageAccess(Box::new(
        std::io::Error::other("synthetic denied delete"),
    )));
    assert!(clear_secret(app, "groq_api_key").is_err());
    assert_eq!(
        get_api_key(app, "groq_api_key").as_deref(),
        Some("synthetic-groq-key")
    );
    clear_api_key(app, "groq_api_key").unwrap();
    crate::commands::licensing::native_tests::email_code_session_lifecycle(
        app,
        |key| {
            credential(key).set_error(keyring::Error::NoStorageAccess(Box::new(
                std::io::Error::other("synthetic locked wallet"),
            )));
        },
        |key| {
            failed_writes.lock().unwrap().insert(key.into(), 1);
        },
    );
    persist_auth_session_material(app, "synthetic-old-access", "synthetic-old-refresh").unwrap();
    failed_writes
        .lock()
        .unwrap()
        .insert(AUTH_SESSION_REFRESH_TOKEN_KEY.into(), 1);
    assert!(
        persist_auth_session_material(app, "synthetic-new-access", "synthetic-new-refresh")
            .is_err()
    );
    let session = load_auth_session_material(app).unwrap();
    assert_eq!(session.access_token, "synthetic-old-access");
    assert_eq!(session.refresh_token, "synthetic-old-refresh");
    clear_auth_session_material(app).unwrap();
    failed_writes
        .lock()
        .unwrap()
        .insert(AUTH_SESSION_REFRESH_TOKEN_KEY.into(), 1);
    assert!(
        persist_auth_session_material(app, "synthetic-new-access", "synthetic-new-refresh")
            .is_err()
    );
    assert!(load_auth_session_material(app).is_none());
    persist_auth_session_material(app, "synthetic-old-access", "synthetic-old-refresh").unwrap();
    failed_writes
        .lock()
        .unwrap()
        .insert(AUTH_SESSION_ACCESS_TOKEN_KEY.into(), 2);
    failed_writes
        .lock()
        .unwrap()
        .insert(AUTH_SESSION_REFRESH_TOKEN_KEY.into(), 1);
    assert!(
        persist_auth_session_material(app, "synthetic-new-access", "synthetic-new-refresh")
            .is_err()
    );
    clear_auth_session_material(app).unwrap();
    keyring::set_default_credential_builder(keyring::default::default_credential_builder());
}
