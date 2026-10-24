//! Real native-window resize / restart regression, with an isolated preference file.
use super::*;
use stepx_companion::window_preferences::{Mode, Size};
#[derive(Default)]
struct Check {
    step: usize,
    last: Option<Instant>,
}
thread_local! { static CHECK: RefCell<Check> = RefCell::new(Check::default()); }
impl Companion {
    pub(super) fn verify_window_memory(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.demo_mode || !std::env::args().any(|a| a == "--verify-window-memory") {
            return;
        }
        let read = std::env::args().any(|a| a == "--window-read");
        CHECK.with(|cell| {
            let mut check = cell.borrow_mut();
            if check
                .last
                .is_some_and(|t| t.elapsed() < Duration::from_millis(1800))
            {
                return;
            }
            check.last = Some(Instant::now());
            let full = Size {
                width: 1240.,
                height: 780.,
            };
            let mini = Size {
                width: 640.,
                height: 820.,
            };
            let assert_size = |ctx: &mut ViewContext<Self>, size: Size| {
                let b = ctx.window_bounds(&ctx.window_id()).unwrap();
                assert!(
                    (b.width() - size.width).abs() < 2. && (b.height() - size.height).abs() < 2.,
                    "actual {b:?} expected {size:?}"
                );
            };
            let resize = |ctx: &mut ViewContext<Self>, size: Size| {
                let b = ctx.window_bounds(&ctx.window_id()).unwrap();
                ctx.windows().set_window_bounds(
                    ctx.window_id(),
                    pathfinder_geometry::rect::RectF::new(
                        b.origin(),
                        vec2f(size.width, size.height),
                    ),
                );
            };
            if read {
                match check.step {
                    0 if self.compact_start_pending => return,
                    0 => {
                        assert_eq!(self.push.shell_mode, ShellMode::MiniNotify);
                        assert_size(ctx, mini);
                        self.set_compact_mode(ShellMode::Full, ctx);
                    }
                    1 => {
                        assert_size(ctx, full);
                        ctx.toggle_maximized_window();
                    }
                    2 => {
                        assert_eq!(self.window_memory.preferences.full, full);
                        ctx.toggle_maximized_window();
                    }
                    3 => {
                        assert_size(ctx, full);
                        self.set_compact_mode(ShellMode::MiniAi, ctx);
                    }
                    4 => {
                        assert_size(ctx, mini);
                        eprintln!("WINDOW_MEMORY_RESTART_PASS");
                        self.remember_window(ctx, true);
                        ctx.close_window();
                    }
                    _ => {}
                }
            } else {
                match check.step {
                    0 => resize(ctx, full),
                    1 => {
                        assert_size(ctx, full);
                        self.set_compact_mode(ShellMode::MiniAi, ctx);
                    }
                    2 => resize(ctx, mini),
                    3 => self.set_compact_mode(ShellMode::MiniNotify, ctx),
                    4 => {
                        assert_size(ctx, mini);
                        self.set_compact_mode(ShellMode::Full, ctx);
                    }
                    5 => {
                        assert_size(ctx, full);
                        self.set_compact_mode(ShellMode::MiniNotify, ctx);
                    }
                    6 => {
                        assert_size(ctx, mini);
                        self.remember_window(ctx, true);
                        assert_eq!(self.window_memory.preferences.mode, Mode::MiniNotify);
                        eprintln!("WINDOW_MEMORY_WRITE_PASS");
                        ctx.close_window();
                    }
                    _ => {}
                }
            }
            check.step += 1;
        });
    }
}
