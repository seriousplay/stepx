mod app;
pub mod clipboard;
mod cursor_theme;
mod keyboard;
mod window_icon;
mod window_manager;
mod zbus;

pub use app::{maybe_register_xlib_error_hook, take_encountered_bad_match_from_dri3_fence_from_fd};
pub use clipboard::*;
pub use cursor_theme::*;
pub use keyboard::LinuxWebviewKey;
pub(crate) use window_icon::icon_from_png;
pub(crate) use window_manager::*;
pub use zbus::*;

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use winit::event_loop::{ActiveEventLoop, ControlFlow};

/// How often the Harness GTK/WebKit child still needs glib polled beside winit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinuxIdleNeed {
    /// No child WebView.
    None,
    /// Visible workbench or still loading the loopback document.
    Interactive,
    /// Attached but parked (palette / other V2 page); keep mux alive.
    Background,
}

const LINUX_WEBVIEW_INTERACTIVE_PUMP: Duration = Duration::from_millis(16);
const LINUX_WEBVIEW_BACKGROUND_PUMP: Duration = Duration::from_millis(100);

static LINUX_IDLE_PUMP: OnceLock<fn() -> LinuxIdleNeed> = OnceLock::new();
static LINUX_AFTER_PRESENT: OnceLock<fn()> = OnceLock::new();
static LINUX_WEBVIEW_KEYBOARD: OnceLock<LinuxWebviewKeyboardHooks> = OnceLock::new();

/// Callbacks that forward parent-window IME and keys into an embedded WebView.
/// The GTK child never receives X keyboard focus, so fcitx stays on the winit XIC.
#[derive(Clone, Copy)]
pub struct LinuxWebviewKeyboardHooks {
    pub owns_keyboard: fn() -> bool,
    pub insert_text: fn(&str),
    pub key_event: fn(LinuxWebviewKey),
    pub ime_caret: fn() -> Option<(f32, f32, f32, f32)>,
}

/// Register a GTK/WebKit idle pump. The return value chooses the next wake.
pub fn register_linux_idle_pump(pump: fn() -> LinuxIdleNeed) {
    let _ = LINUX_IDLE_PUMP.set(pump);
}

/// Register a post-present hook so the Harness X11 child can restack after wgpu.
pub fn register_linux_after_present(hook: fn()) {
    let _ = LINUX_AFTER_PRESENT.set(hook);
}

/// Register keyboard/IME forwarding into the Harness child WebView.
pub fn register_linux_webview_keyboard(hooks: LinuxWebviewKeyboardHooks) {
    let _ = LINUX_WEBVIEW_KEYBOARD.set(hooks);
}

pub(crate) fn poll_linux_idle(window_target: &ActiveEventLoop) {
    let Some(pump) = LINUX_IDLE_PUMP.get() else {
        return;
    };
    let delay = match pump() {
        LinuxIdleNeed::None => return,
        LinuxIdleNeed::Interactive => LINUX_WEBVIEW_INTERACTIVE_PUMP,
        LinuxIdleNeed::Background => LINUX_WEBVIEW_BACKGROUND_PUMP,
    };
    window_target.set_control_flow(ControlFlow::WaitUntil(Instant::now() + delay));
}

pub(crate) fn notify_linux_after_present() {
    if let Some(hook) = LINUX_AFTER_PRESENT.get() {
        hook();
    }
}

pub(crate) fn linux_webview_owns_keyboard() -> bool {
    LINUX_WEBVIEW_KEYBOARD
        .get()
        .is_some_and(|hooks| (hooks.owns_keyboard)())
}

pub(crate) fn linux_webview_insert_text(text: &str) {
    if let Some(hooks) = LINUX_WEBVIEW_KEYBOARD.get() {
        (hooks.insert_text)(text);
    }
}

pub(crate) fn linux_webview_key_event(key: LinuxWebviewKey) {
    if let Some(hooks) = LINUX_WEBVIEW_KEYBOARD.get() {
        (hooks.key_event)(key);
    }
}

pub(crate) fn linux_webview_ime_caret() -> Option<(f32, f32, f32, f32)> {
    LINUX_WEBVIEW_KEYBOARD
        .get()
        .and_then(|hooks| (hooks.ime_caret)())
}
