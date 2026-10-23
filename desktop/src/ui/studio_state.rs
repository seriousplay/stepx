use super::*;
use facloud_ui::{clipboard::ClipboardContent, TypedActionView};
use stepx_companion::studio::{self, CloudLibrary, Work};

#[derive(Clone, Debug)]
pub(super) enum StudioAction {
    Tab(u8),
    Category(usize),
    Select(usize),
    New,
    PasteLink,
    Save,
    Refresh,
    Send,
    OpenPhone,
    Pin,
    PhoneLibrary,
    AmooWorks,
    CheckPhone,
    Share,
    SaveQr,
    Brief(usize),
    CloudRefresh,
    ConnectAccount,
    DisconnectAccount,
    CloudCategory(usize),
    CloudSelect(usize),
    CloudOpen,
    CloudKeep,
}
pub(super) enum StudioOutcome {
    Library(Vec<Work>, Option<String>),
    Qr(String, PathBuf),
    Phone(String, String),
    Cloud(String, Result<CloudLibrary, String>),
}
#[derive(Default)]
pub(super) struct StudioUiState {
    pub tab: u8,
    pub category: usize,
    pub works: Vec<Work>,
    pub selected: Option<usize>,
    pub loaded: bool,
    pub qr: Option<(String, PathBuf)>,
    pub phone_note: Option<String>,
    pub cloud: Option<CloudLibrary>,
    pub cloud_error: Option<String>,
    pub cloud_selected: Option<usize>,
    pub cloud_category: usize,
    pub cloud_checked: Option<Instant>,
    pub connecting_until: Option<Instant>,
}
pub(super) const CATEGORIES: [&str; 3] = ["应用", "智能体", "技能"];
pub(super) const BRIEFS: [(&str,&str,&str);3] = [
    ("随手记账", "应用 · 单手操作、离线记录、导出数据", "请在应用构建器制作一个适合 STEPX 手机的轻量记账应用。手机竖屏优先，底部导航不超过三项；支持金额、分类、备注、编辑撤销、本地保存、按月统计及 CSV 导出。清楚区分本地数据与云端数据，第一次打开显示可跳过的引导，不填入假账单。请实际测试添加、修改、导出和重新打开后数据保留，给出可真机测试的版本。"),
    ("差旅随行助手", "智能体 · 行程、待办、必要时提醒", "请在智能体构建器创建差旅随行助手。先询问目的地、日期和用户已提供的行程；优先使用平台可用连接器与技能，输出分日行程、出行清单和待确认事项。手机阅读时结论在前，详情折叠。不能访问的数据明确告知，禁止编造订单或天气；订票、付款和发消息前请用户确认。请测试信息不足、用户修改日期、没有连接器权限三个场景，再提供手机真机测试。"),
    ("票据整理", "技能 · 提取字段、核对异常、交付表格", "请在技能构建器制作票据整理技能，供手机 Amoo 调用。输入用户授权的票据图片或 PDF，提取日期、商户、金额、币种及票据号，标记模糊、重复或缺失字段，不猜测金额。输出适合手机阅读的汇总与可下载 CSV；修改和上传外部系统前需要确认。定义清晰的输入输出，使用平台支持的文件能力，实际验证正常票据、模糊图片和重复票据，并给出真机测试步骤。"),
];
impl Companion {
    pub(super) fn sync_studio_cloud(&mut self) {
        self.studio.cloud_checked=Some(Instant::now());
        self.with_device("正在用手机账号同步 Studio 作品…", |adb,d| {
            Ok(Outcome::Studio(StudioOutcome::Cloud(d.serial.clone(),studio::sync_cloud(adb,&d).map_err(|e|e.to_string()))))
        });
    }
    pub(super) fn tick_studio_cloud(&mut self) {
        if self.page!=Page::Studio || self.busy || self.demo_mode || self.selected.is_none() || !self.studio.loaded {return;}
        let interval=if self.studio.connecting_until.is_some_and(|t|t>Instant::now()){3}else{60};
        if self.studio.cloud_checked.is_none_or(|t|t.elapsed()>=Duration::from_secs(interval)) {self.sync_studio_cloud();}
    }
    pub(super) fn refresh_studio(&mut self) {
        self.studio.loaded = true;
        self.submit("正在读取本机 Studio 作品…", |_| {
            Ok(Outcome::Studio(StudioOutcome::Library(
                studio::load()?,
                None,
            )))
        });
    }
    pub(super) fn handle_studio_action(
        &mut self,
        action: &StudioAction,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.busy && !matches!(action, StudioAction::Tab(_)) {
            return;
        }
        self.error = false;
        match action {
            StudioAction::CloudRefresh=>self.sync_studio_cloud(),
            StudioAction::ConnectAccount=>{
                self.studio.connecting_until=Some(Instant::now()+Duration::from_secs(120));
                self.studio.cloud_checked=None;
                self.with_device("请在手机选择账号并完成系统授权…",|adb,d|{
                    studio::open_phone(adb,&d,"account",None)?;
                    Ok(Outcome::Studio(StudioOutcome::Phone(d.serial,"请在手机确认账号授权；完成后自动同步作品".into())))
                });
            }
            StudioAction::DisconnectAccount=>{
                self.studio.cloud=None;self.studio.cloud_selected=None;self.studio.connecting_until=None;
                self.with_device("正在断开作品同步…",|adb,d|{
                    studio::phone_call(adb,&d,serde_json::json!({"op":"cloud-disconnect"}))?;
                    Ok(Outcome::Studio(StudioOutcome::Cloud(d.serial,Ok(CloudLibrary::default()))))
                });
            }
            StudioAction::CloudCategory(i)=>{self.studio.cloud_category=(*i).min(2);self.studio.cloud_selected=self.studio.cloud.as_ref().and_then(|c|c.items.iter().position(|w|w.category==CATEGORIES[self.studio.cloud_category]));}
            StudioAction::CloudSelect(i)=>self.studio.cloud_selected=Some(*i),
            StudioAction::CloudOpen|StudioAction::CloudKeep=>{
                let Some(cloud)=self.studio.cloud.as_ref().filter(|c|c.authorized) else{return;};
                let Some(item)=self.studio.cloud_selected.and_then(|i|cloud.items.get(i)).cloned() else{return;};
                let scope=cloud.scope.clone();let keep=matches!(action,StudioAction::CloudKeep);
                if keep {self.studio.tab=1;}
                self.with_device("正在核对当前手机账号…",move |adb,d|{
                    let status=studio::phone_call(adb,&d,serde_json::json!({"op":"cloud-status"}))?;
                    anyhow::ensure!(status["authorized"]==true && status["scope"]==scope,"手机账号已变化，请刷新作品后重试");
                    if let Some(w)=item.experience(){
                        if keep {return Ok(Outcome::Studio(StudioOutcome::Library(studio::save(w.clone())?,Some(w.url))));}
                        studio::send(adb,&d,&w)?;
                        let note=studio::open_phone(adb,&d,"open",Some(&w.id))?;
                        Ok(Outcome::Studio(StudioOutcome::Phone(d.serial,note)))
                    }else{
                        let note=studio::open_phone(adb,&d,"amoo",None)?;
                        Ok(Outcome::Studio(StudioOutcome::Phone(d.serial,note)))
                    }
                });
            }
            StudioAction::Tab(tab) => {
                self.studio.tab = *tab;
                self.scroll = Default::default();
            }
            StudioAction::Category(i) => self.studio.category = (*i).min(2),
            StudioAction::Select(i) => {
                if let Some(w) = self.studio.works.get(*i) {
                    self.fields[F_STUDIO_NAME].set(&w.name);
                    self.fields[F_STUDIO_URL].set(&w.url);
                    self.studio.category = CATEGORIES
                        .iter()
                        .position(|c| *c == w.category)
                        .unwrap_or(0);
                    self.studio.selected = Some(*i);
                    self.studio.qr = None;
                    self.studio.phone_note = None;
                }
            }
            StudioAction::PasteLink => {
                let text = ctx.clipboard().read().plain_text;
                let raw = text
                    .split_whitespace()
                    .find(|v| v.starts_with("https://"))
                    .unwrap_or(text.trim());
                match studio::parse_link(raw) {
                    Ok((url, _)) => {
                        self.studio.qr = None;
                        if let Some(i) = self.studio.works.iter().position(|w| w.url == url) {
                            self.handle_studio_action(&StudioAction::Select(i), ctx);
                        } else {
                            self.fields[F_STUDIO_URL].set(&url);
                            self.studio.selected = None;
                        }
                        self.status = "已粘贴官方作品链接，请填写名称后保存".into();
                    }
                    Err(e) => {
                        self.status = e.to_string();
                        self.error = true;
                    }
                }
            }
            StudioAction::New => {
                self.studio.selected = None;
                self.studio.qr = None;
                self.studio.phone_note = None;
                self.fields[F_STUDIO_NAME].clear();
                self.fields[F_STUDIO_URL].clear();
            }
            StudioAction::Save => {
                match Work::new(
                    &self.fields[F_STUDIO_NAME].text,
                    &self.fields[F_STUDIO_URL].text,
                    CATEGORIES[self.studio.category],
                ) {
                    Ok(w) => {
                        let url = w.url.clone();
                        self.submit("正在保存作品链接…", move |_| {
                            Ok(Outcome::Studio(StudioOutcome::Library(
                                studio::save(w)?,
                                Some(url),
                            )))
                        });
                    }
                    Err(e) => {
                        self.status = e.to_string();
                        self.error = true;
                    }
                }
            }
            StudioAction::Refresh => self.refresh_studio(),
            StudioAction::Send
            | StudioAction::OpenPhone
            | StudioAction::Pin
            | StudioAction::Share => {
                let Some(w) = self
                    .studio
                    .selected
                    .and_then(|i| self.studio.works.get(i))
                    .cloned()
                else {
                    self.status = "请先保存或选择一个作品".into();
                    return;
                };
                if self.fields[F_STUDIO_NAME].text.trim() != w.name
                    || studio::parse_link(&self.fields[F_STUDIO_URL].text)
                        .ok()
                        .is_none_or(|(url, _)| url != w.url)
                    || CATEGORIES[self.studio.category] != w.category
                {
                    self.status = "作品信息已修改，请先保存链接再上机或分享".into();
                    self.error = true;
                    return;
                }
                if !w.shareable() {
                    self.status =
                        "编辑地址仅用于继续创作；请在官网生成体验链接，再发送到手机或分享".into();
                    self.error = true;
                    return;
                }
                if matches!(action, StudioAction::Share) {
                    self.submit("正在生成分享二维码…", move |_| {
                        let qr = studio::qr(&w)?;
                        Ok(Outcome::Studio(StudioOutcome::Qr(w.id, qr)))
                    });
                } else {
                    let action = action.clone();
                    self.with_device("正在与手机作品库通信…", move |adb, d| {
                        studio::send(adb, &d, &w)?;
                        let message = match action {
                            StudioAction::OpenPhone => {
                                studio::open_phone(adb, &d, "open", Some(&w.id))?
                            }
                            StudioAction::Pin => studio::open_phone(adb, &d, "pin", Some(&w.id))?,
                            _ => format!(
                                "手机已确认保存「{}」；可从 StepX伴侣的 Studio 作品打开",
                                w.name
                            ),
                        };
                        Ok(Outcome::Studio(StudioOutcome::Phone(d.serial, message)))
                    });
                }
            }
            StudioAction::PhoneLibrary | StudioAction::AmooWorks => {
                let mode = if matches!(action, StudioAction::AmooWorks) {
                    "amoo"
                } else {
                    "library"
                };
                self.with_device("正在打开手机作品入口…", move |adb, d| {
                    Ok(Outcome::Studio(StudioOutcome::Phone(
                        d.serial.clone(),
                        studio::open_phone(adb, &d, mode, None)?,
                    )))
                });
            }
            StudioAction::CheckPhone => {
                self.with_device("正在回读手机作品与桌面图标…", |adb, d| {
                    let v = studio::phone_call(adb, &d, serde_json::json!({"op":"status"}))?;
                    let n = v["pinned"].as_array().map_or(0, Vec::len);
                    let note = format!(
                        "手机已保存 {} 个作品 · 已确认 {} 个桌面图标{}",
                        v["count"],
                        n,
                        if v["pinSupported"] == false {
                            " · 当前桌面不支持固定图标"
                        } else {
                            ""
                        }
                    );
                    Ok(Outcome::Studio(StudioOutcome::Phone(d.serial, note)))
                })
            }
            StudioAction::SaveQr => {
                if let Some((_, path)) = self.studio.qr.clone() {
                    self.submit("正在导出分享二维码…", move |_| {
                        let destination = native_dialog::FileDialog::new()
                            .set_filename("Studio-作品二维码.png")
                            .add_filter("PNG", &["png"])
                            .show_save_single_file()?;
                        if let Some(dest) = destination {
                            if dest != path {
                                std::fs::copy(&path, &dest)?;
                            }
                            Ok(Outcome::Notice(format!("二维码已保存：{}", dest.display())))
                        } else {
                            Ok(Outcome::Notice("已取消导出".into()))
                        }
                    });
                }
            }
            StudioAction::Brief(i) => {
                if let Some((_, _, brief)) = BRIEFS.get(*i) {
                    ctx.clipboard()
                        .write(ClipboardContent::plain_text((*brief).into()));
                    self.handle_action(
                        &Action::StudioOpenUrl("https://studio.stepx.com/".into()),
                        ctx,
                    );
                    self.status = format!(
                        "已复制创作需求；请在官网选择{}构建器并粘贴",
                        CATEGORIES[(*i).min(2)]
                    );
                }
            }
        }
    }
    pub(super) fn apply_studio(&mut self, result: StudioOutcome) {
        match result {
            StudioOutcome::Cloud(serial,result)=>{
                if !self.selected.as_ref().is_some_and(|d|d.serial==serial){return;}
                self.studio.cloud_checked=Some(Instant::now());
                match result{
                    Ok(cloud)=>{
                        let old_id=self.studio.cloud.as_ref().filter(|c|c.scope==cloud.scope).and_then(|c|self.studio.cloud_selected.and_then(|i|c.items.get(i))).map(|w|w.id.clone());
                        self.studio.cloud_selected=old_id.and_then(|id|cloud.items.iter().position(|w|w.id==id));
                        self.studio.cloud_selected=self.studio.cloud_selected.or_else(||cloud.items.iter().position(|w|w.category==CATEGORIES[self.studio.cloud_category]));
                        self.studio.cloud_error=None;
                        if cloud.authorized {self.studio.connecting_until=None;self.status=format!("已同步 {} 个应用、{} 个智能体、{} 个技能",cloud.totals[0],cloud.totals[1],cloud.totals[2]);}
                        else{self.status="手机账号尚未连接 Studio 作品同步".into();}
                        self.studio.cloud=Some(cloud);
                    }
                    Err(error)=>{self.studio.cloud=None;self.studio.cloud_selected=None;self.studio.cloud_error=Some(error.clone());self.status=error;self.error=true;}
                }
            }
            StudioOutcome::Library(works, url) => {
                let selected_url = url.clone().or_else(|| {
                    self.studio
                        .selected
                        .and_then(|i| self.studio.works.get(i))
                        .map(|w| w.url.clone())
                });
                self.studio.works = works;
                self.studio.loaded = true;
                self.studio.selected =
                    selected_url.and_then(|u| self.studio.works.iter().position(|w| w.url == u));
                if self.studio.selected.is_none()
                    && self.fields[F_STUDIO_NAME].text.is_empty()
                    && self.fields[F_STUDIO_URL].text.is_empty()
                {
                    if let Some(w) = self.studio.works.first() {
                        self.fields[F_STUDIO_NAME].set(&w.name);
                        self.fields[F_STUDIO_URL].set(&w.url);
                        self.studio.category = CATEGORIES
                            .iter()
                            .position(|c| *c == w.category)
                            .unwrap_or(0);
                        self.studio.selected = Some(0);
                    }
                }
                self.studio.qr = None;
                self.status = if url.is_some() {
                    "作品链接已保存到本机".into()
                } else {
                    format!("已读取 {} 个本机作品", self.studio.works.len())
                };
            }
            StudioOutcome::Qr(id, path) => {
                self.studio.qr = Some((id, path));
                self.status = "分享二维码已生成；体验链接有效期与访问权限由 Studio 决定".into();
            }
            StudioOutcome::Phone(serial, note) => {
                if self.selected.as_ref().is_some_and(|d| d.serial == serial) {
                    self.studio.phone_note = Some(note.clone());
                    self.status = note;
                }
            }
        }
    }
}
