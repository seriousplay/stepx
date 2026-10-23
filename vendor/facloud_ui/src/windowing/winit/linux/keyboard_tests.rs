use super::LinuxWebviewKey;

fn key(value: &str, ctrl: bool, shift: bool) -> LinuxWebviewKey {
    LinuxWebviewKey {
        key: value.into(),
        ctrl,
        shift,
        alt: false,
        meta: false,
    }
}

#[test]
fn clipboard_gestures_use_native_paste() {
    assert_eq!(key("v", true, false).paste_command(), Some("Paste"));
    assert_eq!(key("V", true, false).paste_command(), Some("Paste"));
    assert_eq!(key("insert", false, true).paste_command(), Some("Paste"));
    assert_eq!(
        key("v", true, true).paste_command(),
        Some("PasteAsPlainText")
    );
}

#[test]
fn typing_and_unrelated_shortcuts_are_not_paste() {
    for event in [
        key("v", false, false),
        key("insert", false, false),
        key("insert", true, true),
        key("c", true, false),
        LinuxWebviewKey {
            alt: true,
            ..key("v", true, false)
        },
        LinuxWebviewKey {
            meta: true,
            ..key("v", true, false)
        },
    ] {
        assert_eq!(event.paste_command(), None, "{event:?}");
    }
}
