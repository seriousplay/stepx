//! Opt-in native input harness; isolated preferences only, no automatic phone writes.
use super::*;
#[cfg(target_os = "linux")]
impl Companion {
    pub(super) fn verify_shell_details(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.demo_mode || !std::env::args().any(|a| a == "--verify-shell-details") {
            return;
        }
        let Some(dir) = std::env::var_os("STEPX_SHELL_CHECK_DIR").map(PathBuf::from) else {
            return;
        };
        assert!(std::env::var_os("STEPX_SETTINGS_FILE").is_some());
        let window =
            facloud_ui::windowing::winit::winit_window_for_id(ctx, ctx.window_id()).unwrap();
        let o = window.inner_position().unwrap();
        let expansion_buttons = [
            "exp-settings",
            "exp-type-photos",
            "exp-type-videos",
            "exp-type-documents",
            "exp-days-0",
            "exp-days-7",
            "exp-next",
            "exp-previous",
            "exp-cancel",
            "exp-enable",
        ]
        .into_iter()
        .map(|id| {
            (
                id,
                ctx.element_position_by_id(id.to_string())
                    .map(|r| vec![r.origin().x(), r.origin().y(), r.width(), r.height()]),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
        let data = serde_json::json!({"origin":[o.x,o.y],"scale":window.scale_factor(),"page":self.page.title(),"mode":format!("{:?}",self.push.shell_mode),"directory":self.directory,
            "selected_file":self.selected_file,"file_count":self.files.len(),"status":self.status,"busy":self.busy,
            "star_position":ctx.element_position_by_id("github-star-count".to_string()).map(|r|vec![r.origin().x(),r.origin().y(),r.width(),r.height()]),"stars":self.github_stars.count,"recording":self.shortcuts.recording,"shortcut":self.settings.value.compact_shortcut.label(),"shortcut_note":self.shortcuts.note,
            "draft_shortcut":self.settings.draft.compact_shortcut.label(),"dirty":self.settings_dirty(),"feedback":self.settings.feedback,
            "review":self.interaction.review.as_ref().map(|r| &r.title),"draft":self.fields[F_AMOO].text,"attachment_count":self.amoo_attachments.len(),
            "chat_offset":self.scroll.scroll_start().as_f32(), "chat_extent":self.chat_scroll_metrics.get(),
            "chat_session":self.amoo_session_idx,"chat_history":self.compact.history,
            "expansion_step":self.expansion.step,"expansion_settings":self.expansion.settings,
            "expansion_dirty":self.expansion.dirty(),"expansion_feedback":self.expansion.feedback,
            "expansion_draft":self.expansion.draft,"expansion_preview":self.expansion.preview,
            "expansion_buttons":expansion_buttons,
            "update_phase":format!("{:?}",self.desktop_update.phase),"update_open":self.desktop_update.open,
            "update_buttons":(["desktop-update-open","desktop-update-close","desktop-update-download","desktop-update-cancel","desktop-update-install","desktop-update-later","desktop-update-check"].into_iter().map(|id|(id,ctx.element_position_by_id(id.to_string()).map(|r|vec![r.origin().x(),r.origin().y(),r.width(),r.height()]))).collect::<std::collections::BTreeMap<_,_>>())});
        let _ = std::fs::write(dir.join("state.tmp"), data.to_string());
        let _ = std::fs::rename(dir.join("state.tmp"), dir.join("state.json"));
        if std::env::args().any(|a| a == "--verify-expansion-ui") {
            if std::fs::remove_file(dir.join("expansion-fixture")).is_ok() {
                self.page = Page::Expansion;
                self.expansion.snapshot.online = true;
                self.expansion.snapshot.status = serde_json::json!({"permission":true,"policy":{"enabled":true,"photos":true,"videos":true,"documents":false,"retentionDays":7,"desktopId":"test"}});
                self.expansion.config.folder = Some(std::env::temp_dir());
                self.expansion.snapshot.report = serde_json::json!({"checkedAt":1792832924763i64,"counts":{"photos":17,"videos":0,"documents":4},"candidateCount":0,"candidateBytes":0,"retainedCount":17,"writingCount":0,"excludedCount":0,"invalidCount":0,"nextEligibleAt":1792924243970i64});
                self.expansion.snapshot.phase = "17 个文件仍在保留期内，满期后自动迁移".into();
                ctx.notify();
            }
            if std::fs::remove_file(dir.join("expansion-preview")).is_ok() {
                let (tx, rx) = mpsc::channel();
                self.expansion.preview_pending = Some(rx);
                tx.send(Ok(self.expansion.snapshot.report.clone())).unwrap();
                self.expansion.feedback.clear();
                ctx.notify();
            }
        }
        if std::env::args().any(|a| a == "--verify-update-ui") {
            use desktop_update_state::Phase;
            if std::fs::remove_file(dir.join("update-available")).is_ok() {
                self.desktop_update.release = Some(stepx_companion::desktop_update::Release {
                    version: "0.2.20".into(),
                    tag: "v0.2.20".into(),
                    url: String::new(),
                    name: "fixture".into(),
                    size: 64_000_000,
                    sha256: String::new(),
                    checksum_url: None,
                    notes: "优化设备连接稳定性。\n修复文件传输与窗口交互。\n改进更新安装与回滚。"
                        .into(),
                    page: "https://github.com/xydzf520/stepx/releases".into(),
                    platform: stepx_companion::desktop_update::platform(),
                });
                self.desktop_update.phase = Phase::Available;
                ctx.notify();
            }
            if std::fs::remove_file(dir.join("update-downloading")).is_ok() {
                self.desktop_update.phase = Phase::Downloading;
                self.desktop_update.fraction_from = 0.5;
                self.desktop_update.fraction = 0.62;
                self.desktop_update.fraction_at = Instant::now();
                self.desktop_update.total = 64_000_000;
                self.desktop_update.received = 39_680_000;
                self.desktop_update.speed = 4_000_000;
                ctx.notify();
            }
            if std::fs::remove_file(dir.join("update-ready")).is_ok() {
                self.desktop_update.phase = Phase::Ready;
                ctx.notify();
            }
            if std::fs::remove_file(dir.join("update-failed")).is_ok() {
                self.desktop_update.phase = Phase::Failed;
                self.desktop_update.error = "下载校验失败，当前版本未改变。请重试。".into();
                ctx.notify();
            }
            if std::fs::remove_file(dir.join("update-compact")).is_ok() {
                self.set_compact_mode(ShellMode::MiniAi, ctx);
                ctx.notify();
            }
        }
        // Async-history fixture for native scrolling regression; demo + isolated settings only.
        if std::env::args().any(|a| a == "--verify-chat-tail") {
            let consume = |name: &str| std::fs::remove_file(dir.join(name)).is_ok();
            let items = || {
                (1..=40).map(|n| AmooChatItem::UserText {
                attachment: None,
                text: format!("历史消息 {n:02}：这是用于检查滚动位置的长会话。进入会话默认显示最新消息；向上翻阅时应当保持阅读位置。"),
            }).collect::<Vec<_>>()
            };
            if consume("tail-fixture") {
                self.amoo_sessions = (0..2)
                    .map(|n| AmooSessionItem {
                        session_key: format!("scroll-test-{n}"),
                        title: format!("滚动验证 {}", n + 1),
                        agent: "Amoo".into(),
                        status: "已完成".into(),
                        status_kind: 2,
                        meta: String::new(),
                        items: items(),
                    })
                    .collect();
                self.amoo_session_idx = 0;
                self.set_compact_mode(ShellMode::MiniAi, ctx);
                self.reveal_latest_chat();
                ctx.notify();
            }
            if consume("tail-clear") {
                self.amoo_sessions[self.amoo_session_idx].items.clear();
                self.reveal_latest_chat();
                ctx.notify();
            }
            if consume("tail-append") {
                self.amoo_sessions[self.amoo_session_idx]
                    .items
                    .extend(items());
                ctx.notify();
            }
        }
        if dir.join("capture").exists() {
            let _ = std::fs::remove_file(dir.join("capture"));
            let path = dir.join("native.png");
            ctx.windows()
                .platform_window(ctx.window_id())
                .unwrap()
                .as_ctx()
                .request_frame_capture(Box::new(move |f| {
                    image::save_buffer(path, &f.data, f.width, f.height, image::ColorType::Rgba8)
                        .unwrap();
                }));
            ctx.notify();
        }
        if dir.join("busy").exists() {
            self.busy = true;
            self.job_page = self.page;
            self.status = "正在读取目录…".into();
            ctx.notify();
        } else if self.status == "正在读取目录…" && dir.join("idle").exists() {
            self.busy = false;
            self.status = "就绪".into();
            ctx.notify();
        }
        if dir.join("stop").exists() {
            ctx.close_window();
        }
    }
}
