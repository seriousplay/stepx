use super::*;

#[test]
fn detects_windows_platform_from_cfg_values() {
    assert_eq!(
        platform_target_from_cfg(None, "windows"),
        PlatformTarget::Windows
    );
}

#[test]
fn wasm_target_family_takes_precedence_over_target_os() {
    assert_eq!(
        platform_target_from_cfg(Some("wasm"), "unknown"),
        PlatformTarget::Wasm
    );
}

#[test]
fn detects_current_host_platform_from_compile_cfg() {
    cfg_if::cfg_if! {
        if #[cfg(target_family = "wasm")] {
            let expected = PlatformTarget::Wasm;
        } else if #[cfg(target_os = "linux")] {
            let expected = PlatformTarget::Linux;
        } else if #[cfg(target_os = "macos")] {
            let expected = PlatformTarget::Mac;
        } else if #[cfg(target_os = "windows")] {
            let expected = PlatformTarget::Windows;
        } else {
            let expected = PlatformTarget::Other;
        }
    }

    assert_eq!(current_platform_target(), expected);
}
