use super::super::{
    icons::UiIcon, theme::*, widgets::*, Action, Companion, ToolPage, F_APP_SEARCH,
};
use facloud_ui::{elements::*, fonts::Weight};
use stepx_companion::{apps::AppInfo, storage::format_bytes};

impl Companion {
    pub(in super::super) fn apps_actions(&self) -> Vec<El> {
        vec![
            self.button("apps-refresh", "刷新列表", Action::Apps, Btn::Secondary),
            self.button(
                "apps-install",
                "安装 APK…",
                Action::OpenTool(Some(ToolPage::Apk)),
                Btn::Primary,
            ),
        ]
    }

    fn installed_app_icon(&self, app: &AppInfo) -> El {
        let icon = if let Some(path) = app.icon_path.as_ref() {
            Image::new(
                facloud_ui::assets::asset_cache::AssetSource::LocalFile {
                    path: path.to_string_lossy().into_owned(),
                },
                CacheOption::BySize,
            )
            .contain()
            .finish()
        } else {
            Align::new(self.ui_icon(
                if app.package == "com.android.calendar" {
                    UiIcon::Calendar
                } else {
                    UiIcon::Package
                },
                26.,
                self.p().text3,
            ))
            .finish()
        };
        ConstrainedBox::new(icon)
            .with_width(40.)
            .with_height(40.)
            .finish()
    }

    fn installed_app_row(&self, app: &AppInfo) -> El {
        let p = self.p();
        let package = &app.package;
        let version = if app.version_name.is_empty() {
            format!("版本代码 {}", app.version_code)
        } else {
            format!("版本 {}", app.version_name)
        };
        let apk_size = if app.apk_bytes() > 0 {
            format!(" · 安装包 {}", format_bytes(app.apk_bytes()))
        } else {
            String::new()
        };
        let splits = if app.apk_files.len() > 1 {
            format!(" · {} 个 APK", app.apk_files.len())
        } else {
            String::new()
        };
        let kind = if !app.enabled {
            "已停用"
        } else if app.updated_system {
            "系统应用 · 已更新"
        } else if app.system {
            "系统应用"
        } else {
            "用户应用"
        };
        let info = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(S3)
            .with_child(self.installed_app_icon(app))
            .with_child(
                Expanded::new(
                    1.,
                    self.vstack(
                        S1,
                        vec![
                            self.line_w(&app.label, BODY, p.text, Weight::Semibold),
                            self.mono(package, MICRO, p.text2),
                            self.line(
                                format!(
                                    "{version}{apk_size}{splits}{}",
                                    if app.icon_path.is_none() && app.icon_error.is_some() {
                                        " · 手机图标暂不可用"
                                    } else {
                                        ""
                                    }
                                ),
                                MICRO,
                                p.text3,
                            ),
                        ],
                    ),
                )
                .finish(),
            )
            .finish();
        let heading = self.between(info, self.pill_badge(kind, p.text2, p.surface));
        let mut actions = vec![
            if app.launchable && app.enabled {
                self.button(
                    format!("app-open:{package}"),
                    "打开",
                    Action::OpenApp(package.clone()),
                    Btn::Secondary,
                )
            } else {
                self.disabled_button(if app.enabled {
                    "无启动入口"
                } else {
                    "已停用"
                })
            },
            self.button(
                format!("app-export:{package}"),
                "导出 APK",
                Action::ExportApp(package.clone()),
                Btn::Secondary,
            ),
            self.button(
                format!("app-settings:{package}"),
                "应用详情",
                Action::AppSettings(package.clone()),
                Btn::Secondary,
            ),
            self.button(
                format!("app-copy:{package}"),
                "复制包名",
                Action::CopyText(package.clone(), "已复制应用包名".into()),
                Btn::Ghost,
            ),
        ];
        if app.can_uninstall() {
            actions.push(self.button(
                format!("app-stop:{package}"),
                "强行停止",
                Action::ForceStopApp(package.clone()),
                Btn::Ghost,
            ));
            actions.push(self.button(
                format!("app-uninstall:{package}"),
                "卸载",
                Action::RequestUninstallApp(package.clone()),
                Btn::Danger,
            ));
        } else {
            actions.push(self.disabled_button("系统应用不可卸载"));
        }
        let mut contents = vec![heading, self.wrap(actions)];
        if self.pending_app_uninstall.as_deref() == Some(package.as_str()) {
            let confirmation = self.wrap(vec![
                self.text(
                    "卸载后，此应用在手机当前用户中的数据会被删除。",
                    CAPTION,
                    p.danger,
                ),
                self.button(
                    format!("app-cancel-uninstall:{package}"),
                    "取消",
                    Action::CancelUninstallApp,
                    Btn::Secondary,
                ),
                self.button(
                    format!("app-confirm-uninstall:{package}"),
                    "确认卸载",
                    Action::ConfirmUninstallApp(package.clone()),
                    Btn::DangerSolid,
                ),
            ]);
            contents.push(
                Container::new(confirmation)
                    .with_uniform_padding(S2)
                    .with_background_color(p.surface)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                    .finish(),
            );
        }
        Container::new(self.vstack(S3, contents))
            .with_horizontal_padding(S4)
            .with_vertical_padding(S3)
            .finish()
    }

    pub(in super::super) fn apps_page(&self) -> El {
        let p = self.p();
        let filtered = self.filtered_apps();
        let common_count = self
            .apps
            .iter()
            .filter(|app| app.in_default_scope())
            .count();
        let toolbar = self.wrap(vec![
            self.segmented(
                "app-scope",
                vec![
                    (
                        format!("常用应用 ({common_count})"),
                        !self.include_system_apps,
                        Action::ToggleSystemApps(false),
                    ),
                    (
                        format!("全部应用 ({})", self.apps.len()),
                        self.include_system_apps,
                        Action::ToggleSystemApps(true),
                    ),
                ],
            ),
            self.input(F_APP_SEARCH, "搜索应用名称或包名…", Some(260.)),
        ]);
        let description = self.line(
            format!(
                "显示 {} 项 · {} · 导出包含完整安装包，不包含应用数据",
                filtered.len(),
                if self.include_system_apps {
                    "含系统组件"
                } else {
                    "用户安装、可启动系统应用及 Amoo"
                }
            ),
            MICRO,
            p.text3,
        );
        let mut rows = Vec::new();
        for app in &filtered {
            if !rows.is_empty() {
                rows.push(self.divider());
            }
            rows.push(self.installed_app_row(app));
        }
        if rows.is_empty() {
            rows.push(
                Container::new(
                    Align::new(self.vstack(
                        S2,
                        vec![
                            self.line_w(
                                if self.apps_error.is_some() {
                                    "应用列表读取失败"
                                } else if !self.apps_loaded {
                                    "尚未读取应用"
                                } else if self.apps.is_empty() {
                                    "手机当前用户没有已安装应用"
                                } else {
                                    "没有匹配的应用"
                                },
                                HEADING,
                                p.text,
                                Weight::Medium,
                            ),
                            self.line(
                                if let Some(error) = self.apps_error.as_deref() {
                                    error
                                } else if !self.apps_loaded {
                                    "连接手机后点击刷新列表。"
                                } else if self.apps.is_empty() {
                                    "可以安装 APK，或刷新列表重新读取。"
                                } else {
                                    "调整搜索内容，或切换到全部应用。"
                                },
                                CAPTION,
                                p.text2,
                            ),
                        ],
                    ))
                    .finish(),
                )
                .with_uniform_padding(S6)
                .finish(),
            );
        }
        let scroll = self.pane_scroll(self.scroll.clone(), self.vstack(0., rows));
        let card = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(Expanded::new(1., scroll).finish())
                .finish(),
        )
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();
        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(S3)
            .with_child(toolbar)
            .with_child(description)
            .with_child(Expanded::new(1., card).finish())
            .finish()
    }
}
