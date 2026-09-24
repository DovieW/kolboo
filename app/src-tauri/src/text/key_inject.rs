use enigo::{Direction, Enigo, Key, Keyboard};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasteShortcut {
    #[default]
    System,
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
    CmdV,
}

impl PasteShortcut {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "ctrl_v" => Some(Self::CtrlV),
            "ctrl_shift_v" => Some(Self::CtrlShiftV),
            "shift_insert" => Some(Self::ShiftInsert),
            "cmd_v" => Some(Self::CmdV),
            _ => None,
        }
    }

    fn keys(self) -> (&'static [Key], Key) {
        match self {
            Self::System => {
                #[cfg(target_os = "macos")]
                return Self::CmdV.keys();
                #[cfg(not(target_os = "macos"))]
                Self::CtrlV.keys()
            }
            Self::CtrlV => (&[Key::Control], Key::Unicode('v')),
            Self::CtrlShiftV => (&[Key::Control, Key::Shift], Key::Unicode('v')),
            Self::ShiftInsert => {
                // macOS has no Insert key in Enigo. Preserve the intent to paste
                // if this shortcut was carried over from another platform.
                #[cfg(target_os = "macos")]
                {
                    Self::CmdV.keys()
                }
                #[cfg(not(target_os = "macos"))]
                {
                    (&[Key::Shift], Key::Insert)
                }
            }
            Self::CmdV => (&[Key::Meta], Key::Unicode('v')),
        }
    }
}

/// Both clipboard modes use the same chord and modifier cleanup. The keyboard and
/// delay are injectable so failures can be tested without touching the desktop.
pub(crate) fn send_paste_shortcut(
    keyboard: &mut dyn Keyboard,
    shortcut: PasteShortcut,
    delay: &mut dyn FnMut(u64),
) -> Result<(), String> {
    let (modifiers, key) = shortcut.keys();
    let result = (|| {
        for modifier in modifiers {
            keyboard
                .key(*modifier, Direction::Press)
                .map_err(|e| e.to_string())?;
        }
        delay(50);
        #[cfg(target_os = "windows")]
        {
            // Preserve physical V injection on Windows (independent of layout).
            if key == Key::Unicode('v') {
                keyboard
                    .raw(0x2F, Direction::Press)
                    .map_err(|e| e.to_string())?;
                delay(30);
                keyboard
                    .raw(0x2F, Direction::Release)
                    .map_err(|e| e.to_string())?;
            } else {
                keyboard
                    .key(key, Direction::Click)
                    .map_err(|e| e.to_string())?;
            }
        }
        #[cfg(not(target_os = "windows"))]
        keyboard
            .key(key, Direction::Click)
            .map_err(|e| e.to_string())?;
        delay(50);
        Ok(())
    })();
    // Release even on a partial press failure. No stuck Ctrl/Shift after errors.
    for modifier in modifiers.iter().rev() {
        let _ = keyboard.key(*modifier, Direction::Release);
    }
    result
}

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

#[cfg(test)]
#[path = "tests/key_inject.rs"]
mod tests;
