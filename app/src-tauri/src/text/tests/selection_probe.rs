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
