//! Opt-in local UI soak test. Never enabled by a normal desktop build.
use super::*;

#[derive(Default)]
struct Check {
    last_tick: Option<Instant>,
    last_switch: Option<Instant>,
    steps: usize,
    max_tick_ms: u128,
    hidden_elapsed: Option<u64>,
}
thread_local! { static CHECK: RefCell<Check> = RefCell::new(Check::default()); }

impl Companion {
    pub(super) fn verify_ui_performance(&mut self, ctx: &mut ViewContext<Self>) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        if self.demo_mode && std::env::args().any(|a| a == "--verify-ui-minimize") {
            self.verify_minimize(ctx);
            return;
        }
        if !self.demo_mode || !std::env::args().any(|a| a == "--verify-ui-performance") {
            return;
        }
        CHECK.with(|cell| {
            let mut check = cell.borrow_mut();
            let now = Instant::now();
            if let Some(last) = check.last_tick.replace(now) {
                check.max_tick_ms = check.max_tick_ms.max(now.duration_since(last).as_millis());
            }
            if self.page != Page::BackScreen {
                assert!(
                    !self.back_screen.preview_running,
                    "inactive module must not animate"
                );
                if let Some(previous) = check.hidden_elapsed {
                    assert_eq!(
                        previous, self.back_screen.elapsed_ms,
                        "hidden preview advanced"
                    );
                }
                check.hidden_elapsed = Some(self.back_screen.elapsed_ms);
            } else {
                check.hidden_elapsed = None;
            }
            if check
                .last_switch
                .is_some_and(|last| last.elapsed() < Duration::from_millis(750))
            {
                return;
            }
            let pages = [
                Page::BackScreen,
                Page::Overview,
                Page::Amoo,
                Page::Work,
                Page::Tools,
                Page::Files,
                Page::Apps,
                Page::Sms,
                Page::Phone,
                Page::Connect,
                Page::Account,
                Page::About,
            ];
            if check.steps == pages.len() * 4 {
                eprintln!(
                    "UI_PERFORMANCE {}",
                    serde_json::json!({"switches":check.steps,
                    "max_tick_gap_ms":check.max_tick_ms,"hidden_preview_stopped":true})
                );
                std::process::exit(if check.max_tick_ms < 2500 { 0 } else { 1 });
            }
            self.page = pages[check.steps % pages.len()];
            self.back_screen
                .set_preview_visible(self.page == Page::BackScreen);
            self.scroll = Default::default();
            self.scroll_aux = Default::default();
            check.hidden_elapsed = if self.page == Page::BackScreen {
                None
            } else {
                Some(self.back_screen.elapsed_ms)
            };
            check.last_switch = Some(now);
            check.steps += 1;
            eprintln!("UI_PERFORMANCE_PAGE {} {}", check.steps, self.page.title());
            ctx.notify();
        });
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    fn verify_minimize(&mut self, ctx: &mut ViewContext<Self>) {
        CHECK.with(|cell| {
            let mut check = cell.borrow_mut();
            if check
                .last_switch
                .is_some_and(|at| at.elapsed() < Duration::from_secs(2))
            {
                return;
            }
            let window =
                facloud_ui::windowing::winit::winit_window_for_id(ctx, ctx.window_id()).unwrap();
            match check.steps {
                0 => {
                    self.page = Page::BackScreen;
                    self.back_screen.set_preview_visible(true);
                    ctx.notify();
                }
                1 => {
                    assert!(self.back_screen.preview_running);
                    window.set_minimized(true);
                }
                2 => {
                    assert_eq!(window.is_minimized(), Some(true));
                    assert!(!self.back_screen.preview_running);
                    check.hidden_elapsed = Some(self.back_screen.elapsed_ms);
                }
                3 => {
                    assert_eq!(check.hidden_elapsed, Some(self.back_screen.elapsed_ms));
                    window.set_minimized(false);
                    window.focus_window();
                }
                4 => {
                    assert_eq!(window.is_minimized(), Some(false));
                    assert!(self.back_screen.preview_running);
                    assert_ne!(check.hidden_elapsed, Some(self.back_screen.elapsed_ms));
                    eprintln!("UI_MINIMIZE {{\"paused\":true,\"resumed\":true}}");
                    std::process::exit(0);
                }
                _ => unreachable!(),
            }
            check.steps += 1;
            check.last_switch = Some(Instant::now());
        });
    }
}
