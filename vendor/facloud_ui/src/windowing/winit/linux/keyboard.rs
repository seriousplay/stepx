/// A key FaCloud received while the Harness WebView should consume typing.
#[derive(Clone, Debug)]
pub struct LinuxWebviewKey {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl LinuxWebviewKey {
    /// Native WebKit paste command for a clipboard gesture, if any.
    /// Synthetic DOM key events cannot read clipboard image data.
    pub fn paste_command(&self) -> Option<&'static str> {
        if self.alt || self.meta {
            return None;
        }
        if self.ctrl && self.key.eq_ignore_ascii_case("v") {
            return Some(if self.shift {
                "PasteAsPlainText"
            } else {
                "Paste"
            });
        }
        if !self.ctrl && self.shift && self.key.eq_ignore_ascii_case("insert") {
            return Some("Paste");
        }
        None
    }
}

#[cfg(test)]
#[path = "keyboard_tests.rs"]
mod tests;
