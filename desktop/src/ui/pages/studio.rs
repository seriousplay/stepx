use super::super::{
    icons::UiIcon,
    studio_state::{StudioAction as St, CATEGORIES},
    theme::*,
    widgets::*,
    Action, Companion, F_STUDIO_NAME, F_STUDIO_SEARCH, F_STUDIO_URL,
};
use facloud_ui::{assets::asset_cache::AssetSource, elements::*};
use stepx_companion::studio::{CloudWork, Work};

impl Companion {
    pub(in super::super) fn studio_actions(&self) -> Vec<El> {
        vec![
            self.st_button("st-sync", "同步", St::CloudRefresh, Btn::Secondary),
            if self.studio_lab.handle.is_some() {
                self.st_button("st-create", "查看运行任务", St::Tab(2), Btn::Secondary)
            } else if self.busy {
                self.disabled_button("新建作品")
            } else {
                self.button_icon(
                    "st-create",
                    "新建作品",
                    UiIcon::Plus,
                    Action::StudioLab(super::super::studio_lab_state::StudioLabAction::Fresh),
                    Btn::Secondary,
                )
            },
        ]
    }
    fn st_button(&self, id: &str, label: &str, action: St, style: Btn) -> El {
        let navigation = matches!(
            action,
            St::Tab(_)
                | St::Source(_)
                | St::CloudCategory(_)
                | St::CloudSelect(_)
                | St::WebSelect(_)
                | St::Select(_)
                | St::ListPage(_)
                | St::Edit(_)
        );
        if self.busy && !navigation {
            self.disabled_button(label)
        } else {
            let icon = match &action {
                St::CloudRefresh | St::Refresh => Some(UiIcon::Refresh),
                St::New => Some(UiIcon::Plus),
                St::UsePhone(_) | St::OpenPhone | St::PhoneLibrary => Some(UiIcon::Smartphone),
                St::SaveQr => Some(UiIcon::Download),
                St::PasteLink => Some(UiIcon::Clipboard),
                _ => None,
            };
            match icon {
                Some(icon) => self.button_icon(id, label, icon, Action::Studio(action), style),
                None => self.button(id, label, Action::Studio(action), style),
            }
        }
    }
    pub(in super::super) fn studio_page(&self) -> El {
        let p = self.p();
        let tabs = self.segmented(
            "studio-tabs",
            [(2, "工作台"), (0, "作品"), (3, "连接设置")]
                .iter()
                .map(|(i, t)| {
                    (
                        (*t).into(),
                        self.studio.tab == *i,
                        Action::Studio(St::Tab(*i)),
                    )
                })
                .collect(),
        );
        let web = self.studio.web.as_ref().is_some_and(|c| c.authorized);
        let phone = self.studio.cloud.as_ref().is_some_and(|c| c.authorized);
        let connection = if web {
            "官网已连接"
        } else if phone {
            "手机作品已连接"
        } else {
            "等待连接"
        };
        self.vstack(
            S5,
            vec![
                self.between(
                    self.wrap(vec![tabs]),
                    self.line(
                        connection,
                        CAPTION,
                        if web || phone { p.success } else { p.text3 },
                    ),
                ),
                match self.studio.tab {
                    2 => self.studio_lab_page(),
                    3 => self.studio_connections(),
                    _ => self.studio_workspace(),
                },
            ],
        )
    }
    fn studio_workspace(&self) -> El {
        let p = self.p();
        let source = self.studio.source;
        let count = |web: bool| {
            if web {
                self.studio.web.as_ref()
            } else {
                self.studio.cloud.as_ref()
            }
            .filter(|c| c.authorized)
            .map(|c| c.items.len())
        };
        let label = |title: &str, n: Option<usize>| {
            n.map(|n| format!("{title} · {n}")).unwrap_or(title.into())
        };
        let sources = self.segmented(
            "st-sources",
            [
                label("官网项目", count(true)),
                label("手机可用", count(false)),
                format!("我的收藏 · {}", self.studio.works.len()),
            ]
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (
                    t.clone(),
                    source == i as u8,
                    Action::Studio(St::Source(i as u8)),
                )
            })
            .collect(),
        );
        let mut content = vec![self.between(
            self.wrap(vec![sources]),
            self.st_button("st-add-link", "添加链接", St::New, Btn::Ghost),
        )];
        let query = self.fields[F_STUDIO_SEARCH].text.trim().to_lowercase();
        content.push(
            self.between(
                self.input(F_STUDIO_SEARCH, "搜索作品名称或描述…", None),
                self.segmented(
                    "st-types",
                    [(3, "全部"), (0, "应用"), (1, "智能体"), (2, "技能")]
                        .iter()
                        .map(|(i, t)| {
                            (
                                (*t).into(),
                                self.studio.cloud_category == *i,
                                Action::Studio(St::CloudCategory(*i)),
                            )
                        })
                        .collect(),
                ),
            ),
        );
        if self.studio.editing {
            content.push(self.studio_link_form());
        }
        if source == 2 {
            content.push(self.studio_saved(&query));
            return self.vstack(S4, content);
        }
        let library = if source == 0 {
            self.studio.web.as_ref()
        } else {
            self.studio.cloud.as_ref()
        };
        let error = if source == 0 {
            self.studio.web_error.as_ref()
        } else {
            self.studio.cloud_error.as_ref()
        };
        if let Some(e) = error {
            content.push(self.panel(vec![
                self.heading("暂未同步成功"),
                self.text(e, BODY, p.text2),
                self.st_button("st-retry", "重新同步", St::CloudRefresh, Btn::Secondary),
            ]));
        }
        let Some(library) = library.filter(|c| c.authorized) else {
            content.push(self.panel(vec![
                self.ui_icon(if source==0{UiIcon::Monitor}else{UiIcon::Smartphone},36.,p.text2),
                self.heading(if source==0{"把官网项目带到工作台"}else{"连接 Amoo 的手机作品"}),
                self.text(if source==0{"在手机伴侣中完成一次 Studio 官网登录，应用、技能与智能体项目会自动出现在这里，包含尚未提交真机测试的草稿。"}else{"使用手机已有的 Amoo 账号授权，自动读取已提交真机测试的作品。"},BODY,p.text2),
                if self.selected.is_some(){self.st_button("st-connect-source",if source==0{"连接 Studio 官网"}else{"连接手机账号"},if source==0{St::WebsiteLogin}else{St::ConnectAccount},Btn::Primary)}else{self.button("st-device","连接手机",Action::Page(super::super::Page::Connect),Btn::Primary)},
                self.text(if source==0{"官网仍要求首次身份验证；连接 ADB 不代表已登录官网。登录会话保留在手机伴侣中。"}else{"官网创作 → 真机测试 → 自动同步 → 手机验收"},CAPTION,p.text3),
            ]));
            return self.vstack(S4, content);
        };
        let mut filtered: Vec<_> = library
            .items
            .iter()
            .enumerate()
            .filter(|(_, w)| {
                self.studio_matches(
                    &w.category,
                    &format!("{} {}", w.name, w.description),
                    &query,
                )
            })
            .collect();
        filtered.sort_by(|a, b| b.1.updated.cmp(&a.1.updated).then(a.1.name.cmp(&b.1.name)));
        let current = if source == 0 {
            self.studio.web_selected
        } else {
            self.studio.cloud_selected
        };
        let page = self
            .studio
            .list_page
            .min(filtered.len().saturating_sub(1) / 12);
        let selected = filtered
            .iter()
            .skip(page * 12)
            .take(12)
            .find(|(i, _)| Some(*i) == current)
            .copied()
            .or_else(|| filtered.get(page * 12).copied());
        let mut list = vec![];
        for (i, w) in filtered.iter().skip(page * 12).take(12) {
            list.push(
                self.list_row(
                    format!("st-item-{source}-{i}"),
                    self.between(
                        self.lead(
                            S3,
                            self.ui_icon(Self::studio_icon(&w.category), 20., p.text2),
                            Expanded::new(
                                1.,
                                self.vstack(
                                    S1,
                                    vec![
                                        self.heading(&w.name),
                                        self.line(
                                            format!(
                                                "{} · {}",
                                                w.category,
                                                if source == 0 {
                                                    "官网项目"
                                                } else if w.experience().is_some() {
                                                    "可上机体验"
                                                } else {
                                                    "在 Amoo 管理"
                                                }
                                            ),
                                            MICRO,
                                            p.text3,
                                        ),
                                    ],
                                ),
                            )
                            .finish(),
                        ),
                        self.ui_icon(UiIcon::ChevronRight, 14., p.text3),
                    ),
                    selected.is_some_and(|(j, _)| j == *i),
                    Some(Action::Studio(if source == 0 {
                        St::WebSelect(*i)
                    } else {
                        St::CloudSelect(*i)
                    })),
                    None,
                ),
            );
        }
        if list.is_empty() {
            content.push(self.studio_empty(
                if query.is_empty() {
                    "此分类还没有作品"
                } else {
                    "没有匹配的作品"
                },
                if query.is_empty() {
                    "从“新建作品”开始创作；官网完成真机测试后，可以在手机验收。"
                } else {
                    "试试其他名称，或切换到“全部”。"
                },
            ));
        } else {
            let (selected_index, selected_work) = selected.unwrap();
            let details = self.studio_cloud_detail(selected_work, source, selected_index);
            let mut left = vec![self.list(list)];
            if filtered.len() > 12 {
                left.push(self.between(
                    self.st_button(
                        "st-prev",
                        "上一页",
                        St::ListPage(page.saturating_sub(1)),
                        Btn::Ghost,
                    ),
                    self.hstack(
                        S2,
                        vec![
                            self.line(
                                format!("{} / {}", page + 1, filtered.len().div_ceil(12)),
                                MICRO,
                                p.text3,
                            ),
                            self.st_button(
                                "st-next",
                                "下一页",
                                St::ListPage((page + 1).min(filtered.len().saturating_sub(1) / 12)),
                                Btn::Ghost,
                            ),
                        ],
                    ),
                ));
            }
            content.push(self.studio_columns(self.vstack(S3, left), details));
        }
        let time = chrono::DateTime::from_timestamp(library.synced_at, 0)
            .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
            .unwrap_or_default();
        content.push(self.line(
            format!(
                "{} 个作品  ·  {} 更新  ·  当前页面自动同步",
                filtered.len(),
                time
            ),
            MICRO,
            p.text3,
        ));
        self.vstack(S4, content)
    }
    fn studio_matches(&self, category: &str, text: &str, query: &str) -> bool {
        (self.studio.cloud_category == 3 || category == CATEGORIES[self.studio.cloud_category])
            && (query.is_empty() || format!("{category} {text}").to_lowercase().contains(query))
    }
    fn studio_icon(category: &str) -> UiIcon {
        match category {
            "技能" => UiIcon::Bolt,
            "智能体" => UiIcon::Star,
            _ => UiIcon::Apps,
        }
    }
    fn studio_columns(&self, list: El, details: El) -> El {
        Flex::row()
            .with_spacing(S5)
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(Expanded::new(1., list).finish())
            .with_child(Expanded::new(1.25, details).finish())
            .finish()
    }
    fn studio_empty(&self, title: &str, description: &str) -> El {
        self.panel(vec![
            self.ui_icon(UiIcon::Folder, 28., self.p().text3),
            self.heading(title),
            self.text(description, BODY, self.p().text2),
        ])
    }
    fn studio_cloud_detail(&self, w: &CloudWork, source: u8, index: usize) -> El {
        let p = self.p();
        let website = source == 0;
        let title = self.between(
            self.lead(
                S3,
                self.ui_icon(Self::studio_icon(&w.category), 32., p.text),
                Expanded::new(
                    1.,
                    self.vstack(
                        S1,
                        vec![
                            self.heading(&w.name),
                            self.line(&w.category, CAPTION, p.text3),
                        ],
                    ),
                )
                .finish(),
            ),
            self.pill_badge(
                if website {
                    "官网项目"
                } else if w.experience().is_some() {
                    "可验收"
                } else {
                    "手机作品"
                },
                p.text2,
                p.surface,
            ),
        );
        let mut children = vec![
            title,
            self.text(
                if w.description.is_empty() {
                    "打开作品，继续你的创作。"
                } else {
                    &w.description
                },
                BODY,
                p.text2,
            ),
            self.divider(),
        ];
        if website {
            if let Some(url) = w.editor() {
                children.push(self.button_icon(
                    "st-edit-project",
                    "继续编辑",
                    UiIcon::ExternalLink,
                    Action::StudioOpenUrl(url),
                    Btn::Primary,
                ));
            }
            children.push(self.text(
                "直接在独立工作窗口打开对应项目，自动复用伴侣内的官网登录。",
                CAPTION,
                p.text3,
            ));
            let phone = self.studio.cloud.as_ref().and_then(|c| {
                c.items
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.id == w.id && p.category == w.category)
            });
            if let Some((i, item)) = phone {
                children.push(self.heading("手机验收"));
                children.push(self.text("已在 Amoo 我的作品找到此项目。", BODY, p.text2));
                children.push(self.st_button(
                    "st-switch-phone",
                    "查看手机版本",
                    St::PhoneItem(i),
                    Btn::Secondary,
                ));
                if item.experience().is_none() {
                    children.push(self.caption("技能与智能体需要在 Amoo 中确认安装。"));
                }
            } else {
                children.push(self.heading("下一步 · 真机测试"));
                children.push(self.text("在官网点击“真机测试”，测试包构建后可同步到手机验收。测试包已构建不代表手机运行已验证；未生成体验链接的草稿不会作为公开作品分享。",BODY,p.text2));
            }
            children.push(self.st_button(
                "st-keep-project",
                "收藏项目入口",
                St::KeepWeb(index),
                Btn::Ghost,
            ));
        } else {
            children.push(self.st_button(
                "st-use-phone",
                if w.experience().is_some() {
                    "在手机打开"
                } else {
                    "在 Amoo 管理"
                },
                St::UsePhone(index),
                Btn::Primary,
            ));
            let mut actions = vec![];
            if w.experience().is_some() {
                actions.push(self.st_button(
                    "st-share-phone",
                    "分享作品",
                    St::KeepPhone(index),
                    Btn::Secondary,
                ));
            }
            if let Some(url) = w.editor() {
                actions.push(self.button_icon(
                    "st-edit-phone",
                    "继续编辑",
                    UiIcon::ExternalLink,
                    Action::StudioOpenUrl(url),
                    Btn::Secondary,
                ));
            }
            if !actions.is_empty() {
                children.push(self.wrap(actions));
            }
            if !w.version.is_empty() {
                children.push(self.line(format!("版本 {}", w.version), CAPTION, p.text3));
            }
            children.push(self.text(
                if w.experience().is_some() {
                    "打开后会保留手机入口；官方体验作品是否需要联网，由作品本身决定。"
                } else {
                    "安装和更新由 Amoo 官方页面完成，请在手机确认权限。"
                },
                CAPTION,
                p.text3,
            ));
        }
        if let Some(note) = &self.studio.phone_note {
            children.push(self.caption(note));
        }
        self.panel(children)
    }
    fn studio_link_form(&self) -> El {
        self.panel(vec![
            self.between(
                self.heading("添加作品链接"),
                self.st_button("st-cancel-link", "收起", St::Edit(false), Btn::Ghost),
            ),
            self.input(F_STUDIO_NAME, "作品名称", None),
            self.input(F_STUDIO_URL, "官方项目或体验链接", None),
            self.between(
                self.segmented(
                    "st-link-kind",
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
                self.wrap(vec![
                    self.st_button("st-paste", "粘贴链接", St::PasteLink, Btn::Ghost),
                    self.st_button("st-save", "保存", St::Save, Btn::Primary),
                ]),
            ),
        ])
    }
    fn studio_saved(&self, query: &str) -> El {
        let p = self.p();
        let filtered: Vec<_> = self
            .studio
            .works
            .iter()
            .enumerate()
            .filter(|(_, w)| self.studio_matches(&w.category, &w.name, query))
            .collect();
        if filtered.is_empty() {
            return self.studio_empty(
                "还没有收藏的作品",
                "在官网项目中收藏编辑入口，或从手机作品中选择分享；也可以添加官方体验链接。",
            );
        }
        let selected = filtered
            .iter()
            .skip(
                self.studio
                    .list_page
                    .min(filtered.len().saturating_sub(1) / 12)
                    * 12,
            )
            .take(12)
            .find(|(i, _)| Some(*i) == self.studio.selected)
            .copied();
        let page = self
            .studio
            .list_page
            .min(filtered.len().saturating_sub(1) / 12);
        let rows = filtered
            .iter()
            .skip(page * 12)
            .take(12)
            .map(|(i, w)| {
                self.list_row(
                    format!("st-saved-{i}"),
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
                                        "项目入口"
                                    }
                                ),
                                MICRO,
                                p.text3,
                            ),
                        ],
                    ),
                    self.studio.selected == Some(*i),
                    Some(Action::Studio(St::Select(*i))),
                    None,
                )
            })
            .collect();
        let mut left = vec![self.list(rows)];
        if filtered.len() > 12 {
            left.push(self.wrap(vec![
                self.st_button(
                    "st-saved-prev",
                    "上一页",
                    St::ListPage(page.saturating_sub(1)),
                    Btn::Ghost,
                ),
                self.st_button(
                    "st-saved-next",
                    "下一页",
                    St::ListPage((page + 1).min(filtered.len().saturating_sub(1) / 12)),
                    Btn::Ghost,
                ),
            ]));
        }
        self.studio_columns(
            self.vstack(S3, left),
            selected
                .map(|(_, w)| self.studio_saved_detail(w))
                .unwrap_or_else(|| {
                    self.studio_empty("选择一个作品", "打开、发送到手机，或生成分享二维码。")
                }),
        )
    }
    fn studio_saved_detail(&self, w: &Work) -> El {
        let p = self.p();
        let mut content = vec![
            self.heading(&w.name),
            self.line(
                format!(
                    "{} · {}",
                    w.category,
                    if w.shareable() {
                        "体验链接"
                    } else {
                        "私有编辑入口"
                    }
                ),
                CAPTION,
                p.text3,
            ),
        ];
        if w.shareable() {
            content.push(self.st_button(
                "st-open-saved",
                "在手机打开",
                St::OpenPhone,
                Btn::Primary,
            ));
            content.push(self.wrap(vec![
                self.st_button("st-qr", "分享二维码", St::Share, Btn::Secondary),
                self.st_button("st-pin", "添加手机桌面图标", St::Pin, Btn::Secondary),
            ]));
        } else {
            content.push(self.button_icon(
                "st-open-editor",
                "继续编辑",
                UiIcon::ExternalLink,
                Action::StudioOpenUrl(w.url.clone()),
                Btn::Primary,
            ));
            content.push(self.caption("项目入口需要官网账号，不能当作公开体验链接分享。"));
        }
        content.push(self.wrap(vec![
            self.button(
                "st-copy-url",
                "复制链接",
                Action::CopyText(w.url.clone(), "已复制作品地址".into()),
                Btn::Ghost,
            ),
            self.st_button("st-edit-link", "编辑收藏", St::Edit(true), Btn::Ghost),
        ]));
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
                content.extend(vec![
                    self.divider(),
                    qr,
                    self.caption("扫码体验；链接有效期与访问权限由官网决定。"),
                    self.wrap(vec![
                        self.st_button("st-save-qr", "保存二维码", St::SaveQr, Btn::Secondary),
                        self.button(
                            "st-copy-share",
                            "复制分享文案",
                            Action::CopyText(
                                format!("试试我用 Step Studio 制作的「{}」\n{}", w.name, w.url),
                                "已复制分享文案".into(),
                            ),
                            Btn::Secondary,
                        ),
                    ]),
                ]);
            }
        }
        if let Some(note) = &self.studio.phone_note {
            content.push(self.caption(note));
        }
        self.panel(content)
    }
    fn studio_connections(&self) -> El {
        let p = self.p();
        let web = self.studio.web.as_ref().filter(|c| c.authorized);
        let phone = self.studio.cloud.as_ref().is_some_and(|c| c.authorized);
        self.vstack(S4,vec![
            self.studio_lab_settings(),
            self.panel(vec![self.between(self.heading("Studio 官网"),self.pill_badge(if web.is_some(){"已连接"}else{"未登录"},p.text2,p.surface)),self.text(web.map(|c|format!("{} · {} 个官网项目",c.name,c.items.len())).unwrap_or("同步应用、技能与智能体项目，包含官网草稿。首次需在手机完成官网验证。".into()),BODY,p.text2),self.wrap(vec![self.st_button("st-web-login",if web.is_some(){"管理官网登录"}else{"连接官网"},St::WebsiteLogin,Btn::Primary),self.st_button("st-web-off","断开官网",St::WebsiteDisconnect,Btn::Ghost)]),self.caption("点击继续编辑会连接独立的桌面工作窗口，复用伴侣内的官网登录，无需再次登录；不会读取系统浏览器账号。")]),
            self.panel(vec![self.between(self.heading("Amoo 手机作品"),self.pill_badge(if phone{"已授权"}else{"未连接"},p.text2,p.surface)),self.text("读取已提交真机测试的作品。应用可直接上机，技能与智能体可前往 Amoo 确认安装。",BODY,p.text2),self.wrap(vec![self.st_button("st-native-login","连接手机账号",St::ConnectAccount,Btn::Secondary),self.st_button("st-native-off","断开作品同步",St::DisconnectAccount,Btn::Ghost)])]),
            self.panel(vec![self.heading("手机与分享"),self.text("作品发送到手机后可独立打开、分享和添加桌面图标。分享只使用官方体验地址；编辑入口和登录信息不会进入二维码。",BODY,p.text2),self.wrap(vec![self.st_button("st-phone-library","手机作品库",St::PhoneLibrary,Btn::Secondary),self.st_button("st-amoo-library","Amoo 我的作品",St::AmooWorks,Btn::Secondary),self.st_button("st-phone-check","检查连接",St::CheckPhone,Btn::Ghost)])]),
        ])
    }
}
