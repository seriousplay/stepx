pub mod clipboard;
mod network;
mod registry;
mod system_caption_buttons;
mod window_attribute;
mod window_ext;
mod wm_settingchange;

pub use clipboard::*;
pub use network::*;
pub use registry::*;
pub use system_caption_buttons::*;
pub use window_attribute::*;
pub use window_ext::WindowExt;
pub use wm_settingchange::*;

use std::sync::OnceLock;

static WINDOWS_IDLE_PUMP: OnceLock<fn()> = OnceLock::new();
static WINDOWS_AFTER_PRESENT: OnceLock<fn()> = OnceLock::new();

/// Register a post-present restack for a native child HWND (Harness WebView2).
/// winit presents the DXGI swapchain after GPUI paint; the child must be
/// raised again on `AboutToWait` or it stays behind the new composition visual.
pub fn register_windows_idle_pump(pump: fn()) {
    let _ = WINDOWS_IDLE_PUMP.set(pump);
}

/// Mark that the next AboutToWait should raise the child HWND (no extra COM notify).
pub fn register_windows_after_present(hook: fn()) {
    let _ = WINDOWS_AFTER_PRESENT.set(hook);
}

pub(crate) fn poll_windows_idle() {
    if let Some(pump) = WINDOWS_IDLE_PUMP.get() {
        pump();
    }
}

pub(crate) fn notify_windows_after_present() {
    if let Some(hook) = WINDOWS_AFTER_PRESENT.get() {
        hook();
    }
}
