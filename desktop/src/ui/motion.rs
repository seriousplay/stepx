//! Short, paint-only motion. No per-frame data fetch or hit-region rebuild.
use super::*;
use facloud_ui::animation::schedule_animation_tick;
use facloud_ui::color::ColorU;
use pathfinder_geometry::rect::RectF;
pub(super) fn enter(child: Box<dyn Element>, started: Instant, enabled: bool) -> Box<dyn Element> {
    if !enabled || started.elapsed().as_millis() >= 160 {
        return child;
    }
    Box::new(Entrance { child, started })
}
struct Entrance {
    child: Box<dyn Element>,
    started: Instant,
}
impl Element for Entrance {
    fn layout(&mut self, c: SizeConstraint, ctx: &mut LayoutContext, app: &AppContext) -> Vector2F {
        self.child.layout(c, ctx, app)
    }
    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app)
    }
    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        let snapshot = ctx.scene.layer_snapshot();
        self.child.paint(origin, ctx, app);
        let t = (self.started.elapsed().as_secs_f32() / 0.16).min(1.);
        if t < 1. {
            ctx.scene
                .apply_opacity_since(&snapshot, 0.75 + 0.25 * (1. - (1. - t).powi(3)));
            schedule_animation_tick(ctx, 16);
        }
    }
    fn dispatch_event(
        &mut self,
        e: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        self.child.dispatch_event(e, ctx, app)
    }
    fn size(&self) -> Option<Vector2F> {
        self.child.size()
    }
    fn origin(&self) -> Option<Point> {
        self.child.origin()
    }
}
struct LoadingLine {
    start: Instant,
    animate: bool,
    track: ColorU,
    ink: ColorU,
    size: Option<Vector2F>,
    origin: Option<Point>,
}
impl Element for LoadingLine {
    fn layout(&mut self, c: SizeConstraint, _: &mut LayoutContext, _: &AppContext) -> Vector2F {
        let size = vec2f(c.max.x().min(800.).max(c.min.x()), 3.);
        self.size = Some(size);
        size
    }
    fn after_layout(&mut self, _: &mut AfterLayoutContext, _: &AppContext) {}
    fn paint(&mut self, o: Vector2F, ctx: &mut PaintContext, _: &AppContext) {
        self.origin = Some(Point::from_vec2f(o, ctx.scene.z_index()));
        let size = self.size.unwrap();
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(o, size))
            .with_background(Fill::Solid(self.track));
        if self.animate {
            let t = (self.start.elapsed().as_secs_f32() / 1.4) % 1.;
            let x = (1. - (t * std::f32::consts::TAU).cos()) * 0.5 * (size.x() * 0.72);
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(
                    o + vec2f(x, 0.),
                    vec2f(size.x() * 0.28, 3.),
                ))
                .with_background(Fill::Solid(self.ink));
            schedule_animation_tick(ctx, 32);
        }
    }
    fn dispatch_event(
        &mut self,
        _: &DispatchedEvent,
        _: &mut EventContext,
        _: &AppContext,
    ) -> bool {
        false
    }
    fn size(&self) -> Option<Vector2F> {
        self.size
    }
    fn origin(&self) -> Option<Point> {
        self.origin
    }
}
impl Companion {
    pub(super) fn loading_line(&self) -> Box<dyn Element> {
        let p = self.p();
        Box::new(LoadingLine {
            start: self.job_started,
            animate: !self.settings.value.reduced_motion && !self.settings.hidden,
            track: p.border,
            ink: p.text3,
            size: None,
            origin: None,
        })
    }
    pub(super) fn loading_feedback(&self, body: Box<dyn Element>) -> Box<dyn Element> {
        // Loading never adds rows above the content. The fixed footer owns feedback.
        body
    }
    pub(super) fn footer_loading(&self) -> bool {
        self.initializing
            || self.busy
            || self.chat_waiting()
            || (matches!(self.page, Page::Amoo | Page::Work)
                && (self.work.running || self.studio_lab.handle.is_some()))
            || (self.amoo_ui.snapshot_job.is_some() && !self.amoo_gateway_online)
            || (matches!(self.page, Page::Notifications | Page::Sms)
                && self.push.syncing
                && !self.push.loaded)
    }
}

/// A bounded paint-only interpolation; no layout rebuild or polling animation.
struct UpdateProgress {
    from: f32,
    to: f32,
    at: Instant,
    animate: bool,
    track: ColorU,
    ink: ColorU,
    size: Option<Vector2F>,
}
impl Element for UpdateProgress {
    fn layout(&mut self, c: SizeConstraint, _: &mut LayoutContext, _: &AppContext) -> Vector2F {
        let size = vec2f(c.max.x().min(800.).max(c.min.x()), 5.);
        self.size = Some(size);
        size
    }
    fn after_layout(&mut self, _: &mut AfterLayoutContext, _: &AppContext) {}
    fn paint(&mut self, o: Vector2F, ctx: &mut PaintContext, _: &AppContext) {
        let size = self.size.unwrap();
        let t = if self.animate {
            (self.at.elapsed().as_secs_f32() / 0.24).min(1.)
        } else {
            1.
        };
        let f = self.from + (self.to - self.from) * (1. - (1. - t).powi(3));
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(o, size))
            .with_background(Fill::Solid(self.track));
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(o, vec2f(size.x() * f, 5.)))
            .with_background(Fill::Solid(self.ink));
        if t < 1. {
            schedule_animation_tick(ctx, 16);
        }
    }
    fn dispatch_event(
        &mut self,
        _: &DispatchedEvent,
        _: &mut EventContext,
        _: &AppContext,
    ) -> bool {
        false
    }
    fn size(&self) -> Option<Vector2F> {
        self.size
    }
    fn origin(&self) -> Option<Point> {
        None
    }
}
impl Companion {
    pub(super) fn update_progress(&self) -> Box<dyn Element> {
        let u = &self.desktop_update;
        let p = self.p();
        Box::new(UpdateProgress {
            from: u.fraction_from,
            to: u.fraction,
            at: u.fraction_at,
            animate: !self.settings.value.reduced_motion && !self.settings.hidden,
            track: p.border,
            ink: p.success,
            size: None,
        })
    }
}
