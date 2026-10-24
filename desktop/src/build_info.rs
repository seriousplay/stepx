//! Build metadata embedded in the executable, never the app's launch time.
use std::sync::OnceLock;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMPILED_UNIX_SECONDS: &str = env!("STEPX_BUILD_UNIX_SECONDS");

pub fn compiled_at() -> &'static str {
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| {
        let seconds = COMPILED_UNIX_SECONDS
            .parse::<i64>()
            .expect("build.rs validates the embedded timestamp");
        chrono::DateTime::from_timestamp(seconds, 0)
            .expect("build.rs validates the timestamp range")
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 60 * 60).unwrap())
            .format("%Y-%m-%d %H:%M:%S UTC+8")
            .to_string()
    })
}

/// Compact build identifier, consistently expressed in China Standard Time.
pub fn compiled_id() -> &'static str {
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| {
        chrono::DateTime::from_timestamp(COMPILED_UNIX_SECONDS.parse().unwrap(), 0)
            .unwrap()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 60 * 60).unwrap())
            .format("%y%m%d%H%M%S")
            .to_string()
    })
}
