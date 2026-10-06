// Session ownership and cancellation invariants.
use super::*;

#[test]
fn late_login_or_refresh_cannot_restore_a_signed_out_or_replaced_session() {
    let owner = SessionOwner::new();
    let first = owner.begin();
    assert_eq!(owner.current(), first);
    let second = owner.begin();
    assert_eq!(
        owner
            .commit(first, || Ok("old token"))
            .unwrap_err()
            .code
            .as_deref(),
        Some("auth_operation_superseded")
    );
    assert_eq!(
        owner.commit(second, || Ok("new token")).unwrap(),
        "new token"
    );
    owner.invalidate_and_commit(|| Ok(())).unwrap();
    assert!(owner
        .commit::<()>(second, || panic!("late refresh committed"))
        .is_err());
    assert!(owner
        .commit(owner.current(), || Err::<(), _>(CommandError::new(
            "storage failure",
            "auth"
        )))
        .is_err());
}

#[test]
fn poisoned_commit_still_allows_logout_and_never_revives_old_work() {
    let owner = SessionOwner::new();
    let old = owner.begin();
    let _ =
        std::panic::catch_unwind(|| owner.commit::<()>(old, || panic!("synthetic storage panic")));
    let new = owner.begin();
    assert_eq!(owner.current(), new);
    assert_eq!(owner.commit(new, || Ok("recovered")).unwrap(), "recovered");
    owner.invalidate_and_commit(|| Ok(())).unwrap();
    assert!(owner
        .commit::<()>(old, || panic!("old token restored"))
        .is_err());
}
