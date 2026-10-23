//! Time-driven animation primitives for the v2 design tokens.
//!
//! These types are *clocks*: they expose a `0..1` progress value derived from
//! wall-clock time. Callers (typically `Presenter::paint` impls) pair them
//! with the pure-function helpers in
//! [`app::ui_tokens::v2::motion`](../../../../app/src/ui_tokens.rs) such as
//! `pulse_alpha(phase)` or `slide_in_offset_opacity(progress)`.
//!
//! # View-side responsibilities
//!
//! `facloud_ui_core` exposes [`PaintContext::repaint_after`] as the low-level
//! "next-frame please" knob — any leaf [`Element`] can call it from inside
//! `paint()` and the windowing layer will schedule another repaint after the
//! given [`Duration`]. For animations, prefer [`schedule_animation_tick`]: it
//! aligns the requested period to the window's current display cadence so
//! macOS, Windows, and Linux do not drift against high-refresh screens. Lower-level
//! elements that call [`PaintContext::repaint_after`] directly should use
//! [`PaintContext::display_aligned_interval`] for the same reason.
//!
//! The helper [`schedule_animation_tick`] and the wrapper element
//! [`AnimationTick`] in this module turn that primitive into something a
//! v2 module can drop in next to a `Container` / `EventHandler` chain
//! without writing a full `Element` impl. Typical usage:
//!
//! ```ignore
//! // Owning view holds the phase clock so it survives across frames.
//! pub struct UnreadDotView {
//!     phase_clock: AnimatedPhase, // period: 1600 ms — design fc-pulse
//! }
//!
//! // In `View::render`:
//! let alpha_scale = pulse_alpha(self.phase_clock.phase()); // 0.55..=1.0
//! let dot = Container::new(Empty::new().finish())
//!     .with_background_color(base.scale_alpha(alpha_scale))
//!     .finish();
//! // Wrap the dot so every paint schedules the next ~60 fps frame.
//! AnimationTick::new(dot, FRAME_PERIOD_MS_60HZ).finish()
//! ```
//!
//! Both clocks here are pure data — they hold an `Instant` and a duration,
//! and recompute progress from `Instant::now()` on every read. They do not
//! own any presenter state, so they are safe to drop in transient
//! `Presenter::paint` contexts as long as the *owning struct* keeps them
//! alive across frames.

use std::any::Any;
use std::time::Duration;

use instant::Instant;
use pathfinder_geometry::vector::Vector2F;

use crate::elements::Point;
use crate::event::DispatchedEvent;
use crate::{
    AfterLayoutContext, AppContext, Element, EventContext, LayoutContext, PaintContext,
    SizeConstraint,
};

/// One-frame period at ~60 Hz, in milliseconds. Pair with
/// [`schedule_animation_tick`] / [`AnimationTick`] when the call site wants
/// smooth interpolation (pulse, breath, shimmer). The scheduler aligns this
/// period to the current display rather than assuming the display is 60 Hz.
/// For lower-cost CPU paths
/// (e.g. a slow status dot) callers can pass a larger period — anything in
/// the 16..100 ms range looks reasonable in practice.
pub const FRAME_PERIOD_MS_60HZ: u32 = 16;

/// One-frame period at ~30 Hz, in milliseconds. Matches the
/// `shimmering_text` element's repaint cadence. The actual timer is rounded up
/// to a whole number of display frames, so 120 Hz screens get stable pacing
/// without waking ambient animations every 8 ms.
pub const FRAME_PERIOD_MS_30HZ: u32 = 32;

/// Schedule the next repaint of the current paint pass `period_ms`
/// milliseconds from now.
///
/// Thin sugar over [`PaintContext::repaint_after`] — the latter already
/// deduplicates against any earlier-scheduled repaint, so it's safe to call
/// every paint, even from multiple sibling elements in the same tree.
///
/// Call this from inside an `Element::paint` impl (or via
/// [`AnimationTick`]) when the visual contents read a time-varying value
/// such as [`AnimatedPhase::phase`] or [`OneShotAnimation::progress`] —
/// without it the view stays frozen on whatever value was sampled at the
/// last unrelated repaint.
pub fn schedule_animation_tick(ctx: &mut PaintContext, period_ms: u32) {
    let requested_period = Duration::from_millis(period_ms.max(1) as u64);
    ctx.repaint_after(ctx.display_aligned_interval(requested_period));
}

/// Transparent wrapper element that, on every paint, schedules another
/// repaint `period_ms` milliseconds later. Useful when the owning view
/// renders a child whose contents depend on an [`AnimatedPhase`] / a
/// [`OneShotAnimation`] tick.
///
/// `AnimationTick` is layout-/event-transparent: it forwards every
/// `Element` method to its child, only adding the
/// [`schedule_animation_tick`] call on the paint path.
pub struct AnimationTick {
    child: Box<dyn Element>,
    period_ms: u32,
}

impl AnimationTick {
    /// Wrap `child` and schedule a repaint every `period_ms` milliseconds.
    /// `period_ms` is clamped to `>= 1`.
    pub fn new(child: impl Element + 'static, period_ms: u32) -> Self {
        Self {
            child: Box::new(child),
            period_ms: period_ms.max(1),
        }
    }

    /// Same as [`Self::new`] but takes an already-boxed child.
    pub fn from_boxed(child: Box<dyn Element>, period_ms: u32) -> Self {
        Self {
            child,
            period_ms: period_ms.max(1),
        }
    }

    /// The configured tick period in milliseconds (after clamping).
    pub fn period_ms(&self) -> u32 {
        self.period_ms
    }
}

impl Element for AnimationTick {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        self.child.layout(constraint, ctx, app)
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        schedule_animation_tick(ctx, self.period_ms);
        self.child.paint(origin, ctx, app);
    }

    fn size(&self) -> Option<Vector2F> {
        self.child.size()
    }

    fn origin(&self) -> Option<Point> {
        self.child.origin()
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        self.child.dispatch_event(event, ctx, app)
    }

    fn parent_data(&self) -> Option<&dyn Any> {
        self.child.parent_data()
    }
}

/// Always-running phase clock for periodic v2 animations (pulse, breath,
/// shimmer).
///
/// Wraps an `Instant` and a fixed period. Each call to [`phase`](Self::phase)
/// returns the current `0.0..1.0` cycle progress, wrapping at the period
/// boundary. Combine with `app::ui_tokens::v2::motion::pulse_alpha` to drive
/// the design-system breathing animation:
///
/// ```ignore
/// // In the owning presenter:
/// struct AlertDot { clock: AnimatedPhase, /* ... */ }
/// impl AlertDot {
///     pub fn new() -> Self {
///         Self { clock: AnimatedPhase::new(1600) /* design default */ }
///     }
/// }
///
/// // In `Presenter::paint`:
/// let alpha = pulse_alpha(self.clock.phase());
/// let color = base_color.with_alpha(alpha);
/// ```
///
/// The clock does **not** trigger redraws. The owning view is responsible
/// for scheduling the next paint (see the module-level "View-side
/// responsibilities" section).
#[derive(Debug, Clone)]
pub struct AnimatedPhase {
    period_ms: u32,
    started: Instant,
}

impl AnimatedPhase {
    /// Create a new phase clock with the given period in milliseconds.
    /// A `period_ms` of 0 is clamped to 1 to avoid division-by-zero on
    /// [`phase`](Self::phase).
    pub fn new(period_ms: u32) -> Self {
        Self {
            period_ms: period_ms.max(1),
            started: Instant::now(),
        }
    }

    /// The period of this clock, in milliseconds. Clamped to `>= 1` at
    /// construction.
    pub fn period_ms(&self) -> u32 {
        self.period_ms
    }

    /// Current cycle progress, `0.0..1.0`. Wraps at the period boundary.
    pub fn phase(&self) -> f32 {
        self.phase_at(Instant::now())
    }

    /// Test-only variant that lets callers feed a deterministic `now`.
    pub fn phase_at(&self, now: Instant) -> f32 {
        let elapsed_ms = now.saturating_duration_since(self.started).as_secs_f64() * 1000.0;
        let period_ms = self.period_ms as f64;
        let wrapped = elapsed_ms.rem_euclid(period_ms);
        (wrapped / period_ms) as f32
    }
}

/// One-shot entrance animation: progress ramps from 0.0 to 1.0 over
/// `duration_ms`, then stays at 1.0 forever.
///
/// Pairs with `app::ui_tokens::v2::motion::slide_in_offset_opacity(progress)`
/// to play a settle-in motion when a new item appears:
///
/// ```ignore
/// struct NotificationCard { entrance: OneShotAnimation }
/// impl NotificationCard {
///     pub fn new() -> Self {
///         Self { entrance: OneShotAnimation::new(280) /* design default */ }
///     }
/// }
///
/// // In `Presenter::paint`:
/// let (y_offset, opacity) = slide_in_offset_opacity(self.entrance.progress());
/// ```
///
/// Like [`AnimatedPhase`], this clock does not schedule its own redraws; the
/// owning view must repaint until [`finished`](Self::finished) returns true.
#[derive(Debug, Clone)]
pub struct OneShotAnimation {
    duration_ms: u32,
    started: Instant,
}

impl OneShotAnimation {
    /// Create a new one-shot animation that runs for `duration_ms`
    /// milliseconds. A `duration_ms` of 0 means the animation is already
    /// finished — `progress()` will return 1.0 immediately.
    pub fn new(duration_ms: u32) -> Self {
        Self {
            duration_ms,
            started: Instant::now(),
        }
    }

    /// Duration of the animation in milliseconds, as supplied at
    /// construction.
    pub fn duration_ms(&self) -> u32 {
        self.duration_ms
    }

    /// Current progress, clamped to `0.0..=1.0`. Returns 1.0 once the
    /// animation has settled.
    pub fn progress(&self) -> f32 {
        self.progress_at(Instant::now())
    }

    /// Test-only variant for deterministic timing.
    pub fn progress_at(&self, now: Instant) -> f32 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        let elapsed_ms = now.saturating_duration_since(self.started).as_secs_f64() * 1000.0;
        let progress = (elapsed_ms / self.duration_ms as f64).clamp(0.0, 1.0);
        progress as f32
    }

    /// True once `progress()` has reached 1.0. Callers can stop scheduling
    /// repaints when this returns `true`.
    pub fn finished(&self) -> bool {
        self.finished_at(Instant::now())
    }

    /// Test-only variant for deterministic timing.
    pub fn finished_at(&self, now: Instant) -> bool {
        if self.duration_ms == 0 {
            return true;
        }
        let elapsed_ms = now.saturating_duration_since(self.started).as_secs_f64() * 1000.0;
        elapsed_ms >= self.duration_ms as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn animated_phase_starts_near_zero() {
        let clock = AnimatedPhase::new(1000);
        // Phase read at construction time is essentially 0 (modulo a few
        // microseconds spent between `new` and `phase`).
        let p = clock.phase();
        assert!(
            p < 0.05,
            "phase right after construction should be ~0, got {p}"
        );
    }

    #[test]
    fn animated_phase_wraps_at_period_boundary() {
        let clock = AnimatedPhase::new(1000);
        let t0 = clock.started;
        // Exactly one period later -> wrapped phase is 0.
        let p = clock.phase_at(t0 + Duration::from_millis(1000));
        assert!(p < 1e-3, "phase at exact period should wrap to 0, got {p}");

        // Half a period later -> phase is 0.5.
        let p = clock.phase_at(t0 + Duration::from_millis(500));
        assert!(
            (p - 0.5).abs() < 1e-3,
            "phase at half period should be 0.5, got {p}"
        );

        // 2.25 periods later -> phase is 0.25.
        let p = clock.phase_at(t0 + Duration::from_millis(2250));
        assert!(
            (p - 0.25).abs() < 1e-3,
            "phase at 2.25 periods should be 0.25, got {p}"
        );
    }

    #[test]
    fn animated_phase_zero_period_is_clamped_to_one() {
        // Should not panic with divide-by-zero; period is forced to >= 1ms.
        let clock = AnimatedPhase::new(0);
        assert_eq!(clock.period_ms(), 1);
        let _ = clock.phase();
    }

    #[test]
    fn animated_phase_is_in_unit_interval() {
        let clock = AnimatedPhase::new(1600);
        let t0 = clock.started;
        for ms in (0..5000).step_by(37) {
            let p = clock.phase_at(t0 + Duration::from_millis(ms));
            assert!(
                (0.0..1.0).contains(&p),
                "phase out of [0,1): {p} at ms={ms}"
            );
        }
    }

    #[test]
    fn one_shot_progresses_then_settles() {
        let anim = OneShotAnimation::new(280);
        let t0 = anim.started;

        assert!(anim.progress_at(t0) < 1e-3);
        assert!(!anim.finished_at(t0));

        let mid = anim.progress_at(t0 + Duration::from_millis(140));
        assert!(
            (mid - 0.5).abs() < 1e-3,
            "mid progress should be 0.5, got {mid}"
        );
        assert!(!anim.finished_at(t0 + Duration::from_millis(140)));

        let end = anim.progress_at(t0 + Duration::from_millis(280));
        assert!(
            (end - 1.0).abs() < 1e-3,
            "end progress should be 1.0, got {end}"
        );
        assert!(anim.finished_at(t0 + Duration::from_millis(280)));

        // After the duration the progress stays clamped at 1.0.
        let after = anim.progress_at(t0 + Duration::from_millis(5_000));
        assert!(
            (after - 1.0).abs() < 1e-3,
            "post-end progress should stay at 1.0, got {after}"
        );
        assert!(anim.finished_at(t0 + Duration::from_millis(5_000)));
    }

    #[test]
    fn one_shot_zero_duration_is_immediately_finished() {
        let anim = OneShotAnimation::new(0);
        assert_eq!(anim.duration_ms(), 0);
        assert!((anim.progress() - 1.0).abs() < 1e-6);
        assert!(anim.finished());
    }
}
