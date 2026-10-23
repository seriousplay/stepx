pub mod app;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod mac;
#[cfg(target_family = "wasm")]
pub mod wasm;
#[cfg(target_os = "windows")]
pub mod windows;

pub mod headless;

pub mod current {
    cfg_if::cfg_if! {
        if #[cfg(target_family = "wasm")] {
            pub use super::wasm::*;
        } else if #[cfg(target_os = "linux")] {
            pub use super::linux::*;
        } else if #[cfg(target_os = "macos")] {
            pub use super::mac::*;
        } else if #[cfg(target_os = "windows")] {
            pub use super::windows::*;
        } else {
            pub use facloud_ui_core::platform::test::*;
        }
    }
}

pub use facloud_ui_core::platform::*;

pub use app::AppBuilder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlatformTarget {
    Wasm,
    Linux,
    Mac,
    Windows,
    Other,
}

pub(crate) fn platform_target_from_cfg(
    target_family: Option<&str>,
    target_os: &str,
) -> PlatformTarget {
    if target_family == Some("wasm") {
        PlatformTarget::Wasm
    } else {
        match target_os {
            "linux" => PlatformTarget::Linux,
            "macos" => PlatformTarget::Mac,
            "windows" => PlatformTarget::Windows,
            _ => PlatformTarget::Other,
        }
    }
}

pub(crate) fn current_platform_target() -> PlatformTarget {
    let target_family = if cfg!(target_family = "wasm") {
        Some("wasm")
    } else {
        None
    };
    let target_os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "other"
    };
    platform_target_from_cfg(target_family, target_os)
}

/// Returns whether the current device is a mobile device with touch input.
///
/// This is a cross-platform wrapper around the platform-specific implementation.
pub fn is_mobile_device() -> bool {
    match current_platform_target() {
        PlatformTarget::Wasm => {
            #[cfg(target_family = "wasm")]
            {
                wasm::is_mobile_device()
            }
            #[cfg(not(target_family = "wasm"))]
            {
                false
            }
        }
        _ => false,
    }
}

/// A trait for accessing internal per-platform concrete implementations
/// through a wrapper type.
#[allow(dead_code)]
trait AsInnerMut<Inner: ?Sized> {
    fn as_inner_mut(&mut self) -> &mut Inner;
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
