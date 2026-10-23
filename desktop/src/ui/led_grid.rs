//! A decorative matrix is one layout/hit region, not 391 interactive widgets.
use facloud_ui::{color::ColorU, elements::*, event::DispatchedEvent};
use pathfinder_geometry::{
    rect::RectF,
    vector::{vec2f, Vector2F},
};

pub(super) struct LedGrid {
    frame: Vec<u8>,
    dot: f32,
    gap: f32,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl LedGrid {
    pub fn new(frame: &[u8], dot: f32, gap: f32) -> Self {
        Self {
            frame: frame.to_vec(),
            dot,
            gap,
            size: None,
            origin: None,
        }
    }

    fn natural_size(&self) -> Vector2F {
        vec2f(
            17. * self.dot + 16. * self.gap,
            23. * self.dot + 22. * self.gap,
        )
    }
}

impl Element for LedGrid {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        _: &mut LayoutContext,
        _: &AppContext,
    ) -> Vector2F {
        let size = self.natural_size().max(constraint.min).min(constraint.max);
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, _: &mut AfterLayoutContext, _: &AppContext) {}

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, _: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        // Keep the entire thumbnail in its parent's hit layer so selecting a
        // preset still works, including the gaps between LEDs. Individual dots
        // have no actions and must not each rebuild the scene's spatial index.
        ctx.scene
            .draw_rect_with_hit_recording(RectF::new(origin, self.size.unwrap()));
        let radius = CornerRadius::with_all(Radius::Pixels((self.dot * 0.25).min(self.dot / 2.)));
        for y in 0..23 {
            for x in 0..17 {
                if stepx_companion::device::is_backscreen_corner(x, y) {
                    continue;
                }
                let brightness = self.frame.get(y * 17 + x).copied().unwrap_or(0);
                let color = match brightness {
                    191.. => ColorU::new(255, 255, 255, 255),
                    91..=190 => ColorU::new(161, 161, 170, 255),
                    _ => ColorU::new(39, 39, 46, 255),
                };
                let offset = vec2f(x as f32, y as f32) * (self.dot + self.gap);
                ctx.scene
                    .draw_rect_without_hit_recording(RectF::new(
                        origin + offset,
                        vec2f(self.dot, self.dot),
                    ))
                    .with_background(Fill::Solid(color))
                    .with_corner_radius(radius);
            }
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

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    #[test]
    fn malformed_layout_rejects_invalid_bounds_without_hanging_hit_index() {
        let mut scene = facloud_ui::scene::Scene::new(1., Default::default());
        for i in 0..400 {
            scene.draw_rect_with_hit_recording(RectF::new(
                vec2f((i % 20) as f32, (i / 20) as f32),
                vec2f(1., 1.),
            ));
        }
        let started = std::time::Instant::now();
        let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scene
                .draw_rect_with_hit_recording(RectF::new(vec2f(0., 0.), vec2f(f32::INFINITY, 23.)));
        }));
        assert!(rejected.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }
}
