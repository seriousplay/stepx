pub(crate) mod app;
pub mod delegate;
mod event_loop;
pub(crate) mod fonts;
#[cfg(target_os = "linux")]
pub mod linux;

mod notifications;
mod theme;
#[cfg(target_family = "wasm")]
pub mod wasm;

mod window;

#[cfg(target_os = "windows")]
pub mod windows;

use app::CustomEvent;
#[cfg(target_os = "linux")]
pub use app::WindowingSystem;
use event_loop::EventLoop;
#[cfg(target_os = "linux")]
pub use window::get_os_window_manager_name;
use window::Window;

#[cfg(any(windows, target_os = "linux"))]
use crate::{AppContext, WindowId};

/// Returns the winit window for `window_id` so a native child (WebView) can
/// parent itself to the GPUI surface. Linux and Windows use winit; macOS uses AppKit.
#[cfg(any(windows, target_os = "linux"))]
pub fn winit_window_for_id(
    app: &AppContext,
    window_id: WindowId,
) -> Option<std::sync::Arc<winit::window::Window>> {
    let platform = app.windows().platform_window(window_id)?;
    platform.as_any().downcast_ref::<Window>()?.winit_window()
}
