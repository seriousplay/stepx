use crate::platform::SystemTheme;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::windowing::winit::app::CustomEvent;

const SYSTEM_THEME_SUBKEY_PATH: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const LIGHT_MODE_SUBKEY_NAME: &str = "AppsUseLightTheme";

pub fn get_system_theme() -> Result<SystemTheme, std::io::Error> {
    let theme_subkey = RegKey::predef(HKEY_CURRENT_USER).open_subkey(SYSTEM_THEME_SUBKEY_PATH)?;
    let theme_value = theme_subkey.get_value::<u32, _>(LIGHT_MODE_SUBKEY_NAME)?;
    super::super::theme::windows_apps_use_light_theme_value(theme_value).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("System theme value {theme_value:?} was invalid"),
        )
    })
}

#[allow(dead_code)]
pub struct SystemThemeWatcher {
    thread: Option<std::thread::JoinHandle<()>>,
}

pub fn watch_system_theme_changes(
    event_proxy: winit::event_loop::EventLoopProxy<CustomEvent>,
) -> SystemThemeWatcher {
    let thread = std::thread::spawn(move || {
        use windows::Win32::Foundation::WIN32_ERROR;
        use windows::Win32::System::Registry::{
            RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
            REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_FILTER,
        };

        let subkey_wide: Vec<u16> = SYSTEM_THEME_SUBKEY_PATH
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut h_key: HKEY = HKEY::default();
        let open_result = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR(subkey_wide.as_ptr()),
                None,
                KEY_READ,
                &mut h_key,
            )
        };

        if open_result != WIN32_ERROR(0) {
            log::warn!("Failed to open registry key for theme watching: {open_result:?}");
            return;
        }

        loop {
            let notify_result = unsafe {
                RegNotifyChangeKeyValue(
                    h_key,
                    true,
                    REG_NOTIFY_FILTER(REG_NOTIFY_CHANGE_LAST_SET.0),
                    None,
                    false,
                )
            };

            if notify_result != WIN32_ERROR(0) {
                log::warn!("RegNotifyChangeKeyValue failed: {notify_result:?}");
                break;
            }

            let _ = event_proxy.send_event(CustomEvent::SystemThemeChanged);
            log::info!("Windows system theme change detected, sending SystemThemeChanged event");
        }

        unsafe {
            let _ = RegCloseKey(h_key);
        }
    });

    SystemThemeWatcher {
        thread: Some(thread),
    }
}
