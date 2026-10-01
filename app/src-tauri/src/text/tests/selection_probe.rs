use super::*;

#[test]
fn disabled_selection_probe_never_requests_keyboard_access_or_touches_clipboard() {
    assert_eq!(
        probe_selected_text_via_copy(ContextGrabMethod::None),
        Ok(None)
    );
    let mut forbidden = |_: &mut dyn FnMut() -> Result<(), String>| {
        panic!("disabled selection must not request copy approval")
    };
    assert_eq!(
        probe_selected_text_via_copy_impl(ContextGrabMethod::None, Some(&mut forbidden)),
        Ok(None)
    );
}

// Called only by the opted-in native harness, using its private Xvfb clipboard.
#[cfg(target_os = "linux")]
pub(crate) fn native_selection_probe_copy_transaction() {
    assert_eq!(
        std::env::var("KOLBOO_NATIVE_WINDOW_TEST").as_deref(),
        Ok("1")
    );
    let mut clipboard = Clipboard::new().unwrap();
    clipboard
        .set_text("unrelated private clipboard fixture")
        .unwrap();
    let mut denied = |_: &mut dyn FnMut() -> Result<(), String>| Err("denied".into());
    assert_eq!(
        probe_selected_text_via_copy_impl(ContextGrabMethod::CtrlC, Some(&mut denied)),
        Ok(None)
    );
    assert_eq!(
        clipboard.get_text().unwrap(),
        "unrelated private clipboard fixture"
    );

    // Even a malformed successful adapter cannot bypass preparation.
    let mut unprepared = |_: &mut dyn FnMut() -> Result<(), String>| Ok(());
    assert_eq!(
        probe_selected_text_via_copy_impl(ContextGrabMethod::CtrlC, Some(&mut unprepared)),
        Ok(None)
    );
    assert_eq!(
        clipboard.get_text().unwrap(),
        "unrelated private clipboard fixture"
    );

    let mut failed_after_prepare = |prepare: &mut dyn FnMut() -> Result<(), String>| {
        prepare()?;
        Err("delivery failed".into())
    };
    assert_eq!(
        probe_selected_text_via_copy_impl(
            ContextGrabMethod::CtrlC,
            Some(&mut failed_after_prepare)
        ),
        Ok(None)
    );
    assert_eq!(
        clipboard.get_text().unwrap(),
        "unrelated private clipboard fixture"
    );

    let mut approved = |prepare: &mut dyn FnMut() -> Result<(), String>| {
        prepare()?;
        clipboard
            .set_text("fresh selected fixture")
            .map_err(|error| error.to_string())
    };
    assert_eq!(
        probe_selected_text_via_copy_impl(ContextGrabMethod::CtrlC, Some(&mut approved)),
        Ok(Some("fresh selected fixture".into()))
    );
    assert_eq!(
        clipboard.get_text().unwrap(),
        "unrelated private clipboard fixture"
    );
}
