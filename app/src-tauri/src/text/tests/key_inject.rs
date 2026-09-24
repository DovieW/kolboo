use super::*;

#[derive(Default)]
struct KeyboardSpy {
    events: Vec<(Key, Direction)>,
    fail_at: Option<usize>,
}

impl Keyboard for KeyboardSpy {
    fn fast_text(&mut self, _: &str) -> enigo::InputResult<Option<()>> {
        panic!("paste must use a chord")
    }
    fn raw(&mut self, code: u16, direction: Direction) -> enigo::InputResult<()> {
        assert_eq!(code, 0x2F);
        self.key(Key::Unicode('v'), direction)
    }
    fn key(&mut self, key: Key, direction: Direction) -> enigo::InputResult<()> {
        self.events.push((key, direction));
        if self.fail_at == Some(self.events.len()) {
            Err(enigo::InputError::Simulate("injected failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn paste_chords_press_in_order_and_release_in_reverse() {
    let mut chords = vec![
        ("ctrl_v", vec![Key::Control], Key::Unicode('v')),
        (
            "ctrl_shift_v",
            vec![Key::Control, Key::Shift],
            Key::Unicode('v'),
        ),
        ("cmd_v", vec![Key::Meta], Key::Unicode('v')),
    ];
    #[cfg(target_os = "macos")]
    chords.push(("shift_insert", vec![Key::Meta], Key::Unicode('v')));
    #[cfg(not(target_os = "macos"))]
    chords.push(("shift_insert", vec![Key::Shift], Key::Insert));

    for (raw, modifiers, key) in chords {
        let mut keyboard = KeyboardSpy::default();
        let mut delays = vec![];
        send_paste_shortcut(
            &mut keyboard,
            PasteShortcut::parse(raw).unwrap(),
            &mut |ms| delays.push(ms),
        )
        .unwrap();
        let mut expected: Vec<_> = modifiers.iter().map(|k| (*k, Direction::Press)).collect();
        #[cfg(target_os = "windows")]
        if key == Key::Unicode('v') {
            expected.extend([(key, Direction::Press), (key, Direction::Release)]);
        } else {
            expected.push((key, Direction::Click));
        }
        #[cfg(not(target_os = "windows"))]
        expected.push((key, Direction::Click));
        expected.extend(modifiers.iter().rev().map(|k| (*k, Direction::Release)));
        assert_eq!(keyboard.events, expected, "{raw}");
        assert_eq!(delays.first(), Some(&50));
        assert_eq!(delays.last(), Some(&50));
    }
    assert_eq!(PasteShortcut::parse("invalid"), None);
    assert_eq!(PasteShortcut::parse("system"), Some(PasteShortcut::System));
    #[cfg(target_os = "macos")]
    assert_eq!(PasteShortcut::System.keys(), PasteShortcut::CmdV.keys());
    #[cfg(not(target_os = "macos"))]
    assert_eq!(PasteShortcut::System.keys(), PasteShortcut::CtrlV.keys());
}

#[test]
fn paste_failures_do_not_leave_modifiers_held_or_send_later_keys() {
    for fail_at in 1..=3 {
        let mut keyboard = KeyboardSpy {
            fail_at: Some(fail_at),
            ..Default::default()
        };
        let error =
            send_paste_shortcut(&mut keyboard, PasteShortcut::CtrlShiftV, &mut |_| {}).unwrap_err();
        assert!(error.contains("injected failure"));
        assert_eq!(
            &keyboard.events[fail_at..],
            &[
                (Key::Shift, Direction::Release),
                (Key::Control, Direction::Release)
            ]
        );
    }
}
