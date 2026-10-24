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
                super::perf::report();
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

#[derive(Default)]
struct CompactCheck {
    step: usize,
    last: Option<Instant>,
    bounds: Option<pathfinder_geometry::rect::RectF>,
}
thread_local! { static COMPACT_CHECK: RefCell<CompactCheck> = RefCell::new(CompactCheck::default()); }
impl Companion {
    pub(super) fn verify_compact_window(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.demo_mode || !std::env::args().any(|a| a == "--verify-compact") {
            return;
        }
        use facloud_ui::TypedActionView;
        COMPACT_CHECK.with(|c| {
            let mut check=c.borrow_mut();
            if check.last.is_some_and(|t|t.elapsed()<Duration::from_millis(1100)){return;}
            check.last=Some(Instant::now());
            let bounds=ctx.window_bounds(&ctx.window_id()).unwrap();
            match check.step {
                0=> {check.bounds=Some(bounds);self.page=Page::Overview;self.fields[F_AMOO].set("中文草稿保留");self.set_compact_mode(ShellMode::MiniAi,ctx);}
                1=> {assert!((bounds.width()-600.).abs()<4.,"compact width {bounds:?}");assert!(bounds.height()<=764.);assert_eq!(self.page,Page::Amoo);self.set_compact_mode(ShellMode::MiniNotify,ctx);}
                2=> {assert_eq!(self.page,Page::Notifications);assert_eq!(self.fields[F_AMOO].text,"中文草稿保留");assert!(self.compact.restore.is_some());self.handle_action(&Action::Page(Page::Studio),ctx);}
                3=> {assert_eq!(self.push.shell_mode,ShellMode::Full);assert_eq!(self.page,Page::Studio);let before=check.bounds.unwrap();assert!((bounds.width()-before.width()).abs()<4.);assert!((bounds.height()-before.height()).abs()<4.);self.set_compact_mode(ShellMode::MiniAi,ctx);}
                4=> {self.set_compact_mode(ShellMode::Full,ctx);}
                5=> {assert_eq!(self.page,Page::Studio);assert_eq!(self.fields[F_AMOO].text,"中文草稿保留");ctx.toggle_maximized_window();}
                6=> {self.set_compact_mode(ShellMode::MiniAi,ctx);}
                7=> {assert!((bounds.width()-600.).abs()<4.,"maximized -> compact {bounds:?}");self.set_compact_mode(ShellMode::Full,ctx);}
                _=> {
                    assert_eq!(ctx.windows().platform_window(ctx.window_id()).unwrap().fullscreen_state(),platform::FullscreenState::Maximized);
                    self.init_demo_push();
                    let ids:Vec<_>=self.push.snapshot.threads.iter().take(2).map(|t|t.thread_id.clone()).collect();
                    assert_eq!(ids.len(),2);
                    self.handle_push_action(&PushAction::SelectImThread(ids[0].clone()),ctx);
                    self.fields[F_IM_REPLY].set("给第一位联系人的草稿");
                    self.handle_push_action(&PushAction::SelectImThread(ids[1].clone()),ctx);
                    assert!(self.fields[F_IM_REPLY].text.is_empty());
                    self.handle_push_action(&PushAction::SelectImThread(ids[0].clone()),ctx);
                    assert_eq!(self.fields[F_IM_REPLY].text,"给第一位联系人的草稿");
                    self.demo_mode=false;self.selected=None;self.clear_device_state();
                    self.handle_push_action(&PushAction::Refresh,ctx);
                    assert!(!self.push.loaded);assert!(self.error);
                    self.fields[F_IM_REPLY].set("断线不得发送");
                    self.handle_push_action(&PushAction::SendImReply,ctx);
                    assert_eq!(self.fields[F_IM_REPLY].text,"断线不得发送");
                    self.handle_push_outcome(push_state::PushOutcome::Snapshot("old-device".into(),Ok(stepx_companion::push::demo_push_snapshot())),ctx);
                    assert!(self.push.snapshot.threads.is_empty());
                    eprintln!("COMPACT_WINDOW_CHECK passed: resize, tabs, page expansion, draft preservation, restore, maximized restore, per-thread drafts, offline guards, stale device results");ctx.close_window();
                }
            }
            check.step+=1;ctx.notify();
        });
    }
}

impl Companion {
    pub(super) fn text_test_fixture(&self)->Option<Box<dyn Element>> {
        if !self.demo_mode || !std::env::args().any(|a|a=="--verify-text-interaction") {return None;}
        let p=self.p();
        if std::env::args().any(|a|a=="--long-scroll") {
            let lines=(0..100).map(|i|self.line(format!("第 {i} 条消息，用于验证滚轮缓动与阅读位置保留"),16.,p.text)).collect();
            return Some(Container::new(ConstrainedBox::new(self.chat_scroll(self.vstack(12.,lines))).with_height(300.).finish()).with_uniform_padding(32.).finish());
        }
        Some(Container::new(self.vstack(24.,vec![
            self.heading("原生文字交互验证"),
            self.selectable_messages(self.vstack(16.,vec![
                Text::new("会话复制 ABC你好🌟 · 第二段",self.font,20.).with_color(p.text)
                    .with_saved_char_position(0,"test-msg-0".into()).with_saved_char_position(4,"test-msg-4".into()).finish(),
                self.work_rich_text("**格式化回复**\n\n中文正文可以选择。\n\n- 第一项\n- 第二项","selection-test"),
            ])),
            self.amoo_message_composer(true,false),
            self.input(F_WORK_PROMPT,"Work 输入框",None),
            self.speech_icon("test-speech".into(),"你好，这是电脑朗读测试。".into()),
        ])).with_uniform_padding(32.).with_background_color(p.bg).finish())
    }
    #[cfg(target_os="linux")]
    pub(super) fn verify_text_interaction(&mut self,ctx:&mut ViewContext<Self>) {
        if !self.demo_mode || !std::env::args().any(|a|a=="--verify-text-interaction") {return;}
        let Some(dir)=std::env::var_os("STEPX_TEXT_CHECK_DIR").map(PathBuf::from) else{return;};
        static INIT:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
        if !INIT.swap(true,std::sync::atomic::Ordering::SeqCst){self.fields[F_AMOO].set("你好 ABC世界🌟\n第二行文字");self.fields[F_WORK_PROMPT].set("Work 中文输入");self.focus=None;ctx.notify();}
        let window=facloud_ui::windowing::winit::winit_window_for_id(ctx,ctx.window_id()).unwrap();
        let origin=window.inner_position().unwrap();
        let mut positions=serde_json::Map::new();
        for id in ["test-msg-0","test-msg-4","input-caret:5"] {
            if let Some(r)=ctx.element_position_by_id(id.to_owned()){positions.insert(id.into(),serde_json::json!([r.origin().x(),r.origin().y(),r.width(),r.height()]));}
        }
        let data=serde_json::json!({"origin":[origin.x,origin.y],"scale":window.scale_factor(),"positions":positions,
            "context_open":self.context_menu.is_some(),"composer_scroll":self.amoo_ui.composer_scroll.scroll_start().as_f32(),"undo":self.fields[F_AMOO].undo.len(),"scroll":self.scroll.scroll_start().as_f32(),"near_end":self.chat_near_end(),"metrics":self.chat_scroll_metrics.get(),"selected":self.selected_text,"focus":self.focus,"amoo":self.fields[F_AMOO].text,"amoo_cursor":self.fields[F_AMOO].cursor,"amoo_range":self.fields[F_AMOO].selection_range().map(|r|vec![r.start,r.end]),"work":self.fields[F_WORK_PROMPT].text,
            "clipboard":ctx.clipboard().read().plain_text,"speech":self.speech.key,"status":self.status,"error":self.error});
        let _=std::fs::write(dir.join("state.tmp"),data.to_string());let _=std::fs::rename(dir.join("state.tmp"),dir.join("state.json"));
        if dir.join("capture").exists(){let _=std::fs::remove_file(dir.join("capture"));let path=dir.join("native-text.png");ctx.windows().platform_window(ctx.window_id()).unwrap().as_ctx().request_frame_capture(Box::new(move|f|{image::save_buffer(path,&f.data,f.width,f.height,image::ColorType::Rgba8).unwrap();}));ctx.notify();}
        if dir.join("stop").exists(){ctx.close_window();}
    }
}
