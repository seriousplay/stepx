fn function_body<'a>(source: &'a str, signature: &str, next_signature: &str) -> &'a str {
    let start = source.find(signature).expect("function should exist");
    let end = source[start..]
        .find(next_signature)
        .map(|offset| start + offset)
        .expect("next function should exist");
    &source[start..end]
}

#[test]
fn high_frequency_winit_capability_queries_are_quiet() {
    let source = include_str!("delegate.rs");
    let close_ime = function_body(
        source,
        "fn close_ime_async(&self, _window_id: WindowId)",
        "fn is_ime_open(&self)",
    );
    let ime_open = function_body(
        source,
        "fn is_ime_open(&self)",
        "fn open_character_palette(&self)",
    );
    let accessibility = function_body(
        source,
        "fn set_accessibility_contents(&self, _content: accessibility::AccessibilityContent)",
        "fn register_global_shortcut(&self",
    );
    let screen_reader = function_body(
        source,
        "fn is_screen_reader_enabled(&self)",
        "fn microphone_access_state(&self)",
    );

    for body in [close_ime, ime_open, accessibility, screen_reader] {
        assert_eq!(body.contains("log::warn!"), false);
        assert_eq!(body.contains("log::error!"), false);
    }
}
