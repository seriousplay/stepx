//! Cross-platform frame pacing helpers.
//!
//! The UI should be event-driven when idle, but interactive motion should be
//! paced to the display that owns the window. These helpers keep that policy in
//! one place so macOS, Windows, and Linux use the same bounds.

use std::time::Duration;

pub const FALLBACK_REFRESH_RATE_HZ: f32 = 60.0;
const MIN_REFRESH_RATE_HZ: f32 = 30.0;
const MAX_REFRESH_RATE_HZ: f32 = 240.0;

pub fn normalize_refresh_rate_hz(refresh_rate_hz: Option<f32>) -> f32 {
    refresh_rate_hz
        .filter(|hz| hz.is_finite() && *hz > 0.0)
        .unwrap_or(FALLBACK_REFRESH_RATE_HZ)
        .clamp(MIN_REFRESH_RATE_HZ, MAX_REFRESH_RATE_HZ)
}

pub fn frame_interval_for_refresh_rate(refresh_rate_hz: Option<f32>) -> Duration {
    Duration::from_secs_f64(1.0 / normalize_refresh_rate_hz(refresh_rate_hz) as f64)
}

pub fn display_aligned_interval_for_period(
    period: Duration,
    refresh_rate_hz: Option<f32>,
) -> Duration {
    let frame_interval = frame_interval_for_refresh_rate(refresh_rate_hz);
    if period <= frame_interval {
        return frame_interval;
    }

    let frames = (period.as_secs_f64() / frame_interval.as_secs_f64())
        .ceil()
        .max(1.0);
    Duration::from_secs_f64(frame_interval.as_secs_f64() * frames)
}

pub fn frame_period_ms_for_refresh_rate(refresh_rate_hz: Option<f32>) -> u32 {
    let millis = frame_interval_for_refresh_rate(refresh_rate_hz).as_secs_f64() * 1000.0;
    millis.ceil().max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_refresh_rates_fall_back_to_60hz() {
        assert_eq!(normalize_refresh_rate_hz(None), FALLBACK_REFRESH_RATE_HZ);
        assert_eq!(
            normalize_refresh_rate_hz(Some(0.0)),
            FALLBACK_REFRESH_RATE_HZ
        );
        assert_eq!(
            normalize_refresh_rate_hz(Some(f32::NAN)),
            FALLBACK_REFRESH_RATE_HZ
        );
    }

    #[test]
    fn refresh_rates_are_clamped_to_reasonable_bounds() {
        assert_eq!(normalize_refresh_rate_hz(Some(1.0)), 30.0);
        assert_eq!(normalize_refresh_rate_hz(Some(60.0)), 60.0);
        assert_eq!(normalize_refresh_rate_hz(Some(120.0)), 120.0);
        assert_eq!(normalize_refresh_rate_hz(Some(1000.0)), 240.0);
    }

    #[test]
    fn frame_period_rounds_up_to_whole_milliseconds_for_timers() {
        assert_eq!(frame_period_ms_for_refresh_rate(Some(60.0)), 17);
        assert_eq!(frame_period_ms_for_refresh_rate(Some(120.0)), 9);
        assert_eq!(frame_period_ms_for_refresh_rate(Some(240.0)), 5);
    }

    #[test]
    fn display_aligned_interval_never_runs_faster_than_requested_period() {
        assert_duration_close(
            display_aligned_interval_for_period(Duration::from_millis(32), Some(120.0)),
            Duration::from_secs_f64(1.0 / 120.0 * 4.0),
        );
        assert_duration_close(
            display_aligned_interval_for_period(Duration::from_millis(16), Some(120.0)),
            Duration::from_secs_f64(1.0 / 120.0 * 2.0),
        );
        assert_duration_close(
            display_aligned_interval_for_period(Duration::from_millis(1), Some(120.0)),
            Duration::from_secs_f64(1.0 / 120.0),
        );
    }

    fn assert_duration_close(actual: Duration, expected: Duration) {
        assert!(
            actual.abs_diff(expected) <= Duration::from_nanos(1),
            "actual={actual:?} expected={expected:?}"
        );
    }
}
