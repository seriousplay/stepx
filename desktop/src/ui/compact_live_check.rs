//! Explicit, opt-in real-device acceptance driver. Uses production UI actions and workers.
//! File chooser selection alone is supplied by fixtures; never fabricates phone responses.
use super::*;
use facloud_ui::TypedActionView;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
struct Config {
    serial: String,
    directory: PathBuf,
    image: PathBuf,
    file: PathBuf,
    #[serde(default)]
    reply_thread: String,
    #[serde(default)]
    reply_text: String,
    #[serde(default)]
    resume_key: String,
    #[serde(default)]
    preview_only: bool,
    /// Read-only regression for a specific real conversation; no test messages.
    #[serde(default)]
    review_only: bool,
    #[serde(default)]
    expected_reply: String,
    #[serde(default)]
    image_review: bool,
    /// Read-only native tail positioning acceptance; never sends test prompts.
    #[serde(default)]
    tail_review: bool,
}
fn config() -> Option<Config> {
    if !std::env::args().any(|a| a == "--verify-compact-live") {
        return None;
    }
    let path = std::env::var_os("STEPX_COMPACT_CHECK")?;
    Some(
        serde_json::from_slice(&std::fs::read(path).expect("test config"))
            .expect("valid test config"),
    )
}
pub(super) fn attachment_selection(images: bool) -> Option<Vec<PathBuf>> {
    config().map(|c| vec![if images { c.image } else { c.file }])
}
#[derive(Default)]
struct Check {
    step: usize,
    last: Option<Instant>,
    since: Option<Instant>,
    key: String,
    attachments: Vec<AmooAttachment>,
    prompt: String,
    image_url: String,
    bounds: Option<pathfinder_geometry::rect::RectF>,
}
thread_local! { static CHECK: RefCell<Check> = RefCell::new(Check::default()); }
const DRAFT: &str = "待发送草稿：切换简洁与完整窗口后保留，不要自动发送。";
fn capture(ctx: &mut ViewContext<Companion>, path: PathBuf) {
    if let Some(window) = ctx.windows().platform_window(ctx.window_id()) {
        window
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
}
impl Companion {
    pub(super) fn verify_compact_live(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(c) = config() else {
            return;
        };
        assert!(!self.demo_mode, "real acceptance must never use demo data");
        CHECK.with(|cell| {
            let mut check = cell.borrow_mut();
            if check.step == 99 { return; }
            let since = *check.since.get_or_insert_with(Instant::now);
            if since.elapsed() > Duration::from_secs(480) {
                eprintln!("COMPACT_LIVE timeout step={} status={}", check.step, self.status);
                std::process::exit(2);
            }
            if check.last.is_some_and(|t| t.elapsed() < Duration::from_millis(1300)) { return; }
            check.last = Some(Instant::now());
            if self.initializing || self.selected.is_none() || self.busy { return; }
            assert_eq!(self.selected.as_ref().unwrap().serial, c.serial, "wrong phone");
            let key = self.amoo_sessions.get(self.amoo_session_idx).map(|s|s.session_key.clone()).unwrap_or_default();
            if c.tail_review {
                if self.amoo_sessions.get(self.amoo_session_idx).is_none_or(|s| s.items.len() < 2) { return; }
                let (content, viewport) = self.chat_scroll_metrics.get();
                if content <= viewport { return; }
                let gap = (content-viewport).max(0.)-self.scroll.scroll_start().as_f32();
                assert!(gap.abs() < 2., "real chat did not land on latest: gap={gap}");
                std::fs::create_dir_all(&c.directory).unwrap();
                let record=serde_json::json!({"step":check.step,"mode":format!("{:?}",self.push.shell_mode),"content":content,"viewport":viewport,"tail_gap":gap,"message_items":self.amoo_sessions[self.amoo_session_idx].items.len(),"reported_model":self.active_amoo_model_short()});
                std::fs::write(c.directory.join(format!("tail-{}.json",check.step)),record.to_string()).unwrap();
                match check.step {
                    0 => { assert_eq!(self.push.shell_mode, ShellMode::MiniAi); capture(ctx,c.directory.join("phone-compact-latest.png")); }
                    1 => self.set_compact_mode(ShellMode::Full,ctx),
                    2 => self.set_compact_mode(ShellMode::MiniAi,ctx),
                    _ => { eprintln!("COMPACT_TAIL_LIVE passed: initial and full/compact transitions; read-only phone history"); check.step=99;ctx.close_window();return; }
                }
                check.step+=1;
                ctx.notify();
                return;
            }
            let replies: Vec<String> = self.amoo_sessions.iter().find(|s|s.session_key == check.key)
                .map(|s|s.items.iter().filter_map(|i| match i {
                    AmooChatItem::AssistantRich{text:Some(t),..} if !t.trim().is_empty() => Some(t.clone()), _=>None
                }).collect()).unwrap_or_default();
            match check.step {
                0 if c.review_only => {
                    let Some(idx)=self.amoo_sessions.iter().position(|s|s.session_key==c.resume_key) else {return;};
                    check.key=c.resume_key.clone();
                    self.handle_action(&Action::AmooSelectSession(idx),ctx);
                    self.set_compact_mode(ShellMode::MiniAi,ctx);
                    check.step=79;
                }
                80 => {
                    if !replies.iter().any(|r|r.contains(&c.expected_reply)) { return; }
                    assert!(!self.amoo_pending_reply.contains_key(&check.key));
                    assert!(self.fields[F_AMOO].text.is_empty());
                    // Capture the settled view before the next periodic read.
                    self.last_amoo_poll=Instant::now(); self.last_scan=Instant::now();
                    Self::scroll_to_end(&self.scroll);
                }
                81 => capture(ctx,c.directory.join("recovered-conversation.png")),
                82 if c.image_review => {
                    let images=pages::markdown_images(&replies.join("\n"));
                    let img=images.iter().find(|i|i.name=="京东首页").expect("real inline JD image missing");
                    if let Some(error)=self.work.preview_errors.get(&img.url) {panic!("automatic inline image failed: {error}");}
                    if !self.work.image_previews.contains_key(&img.url) { return; }
                    check.image_url=img.url.clone();
                }
                83 if c.image_review => capture(ctx,c.directory.join("inline-image-auto.png")),
                84 if c.image_review => {
                    self.fields[F_AMOO].set(DRAFT);
                    self.handle_action(&Action::AmooAttachFile(true),ctx);
                    check.attachments=self.amoo_attachments.clone();
                    check.bounds=ctx.window_bounds(&ctx.window_id());
                    self.handle_action(&Action::Work(WorkAction::OpenPreview(check.image_url.clone())),ctx);
                }
                85 if c.image_review => {
                    assert_eq!(self.push.shell_mode,ShellMode::MiniAi);
                    assert_eq!(ctx.window_bounds(&ctx.window_id()),check.bounds);
                    assert_eq!(self.work.image_viewer.as_ref(),Some(&check.image_url));
                    capture(ctx,c.directory.join("inline-image-viewer.png"));
                }
                86 if c.image_review => self.handle_action(&Action::Escape,ctx),
                87 if c.image_review => {
                    assert_eq!(self.push.shell_mode,ShellMode::MiniAi);
                    assert_eq!(ctx.window_bounds(&ctx.window_id()),check.bounds);
                    assert!(self.work.image_viewer.is_none());
                    assert_eq!(key,check.key);assert_eq!(self.fields[F_AMOO].text,DRAFT);
                    assert_eq!(self.amoo_attachments,check.attachments);
                    capture(ctx,c.directory.join("inline-image-return.png"));
                }
                88 if c.image_review => {
                    self.fields[F_AMOO].clear();self.amoo_attachments.clear();
                    self.work.image_previews.remove(&check.image_url);
                    self.handle_action(&Action::Work(WorkAction::PreviewAttachment {
                        name:"京东首页".into(),kind:"image".into(),mime:"image/png".into(),url:check.image_url.clone(),
                    }),ctx);
                }
                89 if c.image_review => {
                    if !self.work.image_previews.contains_key(&check.image_url) {return;}
                    assert_eq!(self.push.shell_mode,ShellMode::MiniAi);
                    eprintln!("AMOO_IMAGE_REVIEW passed: automatic inline image, compact viewer, unchanged window/session/draft/attachments, persistent cache reload; no message sent");
                    capture(ctx,c.directory.join("inline-image-final.png"));
                }
                90 if c.image_review => {
                    self.last_scan=Instant::now();self.last_amoo_poll=Instant::now();
                    self.begin_amoo_sync(Some(check.key.clone()),Vec::new(),false);
                    assert!(self.amoo_ui.snapshot_job.is_some() && !self.busy);
                    self.fields[F_AMOO].set(DRAFT);
                    self.handle_action(&Action::AmooAttachFile(false),ctx);
                    assert_eq!(self.amoo_attachments.len(),1);
                    assert_eq!(self.fields[F_AMOO].text,DRAFT);
                    assert!(!self.busy && self.amoo_pending_reply.is_empty());
                    capture(ctx,c.directory.join("passive-sync-composer.png"));
                    eprintln!("AMOO_PASSIVE_INTERACTION passed: active background sync does not lock composer or file attachment");
                }
                91 if c.image_review => {
                    if self.amoo_ui.snapshot_job.is_some() {return;}
                    assert_eq!(self.fields[F_AMOO].text,DRAFT);assert_eq!(self.amoo_attachments.len(),1);
                    self.fields[F_AMOO].clear();self.amoo_attachments.clear();
                    Self::scroll_to_end(&self.scroll);
                }
                92 if c.image_review => {
                    capture(ctx,c.directory.join("inline-image-final.png"));
                    check.step=99;return;
                }
                82 => {
                    eprintln!("AMOO_REVIEW passed: original conversation result displayed, no message sent, composer available");
                    check.step=99; return;
                }
                0 if c.preview_only => {
                    if !self.push.loaded || self.push.syncing {return;}
                    self.set_compact_mode(ShellMode::MiniNotify,ctx);
                    self.handle_push_action(&PushAction::SelectImThread(c.reply_thread.clone()),ctx);
                    self.fields[F_IM_REPLY].clear();check.step=89;
                }
                90 => {capture(ctx,c.directory.join("detail-final.png"));}
                91 => {eprintln!("COMPACT_PREVIEW completed without sending");ctx.close_window();return;}
                0 if !c.resume_key.is_empty() => {
                    let idx=self.amoo_sessions.iter().position(|s|s.session_key==c.resume_key).expect("prior test session missing");
                    check.key=c.resume_key.clone();
                    self.handle_action(&Action::AmooSelectSession(idx),ctx);
                    self.fields[F_AMOO].set(DRAFT);
                    self.handle_action(&Action::AmooAttachFile(true),ctx);
                    self.set_compact_mode(ShellMode::MiniNotify,ctx);
                    check.step=11;
                }
                0 => {
                    self.set_compact_mode(ShellMode::MiniAi, ctx);
                    if self.busy { return; }
                    self.handle_action(&Action::AmooNewChat, ctx);
                }
                1 => {
                    assert!(!key.is_empty());check.key=key.clone();
                    self.fields[F_AMOO].set("这是简洁模式的真实连接测试。请只回复：简洁模式对话通过");
                    self.handle_action(&Action::AmooSend, ctx);
                }
                2 => {
                    if !self.amoo_pending_reply.is_empty() || replies.is_empty() { return; }
                    assert!(replies.join("\n").contains("简洁模式对话通过"), "text reply: {replies:?}");
                    std::fs::write(c.directory.join("text-result.json"), serde_json::to_vec_pretty(&replies).unwrap()).unwrap();
                    capture(ctx,c.directory.join("amoo-text.png"));
                    self.handle_action(&Action::AmooAttachFile(true),ctx);
                    assert_eq!(self.amoo_attachments.len(),1,"image button: {}",self.status);
                    self.handle_action(&Action::AmooAttachFile(false),ctx);
                    assert_eq!(self.amoo_attachments.len(),2,"file button: {}",self.status);
                    check.attachments=self.amoo_attachments.clone();
                    check.prompt="请直接看附件图片，描述各图形的颜色、形状、数量以及图片上的编号；再读取随附 CSV，回答项目代号、三行数量及数量总和。只能根据附件回答，简短列出结果。".into();
                    self.fields[F_AMOO].set(&check.prompt);
                }
                3 => {
                    capture(ctx,c.directory.join("attachments-before.png"));
                    self.set_compact_mode(ShellMode::MiniNotify,ctx);
                }
                4 => {self.handle_action(&Action::Push(PushAction::SetShellMode(ShellMode::Full)),ctx);}
                5 => {self.handle_action(&Action::Page(Page::Work),ctx);}
                6 => {self.set_compact_mode(ShellMode::MiniAi,ctx);}
                7 => {
                    assert_eq!(key,check.key,"session changed while switching");
                    assert_eq!(self.fields[F_AMOO].text,check.prompt,"draft lost");
                    assert_eq!(self.amoo_attachments,check.attachments,"attachments lost");
                    capture(ctx,c.directory.join("attachments-after.png"));
                    self.handle_action(&Action::AmooSend,ctx);
                }
                8 => {
                    if !self.amoo_attachments.is_empty() {
                        if self.error {panic!("attachment send failed: {}",self.status);}
                        return;
                    }
                    self.fields[F_AMOO].set(DRAFT);
                    self.handle_action(&Action::AmooAttachFile(true),ctx);
                    assert_eq!(self.amoo_attachments.len(),1);
                    self.set_compact_mode(ShellMode::MiniNotify,ctx);
                }
                9 => {self.handle_action(&Action::Push(PushAction::SetShellMode(ShellMode::Full)),ctx);}
                10 => {self.set_compact_mode(ShellMode::MiniAi,ctx);}
                11 => {
                    assert_eq!(key,check.key);assert_eq!(self.fields[F_AMOO].text,DRAFT);assert_eq!(self.amoo_attachments.len(),1);
                    if !self.amoo_pending_reply.is_empty() || replies.len()<2 { return; }
                    std::fs::write(c.directory.join("attachment-result.json"),serde_json::to_vec_pretty(&replies).unwrap()).unwrap();
                    eprintln!("COMPACT_LIVE attachments returned: {}",replies.last().unwrap());
                    capture(ctx,c.directory.join("amoo-attachments-result.png"));
                    if c.reply_thread.is_empty(){check.step=14;}else{self.set_compact_mode(ShellMode::MiniNotify,ctx);}
                }
                12 => {
                    if !self.push.loaded || self.push.syncing {return;}
                    let t=self.push.snapshot.threads.iter().find(|t|t.thread_id==c.reply_thread).expect("authorized test notification missing");
                    assert!(t.messages.iter().any(|m|!m.outgoing && m.text=="test"),"test SMS not present");
                    self.handle_push_action(&PushAction::SelectImThread(c.reply_thread.clone()),ctx);
                    self.fields[F_IM_REPLY].set(&c.reply_text);
                    self.set_compact_mode(ShellMode::Full,ctx);
                }
                13 => {
                    self.set_compact_mode(ShellMode::MiniNotify,ctx);
                    assert_eq!(self.fields[F_IM_REPLY].text,c.reply_text);assert_eq!(self.push.selected_im_thread.as_ref(),Some(&c.reply_thread));
                    // Persistent marker prevents accidentally sending twice if an acceptance run is repeated.
                    std::fs::OpenOptions::new().write(true).create_new(true).open(c.directory.join("reply-dispatched.marker")).expect("test reply already dispatched; inspect phone before retrying");
                    self.handle_push_action(&PushAction::SendImReply,ctx);
                }
                14 => {
                    if self.push.sending_thread.is_some(){return;}
                    std::fs::write(c.directory.join("notification-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":self.push.last_reply_status,"draft_restored":!self.fields[F_IM_REPLY].text.is_empty(),"error":self.error})).unwrap()).unwrap();
                    capture(ctx,c.directory.join("notification-result.png"));
                }
                _ => {
                    self.set_compact_mode(ShellMode::MiniAi,ctx);
                    assert_eq!(key,check.key);assert_eq!(self.fields[F_AMOO].text,DRAFT);assert_eq!(self.amoo_attachments.len(),1);
                    std::fs::write(c.directory.join("ui-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"session_key":check.key,"switches":8,"conversation_preserved":true,"attachments_preserved":true,"draft_preserved":true,"real_phone":true})).unwrap()).unwrap();
                    eprintln!("COMPACT_LIVE finished; inspect returned answers and phone SMS record for acceptance");
                    ctx.close_window();return;
                }
            }
            check.step+=1;check.since=Some(Instant::now());
            eprintln!("COMPACT_LIVE step {}",check.step);ctx.notify();
        });
    }
}
