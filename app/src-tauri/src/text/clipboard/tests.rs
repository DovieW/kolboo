use super::*;

#[test]
fn confirmed_write_needs_no_delay() {
    verify_clipboard_text(
        "new",
        || Some("new".into()),
        |_| panic!("unnecessary delay"),
    )
    .unwrap();
}

#[test]
fn retries_busy_and_stale_reads_but_only_accepts_the_current_text() {
    let mut reads = [None, Some("old".into()), Some("new".into())].into_iter();
    let mut waits = Vec::new();
    verify_clipboard_text("new", || reads.next().unwrap(), |delay| waits.push(delay)).unwrap();
    assert_eq!(waits, vec![Duration::from_millis(20); 2]);
}

#[test]
fn unverified_write_fails_closed_with_bounded_waits_and_no_content_in_error() {
    for value in [None, Some("previous private text".to_string())] {
        let mut reads = 0;
        let mut waits = 0;
        let error = verify_clipboard_text(
            "new private text",
            || {
                reads += 1;
                value.clone()
            },
            |_| waits += 1,
        )
        .unwrap_err();
        assert_eq!(reads, 10);
        assert_eq!(waits, 9);
        assert!(error.contains("could not be verified"));
        assert!(!error.contains("private text"));
    }
}
