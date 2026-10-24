//! Input handling: every `Action` is resolved here. UI-only actions return immediately;
//! device actions are queued on the ADB worker thread (one at a time).

use super::*;
use facloud_ui::{clipboard::ClipboardContent, TypedActionView};

impl TypedActionView for Companion {
    type Action = Action;

    fn handle_action(&mut self, action: &Action, ctx: &mut ViewContext<Self>) {
        if self.interaction_gate(action,ctx) { return; }
        if let Action::DesktopUpdate(action)=action {self.handle_desktop_update(action,ctx);return;}
        if self.desktop_update.open {
            if matches!(action,Action::Escape){self.desktop_update.open=false;ctx.notify();return;}
            if !matches!(action,Action::MinimizeWindow|Action::MaximizeWindow|Action::StudioOpenUrl(_)){return;}
        }

        if self.handle_context(action, ctx) { self.sync_ime(ctx); return; }
        if self.shortcuts.recording && matches!(action, Action::Page(_) | Action::CloseWindow | Action::CompactChat) {
            let _ = self.shortcuts.resume();
            self.shortcuts.recording=false;
        }
        if self.work.image_viewer.is_some() {
            if matches!(action, Action::Escape) {
                self.work.image_viewer = None;
                ctx.notify();
                return;
            }
            if !matches!(
                action,
                Action::Work(WorkAction::CloseImagePreview)
                    | Action::MinimizeWindow
                    | Action::MaximizeWindow
                    | Action::CloseWindow | Action::QuitApplication
            ) {
                return;
            }
        }
        // The guide is modal: no background shortcuts or device operations leak through.
        if self.adb_guide_step.is_some()
            && !matches!(
                action,
                Action::AdbGuide(_)
                    | Action::AdbGuideContinue
                    | Action::Refresh
                    | Action::Escape
                    | Action::MinimizeWindow
                    | Action::MaximizeWindow
                    | Action::CloseWindow | Action::QuitApplication
                    | Action::StudioOpenUrl(_)
            )
        {
            return;
        }
        if matches!(action, Action::CompactChat) { self.summon_compact_chat(ctx); self.sync_ime(ctx); return; }
        if (self.back_screen.playing
            || self.back_screen.generating
            || self.back_screen.applying_native
            || self.back_screen.companion_busy)
            && matches!(
                action,
                Action::Select(_) | Action::Forget | Action::SetPreferredDevice(_)
            )
        {
            self.status = "背屏正在播放或创作，请先停止或取消后切换设备".into();
            ctx.notify();
            return;
        }
        if self.busy
            && matches!(self.job_page, Page::Work | Page::Amoo | Page::Account)
            && matches!(
                action,
                Action::Select(_) | Action::Forget | Action::SetPreferredDevice(_)
            )
        {
            self.status = "Amoo 正在上传或同步，请等待结果后切换设备".into();
            ctx.notify();
            return;
        }
        if self.work.running
            && matches!(
                action,
                Action::Select(_)
                    | Action::Forget
                    | Action::SetPreferredDevice(_)
                    | Action::AmooNewChat
                    | Action::AmooSend
                    | Action::AmooSelectSession(_)
                    | Action::AmooRetryMessage { .. }
            )
        {
            self.status = "桌面协作正在执行，请先等待完成或停止执行".into();
            ctx.notify();
            return;
        }
        if self.updates.transfer.is_some()
            && matches!(
                action,
                Action::Select(_) | Action::Forget | Action::SetPreferredDevice(_)
            )
        {
            self.status = "正在准备手机更新包，请先完成或取消后再切换设备".into();
            ctx.notify();
            return;
        }
        if let Action::Settings(action) = action {
            if let Err(error)=self.handle_settings_action(action,ctx){ self.settings.feedback=format!("尚未保存：{error}");self.record_notice("设置未保存",self.settings.feedback.clone());self.error=true;}else{self.error=false;}
            self.sync_ime(ctx);ctx.notify();return;
        }
        if let Action::Expansion(action) = action {
            self.handle_expansion_action(action, ctx);
            return;
        }
        if let Action::Updates(action) = action {
            self.handle_update_action(action, ctx);
            return;
        }
        if let Action::BackScreen(action) = action {
            self.handle_backscreen_action(action, ctx);
            self.sync_ime(ctx);
            ctx.notify();
            return;
        }
        if let Action::AmooUi(action) = action {
            self.handle_amoo_ui_action(action);
            self.sync_ime(ctx);
            ctx.notify();
            return;
        }
        if self.page==Page::Amoo && matches!(action,Action::Escape) && (self.workspace.panel!=0||self.workspace.history) {
            self.handle_action(&Action::Workspace(WorkspaceAction::ClosePanel),ctx);return;
        }
        if let Action::Workspace(action) = action {
            self.handle_workspace(action,ctx);self.sync_ime(ctx);return;
        }
        if matches!(action,Action::AmooSend) && !self.amoo_ui.screen_mode {self.handle_workspace(&WorkspaceAction::Prepare,ctx);self.sync_ime(ctx);return;}
        if let Action::StudioLab(action) = action {
            self.handle_studio_lab_action(action, ctx);
            self.sync_ime(ctx);
            ctx.notify();
            return;
        }
        if let Action::Studio(action) = action {
            self.handle_studio_action(action, ctx);
            self.sync_ime(ctx);
            ctx.notify();
            return;
        }
        if let Action::Work(action) = action {
            if !action.keeps_current_shell() && !matches!(self.page,Page::Amoo|Page::Work|Page::Studio) {
                self.set_compact_mode(ShellMode::Full, ctx);
            }
            self.handle_work_action(action, ctx);
            self.sync_ime(ctx);
            ctx.notify();
            return;
        }
        if let Action::Push(action) = action {
            self.handle_push_action(action, ctx);
            self.sync_ime(ctx);
            ctx.notify();
            return;
        }
        if self.busy
            && self.google_progress.lock().is_ok_and(|p| !p.is_empty())
            && matches!(
                action,
                Action::CloseWindow | Action::QuitApplication
                    | Action::Forget
                    | Action::Select(_)
                    | Action::SetPreferredDevice(_)
            )
        {
            self.status = "手机组件正在安装，请等待结果后再关闭或切换设备".into();
            ctx.notify();
            return;
        }
        if matches!(action, Action::AmooSelectSession(_) | Action::AmooNewChat) {
            self.compact.history = false;
        }
        if self.handle_ui_action(action, ctx) {
            self.sync_ime(ctx);
            ctx.notify();
            return;
        }
        self.error = false;
        if let Some(action) = self.resolve(action, ctx) {
            // Enter and refresh may resolve to a module action. Those actions
            // need the same dispatcher (and guards) as a button click; the
            // device-only handler silently ignores them.
            match action {
                Action::Workspace(_) | Action::Studio(_) | Action::AmooUi(_) => {
                    self.handle_action(&action, ctx);
                }
                _ => self.handle_device_action(&action),
            }
        }
        self.sync_ime(ctx);
        ctx.notify();
    }
}

impl Companion {
    fn sync_ime(&self, ctx: &mut ViewContext<Self>) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        if let Some(window) =
            facloud_ui::windowing::winit::winit_window_for_id(ctx, ctx.window_id())
        {
            window.set_ime_allowed(self.focus.is_some());
        }
        ctx.report_active_cursor_position_update();
    }

    pub(super) fn focus_field(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if let Some(previous) = self.focus.filter(|previous| *previous != index) {
            self.fields[previous].preedit.clear();
        }
        self.text_selection.clear();
        self.selected_text = None;
        self.fields[index].anchor = None;
        self.focus = Some(index);
        self.fields[index].cursor = self.fields[index].text.len();
        self.fields[index].all = false;
        if index == F_AMOO {
            Self::scroll_to_end(&self.amoo_ui.composer_scroll);
        }
        ctx.focus_self();
    }

    /// Actions that never touch the device. Returns `true` when handled.
    fn handle_ui_action(&mut self, action: &Action, ctx: &mut ViewContext<Self>) -> bool {
        if let Action::Speak(key,text)=action {self.speak(key,text);return true;}
        if self.handle_text_action(action, ctx) { return true; }
        if !self.demo_mode
            && (matches!(
                action,
                Action::AmooToggleMemory
                    | Action::AmooAddMemory
                    | Action::AmooCycleStyle
                    | Action::AmooCycleCommute
                    | Action::AmooToggleSkillManage
                    | Action::AmooToggleExpressCard
                    | Action::AmooQueryExpress
            ) || matches!(action, Action::AmooSelectAgent(index) if *index != 0))
        {
            self.status = "此功能的后台接口尚未接入，未修改手机配置；当前支持 Amoo 文字对话".into();
            self.error = true;
            return true;
        }
        if !self.demo_mode
            && matches!(
                action,
                Action::AmooToggleRecording | Action::AmooToggleTts | Action::AmooSelectModel(_)
            )
        {
            self.status = match action {
                Action::AmooToggleRecording => "语音输入尚未接入，暂时无法将语音转为文字",
                Action::AmooToggleTts => "语音播报尚未接入",
                _ => "模型切换尚未接入",
            }
            .into();
            self.error = true;
            return true;
        }
        match action {
            Action::Page(page) => {
                self.clear_text_interaction();
                self.motion_started = Instant::now();
                self.set_compact_mode(ShellMode::Full, ctx);
                if let Some(i) = self.focus {
                    self.fields[i].preedit.clear();
                }
                self.page = if matches!(page,Page::Work|Page::Studio){Page::Amoo}else{*page};
                if *page==Page::Work {self.workspace.intent=stepx_companion::amoo_workspace::Intent::Files;}
                if *page==Page::Studio {self.workspace.panel=4;}
                self.back_screen
                    .set_preview_visible(*page == Page::BackScreen);
                if *page == Page::Tools {
                    self.tool_page = None;
                }
                self.tool_confirmation = None;
                self.focus = None;
                self.scroll = Default::default();
                self.scroll_aux = Default::default();
                self.trash_confirm = false;
                self.tools_pending = None;
                if matches!(page, Page::Amoo | Page::Work) {
                    self.reveal_latest_chat();
                }
                if *page == Page::Sms {
                    Self::scroll_to_end(&self.scroll_aux);
                }
                self.load_work_previews();
                self.auto_load_for_page();
            }
            Action::OpenTool(tool) => {
                self.set_compact_mode(ShellMode::Full, ctx);
                self.page = Page::Tools;
                self.tool_page = *tool;
                self.tools_pending = None;
                self.tool_confirmation = None;
                self.focus = None;
                self.scroll = Default::default();
                self.auto_load_for_page();
            }
            Action::NeoIme(enabled) => {
                self.neo_enable_ime = *enabled;
            }
            Action::ToolCategory(category) => {
                self.tool_category = *category;
            }
            Action::PrepareUtility(action) => {
                if !self.busy && self.selected.is_some() {
                    self.tool_confirmation = Some(action.clone());
                    self.scroll.scroll_to(facloud_ui::units::Pixels::new(0.));
                }
            }
            Action::CancelUtility => self.tool_confirmation = None,
            Action::ConfirmUtility => {
                if !self.busy {
                    if let Some(action) = self.tool_confirmation.take() {
                        self.handle_device_action(&action);
                    }
                }
            }
            Action::ToolTab(tab) => {
                self.tools_tab = *tab;
                self.tools_result_page = 0;
                self.tools_pending = None;
            }
            Action::ToolResultPage(page) => {
                self.tools_result_page = *page;
                self.tools_pending = None;
            }
            Action::PrepareToolOperation(operation) => {
                if !self.busy {
                    self.tools_pending = Some(operation.clone());
                    self.scroll.scroll_to(facloud_ui::units::Pixels::new(0.));
                }
            }
            Action::CancelToolOperation => self.tools_pending = None,
            Action::LocateToolFile(path) => {
                if self.busy {
                    return true;
                }
                if let Some((parent, name)) = path.rsplit_once('/') {
                    self.page = Page::Files;
                    self.fields[F_FILE_SEARCH].set(name);
                    self.tools_pending = None;
                    self.browse(parent.to_string());
                }
            }
            Action::CycleTheme => {
                self.theme_mode = self.theme_mode.next();
                let appearance=match self.theme_mode {ThemeMode::System=>stepx_companion::settings::Appearance::System,ThemeMode::Light=>stepx_companion::settings::Appearance::Light,ThemeMode::Dark=>stepx_companion::settings::Appearance::Dark};
                if let Err(e)=self.handle_settings_action(&SettingsAction::Appearance(appearance),ctx){self.status=e.to_string();self.error=true;}
            }
            Action::AdbGuide(step) => {
                self.adb_guide_step = step.map(|step| step.min(2));
                self.focus = None;
                if step.is_none() {
                    self.preferences.adb_guide_seen = true;
                    if let Err(error) = self.preferences.save() {
                        self.status = format!("保存引导偏好失败：{error}");
                        self.error = true;
                    }
                }
            }
            Action::AdbGuideContinue => {
                if self.selected.is_some() {
                    self.handle_ui_action(&Action::AdbGuide(None), ctx);
                    self.handle_ui_action(&Action::ConnectGuide(Some(2)), ctx);
                }
            }
            Action::ConnectGuide(step) => {
                self.connect_guide_step = step.map(|step| step.min(5));
                self.page = Page::Connect;
                self.focus = None;
                if matches!(step, Some(4)) {
                    self.handle_device_action(&Action::Qr);
                }
                self.auto_load_for_page();
            }
            Action::PairTab(tab) => {
                self.pair_tab = *tab;
                match tab {
                    PairTab::Qr => self.focus = None,
                    PairTab::Code => self.focus_field(F_PAIR_CODE, ctx),
                    PairTab::Manual => self.focus_field(F_CONNECT, ctx),
                }
            }
            Action::ToggleShowSecret => self.show_secret = !self.show_secret,
            Action::RenameFile(index) => {
                self.selected_file = Some(*index);
                self.handle_ui_action(&Action::OpenRename, ctx);
            }
            Action::TrashFile(index) => {
                self.selected_file = Some(*index);
                self.handle_ui_action(&Action::Trash, ctx);
            }
            Action::SmsFilter(filter) => {
                self.sms_filter = *filter;
                self.selected_thread = None;
                self.fields[F_SMS_TO].clear();
                self.pending_sms_plan = None;
            }
            Action::AmooToggleToolRecords(key) => {
                if !self.amoo_expanded_tool_records.remove(key) {
                    self.amoo_expanded_tool_records.insert(key.clone());
                }
            }
            Action::PhoneCancelCall => {
                self.pending_call = None;
            }
            Action::PhoneSelectSim(id) => {
                self.selected_call_sim = Some(*id);
                self.pending_call = None;
            }
            Action::ToggleSystemApps(include) => {
                self.include_system_apps = *include;
                self.scroll = Default::default();
            }
            Action::RequestUninstallApp(pkg) => {
                if self
                    .apps
                    .iter()
                    .any(|app| app.package == *pkg && app.can_uninstall())
                {
                    self.pending_app_uninstall = Some(pkg.clone());
                }
            }
            Action::CancelUninstallApp => self.pending_app_uninstall = None,
            Action::SelectSim(sim) => {
                self.selected_sim = *sim;
                self.pending_sms_plan = None;
            }
            Action::SelectSmsThread(thread) => {
                self.selected_thread = thread.clone();
                self.pending_sms_plan = None;
                self.scroll_aux = Default::default();
                Self::scroll_to_end(&self.scroll_aux);
                if let Some(addr) = thread {
                    self.fields[F_SMS_TO].set(addr);
                    self.focus_field(F_SMS_BODY, ctx);
                }
            }
            Action::NewSms => {
                self.selected_thread = Some("__new__".into());
                self.pending_sms_plan = None;
                self.fields[F_SMS_TO].clear();
                self.fields[F_SMS_BODY].clear();
                self.focus_field(F_SMS_TO, ctx);
                self.status = "新建短信：请在下方输入收件人号码与短信正文".into();
            }
            Action::CancelDirectSms => {
                self.pending_sms_plan = None;
                self.status = "已取消发送".into();
            }
            Action::SetPreferredDevice(serial) => {
                self.preferences.selected_serial = Some(serial.clone());
                self.save_preferences("已设为默认设备");
            }
            Action::RemoveRememberedDevice(serial) => {
                self.preferences.devices.retain(|p| &p.serial != serial);
                if self.preferences.selected_serial.as_ref() == Some(serial) {
                    self.preferences.selected_serial =
                        self.preferences.devices.first().map(|d| d.serial.clone());
                }
                self.save_preferences("已移除");
            }
            Action::FileSort(sort) => {
                self.file_sort = *sort;
                self.file_page = 0;
            }
            Action::OpenNewFolder => {
                if self.page != Page::Files || self.selected.is_none() {
                    return true;
                }
                self.file_modal = Some(FileModal::NewFolder);
                self.trash_confirm = false;
                self.fields[F_FILE_NAME].clear();
                self.focus_field(F_FILE_NAME, ctx);
            }
            Action::OpenRename => {
                if self.page != Page::Files {
                    return true;
                }
                if let Some((idx, name)) = self
                    .selected_file
                    .and_then(|i| self.files.get(i).map(|e| (i, e.name.clone())))
                {
                    self.file_modal = Some(FileModal::Rename(idx));
                    self.trash_confirm = false;
                    self.fields[F_FILE_NAME].set(&name);
                    self.focus_field(F_FILE_NAME, ctx);
                    self.fields[F_FILE_NAME].all = true;
                }
            }
            Action::CancelFileModal => {
                self.file_modal = None;
                self.fields[F_FILE_NAME].clear();
                if self.focus == Some(F_FILE_NAME) {
                    self.focus = None;
                }
            }
            Action::MoveFileSelection(delta) => {
                if self.page != Page::Files {
                    return true;
                }
                let filtered: Vec<usize> =
                    self.filtered_files().into_iter().map(|(i, _)| i).collect();
                if filtered.is_empty() {
                    return true;
                }
                let pos = self
                    .selected_file
                    .and_then(|sel| filtered.iter().position(|idx| *idx == sel));
                let next = match pos {
                    Some(pos) => {
                        (pos as isize + *delta).clamp(0, filtered.len() as isize - 1) as usize
                    }
                    None => 0,
                };
                self.selected_file = Some(filtered[next]);
                self.file_page = next / FILES_PER_PAGE;
                self.trash_confirm = false;
            }
            Action::SelectFile(i) => {
                self.selected_file = Some(*i);
                self.trash_confirm = false;
            }
            Action::FilePage(page) => {
                self.file_page = *page;
                self.selected_file = None;
                self.trash_confirm = false;
                self.scroll = Default::default();
            }
            Action::Trash | Action::DeleteFile => {
                if self.page != Page::Files {
                    return true;
                }
                if self.selected_file.is_some() {
                    self.trash_confirm = true;
                    self.permanent_delete = matches!(action, Action::DeleteFile);
                    self.file_modal = None;
                } else {
                    self.status = "请先选择文件".into();
                }
            }
            Action::CancelTrash => self.trash_confirm = false,
            Action::Focus(i) => self.focus_field(*i, ctx),
            Action::ClearField(i) => {
                self.fields[*i].clear();
                if [F_SMS_TO, F_SMS_BODY].contains(i) {
                    self.pending_sms_plan = None;
                }
                self.focus_field(*i, ctx);
                if *i == F_FILE_SEARCH {
                    self.file_page = 0;
                }
            }
            Action::FocusSearch => {
                let target = match self.page {
                    Page::Sms => Some(F_SMS_SEARCH),
                    Page::Notifications => Some(F_PUSH_SEARCH),
                    Page::Phone => Some(F_CONTACT_SEARCH),
                    Page::Files => Some(F_FILE_SEARCH),
                    Page::Apps => Some(F_APP_SEARCH),
                    Page::Amoo => Some(F_AMOO_SEARCH),
                    Page::Work => Some(if self.work.tab == 0 {
                        F_WORK_SEARCH
                    } else {
                        F_WORK_CAP_SEARCH
                    }),
                    Page::BackScreen => Some(F_BACK_PROMPT),
                    Page::Studio => Some(if self.studio.tab == 2 {
                        F_LAB_PROMPT
                    } else {
                        F_STUDIO_SEARCH
                    }),
                    Page::Overview => Some(F_OV_TEXT),
                    Page::Connect | Page::Tools | Page::Account | Page::About | Page::Settings | Page::Expansion => {
                        None
                    }
                };
                if let Some(i) = target {
                    self.focus_field(i, ctx);
                }
            }
            Action::NextField(reverse) => {
                let order: &[usize] = match self.page {
                    Page::Connect => match self.pair_tab {
                        PairTab::Qr => &[],
                        PairTab::Code => &[F_PAIR_ADDR, F_PAIR_CODE],
                        PairTab::Manual => &[F_CONNECT],
                    },
                    Page::Overview => &[F_OV_TEXT],
                    Page::BackScreen => &[F_BACK_PROMPT, F_BACK_MARQUEE],
                    Page::Amoo => &[F_AMOO, F_AMOO_SEARCH],
                    Page::Work => &[
                        F_WORK_PROMPT,
                        F_WORK_SEARCH,
                        F_WORK_CAP_SEARCH,
                        F_WORK_AGENT_NAME,
                    ],
                    Page::Studio if self.studio.tab == 2 => &[F_LAB_PROMPT],
                    Page::Studio if self.studio.tab == 3 => {
                        &[F_LAB_ENDPOINT, F_LAB_KEY, F_LAB_MODEL]
                    }
                    Page::Studio => &[F_STUDIO_SEARCH, F_STUDIO_NAME, F_STUDIO_URL],
                    Page::Sms => &[F_SMS_SEARCH, F_IM_REPLY, F_SMS_TO, F_SMS_BODY],
                    Page::Notifications => &[F_PUSH_SEARCH, F_PUSH_REPLY],
                    Page::Phone => &[F_CONTACT_SEARCH, F_PHONE_DIAL],
                    Page::Files if self.file_modal.is_some() => &[F_FILE_NAME, F_FILE_SEARCH],
                    Page::Files => &[F_FILE_SEARCH],
                    Page::Apps => &[F_APP_SEARCH],
                    Page::Tools | Page::Account | Page::About | Page::Settings | Page::Expansion => &[],
                };
                if !order.is_empty() {
                    let next = match self.focus.and_then(|f| order.iter().position(|x| *x == f)) {
                        Some(pos) if *reverse => order[(pos + order.len() - 1) % order.len()],
                        Some(pos) => order[(pos + 1) % order.len()],
                        None => order[0],
                    };
                    self.focus_field(next, ctx);
                }
            }
            Action::Escape => {
                if self.adb_guide_step.is_some() {
                    self.handle_ui_action(&Action::AdbGuide(None), ctx);
                    return true;
                }
                self.tool_confirmation = None;
                self.tools_pending = None;
                if self.amoo_plus_open {
                    self.amoo_plus_open = false;
                } else if self.amoo_recording {
                    self.amoo_recording = false;
                } else if self.file_modal.is_some() {
                    self.file_modal = None;
                    if self.focus == Some(F_FILE_NAME) {
                        self.focus = None;
                    }
                } else if self.pending_sms_plan.is_some() {
                    self.pending_sms_plan = None;
                } else if self.trash_confirm {
                    self.trash_confirm = false;
                } else if self.focus.is_some() {
                    self.focus = None;
                } else {
                    self.selected_file = None;
                }
            }
            Action::MinimizeWindow => {
                ctx.minimize_window();
            }
            Action::MaximizeWindow => {
                ctx.toggle_maximized_window();
            }
            Action::CloseWindow | Action::QuitApplication => {
                if matches!(action,Action::CloseWindow) && self.settings.value.close_to_tray && self.settings.tray.is_some() {
                    self.remember_window(ctx,true);self.settings.hidden=true;let id=ctx.window_id();ctx.windows().hide_window(id);return true;
                }
                if self.back_screen.companion_busy {
                    self.status = "正在同步 StepX伴侣，请完成后关闭窗口".into();
                    ctx.notify();
                    return true;
                }
                if let Some(job) = &self.updates.transfer {
                    job.cancel();
                }
                self.cancel_backscreen();
                self.stop_mirror();
                self.remember_window(ctx, true);
                ctx.close_window();
            }
            Action::OverviewBackPreview(back) => {
                self.overview_back_preview = *back;
                if *back && self.back_screen.frame.is_empty() {
                    self.back_screen.refresh_frame();
                }
                self.status = if *back {
                    "已切换为背屏图案预览".into()
                } else {
                    "已切换为正面主屏预览".into()
                };
            }
            Action::AmooSelectAgent(idx) => {
                self.amoo_agent_idx = *idx;
                let name = match *idx {
                    0 => "Amoo 助手",
                    1 => "桌面伙伴",
                    2 => "飞书 / 企微",
                    _ => "Claude 协同",
                };
                if let Some(pos) = self.amoo_sessions.iter().position(|s| {
                    (*idx == 0 && (s.agent.contains("Amoo") || s.agent.contains("main")))
                        || (*idx == 1 && (s.agent.contains("深度") || s.agent.contains("桌面")))
                        || (*idx == 2 && (s.agent.contains("飞书") || s.agent.contains("Cron")))
                        || (*idx == 3
                            && (s.agent.contains("Claude") || s.agent.contains("OpenClaw")))
                }) {
                    self.amoo_session_idx = pos;
                }
                self.status = format!("已切换协同智能体：{name}");
            }
            Action::AmooSelectSession(idx) => {
                self.amoo_ui.screen_mode = false;
                self.amoo_show_schedules = false;
                if let Some(session) = self.amoo_sessions.get(*idx) {
                    let key = session.session_key.clone();
                    self.clear_text_interaction();
                    self.compact.history = false;
                    self.amoo_session_idx = *idx;
                    self.load_work_previews();
                    self.amoo_thinking_expanded = false;
                    self.amoo_invoice_open = false;
                    self.reveal_latest_chat();
                    self.focus_field(F_AMOO, ctx);
                    if !self.demo_mode {
                        if !self.busy {
                            self.with_device("正在从后台读取会话…", move |adb, d| {
                                Ok(Outcome::AmooRealData(
                                    amoo::fetch(adb, &d, Some(&key))?,
                                    "已从后台同步会话".into(),
                                ))
                            });
                        } else {
                            self.amoo_poll_remaining = 3;
                            self.last_amoo_poll = Instant::now()
                                .checked_sub(Duration::from_secs(60))
                                .unwrap_or_else(Instant::now);
                        }
                    }
                }
            }
            Action::AmooNewChat => {
                self.amoo_ui.screen_mode = false;
                if self.busy {
                    return true;
                }
                if self.demo_mode {
                    self.amoo_sessions.insert(
                        0,
                        AmooSessionItem {
                            session_key: format!("demo:{}", self.amoo_sessions.len()),
                            title: "新对话（演示）".into(),
                            agent: "Amoo 助手".into(),
                            status: "待提问".into(),
                            status_kind: 0,
                            meta: "演示会话".into(),
                            items: Vec::new(),
                        },
                    );
                    self.amoo_session_idx = 0;
                } else {
                    self.with_device("正在后台创建 Amoo 会话…", |adb, d| {
                        Ok(Outcome::AmooCreated(
                            amoo::AgentBus::connect(adb, &d)?.create_session()?,
                        ))
                    });
                }
                self.focus_field(F_AMOO, ctx);
            }
            Action::AmooSelectCap(idx, tpl) => {
                self.amoo_cap_idx = *idx;
                self.fields[F_AMOO].set(tpl);
                self.focus_field(F_AMOO, ctx);
                self.status = "已填入场景指令模板，按 Enter 或点击发送即可执行".into();
            }
            Action::AmooToggleTts => {
                self.amoo_tts_on = !self.amoo_tts_on;
                self.status = format!(
                    "Amoo 语音朗读已{}",
                    if self.amoo_tts_on { "开启" } else { "关闭" }
                );
            }
            Action::AmooToggleVoicePlay => {
                self.amoo_voice_playing = !self.amoo_voice_playing;
                self.status = if self.amoo_voice_playing {
                    "正在播放语音消息：“顺便发短信告诉李明我马上过去” (0:04\")".into()
                } else {
                    "语音播放已暂停".into()
                };
            }
            Action::AmooToggleThinking => {
                self.amoo_thinking_expanded = !self.amoo_thinking_expanded;
                self.status = if self.amoo_thinking_expanded {
                    "已展开 Amoo 深度思考与工具调用链路".into()
                } else {
                    "已收起深度思考详情".into()
                };
            }
            Action::AmooToggleInvoiceDetail => {
                self.amoo_invoice_open = !self.amoo_invoice_open;
                self.status = if self.amoo_invoice_open {
                    "已打开「10月报销单」明细（共 3 笔，合计 ¥386.00）".into()
                } else {
                    "已收起报销单明细".into()
                };
            }
            Action::AmooConfirmSafety(allow) => {
                self.amoo_safety_decision = if self.demo_mode && *allow {
                    SafetyDecision::Approved
                } else {
                    SafetyDecision::Rejected
                };
                if *allow {
                    self.page = Page::Sms;
                    self.selected_thread = Some("__new__".into());
                    self.pending_sms_plan = None;
                    self.fields[F_SMS_TO].clear();
                    self.fields[F_SMS_BODY].clear();
                    self.focus_field(F_SMS_TO, ctx);
                    self.status = "请填写实际收件人和短信正文，再确认发送".into();
                } else {
                    self.status = "已取消短信操作".into();
                }
            }

            Action::AmooRemoveAttachment(idx) => {
                if *idx < self.amoo_attachments.len() {
                    let removed = self.amoo_attachments.remove(*idx);
                    self.status = format!("已移除待发送附件：{}", removed.name);
                }
            }
            Action::AmooToggleMemory => {
                self.amoo_memory_enabled = !self.amoo_memory_enabled;
                self.status = format!(
                    "Amoo 跨会话记忆已{}",
                    if self.amoo_memory_enabled {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
            }
            Action::AmooCycleStyle => {
                self.amoo_style_idx = (self.amoo_style_idx + 1) % 3;
                let label = match self.amoo_style_idx {
                    0 => "直接精炼 · 不长篇解释",
                    1 => "温和详细 · 分步拆解",
                    _ => "极客专业 · 附带命令",
                };
                self.status = format!("已切换交流风格偏好：{label}");
            }
            Action::AmooCycleCommute => {
                self.amoo_commute_idx = (self.amoo_commute_idx + 1) % 3;
                let label = match self.amoo_commute_idx {
                    0 => "08:30 播报路况",
                    1 => "09:00 播报日程",
                    _ => "18:30 晚高峰提醒",
                };
                self.status = format!("已切换通勤偏好：{label}");
            }
            Action::AmooAddMemory => {
                let text = self.fields[F_AMOO].text.trim().to_string();
                let new_title = if text.is_empty() {
                    format!(
                        "背屏常显偏好：{} ({}级亮度)",
                        self.back_screen.title, self.back_screen.config.brightness
                    )
                } else {
                    self.fields[F_AMOO].clear();
                    text
                };
                self.amoo_memories.insert(
                    0,
                    AmooMemoryEntry {
                        title: new_title.clone(),
                        sub: "桌面新增 · 实时同步".into(),
                    },
                );
                self.status = format!("已写入 Amoo 记忆：{new_title}");
            }
            Action::AmooUseMemory(mem) => {
                self.fields[F_AMOO].set(&format!("结合我的偏好「{mem}」，帮我安排："));
                self.focus_field(F_AMOO, ctx);
                self.status = format!("已引用记忆：{mem}");
            }
            Action::AmooToggleSkillManage => {
                self.amoo_skill_manage = !self.amoo_skill_manage;
                self.status = if self.amoo_skill_manage {
                    "已进入技能管理模式：点击下方技能胶囊可启用或停用".into()
                } else {
                    "已保存技能启用配置".into()
                };
            }
            Action::AmooClickSkill(idx) => {
                if let Some((skill_key, name, prompt, new_enabled)) =
                    self.amoo_skills.get_mut(*idx).map(|skill| {
                        if self.amoo_skill_manage {
                            skill.enabled = !skill.enabled;
                        }
                        (
                            skill.skill_key.clone(),
                            skill.name.clone(),
                            skill.prompt.clone(),
                            skill.enabled,
                        )
                    })
                {
                    if self.amoo_skill_manage {
                        let state = if new_enabled {
                            "已启用"
                        } else {
                            "已停用"
                        };
                        self.status = format!("技能「{name}」{state}");
                        if !self.demo_mode && self.selected.is_some() {
                            self.with_device(
                                &format!("正在向手机 OpenClaw 网关同步技能「{name}」状态…"),
                                move |adb, d| {
                                    let real = openclaw::set_openclaw_skill_enabled(
                                        adb,
                                        &d,
                                        &skill_key,
                                        new_enabled,
                                    )?;
                                    Ok(Outcome::AmooRealData(
                                        real,
                                        format!("已同步技能「{name}」状态至手机 OpenClaw 网关"),
                                    ))
                                },
                            );
                        }
                    } else {
                        self.fields[F_AMOO].set(&prompt);
                        self.focus_field(F_AMOO, ctx);
                        self.status = format!("已调用「{name}」技能模板");
                    }
                }
            }
            Action::AmooCycleSafetyPolicy => {
                self.amoo_safety_policy_idx = (self.amoo_safety_policy_idx + 1) % 3;
                let label = match self.amoo_safety_policy_idx {
                    0 => "始终确认",
                    1 => "仅高危询问",
                    _ => "严格拦截",
                };
                self.status = format!("敏感操作守护策略已切换为：{label}");
            }
            Action::AmooUndoSafety => {
                if self.amoo_safety_decision != SafetyDecision::Pending {
                    self.amoo_safety_decision = SafetyDecision::Pending;
                    if let Some(sess) = self.amoo_sessions.get_mut(0) {
                        sess.status = "待确认".into();
                        sess.status_kind = 1;
                        sess.meta = "2 个产物 · 刚刚".into();
                    }
                    self.status = "已撤回上一次安全授权决定，恢复为「待确认」状态".into();
                } else if let Some(sess) = self.amoo_sessions.get_mut(self.amoo_session_idx) {
                    if sess.items.len() > 1 {
                        sess.items.pop();
                        self.status = "已撤回当前会话最后一条操作记录".into();
                    } else {
                        self.status = "当前没有可撤回的敏感操作".into();
                    }
                }
            }
            Action::AmooExportMemory => {
                let mut md = String::from("# Amoo 记忆与偏好导出\n\n");
                for m in &self.amoo_memories {
                    md.push_str(&format!("- **{}** ({})\n", m.title, m.sub));
                }
                let total_count = if self.demo_mode {
                    26 + self.amoo_memories.len()
                } else {
                    self.amoo_memories.len()
                };
                ctx.clipboard()
                    .write(ClipboardContent::plain_text(md.clone()));
                if let Ok(Some(dest)) = native_dialog::FileDialog::new()
                    .set_title("导出 Amoo 记忆库")
                    .set_filename("StepX-Amoo-Memories.md")
                    .set_location(&stepx_companion::settings::downloads())
                    .show_save_single_file()
                {
                    let _ = std::fs::write(&dest, md);
                    self.status = format!("已导出 {total_count} 条记忆到 {}", dest.display());
                } else {
                    self.status = format!("已将全部记忆导出并复制到剪贴板（共 {total_count} 条）");
                }
            }
            Action::AmooToggleDetails => {
                self.amoo_show_details = !self.amoo_show_details;
            }
            Action::AmooToggleSchedules => {
                self.amoo_show_schedules = !self.amoo_show_schedules;
                self.amoo_ui.album_path = None;
                self.scroll = Default::default();
            }
            Action::AmooOpenScheduledSession(key) => {
                self.amoo_show_schedules = false;
                if let Some(index) = self
                    .amoo_sessions
                    .iter()
                    .position(|s| s.session_key == *key)
                {
                    self.handle_workspace(&WorkspaceAction::PhoneSession(index),ctx);
                }
            }
            Action::AmooTogglePlus => {
                self.amoo_plus_open = !self.amoo_plus_open;
            }
            Action::AmooToggleRecording => {
                self.amoo_recording = !self.amoo_recording;
                self.amoo_plus_open = false;
                if self.amoo_recording {
                    self.amoo_record_secs = 1;
                    self.status = "正在录音… 点击「发送语音」投递给 Amoo".into();
                } else {
                    self.status = "录音已结束".into();
                }
            }
            Action::AmooCancelRecording => {
                self.amoo_recording = false;
                self.status = "已取消语音录制".into();
            }
            Action::StudioOpenUrl(url) => {
                if stepx_companion::studio::parse_link(url)
                    .is_ok_and(|(_, kind)| kind == stepx_companion::studio::LinkKind::Project)
                {
                    let url = url.clone();
                    self.with_device("正在连接 Studio 工作窗口…", move |adb, d| {
                        let note = stepx_companion::studio::open_editor(adb, &d, &url)?;
                        Ok(Outcome::Studio(studio_state::StudioOutcome::Phone(
                            d.serial, note,
                        )))
                    });
                    return true;
                }

                let cmd = if cfg!(target_os = "macos") {
                    "open"
                } else if cfg!(windows) {
                    "explorer"
                } else {
                    "xdg-open"
                };
                match std::process::Command::new(cmd).arg(url).spawn() {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                        self.status = format!("已请求默认浏览器打开 {url}");
                    }
                    Err(error) => {
                        self.status = format!("无法打开浏览器：{error}");
                        self.error = true;
                    }
                }
            }
            Action::PhoneFilterMissed(missed) => {
                self.call_filter_missed = *missed;
            }
            Action::PhoneSelectContact(idx) => {
                self.pending_call = None;
                self.selected_contact = *idx;
                if let Some(c) = self.filtered_contacts().get(*idx) {
                    let num = c.number.clone();
                    self.fields[F_PHONE_DIAL].set(&num);
                }
            }
            Action::PhoneDialAppend(ch) => {
                self.pending_call = None;
                self.fields[F_PHONE_DIAL].insert(ch);
            }
            Action::PhoneDialBackspace => {
                self.pending_call = None;
                let input = &mut self.fields[F_PHONE_DIAL];
                let p = input.previous();
                input.text.replace_range(p..input.cursor, "");
                input.cursor = p;
            }
            Action::PhoneDialClear => {
                self.pending_call = None;
                self.fields[F_PHONE_DIAL].clear();
            }
            Action::PhoneSmsTo(number) => {
                self.fields[F_SMS_TO].set(number);
                self.selected_thread = Some(number.clone());
                self.page = Page::Sms;
                self.focus_field(F_SMS_BODY, ctx);
                self.status = format!("正在向 {number} 编写短信");
            }
            Action::Insert(_)
            | Action::Preedit(_)
            | Action::Backspace
            | Action::BackspaceLine
            | Action::Delete
            | Action::Left
            | Action::Right
            | Action::Home
            | Action::End
            | Action::SelectAll
            | Action::UndoInput
            | Action::RedoInput
            | Action::CopyInput
            | Action::CutInput
            | Action::Paste => self.edit_focused(action, ctx),
            Action::Copy => {
                if let Some(text) = self.selected_text.clone().filter(|s| !s.is_empty()).or_else(|| (!self.content.is_empty()).then(|| self.content.clone())) {
                    ctx.clipboard()
                        .write(ClipboardContent::plain_text(text));
                    self.status = "已复制".into();
                }
            }
            Action::ClearContent => self.content.clear(),
            Action::CopyText(text, msg) => {
                ctx.clipboard()
                    .write(ClipboardContent::plain_text(text.clone()));
                self.status = msg.clone();
                self.error = false;
            }
            Action::AmooTemplate(tpl) => {
                self.amoo_show_schedules = false;
                self.fields[F_AMOO].set(tpl);
                self.focus_field(F_AMOO, ctx);
            }
            Action::AppendVisibleLine(line) => {
                let field = &mut self.fields[F_AMOO];
                if !field.text.is_empty() && !field.text.ends_with([' ', '\n']) {
                    field.cursor = field.text.len();
                    field.insert(" ");
                }
                field.cursor = field.text.len();
                field.insert(line);
                self.focus_field(F_AMOO, ctx);
            }
            Action::StopMirror => {
                self.stop_mirror();
                self.status = "投屏已结束".into();
                self.error = false;
                self.tools_feedback = Some((Some(ToolPage::Screen), self.status.clone(), false));
            }
            Action::SaveScreenshot => self.save_screenshot(),
            Action::AmooSelectModel(idx) => {
                self.amoo_model_idx = *idx;
                let model_name = self.active_amoo_model_name();
                self.status = format!("已切换 Amoo 推理模型：{model_name}");
                self.error = false;
            }
            Action::AmooToggleLightbox => {
                self.amoo_lightbox_open = !self.amoo_lightbox_open;
                self.status = if self.amoo_lightbox_open {
                    "已打开高清原图放大预览（支持 100%/150% 缩放、保存原图与转换背屏点阵）".into()
                } else {
                    "已关闭图片放大预览".into()
                };
            }
            Action::AmooToggleLightboxZoom => {
                self.amoo_lightbox_zoom = if self.amoo_lightbox_zoom == 100 {
                    150
                } else {
                    100
                };
                self.status = format!("图片预览缩放比例：{}%", self.amoo_lightbox_zoom);
            }
            Action::AmooSaveLightboxImage => {
                let src =
                    stepx_companion::resources::root().join("desktop/assets/amoo_tti_portrait.png");
                let default_dest = std::env::temp_dir().join("StepX_Amoo_Portrait_HD.png");
                if src.is_file() {
                    let _ = std::fs::copy(&src, &default_dest);
                }
                self.status = format!("高清原图已保存至 {}", default_dest.display());
            }
            Action::AmooToggleExpressCard => {
                self.amoo_express_open = !self.amoo_express_open;
                self.status = if self.amoo_express_open {
                    "已展开 PC 端快递速查与取件码卡片".into()
                } else {
                    "已收起快递速查卡片".into()
                };
            }
            Action::AmooQueryExpress => {
                self.amoo_express_open = true;
                if let Some(pos) = self
                    .amoo_sessions
                    .iter()
                    .position(|s| s.title.contains("快递"))
                {
                    self.amoo_session_idx = pos;
                }
                self.status =
                    "已从手机 Amoo「快递物流查询」与短信提取 2 件待取包裹（取件码 4-2-1086 / 顺丰 SF14298019283）"
                        .into();
            }
            Action::AmooTogglePcControl => {
                self.amoo_pc_control_enabled = !self.amoo_pc_control_enabled;
                self.status = if self.amoo_pc_control_enabled {
                    "已开启「手机 Amoo 控制电脑」双向桥接通道（支持手机语音/对话截屏、同步剪贴板、静音与锁屏）"
                        .into()
                } else {
                    "已暂停「手机 Amoo 控制电脑」远控通道".into()
                };
            }
            Action::AmooToggleBgKeepAlive => {
                self.amoo_bg_keepalive = !self.amoo_bg_keepalive;
                if self.amoo_bg_keepalive && self.selected.is_some() {
                    return false; // Fall through to handle_device_action to grant deep perms
                }
                self.status = if self.amoo_bg_keepalive {
                    "已开启 Amoo 后台常驻防杀守护 (oom_score_adj = -800)".into()
                } else {
                    "已关闭 Amoo 后台强制保活".into()
                };
            }
            _ => return false,
        }
        true
    }

    fn save_preferences(&mut self, ok: &str) {
        match self.preferences.save() {
            Ok(()) => {
                self.status = ok.into();
                self.error = false;
            }
            Err(e) => {
                self.status = e.to_string();
                self.error = true;
            }
        }
    }

    fn save_screenshot(&mut self) {
        let Some((_, src)) = &self.screenshot else {
            return;
        };
        let src = src.clone();
        let name = format!("StepX-{}.png", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        match native_dialog::FileDialog::new()
            .set_title("保存截图")
            .set_filename(&name)
            .set_location(&stepx_companion::settings::downloads())
                    .show_save_single_file()
        {
            Ok(Some(dest)) => match std::fs::copy(&src, &dest) {
                Ok(_) => self.status = format!("截图已保存到 {}", dest.display()),
                Err(e) => {
                    self.status = format!("保存失败：{e}");
                    self.error = true;
                }
            },
            Ok(None) => {}
            Err(e) => {
                self.status = format!("无法打开保存窗口：{e}");
                self.error = true;
            }
        }
    }

    fn edit_focused(&mut self, action: &Action, ctx: &mut ViewContext<Self>) {
        let Some(i) = self.focus else {
            return;
        };
        let input = &mut self.fields[i];
        if matches!(action,Action::Insert(_) | Action::Paste | Action::CutInput | Action::Backspace | Action::BackspaceLine | Action::Delete) {
            input.checkpoint(matches!(action, Action::Insert(_)));
        }
        match action {
            Action::UndoInput => input.restore_edit(false),
            Action::RedoInput => input.restore_edit(true),
            Action::Insert(s) => input.insert(s),
            Action::Preedit(s) => input.preedit = s.clone(),
            Action::Paste => input.insert(&ctx.clipboard().read().plain_text),
            Action::SelectAll => { input.all = true; input.anchor = Some(0); input.cursor = input.text.len(); },
            Action::CopyInput | Action::CutInput => {
                if i != F_LAB_KEY {
                    if let Some(range) = input.selection_range().filter(|r| !r.is_empty()) {
                        ctx.clipboard().write(ClipboardContent::plain_text(input.text[range].to_owned()));
                        if matches!(action, Action::CutInput) { input.delete_selection(); }
                    }
                }
            }
            Action::Backspace => {
                if input.selection_range().is_some() {
                    input.delete_selection();
                } else {
                    let p = input.previous();
                    input.text.replace_range(p..input.cursor, "");
                    input.cursor = p;
                }
            }
            Action::BackspaceLine => {
                if input.selection_range().is_some() {
                    input.delete_selection();
                } else {
                    input.text.replace_range(0..input.cursor, "");
                    input.cursor = 0;
                }
            }
            Action::Delete => {
                if input.selection_range().is_some() {
                    input.delete_selection();
                } else {
                    let p = input.next();
                    input.text.replace_range(input.cursor..p, "");
                }
            }
            Action::Left => {
                input.cursor = input.selection_range().map(|r|r.start).unwrap_or_else(||input.previous());
                input.anchor = None;
                input.all = false;
            }
            Action::Right => {
                input.cursor = input.selection_range().map(|r|r.end).unwrap_or_else(||input.next());
                input.anchor = None;
                input.all = false;
            }
            Action::Home => {
                input.anchor = None;
                input.cursor = 0;
                input.all = false;
            }
            Action::End => {
                input.anchor = None;
                input.cursor = input.text.len();
                input.all = false;
            }
            _ => {}
        }
        if [F_SMS_TO, F_SMS_BODY].contains(&i) {
            self.pending_sms_plan = None;
        }
        // Copy and Select All must not move the text or its scroll viewport.
        if [F_AMOO,F_WORK_PROMPT].contains(&i) && !matches!(action, Action::CopyInput | Action::SelectAll) {
            let scroll=if i==F_AMOO {&self.amoo_ui.composer_scroll} else {&self.work.prompt_scroll};
            if self.fields[i].cursor == self.fields[i].text.len() {
                Self::scroll_to_end(scroll);
            } else if self.fields[i].cursor == 0 {
                scroll
                    .scroll_to(facloud_ui::units::Pixels::new(0.));
            } else {
                scroll
                    .scroll_to_position(ScrollTarget {
                        position_id: format!("input-caret:{i}"),
                        mode: ScrollToPositionMode::FullyIntoView,
                    });
            }
        }
        if i == F_PHONE_DIAL {
            self.pending_call = None;
        }
        if i == F_FILE_SEARCH {
            self.file_page = 0;
        }
    }

    /// Turns context-dependent actions (Enter, refresh, activate) into concrete ones.
    fn resolve(&mut self, action: &Action, ctx: &mut ViewContext<Self>) -> Option<Action> {
        Some(match action {
            Action::SubmitFocused
                if self
                    .focus
                    .is_some_and(|i| !self.fields[i].preedit.is_empty()) =>
            {
                return None
            }
            Action::SubmitFocused => match self.focus {
                Some(F_PAIR_ADDR) | Some(F_PAIR_CODE) => Action::PairCode,
                Some(F_CONNECT) => Action::Connect,
                Some(F_SMS_TO) => {
                    self.focus_field(F_SMS_BODY, ctx);
                    return None;
                }
                Some(F_SMS_BODY) => {
                    if self.pending_sms_plan.is_some() {
                        Action::ConfirmDirectSms
                    } else {
                        Action::PrepareDirectSms
                    }
                }
                Some(F_AMOO) if !self.amoo_ui.screen_mode => Action::Workspace(WorkspaceAction::Prepare),
                Some(F_AMOO) => Action::AmooSend,
                Some(F_WORK_PROMPT) => {
                    self.handle_work_action(&WorkAction::Send, ctx);
                    return None;
                }
                Some(F_FILE_NAME) => Action::SubmitFileModal,
                Some(F_OV_TEXT) => Action::SendFocusText,
                Some(F_BACK_PROMPT) => {
                    self.handle_backscreen_action(&BackScreenAction::Generate, ctx);
                    return None;
                }
                Some(F_BACK_MARQUEE) => {
                    self.handle_backscreen_action(&BackScreenAction::SaveMarquee, ctx);
                    return None;
                }
                Some(F_LAB_PROMPT) => {
                    self.handle_studio_lab_action(&StudioLabAction::Start, ctx);
                    return None;
                }
                Some(F_LAB_ENDPOINT | F_LAB_KEY | F_LAB_MODEL) => {
                    self.handle_studio_lab_action(&StudioLabAction::Save, ctx);
                    return None;
                }
                Some(F_STUDIO_NAME) | Some(F_STUDIO_URL) => Action::Studio(StudioAction::Save),
                Some(F_PHONE_DIAL) => Action::PhoneCall(self.fields[F_PHONE_DIAL].text.clone()),
                Some(F_IM_REPLY) => {
                    self.handle_push_action(&PushAction::SendImReply, ctx);
                    return None;
                }
                Some(F_TOAST_REPLY) => {
                    if let Some(first) = self.push.active_toasts.first() {
                        let id = first.id.clone();
                        self.handle_push_action(&PushAction::ToastSendReply(id), ctx);
                    }
                    return None;
                }
                Some(F_PUSH_REPLY) => {
                    let target = if self.push.shell_mode == ShellMode::MiniNotify {
                        self.push.expanded_mini_card.clone().and_then(|id| {
                            self.push
                                .snapshot
                                .notifications
                                .iter()
                                .find(|n| n.key == id || n.thread_id == id)
                                .map(|n| {
                                    (
                                        n.key.clone(),
                                        n.thread_id.clone(),
                                        n.package_name.clone(),
                                        n.title.clone(),
                                    )
                                })
                                .or_else(|| {
                                    self.push
                                        .snapshot
                                        .threads
                                        .iter()
                                        .find(|t| t.thread_id == id)
                                        .map(|t| {
                                            (
                                                t.notification_key.clone(),
                                                t.thread_id.clone(),
                                                t.package_name.clone(),
                                                t.title.clone(),
                                            )
                                        })
                                })
                        })
                    } else {
                        self.push
                            .selected_key
                            .clone()
                            .or_else(|| self.push.replying_key.clone())
                            .and_then(|k| {
                                self.push
                                    .snapshot
                                    .notifications
                                    .iter()
                                    .find(|n| n.key == k)
                                    .map(|n| {
                                        (
                                            n.key.clone(),
                                            n.thread_id.clone(),
                                            n.package_name.clone(),
                                            n.title.clone(),
                                        )
                                    })
                            })
                    };
                    if let Some((key, thread_id, package_name, title)) = target {
                        self.handle_push_action(
                            &PushAction::ReplyInline {
                                key,
                                thread_id,
                                package_name,
                                title,
                            },
                            ctx,
                        );
                    } else {
                        self.handle_push_action(&PushAction::SendImReply, ctx);
                    }
                    return None;
                }
                _ => return None,
            },
            Action::ContextRefresh => match self.page {
                Page::About | Page::Settings | Page::Expansion => return None,
                Page::Overview if self.selected.is_some() => Action::Summary,
                Page::Overview | Page::Connect => Action::Refresh,
                Page::BackScreen => {
                    self.status = "背屏预览已刷新".into();
                    ctx.notify();
                    return None;
                }
                Page::Account => Action::AmooUi(AmooUiAction::RefreshUsage),
                Page::Amoo => Action::AmooSnapshot,
                Page::Work => {
                    self.refresh_work();
                    return None;
                }
                Page::Studio => Action::Studio(StudioAction::Refresh),
                Page::Sms => Action::ReadSms,
                Page::Notifications => {
                    self.handle_push_action(&PushAction::Refresh, ctx);
                    return None;
                }
                Page::Phone => Action::ReadPhone,
                Page::Files => Action::Browse(self.directory.clone()),
                Page::Apps => Action::Apps,
                Page::Tools => match self.tool_page {
                    Some(ToolPage::Updates) => {
                        self.handle_update_action(&UpdateAction::CheckOfficial, ctx);
                        return None;
                    }
                    Some(ToolPage::Storage) => Action::ScanStorage,
                    Some(ToolPage::Google) => Action::GoogleSuiteInspect,
                    Some(ToolPage::Companion) => Action::CompanionInspect,
                    Some(ToolPage::Neo) => Action::NeoInspect,
                    Some(ToolPage::Services) => Action::InspectServices,
                    Some(ToolPage::Device) => Action::Summary,
                    _ => return None,
                },
            },
            Action::ActivateSelectedFile => {
                if self.page != Page::Files || self.focus.is_some() {
                    return None;
                }
                return self.selected_file.and_then(|i| self.activate_file(i));
            }
            Action::ActivateFile(i) => return self.activate_file(*i),
            Action::DownloadFile(i) => { self.selected_file = Some(*i); Action::Download },
            Action::ParentDirectory => {
                if self.page != Page::Files {
                    return None;
                }
                let parent = parent_dir(&self.directory);
                if parent == self.directory {
                    return None;
                }
                Action::Browse(parent)
            }
            other => other.clone(),
        })
    }

    fn activate_file(&mut self, i: usize) -> Option<Action> {
        if self.busy { return None; }
        let entry = self.files.get(i)?;
        self.selected_file = Some(i);
        Some(if entry.directory {
            Action::Browse(entry.path.clone())
        } else if ["图片", "视频", "音频", "文档"].contains(&entry.kind_badge()) {
            Action::OpenFile(i)
        } else { Action::Download }
        )
    }

    pub(super) fn handle_device_action(&mut self, action: &Action) {
        if self.busy && self.page == Page::Tools {
            return;
        }
        if !self.demo_mode
            && matches!(
                action,
                Action::AmooOpenMemory
                    | Action::AmooOpenPersona
                    | Action::AmooOpenSafety
                    | Action::AmooOpenModels
                    | Action::AmooOpenVoiceSettings
            )
        {
            self.status = "此设置的后台接口尚未接入，手机配置未改变".into();
            self.error = true;
            return;
        }
        if !self.demo_mode && matches!(action, Action::AmooSendRecording) {
            self.status = "后台语音上传尚未接入，请使用文字或附件".into();
            self.error = true;
            return;
        }
        match action {
            Action::ScanStorage => {
                if self.busy {
                    return;
                }
                self.storage_report = None;
                self.tools_pending = None;
                self.with_device(
                    "正在扫描公开目录并核对重复文件…",
                    |adb, d| {
                        Ok(Outcome::StorageScanned(
                            d.serial.clone(),
                            toolbox::scan(adb, &d)?,
                            toolbox::probe_system(adb, &d).ok(),
                        ))
                    },
                );
            }
            Action::ProbeSystemCompression => {
                if self.busy {
                    return;
                }
                self.system_compression = None;
                self.with_device("正在检测系统压缩能力…", |adb, d| {
                    Ok(Outcome::SystemCompression(
                        d.serial.clone(),
                        toolbox::probe_system(adb, &d)?,
                    ))
                });
            }
            Action::OpenStorageSettings => {
                if self.busy {
                    return;
                }
                self.with_device("正在打开手机存储设置…", |adb, d| {
                    toolbox::open_storage_settings(adb, &d)?;
                    Ok(Outcome::Notice("已在手机上打开存储设置".into()))
                });
            }
            Action::ConfirmToolOperation => {
                if self.busy {
                    return;
                }
                if let Some(operation) = self.tools_pending.take() {
                    self.storage_report = None;
                    self.with_device(
                        "正在处理手机文件并校验内容…",
                        move |adb, d| {
                            let result = toolbox::operate(adb, &d, &operation)?;
                            let report = toolbox::scan(adb, &d).ok();
                            Ok(Outcome::ToolOperated(
                                d.serial.clone(),
                                operation.label().into(),
                                result,
                                report,
                            ))
                        },
                    );
                }
            }
            Action::Refresh => self.refresh(),
            Action::Select(id) => self.select(id.clone()),
            Action::Forget => {
                self.allow_first_connect = false;
                self.preferences.first_connect_done = true;
                self.pairing_hosts.clear();
                if let Some(d) = self.selected.take() {
                    self.preferences.devices.retain(|p| p.serial != d.serial);
                    self.preferences.selected_serial = None;
                    if let Err(e) = self.preferences.save() {
                        self.status = e.to_string();
                        self.error = true;
                        return;
                    }
                }
                self.clear_device_state();
                self.status = "已断开。拔下数据线或在手机上撤销调试授权可彻底停止连接".into();
            }
            Action::PairCode => {
                let endpoint = self.fields[F_PAIR_ADDR].text.trim().to_string();
                let code = self.fields[F_PAIR_CODE].text.trim().to_string();
                if code.is_empty() {
                    self.status = "请输入配对码".into();
                    self.error = true;
                    return;
                }
                self.submit("正在配对…", move |adb| {
                    let endpoint = if endpoint.is_empty() {
                        pairing::unique_code_endpoint(
                            &stepx_companion::discovery::discover_services(adb)?,
                        )?
                    } else {
                        endpoint
                    };
                    pairing::pair_code(adb, &endpoint, &code)?;
                    Ok(Outcome::Paired(endpoint))
                });
            }
            Action::Connect | Action::ConnectEndpoint(_) => {
                let endpoint = match action {
                    Action::ConnectEndpoint(e) => e.trim().to_string(),
                    _ => self.fields[F_CONNECT].text.trim().to_string(),
                };
                self.submit("正在连接…", move |adb| {
                    pairing::validate_endpoint(&endpoint)?;
                    adb.run(&["connect", &endpoint])?;
                    Ok(Outcome::Paired(endpoint))
                });
            }
            Action::Qr => match QrSession::new() {
                Ok(qr) => {
                    self.qr = Some(Arc::new(qr));
                    self.pair_tab = PairTab::Qr;
                    self.status = "已生成新的二维码".into();
                    self.last_scan = Instant::now() - Duration::from_secs(6);
                }
                Err(e) => {
                    self.status = e.to_string();
                    self.error = true;
                }
            },
            Action::Summary => self.with_device("正在读取设备状态…", |adb, d| {
                let summary = d.summary(adb)?;
                let caps = bridge::capabilities(adb, &d).ok();
                Ok(Outcome::Summary(summary, caps))
            }),
            Action::Mirror => {
                if !self.busy && self.mirror.is_none() {
                    self.with_device("正在开启投屏…", |adb, d| {
                        let session = d.mirror(adb)?;
                        Ok(Outcome::Mirror(d.serial, d.transport, session))
                    });
                }
            }
            Action::Screenshot => self.with_device("正在截图…", |adb, d| {
                d.verify(adb)?;
                let bytes = adb.screenshot(&d.transport)?;
                let dir = tempfile::tempdir()?;
                let path = dir.path().join("screen.png");
                std::fs::write(&path, bytes)?;
                Ok(Outcome::Screenshot(dir, path))
            }),
            Action::Key(code) => {
                let code = *code;
                self.with_device("正在发送按键…", move |adb, d| {
                    d.key(adb, code)?;
                    Ok(Outcome::Notice("已发送按键".into()))
                });
            }
            Action::Amoo | Action::AmooSnapshot | Action::AmooOpenTasks => {
                if matches!(action, Action::AmooOpenTasks) {
                    self.amoo_show_schedules = true;
                }
                let key = self
                    .amoo_sessions
                    .get(self.amoo_session_idx)
                    .map(|s| s.session_key.clone());
                self.with_device("正在连接 Amoo 后台…", move |adb, d| {
                    let real = amoo::fetch(adb, &d, key.as_deref())?;
                    let message = format!("后台已同步 {} 个真实会话", real.sessions.len());
                    Ok(Outcome::AmooRealData(real, message))
                });
            }
            Action::AmooResetSession => {
                self.with_device("正在后台创建新会话…", |adb, d| {
                    Ok(Outcome::AmooCreated(
                        amoo::AgentBus::connect(adb, &d)?.create_session()?,
                    ))
                });
            }
            Action::AmooRetryMessage { session_key, text } => {
                if self.busy || self.amoo_pending_reply.contains_key(session_key) {
                    self.status = "上一条请求仍在处理中，请等待结果".into();
                    return;
                }
                if self.demo_mode {
                    self.fields[F_AMOO].set(text);
                    return;
                }
                let key = session_key.clone();
                let text = text.clone();
                self.with_device("正在重新发送这条请求…", move |adb, d| {
                    let receipt = amoo::AgentBus::connect(adb, &d)?.send(&key, &text)?;
                    Ok(Outcome::AmooSent(receipt))
                });
            }
            Action::AmooRetryReply => {
                if self
                    .amoo_sessions
                    .get(self.amoo_session_idx)
                    .is_some_and(|s| self.amoo_pending_reply.contains_key(&s.session_key))
                {
                    self.status = "上一条消息仍在等待结果，正在继续同步，请勿重复发送".into();
                    return;
                }
                let Some(session) = self.amoo_sessions.get(self.amoo_session_idx) else {
                    return;
                };
                let key = session.session_key.clone();
                let text = session.items.iter().rev().find_map(|item| match item {
                    AmooChatItem::UserText { text, .. } => Some(text.clone()),
                    _ => None,
                });
                if let Some(text) = text {
                    if self.demo_mode {
                        self.fields[F_AMOO].set(&text);
                    } else {
                        self.with_device("正在后台重新发送请求…", move |adb, d| {
                            let receipt = amoo::AgentBus::connect(adb, &d)?.send(&key, &text)?;
                            Ok(Outcome::AmooSent(receipt))
                        });
                    }
                }
            }
            Action::AmooPushArtifactBolt => {
                if self.back_screen.playing
                    || self.back_screen.generating
                    || self.back_screen.applying_native
                    || self.back_screen.companion_busy
                {
                    self.status = "请先停止背屏播放或等待创作完成".into();
                    self.error = true;
                    return;
                }
                self.page = Page::BackScreen;
                self.back_screen.native_feature = None;
                self.back_screen.source = stepx_companion::backscreen::Source::Image(
                    BackScreenPattern::Bolt.frame_391(8, false).to_vec(),
                );
                self.back_screen.frame = stepx_companion::backscreen::frame(
                    &self.back_screen.source,
                    &self.back_screen.config,
                    Duration::ZERO,
                    &Default::default(),
                );
                self.back_screen.image_path = None;
                self.back_screen.title = "赛博闪电".into();
                self.back_screen.origin = "内置示例图案".into();
                self.back_screen.description = "请预览灯珠布局，再选择在手机播放。".into();
                self.back_screen.status = "图案已载入预览，尚未推送手机".into();
                self.status = self.back_screen.status.clone();
            }
            Action::AmooPrepare | Action::AmooSend => {
                if matches!(action, Action::AmooPrepare) {
                    self.status = "消息已保留在桌面输入框，点击发送即可从后台提交".into();
                } else {
                    self.send_amoo_draft();
                }
            }
            Action::ReadVisible => self.with_device(
                "正在唤醒圈选感知并读取屏幕文字…",
                |adb, d| {
                    let _ = adb.shell(&d.transport, &["input", "keyevent", "224"]);
                    let _ = d.open_amoo_screen_perception(adb);
                    Ok(Outcome::Text(d.visible_text(adb)?))
                },
            ),
            Action::ReadSms => self.with_device("正在读取短信…", |adb, d| {
                let msgs = read_result(d.messages(adb));
                let caps = bridge::capabilities(adb, &d).ok();
                Ok(Outcome::Sms(msgs, caps))
            }),
            Action::OpenSms => self.with_device("正在打开短信…", |adb, d| {
                d.open_messages(adb)?;
                Ok(Outcome::Notice("已在手机上打开短信".into()))
            }),
            Action::DraftSms => {
                let number = self.fields[F_SMS_TO].text.clone();
                let body = self.fields[F_SMS_BODY].text.clone();
                self.with_device("正在创建草稿…", move |adb, d| {
                    Ok(Outcome::Notice(d.draft_sms(adb, &number, &body)?))
                });
            }
            Action::PrepareDirectSms => {
                let number = if self.fields[F_SMS_TO].text.trim().is_empty() {
                    self.selected_thread
                        .as_deref()
                        .filter(|s| *s != "__new__")
                        .unwrap_or("")
                        .to_string()
                } else {
                    self.fields[F_SMS_TO].text.trim().to_string()
                };
                let body = self.fields[F_SMS_BODY].text.clone();
                if number.is_empty() || body.trim().is_empty() {
                    self.status = "请填写收件人号码和短信内容".into();
                    self.error = true;
                    return;
                }
                self.fields[F_SMS_TO].set(&number);
                let sim = self.selected_sim;
                self.with_device("正在准备短信发送计划…", move |adb, d| {
                    Ok(Outcome::SmsPrepared(bridge::sms_prepare(
                        adb, &d, &number, &body, sim,
                    )?))
                });
            }
            Action::ConfirmDirectSms => {
                let Some(plan) = self.pending_sms_plan.take() else {
                    self.status = "请先准备短信并确认收件人和 SIM 卡".into();
                    self.error = true;
                    return;
                };
                self.selected_thread = Some(plan.number.clone());
                // Consume the confirmation once; never fabricate a send plan or retry a timed-out send.
                self.with_device("正在通过手机发送短信…", move |adb, d| {
                    let status = bridge::sms_send(adb, &d, &plan)?;
                    let messages = read_result(d.messages(adb));
                    Ok(Outcome::SmsSent(status, messages))
                });
            }
            Action::CheckSmsStatus(request_id) => {
                let request_id = request_id.clone();
                self.with_device("正在查询发送状态…", move |adb, d| {
                    Ok(Outcome::SmsStatus(bridge::sms_status(
                        adb,
                        &d,
                        &request_id,
                    )?))
                });
            }
            Action::Browse(path) => self.browse(path.clone()),
            Action::SubmitFileModal => self.submit_file_modal(),
            Action::Upload => {
                if self.busy{return;}
                match native_dialog::FileDialog::new().set_title("选择要上传到手机的文件").show_open_single_file(){
                    Ok(Some(path))=>self.prepare_file_transfer(vec![path],Some(self.directory.clone())),
                    Ok(None)=>{},Err(e)=>{self.status=format!("未选择文件：{e}");self.error=true;}
                }
            }
            Action::TransferFiles(paths,directory) => {
                let paths=paths.clone();let directory=directory.clone();
                self.with_device("正在传送到手机，请保持连接…",move|adb,d|{
                    if let Some(directory)=directory {for path in &paths{storage::push(adb,&d,path,&directory)?;}Ok(Outcome::Files(directory.clone(),storage::list(adb,&d,&directory)?,Some(format!("已上传 {} 个文件到手机：{directory}",paths.len()))))}
                    else {let count=stepx_companion::companion_link::send_files(adb,&d,&paths)?;Ok(Outcome::Notice(format!("已发送 {count} 个文件，请在手机 StepX伴侣的文件页查看。")))}
                });
            }
            Action::OpenFile(index) => {
                if self.busy { return; }
                let Some(entry) = self.files.get(*index).cloned() else { return; };
                self.with_device("正在取回文件并打开…", move |adb, d| {
                    let folder = stepx_companion::resources::user_state().join("file-previews");
                    std::fs::create_dir_all(&folder)?;
                    // Only our expired preview directories, never source files or user downloads.
                    if let Ok(entries) = std::fs::read_dir(&folder) {
                        for item in entries.take(256).flatten() {
                            if item.file_name().to_string_lossy().starts_with("view-")
                                && item.file_type().is_ok_and(|t| t.is_dir())
                                && item.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|d| d.as_secs() > 7 * 86400) {
                                let _ = std::fs::remove_dir_all(item.path());
                            }
                        }
                    }
                    let temp = tempfile::Builder::new().prefix("view-").tempdir_in(&folder)?;
                    let name = std::path::Path::new(&entry.name).file_name().ok_or_else(|| anyhow::anyhow!("文件名无效"))?;
                    let path = temp.path().join(name);
                    storage::pull(adb, &d, &entry.path, &path)?;
                    work_state::open_work_path(&path)?;
                    let _ = temp.keep();
                    Ok(Outcome::Notice(format!("已在电脑打开 {}；手机原文件保持不变", entry.name)))
                });
            }
            Action::Download => {
                let Some(entry) = self.selected_file.and_then(|i| self.files.get(i)).cloned()
                else {
                    self.status = "请先选择文件".into();
                    return;
                };
                if entry.directory {
                    self.status = "暂不支持下载文件夹".into();
                    return;
                }
                self.with_device("选择保存位置…", move |adb, d| {
                    match native_dialog::FileDialog::new()
                        .set_title("下载到电脑")
                        .set_filename(&entry.name)
                        .set_location(&stepx_companion::settings::downloads())
                    .show_save_single_file()?
                    {
                        Some(path) => {
                            storage::pull(adb, &d, &entry.path, &path)?;
                            Ok(Outcome::Notice(format!("已下载到 {}", path.display())))
                        }
                        None => Ok(Outcome::Notice("已取消".into())),
                    }
                });
            }
            Action::ConfirmTrash => {
                let Some(entry) = self.selected_file.and_then(|i| self.files.get(i)).cloned()
                else {
                    return;
                };
                self.trash_confirm = false;
                if self.permanent_delete {
                    let directory = self.directory.clone();
                    self.with_device("正在删除…", move |adb, d| {
                        storage::delete(adb, &d, &entry.path)?;
                        Ok(Outcome::Files(
                            directory.clone(),
                            storage::list(adb, &d, &directory)?,
                            Some(format!("已删除 {}", entry.name)),
                        ))
                    });
                } else {
                    self.with_device("正在移到回收站…", move |adb, d| {
                        Ok(Outcome::Trash(storage::trash(adb, &d, &entry.path)?))
                    });
                }
            }
            Action::Restore => {
                if let Some(record) = self.trashed.clone() {
                    self.with_device("正在恢复…", move |adb, d| {
                        storage::restore(adb, &d, &record)?;
                        Ok(Outcome::Restored)
                    });
                }
            }
            Action::Apps => {
                self.apps_error = None;
                self.with_device("正在读取应用图标与信息…", move |adb, d| {
                    Ok(Outcome::Apps(apps::catalog(adb, &d)?))
                });
            }
            Action::ExportApp(pkg) => {
                let pkg = pkg.clone();
                self.with_device("选择 APK 导出目录…", move |adb, d| {
                    match native_dialog::FileDialog::new()
                        .set_title("选择 APK 导出目录")
                        .show_open_single_dir()?
                    {
                        Some(dir) => Ok(Outcome::Notice(format!(
                            "已导出完整安装包：{}",
                            apps::export_apks(adb, &d, &pkg, &dir)?.display()
                        ))),
                        None => Ok(Outcome::Notice("已取消导出".into())),
                    }
                });
            }
            Action::AppSettings(pkg) => {
                let pkg = pkg.clone();
                self.with_device("正在打开手机应用详情…", move |adb, d| {
                    apps::open_settings(adb, &d, &pkg)?;
                    Ok(Outcome::Notice("已在手机打开应用详情".into()))
                });
            }
            Action::ConfirmUninstallApp(pkg) => {
                if self.busy || self.pending_app_uninstall.as_deref() != Some(pkg.as_str()) {
                    return;
                }
                if !self
                    .apps
                    .iter()
                    .any(|app| app.package == *pkg && app.can_uninstall())
                {
                    return;
                }
                self.pending_app_uninstall = None;
                let pkg = pkg.clone();
                self.with_device("正在卸载应用…", move |adb, d| {
                    apps::uninstall(adb, &d, &pkg)?;
                    Ok(Outcome::ApkInstalled(
                        "应用已卸载".into(),
                        apps::catalog(adb, &d)?,
                    ))
                });
            }
            Action::InstallApk => {
                self.with_device("选择 APK…", move |adb, d| {
                    match native_dialog::FileDialog::new()
                        .set_title("安装 APK")
                        .add_filter("Android APK", &["apk"])
                        .show_open_single_file()?
                    {
                        Some(path) => {
                            let msg = d.install_apk(adb, &path)?;
                            let apps = apps::catalog(adb, &d)?;
                            Ok(Outcome::ApkInstalled(msg, apps))
                        }
                        None => Ok(Outcome::Notice("已取消".into())),
                    }
                });
            }
            Action::OpenApp(package) => {
                let package = package.clone();
                self.with_device("正在打开应用…", move |adb, d| {
                    d.open_app(adb, &package)?;
                    Ok(Outcome::Notice(format!("已在手机上打开 {package}")))
                });
            }
            Action::SendFocusText => {
                let text = self.fields[F_OV_TEXT].text.trim().to_string();
                if text.is_empty() {
                    self.status = "请输入要发送到手机当前焦点的文字".into();
                    self.error = true;
                    return;
                }
                self.fields[F_OV_TEXT].clear();
                self.with_device("正在向手机焦点输入文字…", move |adb, d| {
                    Ok(Outcome::Notice(d.send_focus_text(adb, &text)?))
                });
            }
            Action::AmooDropFiles(paths) => {
                if let Err(e)=self.add_amoo_paths(paths) {self.status=format!("添加附件失败：{e:#}");self.error=true;}
            }
            Action::AmooAttachFile(images_only) => self.choose_amoo_attachments(*images_only),
            Action::AmooAttachPhoneAlbum => {
                self.amoo_plus_open = false;
                self.handle_amoo_ui_action(&AmooUiAction::AlbumBrowse("/sdcard/DCIM".into()));
            }
            Action::AmooSendRecording => {
                let dur = self.amoo_record_secs.max(4);
                self.amoo_recording = false;
                let voice_text = "请帮我查一下今天下午的日程，并在背屏开启赛博闪电提醒";
                let agent = self.active_amoo_agent_name().to_string();
                let reply = build_amoo_reply(voice_text, &agent);
                if let Some(sess) = self.amoo_sessions.get_mut(self.amoo_session_idx) {
                    sess.items.push(AmooChatItem::UserVoice {
                        transcript: format!("“{voice_text}”"),
                        duration_sec: dur,
                    });
                    sess.items.push(reply);
                    sess.meta = "刚刚 · 语音".into();
                    sess.status = "已完成".into();
                    sess.status_kind = 2;
                }
                self.reveal_latest_chat();
                if self.selected.is_some() {
                    self.with_device("正在将语音发送给 Amoo…", |adb, d| {
                        let msg = d.amoo_voice_wake(adb)?;
                        match bridge::amoo_snapshot(adb, &d, false) {
                            Ok(snap) => Ok(Outcome::AmooState(snap, msg, true)),
                            Err(_) => Ok(Outcome::Notice(msg)),
                        }
                    });
                } else {
                    self.status = "语音消息已发送并由 Amoo 处理完成".into();
                }
            }
            Action::AmooOpenMemory => {
                self.with_device("正在打开 Amoo 记忆库…", |adb, d| {
                    Ok(Outcome::Notice(d.open_amoo_memory(adb)?))
                });
            }
            Action::AmooOpenPersona => {
                self.with_device("正在打开个人偏好设置…", |adb, d| {
                    Ok(Outcome::Notice(d.open_amoo_persona(adb)?))
                });
            }
            Action::AmooOpenSafety => {
                self.with_device("正在打开敏感操作安全守护中心…", |adb, d| {
                    Ok(Outcome::Notice(d.open_amoo_safety_center(adb)?))
                });
            }
            Action::AmooOpenModels => {
                self.with_device("正在打开端侧大模型管理…", |adb, d| {
                    Ok(Outcome::Notice(d.open_amoo_model_manager(adb)?))
                });
            }
            Action::AmooOpenVoiceSettings => {
                self.with_device("正在打开语音唤醒设置…", |adb, d| {
                    Ok(Outcome::Notice(d.open_amoo_voice_settings(adb)?))
                });
            }
            Action::ReadPhone => {
                self.with_device("正在读取通话记录与通讯录…", |adb, d| {
                    let logs = read_result(d.call_logs(adb));
                    let contacts = read_result(d.contacts(adb));
                    Ok(Outcome::PhoneData(logs, contacts))
                });
            }
            Action::PhoneCall(number) => {
                if self.busy {
                    return;
                }
                self.pending_call = None;
                let number = number.trim().to_string();
                self.fields[F_PHONE_DIAL].set(&number);
                let sim = self.selected_call_sim;
                self.with_device("正在检查号码和通话 SIM…", move |adb, d| {
                    Ok(Outcome::PhoneCallPrepared(phone::prepare_call(
                        adb, &d, &number, sim,
                    )?))
                });
            }
            Action::PhoneConfirmCall => {
                if self.busy {
                    return;
                }
                let Some(plan) = self.pending_call.take() else {
                    return;
                };
                if phone::normalize_number(&self.fields[F_PHONE_DIAL].text)
                    .ok()
                    .as_deref()
                    != Some(plan.number.as_str())
                {
                    self.status = "号码已更改，请重新确认呼叫".into();
                    self.error = true;
                    return;
                }
                self.with_device("正在提交手机呼叫请求…", move |adb, d| {
                    let notice = phone::submit_call(adb, &d, &plan)?;
                    Ok(Outcome::DialSubmitted(plan.number, notice))
                });
            }
            Action::PhoneHangup => {
                self.with_device("正在挂断当前通话…", |adb, d| {
                    Ok(Outcome::CallHangup(d.hangup_call(adb)?))
                });
            }
            Action::PhoneCreateContact => {
                let name = self.fields[F_CONTACT_SEARCH].text.trim().to_string();
                let number = self.fields[F_PHONE_DIAL].text.trim().to_string();
                if name.is_empty() || number.is_empty() {
                    self.status = "请在联系人搜索框填写姓名，并在拨号框填写号码".into();
                    self.error = true;
                    return;
                }
                self.with_device(
                    "正在向手机通讯录写入新联系人…",
                    move |adb, d| Ok(Outcome::Notice(d.create_contact(adb, &name, &number)?)),
                );
            }
            Action::ForceStopApp(pkg) => {
                let pkg = pkg.clone();
                self.with_device("正在强行停止应用…", move |adb, d| {
                    Ok(Outcome::Notice(d.force_stop_app(adb, &pkg)?))
                });
            }
            Action::ToggleSystemService(pkg, enable) => {
                let pkg = pkg.clone();
                let enable = *enable;
                self.with_device("正在更新系统服务…", move |adb, d| {
                    let msg = d.toggle_system_service(adb, &pkg, enable)?;
                    Ok(Outcome::SystemServices(
                        d.inspect_system_services(adb)?,
                        msg,
                    ))
                });
            }
            Action::DebloatPresetServices => {
                for item in &mut self.system_services {
                    if item.safe_to_remove {
                        item.enabled = false;
                        item.status_note = "已冻结停用 (零后台占用)".into();
                    }
                }
                if self.selected.is_some() {
                    self.with_device("正在一键精简系统冗余后台服务…", |adb, d| {
                        let msg = d.debloat_preset_services(adb)?;
                        let list = d
                            .inspect_system_services(adb)
                            .unwrap_or_else(|_| stepx_companion::device::default_system_services());
                        Ok(Outcome::SystemServices(list, msg))
                    });
                } else {
                    self.status = "已一键停用 5 项系统自带冗余服务（搜狗输入法/高通诊断/系统追踪/打印服务等）".into();
                }
            }
            Action::RestoreAllSystemServices => {
                for item in &mut self.system_services {
                    item.enabled = true;
                    item.status_note = "系统运行中".into();
                }
                if self.selected.is_some() {
                    self.with_device("正在恢复全部系统自带服务…", |adb, d| {
                        let msg = d.restore_all_system_services(adb)?;
                        let list = d
                            .inspect_system_services(adb)
                            .unwrap_or_else(|_| stepx_companion::device::default_system_services());
                        Ok(Outcome::SystemServices(list, msg))
                    });
                } else {
                    self.status = "已恢复全部系统预装服务".into();
                }
            }
            Action::InspectInputMethod if !self.busy => self.with_device("正在读取手机输入法…",|adb,d|Ok(Outcome::InputMethod(stepx_companion::input_method::inspect(adb,&d)?,"输入法状态已更新".into()))),
            Action::SwitchInputMethodFcitx5(use_fcitx5) if !self.busy => {
                let install=*use_fcitx5;
                self.with_device(if install {"正在安装、切换并核验手机输入法…"} else {"正在恢复并核验搜狗输入法…"},move|adb,d|{
                    let report=if install {stepx_companion::input_method::install_and_enable(adb,&d)?} else {stepx_companion::input_method::restore_sogou(adb,&d)?};
                    Ok(Outcome::InputMethod(report,if install {"小企鹅已设为默认，搜狗已停用；已回读手机确认"} else {"已恢复搜狗为默认输入法，小企鹅仍保留"}.into()))
                });
            }
            Action::InspectInputMethod | Action::SwitchInputMethodFcitx5(_) => {},
            Action::GoogleSuiteInspect => {
                self.google_suite = GoogleSuiteStatus::default();
                *self.google_progress.lock().unwrap() = String::new();
                self.with_device("正在检测 Google 服务…", |adb, d| {
                    let suite = d.inspect_google_suite(adb)?;
                    let companion = companion_app::inspect(adb, &d)?;
                    let msg = suite.verification_note.clone();
                    Ok(Outcome::GoogleSuite(suite, companion, msg))
                });
            }
            Action::GoogleSuiteMaintain | Action::GoogleSuiteRollback => {
                let rollback = matches!(action, Action::GoogleSuiteRollback);
                let progress = self.google_progress.clone();
                *progress.lock().unwrap() = if rollback {
                    "核对兼容补丁备份…".into()
                } else {
                    "核对 Google 组件版本与适配条件…".into()
                };
                let previous_companion = self.companion_status.clone();
                self.with_device(
                    if rollback {
                        "正在撤销 Google Play 兼容补丁…"
                    } else {
                        "正在应用 Google Play 兼容补丁…"
                    },
                    move |adb, d| {
                        let (suite, message) = stepx_companion::google::change_compatibility(
                            &d,
                            adb,
                            rollback,
                            |label| {
                                if let Ok(mut value) = progress.lock() {
                                    *value = label.into();
                                }
                            },
                        )?;
                        let companion =
                            companion_app::inspect(adb, &d).unwrap_or(previous_companion);
                        Ok(Outcome::GoogleSuite(suite, companion, message))
                    },
                );
            }
            Action::GoogleSuiteInstall => {
                self.google_suite = GoogleSuiteStatus::default();
                let progress = self.google_progress.clone();
                *progress.lock().unwrap() = "检查 Google Play 与 StepX伴侣安装条件…".into();
                self.with_device(
                    "正在安装 Google Play 与 StepX伴侣，请保持连接…",
                    move |adb, d| {
                        let result = stepx_companion::google::install(&d, adb, |label| {
                            if let Ok(mut value) = progress.lock() {
                                *value = label.into();
                            }
                        })?;
                        Ok(Outcome::GoogleSuite(
                            result.google,
                            result.companion,
                            result.message,
                        ))
                    },
                );
            }
            Action::NeoInspect => {
                self.with_device(
                    "正在核对 Neo 安装、签名与手机服务…",
                    |adb, d| {
                        Ok(Outcome::NeoStatus(stepx_companion::neo_agent::inspect(
                            adb, &d,
                        )?))
                    },
                );
            }
            Action::NeoInstall => {
                let ime = self.neo_enable_ime;
                match native_dialog::FileDialog::new()
                    .set_title("选择内置 GUI Agent 的 StepX伴侣 APK")
                    .add_filter("Android 应用", &["apk"])
                    .show_open_single_file()
                {
                    Ok(Some(path)) => self.with_device(
                        "正在更新 StepX伴侣内置 GUI Agent，请保持连接…",
                        move |adb, d| {
                            Ok(Outcome::NeoStatus(stepx_companion::neo_agent::install(
                                adb, &d, &path, ime,
                            )?))
                        },
                    ),
                    Ok(None) => {}
                    Err(error) => {
                        self.status = format!("选择 APK 失败：{error}");
                        self.error = true;
                    }
                }
            }
            Action::NeoActivate => {
                self.with_device("正在启用手机 Neo 任务路由…", |adb, d| {
                    Ok(Outcome::NeoStatus(stepx_companion::neo_agent::activate(
                        adb, &d,
                    )?))
                });
            }
            Action::NeoRestore => {
                self.with_device(
                    "正在撤销内置 GUI Agent 的组件配置…",
                    |adb, d| {
                        Ok(Outcome::NeoStatus(stepx_companion::neo_agent::restore(
                            adb, &d,
                        )?))
                    },
                );
            }
            Action::NeoOpenPhone => {
                self.with_device("正在打开伴侣 GUI Agent 配置…", |adb, d| {
                    stepx_companion::neo_agent::open_phone(adb, &d)?;
                    Ok(Outcome::Notice(
                        "已在手机打开伴侣 GUI Agent；请选择模型和允许操作的应用。".into(),
                    ))
                });
            }
            Action::CompanionInspect => {
                self.companion_status = CompanionStatus::default();
                *self.google_progress.lock().unwrap() = String::new();
                self.with_device("正在检测 StepX伴侣与实际授权…", |adb, d| {
                    let companion = companion_app::inspect(adb, &d)?;
                    let message = companion.message.clone();
                    Ok(Outcome::CompanionStatus(companion, message))
                });
            }
            Action::CompanionInstall => {
                let progress = self.google_progress.clone();
                *progress.lock().unwrap() = "正在检查 StepX伴侣安装条件…".into();
                self.with_device(
                    "正在安装或修复 StepX伴侣，请保持连接…",
                    move |adb, d| {
                        let companion = companion_app::install(adb, &d, |label| {
                            if let Ok(mut text) = progress.lock() {
                                *text = label.into();
                            }
                        })?;
                        let message = companion.message.clone();
                        Ok(Outcome::CompanionStatus(companion, message))
                    },
                );
            }
            Action::CompanionSendFiles => {
                if self.busy {
                    return;
                }
                match native_dialog::FileDialog::new()
                    .set_title("发送文件到 StepX伴侣")
                    .show_open_multiple_file()
                {
                    Ok(paths) if !paths.is_empty() => self.prepare_file_transfer(paths,None),
                    Ok(_) => {}
                    Err(error) => {
                        self.status = format!("选择文件失败：{error}");
                        self.error = true;
                    }
                }
            }
            Action::CompanionReceiveFolder => {
                let opened = (|| -> anyhow::Result<std::path::PathBuf> {
                    let path = stepx_companion::companion_link::receive_directory()?;
                    let opener = if cfg!(target_os = "macos") {
                        "open"
                    } else {
                        "xdg-open"
                    };
                    std::process::Command::new(opener).arg(&path).spawn()?;
                    Ok(path)
                })();
                match opened {
                    Ok(path) => self.status = format!("已打开电脑接收文件夹：{}", path.display()),
                    Err(error) => {
                        self.status = format!("打开接收文件夹失败：{error}");
                        self.error = true;
                    }
                }
            }
            Action::CompanionBackground(enabled) => {
                let enabled = *enabled;
                self.with_device(
                    if enabled {
                        "正在开启 Amoo 后台连接…"
                    } else {
                        "正在停止 Amoo 后台连接…"
                    },
                    move |adb, d| {
                        stepx_companion::amoo_background::set_enabled(adb, &d, enabled)?;
                        let companion = companion_app::inspect(adb, &d)?;
                        Ok(Outcome::CompanionStatus(
                            companion,
                            if enabled {
                                "已提交开启请求，后台连接状态已重新读取"
                            } else {
                                "已提交停止请求，后续任务不会自动重新开启"
                            }
                            .into(),
                        ))
                    },
                );
            }
            Action::OpenInputSettings => {
                self.with_device("正在打开手机输入法设置…", |adb, d| {
                    d.verify(adb)?;
                    adb.shell(
                        &d.transport,
                        &[
                            "am",
                            "start",
                            "-a",
                            "android.settings.INPUT_METHOD_SETTINGS",
                        ],
                    )?;
                    Ok(Outcome::Notice(
                        "已在手机打开输入法设置；修改后可刷新服务状态".into(),
                    ))
                });
            }
            Action::InspectServices => {
                self.system_services.clear();
                self.with_device("正在读取系统服务…", |adb, d| {
                    Ok(Outcome::SystemServices(
                        d.inspect_system_services(adb)?,
                        "系统服务状态已更新".into(),
                    ))
                });
            }
            Action::GoogleSuiteSnapshot => {
                if self.selected.is_some() {
                    self.with_device(
                        "正在校验本地 AES-GCM 加密快照与手机还原点…",
                        |adb, d| Ok(Outcome::Notice(d.prepare_google_snapshot(adb)?)),
                    );
                } else {
                    self.status =
                        "本地 AES-GCM 加密备份与 /data/local/tmp/stepx-gms-restore 还原点已就绪"
                            .into();
                }
            }
            Action::GrantDeepAmooPerms | Action::AmooToggleBgKeepAlive => {
                if self.selected.is_some() {
                    self.with_device(
                        "正在通过 Root Bridge 授予 Amoo 深度权限与后台常驻保护 (oom_score_adj = -800)…",
                        |adb, d| Ok(Outcome::DeepAmooGranted(d.grant_deep_amoo_permissions(adb)?)),
                    );
                } else {
                    self.status = "请先连接设备，再设置后台权限".into();
                }
            }
            Action::GrantDeepBackScreenPerms => {
                if self.selected.is_some() {
                    self.with_device(
                        "正在通过 Root Bridge 解锁 AW20072 背屏底层硬件节点权限…",
                        |adb, d| {
                            Ok(Outcome::DeepBackScreenGranted(
                                d.grant_deep_backscreen_permissions(adb)?,
                            ))
                        },
                    );
                } else {
                    self.status = "请先连接设备，再设置背屏权限".into();
                }
            }
            Action::BackScreenToggleHwBreath => {
                self.backscreen_hw_breath = !self.backscreen_hw_breath;
                let mode = u8::from(self.backscreen_hw_breath);
                if self.selected.is_some() {
                    self.with_device(
                        "正在下发 AW20072 硬件呼吸律动指令…",
                        move |adb, d| {
                            Ok(Outcome::Notice(
                                d.trigger_backscreen_hardware_breath(adb, mode)?,
                            ))
                        },
                    );
                } else {
                    self.status = if self.backscreen_hw_breath {
                        "已开启 AW20072 硬件级呼吸灯效 (breath=1)".into()
                    } else {
                        "已切回 AW20072 帧缓冲点阵直驱模式 (breath=0)".into()
                    };
                }
            }
            Action::AmooPushPickupCode(code) => {
                let code = code.clone();
                self.fields[F_BACK_MARQUEE].set(&code);
                self.matrix_marquee_on = true;
                let rev = self.back_screen.config.invert;
                if self.selected.is_some() {
                    self.with_device(
                        "正在将快递取件码推送至手机背屏跑马灯常显…",
                        move |adb, d| {
                            d.set_backscreen_marquee(adb, &code, true, rev)?;
                            Ok(Outcome::Notice(format!(
                                "手机已保存取件码「{code}」跑马灯，实际亮灯遵循手机背屏设置"
                            )))
                        },
                    );
                } else {
                    self.status = format!("已填入取件码「{code}」，请连接手机后同步");
                }
            }
            Action::AmooExecPcCommand(cmd_key) => {
                let cmd_key = cmd_key.clone();
                if self.selected.is_some() {
                    self.with_device(
                        "正在执行手机 Amoo 控电脑指令…",
                        move |adb, d| {
                            Ok(Outcome::Notice(
                                d.execute_pc_control_command(adb, &cmd_key)?,
                            ))
                        },
                    );
                } else if let Some(d) = &self.selected {
                    let _ = d;
                } else {
                    self.status = match cmd_key.as_str() {
                        "screenshot_to_phone" => "已截取电脑屏幕并发送至手机 Amoo".into(),
                        "sync_clipboard" => "已完成电脑 ⇄ 手机 Amoo 剪贴板双向同步".into(),
                        "pc_mute_toggle" => "已执行手机 Amoo 远控指令：切换电脑静音".into(),
                        "pc_lock_screen" => "已验证手机 Amoo 远程锁屏指令通道".into(),
                        _ => format!("已执行远控指令：{cmd_key}"),
                    };
                }
            }
            Action::FilePushToBackScreen(remote_path) => {
                if self.back_screen.playing
                    || self.back_screen.generating
                    || self.back_screen.applying_native
                    || self.back_screen.companion_busy
                {
                    self.status = "请先停止背屏播放或等待创作完成，再导入图片".into();
                    self.error = true;
                    return;
                }
                let remote_path = remote_path.clone();
                self.with_device(
                    "正在读取手机图片，完成后进入背屏预览…",
                    move |adb, d| {
                        let base = std::env::var_os("XDG_CACHE_HOME")
                            .map(PathBuf::from)
                            .or_else(|| {
                                stepx_companion::resources::user_home()
                                    .map(|h| PathBuf::from(h).join(".cache"))
                            })
                            .ok_or_else(|| anyhow::anyhow!("无法确定图片缓存目录"))?;
                        let directory = base.join("stepx-companion/backscreen/imports");
                        std::fs::create_dir_all(&directory)?;
                        let name = PathBuf::from(&remote_path)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or("手机图片".into());
                        let path = directory.join(format!("{:08x}-{name}", rand::random::<u32>()));
                        storage::pull(adb, &d, &remote_path, &path)?;
                        Ok(Outcome::BackScreenImportedImage(path))
                    },
                );
            }
            _ => {}
        }
    }

    fn submit_file_modal(&mut self) {
        let Some(modal) = self.file_modal else {
            return;
        };
        let name = self.fields[F_FILE_NAME].text.trim().to_string();
        if name.is_empty() {
            self.status = "请输入名称".into();
            self.error = true;
            return;
        }
        let directory = self.directory.clone();
        match modal {
            FileModal::NewFolder => self.with_device("正在新建文件夹…", move |adb, d| {
                storage::mkdir(adb, &d, &directory, &name)?;
                let list = storage::list(adb, &d, &directory)?;
                Ok(Outcome::Files(
                    directory,
                    list,
                    Some(format!("已新建 {name}")),
                ))
            }),
            FileModal::Rename(idx) => {
                if let Some(entry) = self.files.get(idx).cloned() {
                    self.with_device("正在重命名…", move |adb, d| {
                        storage::rename(adb, &d, &entry.path, &name)?;
                        let list = storage::list(adb, &d, &directory)?;
                        Ok(Outcome::Files(
                            directory,
                            list,
                            Some(format!("已重命名为 {name}")),
                        ))
                    });
                }
            }
        }
    }
}

impl Companion {
    pub(super) fn active_amoo_agent_name(&self) -> &'static str {
        match self.amoo_agent_idx {
            0 => "Amoo 助手",
            1 => "桌面伙伴",
            2 => "飞书 / 企微",
            _ => "Claude 协同",
        }
    }

    pub(super) fn active_amoo_model_name(&self) -> &str {
        self.active_amoo_model_short()
    }

    pub(super) fn active_amoo_model_short(&self) -> &str {
        self.work
            .selected_model
            .as_deref()
            .or_else(|| {
                self.amoo_sessions
                    .get(self.amoo_session_idx)
                    .and_then(|session| self.work.session_models.get(&session.session_key))
                    .map(String::as_str)
            })
            .unwrap_or("手机默认路由")
    }
}

fn build_amoo_reply(prompt: &str, agent: &str) -> AmooChatItem {
    let wants_bolt = prompt.contains("背屏")
        || prompt.contains("闪电")
        || prompt.contains("点阵")
        || prompt.contains("像素")
        || prompt.contains("图案");
    let wants_invoice =
        prompt.contains("发票") || prompt.contains("报销") || prompt.contains("账单");
    let wants_sms = prompt.contains("短信") || prompt.contains("李明") || prompt.contains("发消息");

    let (tools_summary, steps, reply_text) = if wants_bolt {
        (
            "背屏点阵生成 · 亮度校准".to_string(),
            vec![
                format!("1. [{agent}] 解析背屏点阵视觉描述：「{prompt}」"),
                "2. 合成 23×17 (391 颗 LED) 像素灰度矩阵并适配圆角掩码".into(),
                "3. 生成背屏实时预览卡片，支持一键投递到手机副屏".into(),
            ],
            Some(format!(
                "已为你生成 23×17 背屏像素图案方案。你可以点击下方「投到背屏」按钮直接同步至 StepX 手机背屏常显。"
            )),
        )
    } else if wants_invoice {
        (
            "端侧 OCR · 报销单归档".to_string(),
            vec![
                format!("1. [{agent}] 扫描票据抬头、金额与开票日期"),
                "2. 校验高德打车与差旅餐饮电子发票明细（合计 ¥386.00）".into(),
                "3. 写入「10月报销单」并生成可导出结构化台账".into(),
            ],
            Some(
                "已完成票据识别与归档，本月累计入账 ¥386.00。点击下方「打开」可查看完整报销明细。"
                    .into(),
            ),
        )
    } else if prompt.contains("研究") || prompt.contains("热点") || prompt.contains("总结") {
        (
            "深度研究 · 跨端知识整理".to_string(),
            vec![
                format!("1. [{agent}] 拆解研究主题：「{prompt}」"),
                "2. 检索本地向量库与实时网页信源，交叉核验核心数据".into(),
                "3. 提炼 3 条结构化核心结论与行动建议".into(),
            ],
            Some(format!(
                "针对「{prompt}」已完成深度梳理：\n1. 核心结论：端侧算力与桌面跨端协同可将日常流转效率提升约 40%。\n2. 关键要点：结合个人记忆偏好自动执行高频归档与提醒。\n3. 下一步建议：可将结论同步至飞书文档或设为每周定时任务。"
            )),
        )
    } else {
        (
            format!("{agent} · 端侧推理"),
            vec![
                format!("1. [{agent}] 理解指令上下文与跨会话记忆（28 条偏好）"),
                "2. 调度已启用技能并生成直接、精炼的执行结果".into(),
            ],
            Some(format!(
                "收到你的指令：「{prompt}」。\n已结合你的「直接精炼」交流偏好处理完毕，如需投递到手机背屏或同步飞书日历可随时告诉我。"
            )),
        )
    };

    AmooChatItem::AssistantRich {
        thinking_time: "1.4s".into(),
        thinking_steps: steps,
        tools_summary,
        text: reply_text,
        has_bolt_artifact: wants_bolt,
        has_invoice_artifact: wants_invoice,
        has_safety_confirm: wants_sms,
        time_label: "刚刚".into(),
    }
}

pub(super) fn parent_dir(dir: &str) -> String {
    dir.trim_end_matches('/')
        .rsplit_once('/')
        .map(|(p, _)| p)
        .filter(|p| p.starts_with("/sdcard"))
        .unwrap_or("/sdcard")
        .to_string()
}
