#[cfg(any(test, target_os = "windows"))]
use crate::platform::SystemTheme;
use winit::event::{Event, WindowEvent};

use super::app::CustomEvent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AppearanceChangeSource {
    SystemThemeChangedEvent,
    WindowThemeChangedEvent,
}

pub(super) fn appearance_change_source(
    event: &Event<CustomEvent>,
) -> Option<AppearanceChangeSource> {
    match event {
        Event::UserEvent(CustomEvent::SystemThemeChanged) => {
            Some(AppearanceChangeSource::SystemThemeChangedEvent)
        }
        Event::WindowEvent {
            event: WindowEvent::ThemeChanged(_),
            ..
        } => Some(AppearanceChangeSource::WindowThemeChangedEvent),
        _ => None,
    }
}

#[cfg(any(test, target_os = "windows"))]
pub(super) fn windows_apps_use_light_theme_value(value: u32) -> Option<SystemTheme> {
    match value {
        1 => Some(SystemTheme::Light),
        0 => Some(SystemTheme::Dark),
        _ => None,
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
