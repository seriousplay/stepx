use super::super::{
    studio_state::{StudioAction as St, BRIEFS, CATEGORIES},
    theme::*,
    widgets::*,
    Action, Companion, F_STUDIO_NAME, F_STUDIO_URL,
};
use facloud_ui::{assets::asset_cache::AssetSource, elements::*};
use stepx_companion::studio::LinkKind;

impl Companion {
    pub(in super::super) fn studio_actions(&self) -> Vec<El> {
        vec![
            self.button(
                "st-web",
                "打开 Studio",
                Action::StudioOpenUrl("https://studio.stepx.com/".into()),
                Btn::Secondary,
            ),
            self.button(
                "st-projects",
                "官网项目",
                Action::StudioOpenUrl("https://studio.stepx.com/projects".into()),
                Btn::Secondary,
            ),
        ]
    }
    fn st_button(&self, id: &str, label: &str, action: St, style: Btn) -> El {
        if self.busy {
            self.disabled_button(label)
        } else {
            self.button(id, label, Action::Studio(action), style)
        }
    }
    pub(in super::super) fn studio_page(&self) -> El {
        let p = self.p();
        let tabs = self.segmented(
            "studio-tabs",
            ["手机账号作品", "收藏与分享", "开始创作", "连接与上机"]
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    (
                        (*s).into(),
                        self.studio.tab == i as u8,
                        Action::Studio(St::Tab(i as u8)),
                    )
                })
                .collect(),
        );
        let body = match self.studio.tab {
            1 => self.studio_library(),
            2 => self.studio_create(),
            3 => self.studio_guide(),
            _ => self.studio_cloud(),
        };
        self.vstack(
            S5,
            vec![
                self.between(tabs, self.line("Step Studio × Companion", CAPTION, p.text3)),
                body,
            ],
        )
    }
    fn studio_cloud(&self) -> El {
        let p=self.p();
        let connected=self.selected.is_some();
        let authorized=self.studio.cloud.as_ref().is_some_and(|c|c.authorized);
        let mut rows=vec![self.panel(vec![
            self.between(self.vstack(S1,vec![self.heading("手机账号 · 自动同步"),self.text("同步 Amoo「我的作品」里的应用、技能与智能体。官网提交真机测试后会出现在这里，无需逐个复制链接。",BODY,p.text2)]),
                if connected {self.st_button("st-cloud-sync",if authorized {"同步作品"} else {"连接手机账号"},if authorized {St::CloudRefresh}else{St::ConnectAccount},Btn::Primary)}else{self.disabled_button("请先连接手机")}),
            self.text("连接手机账号用于作品同步；官网编辑器仍使用浏览器登录。尚未提交真机测试的官网草稿不在此目录。",CAPTION,p.text3),
        ])];
        if let Some(error)=&self.studio.cloud_error {rows.push(self.panel(vec![self.heading("同步未完成"),self.text(error,BODY,p.text2),self.st_button("st-cloud-retry","重新同步",St::CloudRefresh,Btn::Secondary)]));}
        let Some(cloud)=self.studio.cloud.as_ref().filter(|c|c.authorized) else{
            rows.push(self.panel(vec![self.heading(if connected {"在手机确认一次，即可开始"}else{"连接你的 STEPX 手机"}),self.text("选择手机已有的 Amoo 账号，在系统授权页确认。之后进入此页面自动读取最新作品，离开页面停止刷新。账号令牌始终保留在手机。",BODY,p.text2)]));
            return self.vstack(S4,rows);
        };
        let selected=self.studio.cloud_selected.and_then(|i|cloud.items.get(i));
        let index=self.studio.cloud_category;
        rows.push(self.between(self.segmented("st-cloud-types",CATEGORIES.iter().enumerate().map(|(i,c)|(format!("{} · {}",c,cloud.totals[i]),i==index,Action::Studio(St::CloudCategory(i)))).collect()),self.line(format!("更新于 {}",chrono::DateTime::from_timestamp(cloud.synced_at,0).map(|t|t.with_timezone(&chrono::Local).format("%H:%M:%S").to_string()).unwrap_or_default()),CAPTION,p.text3)));
        let mut list=vec![];
        for (i,w) in cloud.items.iter().enumerate().filter(|(_,w)|w.category==CATEGORIES[index]) {
            list.push(self.list_row(format!("st-cloud-{i}"),self.vstack(S1,vec![self.heading(&w.name),self.line(format!("{}{}",w.category,if w.version.is_empty(){String::new()}else{format!(" · v{}",w.version)}),CAPTION,p.text2)]),self.studio.cloud_selected==Some(i),Some(Action::Studio(St::CloudSelect(i))),None));
        }
        if list.is_empty(){list.push(self.text("还没有可同步的作品。在 Studio 完成创作后，点击“真机测试”，再回到这里同步。",BODY,p.text2));}
        if cloud.totals[index]>200 {list.push(self.text("当前展示前 200 个作品，其余可在 Amoo 我的作品查看。",CAPTION,p.text3));}
        let details=if let Some(w)=selected {
            let mut detail=vec![self.heading(&w.name),self.text(if w.description.is_empty(){"已从官方作品服务同步，可直接在手机验收。"}else{&w.description},BODY,p.text2)];
            let mut actions=vec![self.st_button("st-cloud-open",if w.experience().is_some(){"在手机打开"}else{"在 Amoo 管理"},St::CloudOpen,Btn::Primary)];
            if let Some(url)=w.editor(){actions.push(self.button("st-cloud-edit","继续创作",Action::StudioOpenUrl(url),Btn::Secondary));}
            if w.experience().is_some(){actions.push(self.st_button("st-cloud-keep","收藏并分享",St::CloudKeep,Btn::Secondary));}
            detail.push(self.wrap(actions));
            detail.push(self.text(if w.category=="应用"{"体验地址随官网同步更新。若构建中或链接过期，请在官网重新提交真机测试。"}else{"安装、更新与授权由 Amoo 官方页面完成；同步目录不会自动安装或发布。"},CAPTION,p.text3));
            self.panel(detail)
        }else{self.panel(vec![self.heading("选择作品"),self.text("查看详情、继续创作，或在手机验证。",BODY,p.text2)])};
        rows.push(Flex::row().with_spacing(S5).with_cross_axis_alignment(CrossAxisAlignment::Start).with_child(ConstrainedBox::new(self.panel(list)).with_width(260.).finish()).with_child(Expanded::new(1., details).finish()).finish());
        rows.push(self.between(self.line("此页每分钟同步 · 使用手机官方账号",CAPTION,p.text3),self.st_button("st-cloud-disconnect","断开作品同步",St::DisconnectAccount,Btn::Ghost)));
        self.vstack(S4,rows)
    }
    fn studio_library(&self) -> El {
        let p = self.p();
        let mut left = vec![self.between(
            self.heading(format!("本机作品 · {}", self.studio.works.len())),
            self.st_button("st-refresh", "刷新", St::Refresh, Btn::Ghost),
        )];
        if self.studio.works.is_empty() {
            left.push(self.text(
                "保存官网的作品地址。体验链接可直接发送到手机，也可生成二维码分享。",
                BODY,
                p.text2,
            ));
        }
        for (i, w) in self.studio.works.iter().enumerate() {
            left.push(self.list_row(
                format!("st-work-{i}"),
                self.vstack(
                    S1,
                    vec![
                        self.heading(&w.name),
                        self.line(
                            format!(
                                "{} · {}",
                                w.category,
                                if w.shareable() {
                                    "体验链接"
                                } else {
                                    "创作项目"
                                }
                            ),
                            CAPTION,
                            p.text2,
                        ),
                    ],
                ),
                self.studio.selected == Some(i),
                Some(Action::Studio(St::Select(i))),
                None,
            ));
        }
        left.push(self.st_button("st-new", "添加作品", St::New, Btn::Secondary));
        let editor = self.panel(vec![
            self.between(
                self.heading(if self.studio.selected.is_some() {
                    "作品地址"
                } else {
                    "添加作品"
                }),
                self.st_button("st-paste-link", "粘贴链接", St::PasteLink, Btn::Ghost),
            ),
            self.segmented(
                "st-category",
                CATEGORIES
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        (
                            (*c).into(),
                            self.studio.category == i,
                            Action::Studio(St::Category(i)),
                        )
                    })
                    .collect(),
            ),
            self.input(F_STUDIO_NAME, "作品名称", None),
            self.input(F_STUDIO_URL, "粘贴官网的体验链接或构建器地址", None),
            self.between(
                self.text(
                    "官网 → 真机测试或生成体验二维码 → 复制链接",
                    CAPTION,
                    p.text3,
                ),
                self.st_button("st-save", "保存链接", St::Save, Btn::Secondary),
            ),
        ]);
        let mut right = vec![editor];
        if let Some(w) = self.studio.selected.and_then(|i| self.studio.works.get(i)) {
            let mut details = vec![
                self.between(
                    self.heading(&w.name),
                    self.pill_badge(
                        if w.shareable() {
                            "可上机体验"
                        } else {
                            "编辑地址"
                        },
                        p.text2,
                        p.surface,
                    ),
                ),
                self.text(&w.url, CAPTION, p.text2),
            ];
            if w.shareable() {
                details.push(self.wrap(vec![
                    self.st_button("st-send", "发送到手机", St::Send, Btn::Primary),
                    self.st_button("st-open-phone", "在手机打开", St::OpenPhone, Btn::Secondary),
                    self.st_button("st-pin", "添加桌面图标", St::Pin, Btn::Secondary),
                    self.st_button("st-share", "分享二维码", St::Share, Btn::Secondary),
                ]));
                details.push(self.text(
                    "保存在手机 StepX伴侣的作品库。网页需要联网；添加图标可能需要在手机确认。",
                    CAPTION,
                    p.text3,
                ));
            } else {
                details.push(self.text(
                    match w.kind {
                        LinkKind::Project => {
                            "这是创作项目，需要官网账号。生成官方体验链接后再上机或分享。"
                        }
                        _ => {
                            "这是编辑器预览地址。请使用官网生成的体验链接，避免分享需要登录的页面。"
                        }
                    },
                    BODY,
                    p.text2,
                ));
            }
            details.push(self.wrap(vec![
                self.button(
                    "st-edit-web",
                    "电脑打开",
                    Action::StudioOpenUrl(w.url.clone()),
                    Btn::Ghost,
                ),
                self.button(
                    "st-copy-link",
                    "复制链接",
                    Action::CopyText(w.url.clone(), "已复制作品地址".into()),
                    Btn::Ghost,
                ),
            ]));
            if let Some(note) = &self.studio.phone_note {
                details.push(self.text(note, CAPTION, p.text2));
            }
            right.push(self.panel(details));
            if let Some((id, path)) = &self.studio.qr {
                if id == &w.id {
                    let qr = ConstrainedBox::new(
                        Image::new(
                            AssetSource::LocalFile {
                                path: path.to_string_lossy().into_owned(),
                            },
                            CacheOption::BySize,
                        )
                        .contain()
                        .finish(),
                    )
                    .with_width(180.)
                    .with_height(180.)
                    .finish();
                    right.push(self.panel(vec![self.heading("分享作品"),self.hstack(S5,vec![qr,Expanded::new(1.,self.vstack(S3,vec![self.text(format!("扫码体验「{}」",w.name),BODY,p.text),self.text("发送给朋友前请确认作品中没有私人数据。二维码使用官方体验链接，过期后请在官网重新生成。",CAPTION,p.text2),self.wrap(vec![self.st_button("st-export-qr","保存二维码",St::SaveQr,Btn::Secondary),self.button("st-copy-share","复制分享文案",Action::CopyText(format!("试试我用 Step Studio 制作的「{}」\n{}",w.name,w.url),"已复制分享文案".into()),Btn::Secondary)])])).finish()]) ]));
                }
            }
        }
        right.push(self.wrap(vec![
            self.st_button(
                "st-phone-library",
                "手机作品库",
                St::PhoneLibrary,
                Btn::Ghost,
            ),
            self.st_button("st-amoo-works", "Amoo 我的作品", St::AmooWorks, Btn::Ghost),
            self.st_button("st-check-phone", "核对手机状态", St::CheckPhone, Btn::Ghost),
        ]));
        Flex::row()
            .with_spacing(S5)
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(
                ConstrainedBox::new(self.panel(left))
                    .with_width(240.)
                    .finish(),
            )
            .with_child(Expanded::new(1., self.vstack(S4, right)).finish())
            .finish()
    }
    fn studio_create(&self) -> El {
        let p = self.p();
        let mut rows=vec![self.heading("从一个真正能用的小作品开始"),self.text("先在官网创作，再回到这里上机验证和分享。下面提供可修改的需求起点，不是已经生成的成品。",BODY,p.text2)];
        for (i, (title, desc, _)) in BRIEFS.iter().enumerate() {
            rows.push(self.panel(vec![
                self.between(
                    self.vstack(
                        S1,
                        vec![self.heading(*title), self.line(*desc, CAPTION, p.text2)],
                    ),
                    self.st_button(
                        &format!("st-brief-{i}"),
                        "复制需求并打开",
                        St::Brief(i),
                        Btn::Secondary,
                    ),
                ),
                self.text(
                    match i {
                        0 => "重点验收：添加与撤销、导出、重新打开后数据保留。",
                        1 => "重点验收：缺少信息、没有权限、任务中途修改。",
                        _ => "重点验收：模糊图片、重复票据、真实文件交付。",
                    },
                    CAPTION,
                    p.text3,
                ),
            ]));
        }
        rows.push(self.panel(vec![self.heading("把手机变成测试设备"),self.text("应用：先验证小屏布局、键盘、返回与数据保存。智能体与技能：在官网配置平台可用的连接器，真机测试后到 Amoo 我的作品安装。背屏与 GUI 等手机能力需通过固件支持的工具接入，普通网页不会自动获得系统权限。",BODY,p.text2),self.st_button("st-go-guide","查看上机流程",St::Tab(3),Btn::Secondary)]));
        self.vstack(S4, rows)
    }
    fn studio_guide(&self) -> El {
        let p = self.p();
        self.vstack(S4,vec![
            self.panel(vec![self.heading("手机账号与官网登录"),self.text("使用与手机 Amoo 相同的账号登录官方 Studio。desktop 调用系统默认浏览器，浏览器会保留自己的登录状态；下次打开可继续使用。",BODY,p.text2),self.text("连接手机只代表设备已授权。手机系统授权后可自动同步 Amoo 我的作品，令牌不会传到电脑。官网编辑会话独立，当前固件的手机令牌不能直接登录官网；浏览器登录一次后保留其自己的会话。",CAPTION,p.text3),self.button("st-login","打开官方登录入口",Action::StudioOpenUrl("https://studio.stepx.com/".into()),Btn::Secondary)]),
            self.panel(vec![self.heading("应用与网页游戏"),self.text("1. 在官网创建应用，检查预览并生成“真机测试 / 体验二维码”。\n2. 回到“手机账号作品”自动同步。\n3. 选择作品并点击“在手机打开”，测试触摸、返回、横竖屏与网络。\n4. 满意后保存分享二维码；需要长期使用时在官网发布并更新作品链接。",BODY,p.text2),self.text("发送链接不会伪装成安装 APK；原生安装包按官网提示安装。体验链接可能失效，手机作品库会保留入口以便更新。",CAPTION,p.text3)]),
            self.panel(vec![self.heading("智能体与技能"),self.text("1. 在官网切换智能体或技能构建器，选择需要的连接器。\n2. 完成调试，使用官网“真机测试”。\n3. 同账号手机打开 Amoo → 我的作品，查看构建状态并确认安装。\n4. 回到 Amoo 对话验收，核对实际结果。",BODY,p.text2),self.st_button("st-guide-amoo","打开手机我的作品",St::AmooWorks,Btn::Primary)]),
            self.panel(vec![self.heading("断开电脑，也能用"),self.text("已发送的体验链接保存在 StepX伴侣 → 任务 → Studio 作品，可打开、分享、添加桌面图标。这里只管理作品入口；平台发布、云端构建和账号权限仍由官方 Studio 管理。",BODY,p.text2),self.wrap(vec![self.st_button("st-guide-phone","打开手机作品库",St::PhoneLibrary,Btn::Secondary),self.st_button("st-guide-check","检查手机支持",St::CheckPhone,Btn::Secondary)])]),
        ])
    }
}
