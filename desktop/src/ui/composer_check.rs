//! Native layout / draft regression. Demo only; never sends a message or opens a picker.
use super::*;
use facloud_ui::TypedActionView;

#[derive(Default)]
struct Check {
    step: usize,
    last: Option<Instant>,
    draft: String,
    attachments: Vec<AmooAttachment>,
}
thread_local! { static CHECK: RefCell<Check> = RefCell::new(Check::default()); }

fn capture(ctx: &mut ViewContext<Companion>, path: PathBuf) {
    ctx.windows()
        .platform_window(ctx.window_id())
        .unwrap()
        .as_ctx()
        .request_frame_capture(Box::new(move |frame| {
            image::save_buffer(
                path,
                &frame.data,
                frame.width,
                frame.height,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }));
}

impl Companion {
    pub(super) fn verify_composer(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.demo_mode || !std::env::args().any(|a| a == "--verify-composer") {
            return;
        }
        let Some(directory) = self
            .capture
            .as_ref()
            .and_then(|p| p.parent())
            .map(PathBuf::from)
        else {
            return;
        };
        CHECK.with(|cell| {
            let mut c = cell.borrow_mut();
            if c.last.is_some_and(|t|t.elapsed()<Duration::from_millis(1200)) { return; }
            c.last = Some(Instant::now());
            match c.step {
                0 => {
                    c.attachments = self.amoo_attachments.clone();
                    self.amoo_attachments.clear();self.fields[F_AMOO].clear();
                    self.set_compact_mode(ShellMode::MiniAi,ctx);
                }
                1 => capture(ctx,directory.join("01-empty.png")),
                2 => {
                    self.amoo_attachments = c.attachments.clone();
                    if let Some(path) = std::env::var_os("STEPX_COMPOSER_IMAGE") {
                        self.amoo_attachments[0].local_path=Some(PathBuf::from(path));
                    }
                    self.handle_action(&Action::Focus(F_AMOO),ctx);
                    self.handle_action(&Action::Preedit("ni hao".into()),ctx);
                    assert!(self.fields[F_AMOO].text.is_empty());
                }
                3 => capture(ctx,directory.join("02-ime-attachments.png")),
                4 => {
                    self.handle_action(&Action::Insert("请分析这张图片，并整理附件里的要点。\n保留中文、标点和换行。".into()),ctx);
                    assert!(self.fields[F_AMOO].preedit.is_empty());
                    c.draft=self.fields[F_AMOO].text.clone();
                }
                5 => capture(ctx,directory.join("03-draft.png")),
                6 => {
                    self.handle_action(&Action::Insert("\n长草稿需要在框内滚动，发送按钮仍应可见。".repeat(20)),ctx);
                }
                7 => {
                    eprintln!("composer caret={:?} scroll={} bytes={}",ctx.element_position_by_id(format!("input-caret:{F_AMOO}")),self.amoo_ui.composer_scroll.scroll_start().as_f32(),self.fields[F_AMOO].text.len());
                    capture(ctx,directory.join("04-long-draft.png"));
                }
                8 => {
                    assert!(self.amoo_ui.composer_scroll.scroll_start().as_f32()>0.,"caret should scroll into view");
                    self.fields[F_AMOO].set(&c.draft);
                    self.amoo_ui.composer_scroll.scroll_to(facloud_ui::units::Pixels::new(0.));
                    c.attachments=self.amoo_attachments.clone();
                    self.set_compact_mode(ShellMode::MiniNotify,ctx);
                    self.set_compact_mode(ShellMode::Full,ctx);
                    self.handle_action(&Action::Page(Page::Amoo),ctx);
                }
                9 => {assert_eq!(self.fields[F_AMOO].text,c.draft);assert_eq!(self.amoo_attachments,c.attachments);capture(ctx,directory.join("05-full.png"));}
                10 => {
                    self.set_compact_mode(ShellMode::MiniAi,ctx);
                    self.theme_mode=theme::ThemeMode::Dark;
                }
                11 => capture(ctx,directory.join("06-dark.png")),
                12 => {
                    let bounds=ctx.window_bounds(&ctx.window_id()).unwrap();
                    ctx.windows().set_window_bounds(ctx.window_id(),pathfinder_geometry::rect::RectF::new(bounds.origin(),vec2f(460.,640.)));
                }
                13 => capture(ctx,directory.join("07-narrow.png")),
                14 => {
                    assert_eq!(self.fields[F_AMOO].text,c.draft);assert_eq!(self.amoo_attachments,c.attachments);
                    self.handle_action(&Action::AmooRemoveAttachment(0),ctx);
                    assert_eq!(self.amoo_attachments.len()+1,c.attachments.len());
                    assert_eq!(self.fields[F_AMOO].text,c.draft);
                    eprintln!("COMPOSER_CHECK passed: measured multiline layout, caret scrolling, IME commit, attachment removal, compact/full draft retention; no sends");
                    ctx.close_window();return;
                }
                _ => unreachable!(),
            }
            c.step+=1;ctx.notify();
        });
    }
}
