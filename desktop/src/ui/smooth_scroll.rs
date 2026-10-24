//! Wheel easing only. Precision touchpads keep the OS's own momentum unchanged.
use super::*;
use facloud_ui::{animation::schedule_animation_tick, units::Pixels};
use std::rc::Rc;
#[derive(Default)]
pub(super) struct WheelMotion {
    from: f32,
    to: f32,
    expected: f32,
    start: Option<Instant>,
}
pub(super) type WheelHandle = Rc<RefCell<WheelMotion>>;
pub(super) struct SmoothScroll {
    pub child: widgets::El,
    pub scroll: ClippedScrollStateHandle,
    pub motion: WheelHandle,
    pub enabled: bool,
}
impl Element for SmoothScroll {
    fn layout(&mut self, c: SizeConstraint, ctx: &mut LayoutContext, app: &AppContext) -> Vector2F {
        let mut m = self.motion.borrow_mut();
        if let Some(start) = m.start {
            let current = self.scroll.scroll_start().as_f32();
            if !self.enabled || (current - m.expected).abs() > 1. {
                m.start = None;
            } else {
                let t = (start.elapsed().as_secs_f32() / 0.12).min(1.);
                let y = m.from + (m.to - m.from) * (1. - (1. - t).powi(3));
                self.scroll.scroll_to(Pixels::new(y));
                m.expected = y;
                if t >= 1. {
                    m.start = None;
                }
            }
        }
        drop(m);
        let size = self.child.layout(c, ctx, app);
        let mut m = self.motion.borrow_mut();
        let clamped = self.scroll.scroll_start().as_f32();
        if (clamped - m.expected).abs() > 1. {
            m.start = None;
        }
        size
    }
    fn after_layout(&mut self, c: &mut AfterLayoutContext, a: &AppContext) {
        self.child.after_layout(c, a)
    }
    fn paint(&mut self, o: Vector2F, c: &mut PaintContext, a: &AppContext) {
        self.child.paint(o, c, a);
        if self.motion.borrow().start.is_some() {
            schedule_animation_tick(c, 16);
        }
    }
    fn size(&self) -> Option<Vector2F> {
        self.child.size()
    }
    fn origin(&self) -> Option<Point> {
        self.child.origin()
    }
    fn dispatch_event(
        &mut self,
        e: &DispatchedEvent,
        c: &mut EventContext,
        a: &AppContext,
    ) -> bool {
        let before = self.scroll.scroll_start().as_f32();
        let handled = self.child.dispatch_event(e, c, a);
        let after = self.scroll.scroll_start().as_f32();
        if let Event::ScrollWheel { precise, .. } = e.raw_event() {
            let mut m = self.motion.borrow_mut();
            if *precise || !self.enabled {
                m.start = None;
            } else if (after - before).abs() > 0.1 {
                let remaining = if m.start.is_some() { m.to - before } else { 0. };
                let delta = after - before;
                // Reversing the wheel cancels old momentum immediately.
                let tail = if remaining.signum() == delta.signum() {
                    remaining
                } else {
                    0.
                };
                m.from = before;
                m.to = (after + tail).max(0.);
                m.expected = before;
                m.start = Some(Instant::now());
                self.scroll.scroll_to(Pixels::new(before));
                c.notify();
            }
        } else if matches!(e.raw_event(), Event::LeftMouseDown { .. }) {
            self.motion.borrow_mut().start = None;
        }
        handled
    }
}
impl Companion {
    pub(super) fn reveal_latest_chat(&self) {
        self.chat_tail_requested.set(true);
    }
    pub(super) fn chat_near_end(&self) -> bool {
        let (content, viewport) = self.chat_scroll_metrics.get();
        content - viewport - self.scroll.scroll_start().as_f32() <= 80.
    }
    #[track_caller]
    pub(super) fn chat_scroll(&self, content: widgets::El) -> widgets::El {
        let content = ChatExtent {
            child: content,
            metrics: self.chat_scroll_metrics.clone(),
            viewport: false,
            tail: None,
        }
        .finish();
        ChatExtent {
            child: self.chat_pane_scroll(content, false),
            metrics: self.chat_scroll_metrics.clone(),
            viewport: true,
            tail: Some((self.scroll.clone(), self.chat_tail_requested.clone(), self.selected_text.is_none())),
        }
        .finish()
    }
    // Overlay the scrollbar so messages and the pinned composer share one axis,
    // whether the conversation is empty, short or long enough to scroll.
    pub(super) fn chat_pane_scroll(&self, content: widgets::El, centered: bool) -> widgets::El {
        let p = self.p();
        let thumb = facloud_ui::color::ColorU::new(p.text3.r, p.text3.g, p.text3.b, 90);
        let active = facloud_ui::color::ColorU::new(p.text3.r, p.text3.g, p.text3.b, 160);
        let make = if centered { ClippedScrollable::vertical_centered } else { ClippedScrollable::vertical };
        self.ease_scroll(make(self.scroll.clone(), content, ScrollbarWidth::Auto, thumb.into(), active.into(), Fill::None)
            .with_overlayed_scrollbar().finish(), self.scroll.clone())
    }
    #[track_caller]
    pub(super) fn ease_scroll(
        &self,
        child: widgets::El,
        scroll: ClippedScrollStateHandle,
    ) -> widgets::El {
        let loc = std::panic::Location::caller();
        let handle = self
            .wheel_motions
            .borrow_mut()
            .entry((loc.file(), loc.line()))
            .or_default()
            .clone();
        SmoothScroll {
            child,
            scroll,
            motion: handle,
            enabled: !self.settings.value.reduced_motion,
        }
        .finish()
    }
}

struct ChatExtent {
    child: widgets::El,
    metrics: Rc<std::cell::Cell<(f32, f32)>>,
    viewport: bool,
    tail: Option<(ClippedScrollStateHandle, Rc<std::cell::Cell<bool>>, bool)>,
}

/// Pin only on entry or when a tail-following reader's layout grows/resizes.
/// An empty first layout stays at the tail when async history/images arrive.
fn tail_offset(old: (f32, f32), new: (f32, f32), offset: f32, requested: bool, follow: bool) -> Option<f32> {
    let changed = (old.0-new.0).abs() > 0.5 || (old.1-new.1).abs() > 0.5;
    (requested || (changed && follow && old.0-old.1-offset <= 80.))
        .then(|| (new.0-new.1).max(0.))
}
impl Element for ChatExtent {
    fn layout(&mut self, c: SizeConstraint, x: &mut LayoutContext, a: &AppContext) -> Vector2F {
        let old = self.metrics.get();
        let before = self.tail.as_ref().map(|(scroll, request, follow)|
            (scroll.scroll_start().as_f32(), request.replace(false), *follow));
        let size = self.child.layout(c, x, a);
        let (content, viewport) = self.metrics.get();
        self.metrics.set(if self.viewport {
            (content, size.y())
        } else {
            (size.y(), viewport)
        });
        if let (Some((scroll, _, _)), Some((offset, requested, follow))) = (&self.tail, before) {
            if let Some(target) = tail_offset(old, self.metrics.get(), offset, requested, follow) {
                scroll.scroll_to(Pixels::new(target));
            }
        }
        size
    }
    fn after_layout(&mut self, c: &mut AfterLayoutContext, a: &AppContext) {
        self.child.after_layout(c, a)
    }
    fn paint(&mut self, o: Vector2F, c: &mut PaintContext, a: &AppContext) {
        self.child.paint(o, c, a)
    }
    fn size(&self) -> Option<Vector2F> {
        self.child.size()
    }
    fn origin(&self) -> Option<Point> {
        self.child.origin()
    }
    fn dispatch_event(
        &mut self,
        e: &DispatchedEvent,
        c: &mut EventContext,
        a: &AppContext,
    ) -> bool {
        self.child.dispatch_event(e, c, a)
    }
}

#[cfg(test)]
mod tail_tests {
    use super::tail_offset;
    #[test]
    fn initial_async_history_and_late_images_follow_latest() {
        assert_eq!(tail_offset((0.,0.), (0.,500.), 0.,true,true),Some(0.));
        assert_eq!(tail_offset((0.,500.), (4000.,500.),0.,false,true),Some(3500.));
        assert_eq!(tail_offset((4000.,500.), (4500.,500.),3500.,false,true),Some(4000.));
    }
    #[test]
    fn manual_history_reading_and_selection_stay_put() {
        assert_eq!(tail_offset((4000.,500.), (4500.,500.),2000.,false,true),None);
        assert_eq!(tail_offset((4000.,500.), (4500.,500.),3500.,false,false),None);
        assert_eq!(tail_offset((4000.,500.), (4000.,500.),3450.,false,true),None);
    }
    #[test]
    fn mode_session_and_resize_land_on_latest() {
        assert_eq!(tail_offset((4000.,800.), (5000.,400.),0.,true,true),Some(4600.));
        assert_eq!(tail_offset((4000.,800.), (5000.,400.),3200.,false,true),Some(4600.));
        assert_eq!(tail_offset((5000.,400.), (4000.,800.),4600.,false,true),Some(3200.));
    }
}
