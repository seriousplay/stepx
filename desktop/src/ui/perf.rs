//! Opt-in timing without message contents, account details, or file paths.
use super::*;
#[derive(Default, serde::Serialize)]
struct Stat {
    count: u64,
    total_us: u64,
    max_us: u64,
    over_16ms: u64,
}
thread_local! {static STATS:RefCell<std::collections::BTreeMap<&'static str,Stat>>=Default::default();}
pub(super) struct Span(&'static str, Option<Instant>);
impl Span {
    pub fn new(name: &'static str) -> Self {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        Self(
            name,
            ENABLED
                .get_or_init(|| std::env::var_os("STEPX_PERF_TRACE").is_some())
                .then(Instant::now),
        )
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(at) = self.1 {
            let us = at.elapsed().as_micros() as u64;
            STATS.with(|s| {
                let mut s = s.borrow_mut();
                let v = s.entry(self.0).or_default();
                v.count += 1;
                v.total_us += us;
                v.max_us = v.max_us.max(us);
                v.over_16ms += u64::from(us > 16_000);
            });
        }
    }
}
pub(super) fn report() {
    STATS.with(|s| eprintln!("UI_PHASES {}", serde_json::to_string(&*s.borrow()).unwrap()));
}
pub(super) struct Measured(pub widgets::El);
impl Element for Measured {
    fn layout(&mut self, c: SizeConstraint, x: &mut LayoutContext, a: &AppContext) -> Vector2F {
        let _s = Span::new("layout");
        self.0.layout(c, x, a)
    }
    fn after_layout(&mut self, x: &mut AfterLayoutContext, a: &AppContext) {
        self.0.after_layout(x, a)
    }
    fn paint(&mut self, o: Vector2F, x: &mut PaintContext, a: &AppContext) {
        let _s = Span::new("paint");
        self.0.paint(o, x, a)
    }
    fn dispatch_event(
        &mut self,
        e: &DispatchedEvent,
        x: &mut EventContext,
        a: &AppContext,
    ) -> bool {
        let _s = Span::new("event");
        self.0.dispatch_event(e, x, a)
    }
    fn size(&self) -> Option<Vector2F> {
        self.0.size()
    }
    fn origin(&self) -> Option<Point> {
        self.0.origin()
    }
}
