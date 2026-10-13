use super::*;

#[test]
fn login_default_only_registers_new_installations_and_tolerates_os_failure() {
    let mut writes = 0;
    apply_login_default(false, || {
        writes += 1;
        Ok::<_, ()>(())
    });
    assert_eq!(
        writes, 0,
        "upgrades preserve both enabled and disabled OS state"
    );
    apply_login_default(true, || {
        writes += 1;
        Ok::<_, ()>(())
    });
    assert_eq!(writes, 1);
    apply_login_default(true, || {
        writes += 1;
        Err::<(), _>(())
    });
    assert_eq!(writes, 2, "a failed OS registration must not abort startup");
}

#[test]
fn first_run_is_visible_and_later_launches_follow_the_window_preference() {
    for guide in ["completed", "skipped", "unknown"] {
        assert!(!should_show_window(guide, false));
        assert!(should_show_window(guide, true));
    }
    assert!(should_show_window("pending", false));
    assert!(should_show_window("pending", true));
}
