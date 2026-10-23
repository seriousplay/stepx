use pathfinder_geometry::vector::Vector2F;

use super::Point;
use crate::{
    event::DispatchedEvent, AfterLayoutContext, AppContext, Element, EventContext, LayoutContext,
    PaintContext, SizeConstraint,
};
use std::any::Any;

/// Wraps an arbitrary element and uniformly scales the alpha of every primitive
/// it emits during paint. Used to drive opacity-based fades (for example, a
/// `hover_progress()`-driven fade-in) on subtrees where adding a per-element
/// `with_opacity` API would be invasive.
///
/// The implementation snapshots the active scene's per-layer primitive counts
/// before the child paints, then walks the newly-added primitives afterward and
/// multiplies their alpha by `opacity` via [`Scene::apply_opacity_since`].
/// `opacity` is clamped to `[0, 1]`; a value of `1.0` is a fast path that
/// skips the snapshot walk entirely.
pub struct Opacity {
    child: Box<dyn Element>,
    opacity: f32,
}

impl Opacity {
    pub fn new(child: impl Element + 'static, opacity: f32) -> Self {
        Self {
            child: Box::new(child),
            opacity,
        }
    }

    pub fn from_boxed(child: Box<dyn Element>, opacity: f32) -> Self {
        Self { child, opacity }
    }
}

impl Element for Opacity {
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
        let opacity = self.opacity.clamp(0.0, 1.0);
        if (opacity - 1.0).abs() < f32::EPSILON {
            self.child.paint(origin, ctx, app);
            return;
        }
        let snapshot = ctx.scene.layer_snapshot();
        self.child.paint(origin, ctx, app);
        ctx.scene.apply_opacity_since(&snapshot, opacity);
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

    #[cfg(any(test, feature = "test-util"))]
    fn debug_text_content(&self) -> Option<String> {
        self.child.debug_text_content()
    }
}
