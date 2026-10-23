pub mod fonts;
pub mod platform;
pub mod rendering;
pub mod windowing;

// Re-export everything from the core crate.
pub use facloud_ui_core::*;

/// Parent handle for embedding a native child window (WebView) in the GPUI
/// window. Linux and Windows use winit; macOS uses AppKit instead of winit.
#[cfg(all(any(windows, target_os = "linux"), not(target_family = "wasm")))]
pub use windowing::winit::winit_window_for_id;

#[cfg(target_os = "linux")]
pub use windowing::winit::linux::{
    register_linux_after_present, register_linux_idle_pump, register_linux_webview_keyboard,
    LinuxIdleNeed, LinuxWebviewKey, LinuxWebviewKeyboardHooks,
};

#[cfg(windows)]
pub use windowing::winit::windows::{register_windows_after_present, register_windows_idle_pump};

#[cfg(target_os = "macos")]
pub use platform::mac::{register_macos_after_present, restack_nsview_front};

/// Parent NSView for embedding a child WKWebView in the GPUI window.
#[cfg(target_os = "macos")]
pub fn macos_nsview_for_id(
    app: &AppContext,
    window_id: WindowId,
) -> Option<std::ptr::NonNull<std::ffi::c_void>> {
    let platform = app.windows().platform_window(window_id)?;
    let window = platform.as_any().downcast_ref::<platform::mac::Window>()?;
    std::ptr::NonNull::new(window.ns_view_ptr())
}
