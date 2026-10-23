use super::*;
use winit::event::WindowEvent;
use winit::window::{Theme, WindowId as WinitWindowId};

#[test]
fn detects_custom_system_theme_event_as_appearance_change() {
    let event = Event::UserEvent(CustomEvent::SystemThemeChanged);

    assert_eq!(
        appearance_change_source(&event),
        Some(AppearanceChangeSource::SystemThemeChangedEvent)
    );
}

#[test]
fn detects_winit_theme_changed_event_as_appearance_change() {
    let event = Event::WindowEvent {
        window_id: WinitWindowId::from(1_u64),
        event: WindowEvent::ThemeChanged(Theme::Dark),
    };

    assert_eq!(
        appearance_change_source(&event),
        Some(AppearanceChangeSource::WindowThemeChangedEvent)
    );
}

#[test]
fn ignores_non_theme_events_for_appearance_callback() {
    let event = Event::Suspended;

    assert_eq!(appearance_change_source(&event), None);
}

#[test]
fn maps_windows_registry_light_theme_values() {
    assert_eq!(
        windows_apps_use_light_theme_value(1),
        Some(SystemTheme::Light)
    );
    assert_eq!(
        windows_apps_use_light_theme_value(0),
        Some(SystemTheme::Dark)
    );
    assert_eq!(windows_apps_use_light_theme_value(2), None);
}
