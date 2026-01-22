use enigo::{Direction, Enigo, Key, Keyboard};

/// Press a modifier key, run some work, and always attempt to release the modifier afterwards.
///
/// This is used in multiple places where we inject key chords (copy/paste) and want to avoid
/// leaving modifiers "stuck" if something errors mid-injection.
pub(crate) fn with_pressed_key<T>(
    enigo: &mut Enigo,
    key: Key,
    work: impl FnOnce(&mut Enigo) -> Result<T, String>,
) -> Result<T, String> {
    enigo
        .key(key, Direction::Press)
        .map_err(|e| e.to_string())?;

    // Ensure we always release, even if `work` fails.
    let result = work(enigo);
    let _ = enigo.key(key, Direction::Release);
    result
}

pub(crate) fn release_common_modifiers_best_effort(enigo: &mut Enigo) {
    // If we ever miss a key-up (or a release fails), users can experience "stuck" modifiers.
    // Best-effort attempt to reset common modifiers.
    let _ = enigo.key(Key::Shift, Direction::Release);
    let _ = enigo.key(Key::Control, Direction::Release);
    let _ = enigo.key(Key::Alt, Direction::Release);
    let _ = enigo.key(Key::Meta, Direction::Release);
}
