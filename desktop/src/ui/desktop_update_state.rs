//! One startup check, explicit downloads, and a fixed-footer entry in both shells.
use super::{theme::*, widgets::*, *};
use stepx_companion::{desktop_update as updater, storage::format_bytes};
#[derive(Clone, Debug)]
pub(super) enum DesktopUpdateAction {
    Open,
    Close,
    Check,
    Download,
    Cancel,
    Install,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Phase {
    #[default]
    Idle,
    Checking,
    Current,
    Ahead,
    Available,
    Downloading,
    Verifying,
    Extracting,
    Ready,
    Preparing,
    Cancelled,
    Failed,
}
pub(super) struct DesktopUpdateState {
    pub phase: Phase,
    pub open: bool,
    pub release: Option<updater::Release>,
    pub error: String,
    pub received: u64,
    pub total: u64,
    pub speed: u64,
    pub checked: String,
    pub notice: String,
    pub ready: Option<PathBuf>,
    pub job: Option<updater::Job>,
    pub started: Instant,
    pub fraction_from: f32,
    pub fraction: f32,
    pub fraction_at: Instant,
}
impl Default for DesktopUpdateState {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            open: false,
            release: None,
            error: String::new(),
            received: 0,
            total: 0,
            speed: 0,
            checked: String::new(),
            notice: String::new(),
            ready: None,
            job: None,
            started: Instant::now(),
            fraction_from: 0.,
            fraction: 0.,
            fraction_at: Instant::now(),
        }
    }
}
impl DesktopUpdateState {
    pub fn label(&self) -> String {
        match self.phase {
            Phase::Available => format!(
                "检测到新版本 v{}",
                self.release.as_ref().map_or("", |r| r.version.as_str())
            ),
            Phase::Downloading => format!("下载更新 {}%", (self.fraction * 100.) as u8),
            Phase::Verifying => "正在校验更新".into(),
            Phase::Extracting => "正在准备更新".into(),
            Phase::Ready => "更新已准备好".into(),
            Phase::Preparing => "正在准备重启".into(),
            Phase::Checking => "正在检查更新".into(),
            Phase::Current => "已是最新版本".into(),
            Phase::Ahead => "当前版本领先于公开版".into(),
            Phase::Cancelled => "下载已取消".into(),
            Phase::Failed => "更新未完成".into(),
            Phase::Idle => "检查更新".into(),
        }
    }
    pub fn shown(&self) -> bool {
        matches!(
            self.phase,
            Phase::Available
                | Phase::Downloading
                | Phase::Verifying
                | Phase::Extracting
                | Phase::Ready
                | Phase::Preparing
                | Phase::Cancelled
                | Phase::Failed
        )
    }
}
impl Companion {
    pub(super) fn initialize_desktop_updates(&mut self, ctx: &mut ViewContext<Self>) {
        if self.demo_mode || self.capture.is_some() {
            return;
        }
        self.desktop_update.notice = updater::last_result().unwrap_or_default();
        if let Some((dir, release)) = updater::cached_ready() {
            self.desktop_update.ready = Some(dir);
            self.desktop_update.release = Some(release);
        }
        self.start_desktop_update_check();
        ctx.spawn(
            async {
                facloud_ui::r#async::Timer::after(Duration::from_secs(3)).await;
            },
            |me, _, ctx| {
                let _ = updater::acknowledge_startup();
                if let Some(note) = updater::last_result() {
                    me.desktop_update.notice = note;
                    ctx.notify();
                }
            },
        );
    }
    fn start_desktop_update_check(&mut self) {
        if self.desktop_update.job.is_some() {
            return;
        }
        self.desktop_update.error.clear();
        self.desktop_update.phase = Phase::Checking;
        match updater::check() {
            Ok(job) => self.desktop_update.job = Some(job),
            Err(e) => self.desktop_update_failure(e.to_string()),
        }
    }
    fn desktop_update_failure(&mut self, message: String) {
        self.desktop_update.phase = Phase::Failed;
        self.desktop_update.error = message;
    }
    pub(super) fn handle_desktop_update(
        &mut self,
        action: &DesktopUpdateAction,
        ctx: &mut ViewContext<Self>,
    ) {
        use DesktopUpdateAction as A;
        match action {
            A::Open => {
                self.desktop_update.open = true;
                self.desktop_update.started = Instant::now();
                if self.desktop_update.phase == Phase::Idle {
                    self.start_desktop_update_check();
                }
            }
            A::Close => self.desktop_update.open = false,
            A::Check => {
                self.desktop_update.open = true;
                self.desktop_update.started = Instant::now();
                self.start_desktop_update_check();
            }
            A::Download if self.desktop_update.job.is_none() => {
                if let Some(release) = &self.desktop_update.release {
                    match updater::download(release) {
                        Ok(job) => {
                            self.desktop_update.job = Some(job);
                            self.desktop_update.phase = Phase::Downloading;
                            self.desktop_update.error.clear();
                            self.desktop_update.ready = None;
                            self.desktop_update.fraction = 0.;
                            self.desktop_update.fraction_from = 0.;
                            self.desktop_update.received = 0;
                            self.desktop_update.total = release.size;
                        }
                        Err(e) => self.desktop_update_failure(e.to_string()),
                    }
                }
            }
            A::Cancel => {
                if let Some(job) = &self.desktop_update.job {
                    job.cancel();
                    self.desktop_update.error = "正在取消下载…".into();
                }
            }
            A::Install if self.desktop_update.phase == Phase::Ready => {
                let blocked = self.busy
                    || self.work.running
                    || self.chat_waiting()
                    || self.updates.busy
                    || self.updates.transfer.is_some()
                    || self.expansion.snapshot.active.is_some()
                    || self.expansion.pending.is_some()
                    || self.back_screen.companion_busy
                    || self.back_screen.generating
                    || self.back_screen.applying_native
                    || self.amoo_ui.gui_running
                    || self.settings_dirty()
                    || self.expansion.dirty()
                    || self.studio_config_dirty()
                    || [
                        F_AMOO,
                        F_WORK_PROMPT,
                        F_SMS_BODY,
                        F_PUSH_REPLY,
                        F_IM_REPLY,
                        F_TOAST_REPLY,
                        F_LAB_PROMPT,
                    ]
                    .iter()
                    .any(|i| !self.fields[*i].text.trim().is_empty())
                    || !self.amoo_attachments.is_empty()
                    || !self.work.attachments.is_empty();
                if blocked {
                    self.desktop_update.error="请先完成正在执行的任务，并发送或清空对话草稿、保存设置后再重启。安装包已保留。".into();
                } else if let Some(dir) = self.desktop_update.ready.clone() {
                    self.desktop_update.error.clear();
                    match updater::prepare(&dir) {
                        Ok(job) => {
                            self.desktop_update.job = Some(job);
                            self.desktop_update.phase = Phase::Preparing;
                        }
                        Err(e) => self.desktop_update_failure(e.to_string()),
                    }
                }
            }
            _ => {}
        }
        ctx.notify();
    }
    pub(super) fn poll_desktop_update(&mut self, ctx: &mut ViewContext<Self>) {
        while let Some(event) = self.desktop_update.job.as_ref().and_then(|j| j.poll()) {
            let terminal = !matches!(event, updater::Event::Progress { .. });
            match event {
                updater::Event::Release { release } => {
                    let cached_matches = self.desktop_update.ready.is_some()
                        && self.desktop_update.release.as_ref().is_some_and(|r| {
                            r.version == release.version
                                && r.sha256 == release.sha256
                                && r.url == release.url
                        });
                    self.desktop_update.checked = chrono::Local::now().format("%H:%M").to_string();
                    match stepx_companion::updates::compare_versions(
                        &release.version,
                        stepx_companion::build_info::VERSION,
                    ) {
                        Ok(std::cmp::Ordering::Greater) => {
                            self.desktop_update.phase = Phase::Available
                        }
                        Ok(std::cmp::Ordering::Equal) => self.desktop_update.phase = Phase::Current,
                        Ok(_) => self.desktop_update.phase = Phase::Ahead,
                        Err(e) => self.desktop_update_failure(e.to_string()),
                    }
                    if cached_matches && self.desktop_update.phase == Phase::Available {
                        self.desktop_update.phase = Phase::Ready;
                    } else {
                        self.desktop_update.ready = None;
                    }
                    self.desktop_update.release = Some(release);
                }
                updater::Event::Progress {
                    phase,
                    received,
                    total,
                    speed,
                } => {
                    let u = &mut self.desktop_update;
                    u.phase = match phase.as_str() {
                        "verify" => Phase::Verifying,
                        "extract" => Phase::Extracting,
                        _ => Phase::Downloading,
                    };
                    u.received = received;
                    u.total = total;
                    u.speed = speed;
                    u.fraction_from = u.fraction;
                    u.fraction = (received as f32 / total.max(1) as f32).clamp(0., 1.);
                    u.fraction_at = Instant::now();
                }
                updater::Event::Ready { job } => {
                    let _ = updater::remember_ready(&job);
                    self.desktop_update.ready = Some(job);
                    self.desktop_update.phase = Phase::Ready;
                    self.desktop_update.error.clear();
                }
                updater::Event::Prepared { job: _ } => {
                    self.remember_window(ctx, true);
                    self.cancel_backscreen();
                    self.stop_mirror();
                    self.close_mirror_controls(ctx);
                    ctx.close_window();
                }
                updater::Event::Cancelled => {
                    self.desktop_update.phase = Phase::Cancelled;
                    self.desktop_update.error.clear();
                }
                updater::Event::Error { message } => self.desktop_update_failure(message),
            }
            if terminal {
                self.desktop_update.job = None;
            }
            ctx.notify();
        }
    }
    pub(super) fn desktop_update_link(&self) -> El {
        if !self.desktop_update.shown() {
            return Empty::new().finish();
        }
        let p = self.p();
        let label = self.desktop_update.label();
        let icon = if self.desktop_update.phase == Phase::Ready {
            icons::UiIcon::Check
        } else {
            icons::UiIcon::Download
        };
        Hoverable::new(self.mouse_state("desktop-update-footer"), move |mouse| {
            Container::new(self.hstack(
                S1,
                vec![
                        self.ui_icon(icon, 13., p.text2),
                        Text::new(label.clone(), self.font, MICRO)
                            .with_color(p.text)
                            .with_line_height_ratio(1.)
                            .soft_wrap(false)
                            .with_saved_char_position(0, "desktop-update-open".into())
                            .finish(),
                    ],
            ))
            .with_horizontal_padding(S2)
            .with_vertical_padding(2.)
            .with_background_color(if mouse.is_mouse_over_element() {
                p.hover
            } else {
                p.bg
            })
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
            .finish()
        })
        .with_cursor(platform::Cursor::PointingHand)
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(Action::DesktopUpdate(DesktopUpdateAction::Open))
        })
        .finish()
    }
    pub(super) fn desktop_update_card(&self) -> El {
        let p = self.p();
        let mut parts = vec![
            self.between(
                self.heading("软件更新"),
                self.button(
                    "desktop-update-settings",
                    "检查更新",
                    Action::DesktopUpdate(DesktopUpdateAction::Check),
                    Btn::Secondary,
                ),
            ),
            self.text(self.desktop_update.label(), BODY, p.text2),
            self.text(
                "启动时自动检查 GitHub 正式版本；下载与安装由你确认。",
                CAPTION,
                p.text3,
            ),
        ];
        if !self.desktop_update.notice.is_empty() {
            parts.push(self.text(&self.desktop_update.notice, CAPTION, p.text3));
        }
        self.panel(parts)
    }
    pub(super) fn desktop_update_overlay(&self) -> Option<El> {
        if !self.desktop_update.open {
            return None;
        }
        let u = &self.desktop_update;
        let p = self.p();
        let a = |v| Action::DesktopUpdate(v);
        let mut parts = vec![
            self.between(
                self.heading("更新 StepX Companion"),
                self.button(
                    "desktop-update-close",
                    "收起",
                    a(DesktopUpdateAction::Close),
                    Btn::Secondary,
                ),
            ),
            self.line_w(u.label(), 22., p.text, facloud_ui::fonts::Weight::Semibold),
        ];
        if let Some(r) = &u.release {
            parts.push(self.text(
                format!(
                    "当前 v{}  →  GitHub v{}",
                    stepx_companion::build_info::VERSION,
                    r.version
                ),
                BODY,
                p.text2,
            ));
            if matches!(
                u.phase,
                Phase::Available | Phase::Current | Phase::Ahead | Phase::Cancelled | Phase::Failed
            ) {
                let notes = r
                    .notes
                    .lines()
                    .filter(|s| !s.trim().is_empty())
                    .take(6)
                    .map(|s| s.trim_start_matches(['#', ' ', '-']).to_owned())
                    .collect::<Vec<_>>()
                    .join("\n");
                parts.push(self.text(Self::truncate_chars(&notes, 480), CAPTION, p.text2));
            }
        }
        if matches!(
            u.phase,
            Phase::Downloading | Phase::Verifying | Phase::Extracting
        ) {
            parts.push(self.update_progress());
            parts.push(self.between(
                self.mono(
                    format!("{} / {}", format_bytes(u.received), format_bytes(u.total)),
                    CAPTION,
                    p.text2,
                ),
                self.mono(
                    if u.speed > 0 {
                        format!("{}/s", format_bytes(u.speed))
                    } else {
                        "SHA-256 · 安装包校验".into()
                    },
                    CAPTION,
                    p.text3,
                ),
            ));
            parts.push(self.text("可继续使用客户端，收起面板不会中断下载。", CAPTION, p.text3));
        }
        if matches!(u.phase, Phase::Checking | Phase::Preparing) {
            parts.push(self.loading_line());
        }
        if u.phase == Phase::Ready {
            parts.push(self.text("安装包已校验。点击下方按钮将关闭并重新打开客户端，更新后生效。设置与个人文件会保留。",BODY,p.text2));
        }
        if u.phase == Phase::Ahead {
            parts.push(self.text("本机版本较新，继续使用即可；不会降级安装。", BODY, p.text2));
        }
        if !u.error.is_empty() {
            parts.push(self.text(&u.error, CAPTION, p.danger));
        }
        parts.push(self.divider());
        let mut buttons = vec![];
        match u.phase {
            Phase::Available | Phase::Cancelled => buttons.push(self.button(
                "desktop-update-download",
                "下载更新",
                a(DesktopUpdateAction::Download),
                Btn::Primary,
            )),
            Phase::Failed
                if u.release
                    .as_ref()
                    .is_some_and(|r| updater::newer(r).unwrap_or(false)) =>
            {
                buttons.push(self.button(
                    "desktop-update-download",
                    "重新下载",
                    a(DesktopUpdateAction::Download),
                    Btn::Primary,
                ))
            }
            Phase::Downloading | Phase::Verifying | Phase::Extracting => buttons.push(self.button(
                "desktop-update-cancel",
                "取消下载",
                a(DesktopUpdateAction::Cancel),
                Btn::Secondary,
            )),
            Phase::Ready => buttons.push(self.button(
                "desktop-update-install",
                "安装并重启",
                a(DesktopUpdateAction::Install),
                Btn::Primary,
            )),
            Phase::Current | Phase::Ahead | Phase::Idle | Phase::Failed => {
                buttons.push(self.button(
                    "desktop-update-check",
                    "重新检查",
                    a(DesktopUpdateAction::Check),
                    Btn::Secondary,
                ))
            }
            _ => {}
        }
        buttons.push(self.button(
            "desktop-update-later",
            if u.phase == Phase::Ready {
                "稍后重启"
            } else {
                "稍后"
            },
            a(DesktopUpdateAction::Close),
            Btn::Secondary,
        ));
        parts.push(self.wrap(buttons));
        parts.push(
            self.link_muted(
                "desktop-update-release",
                "查看 GitHub 发布说明",
                Action::StudioOpenUrl(
                    u.release
                        .as_ref()
                        .map_or("https://github.com/xydzf520/stepx/releases".into(), |r| {
                            r.page.clone()
                        }),
                ),
            ),
        );
        if !u.checked.is_empty() {
            parts.push(self.line(
                format!("上次检查 {} · 正式版渠道", u.checked),
                MICRO,
                p.text3,
            ));
        }
        let sheet = Container::new(self.vstack(S3, parts))
            .with_uniform_padding(S5)
            .with_background_color(p.surface)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish();
        let sheet = motion::enter(sheet, u.started, !self.settings.value.reduced_motion);
        Some(
            Container::new(
                Align::new(ConstrainedBox::new(sheet).with_max_width(510.).finish())
                    .bottom_right()
                    .finish(),
            )
            .with_uniform_padding(S4)
            .with_padding_bottom(38.)
            .with_background_color(rgba(0, 0, 0, 45))
            .finish(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn footer_is_reserved_for_actionable_updates() {
        let mut state=DesktopUpdateState::default();
        for phase in [Phase::Idle,Phase::Checking,Phase::Current,Phase::Ahead] {
            state.phase=phase;assert!(!state.shown());
        }
        for phase in [Phase::Available,Phase::Downloading,Phase::Ready,Phase::Failed] {
            state.phase=phase;assert!(state.shown());
        }
        state.phase=Phase::Downloading;state.fraction=0.62;
        assert_eq!(state.label(),"下载更新 62%");
        state.phase=Phase::Ready;assert_eq!(state.label(),"更新已准备好");
    }
}
