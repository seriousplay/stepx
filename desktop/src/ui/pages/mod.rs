//! One module per page. Each page exposes `<name>_page()` and `<name>_actions()`.

mod about;
mod amoo;
mod apps;
mod backscreen;
mod connect;
mod files;
mod overview;
mod phone;
mod sms;
mod studio;
mod tools;
mod work;
mod work_rich_text;

use super::{theme::*, widgets::*, Companion, Page};
use facloud_ui::elements::*;

impl Companion {
    /// Phone-uploaded documents may retain a shared-storage path rather than a
    /// remote URL. Keep that provenance and save through the verified ADB path.
    pub(in super::super) fn amoo_phone_attachment_card(
        &self,
        name: &str,
        kind: &str,
        mime: &str,
        url: &str,
        path: &str,
        size: Option<u64>,
    ) -> El {
        if !url.is_empty()
            || !path.starts_with("/sdcard/")
            || stepx_companion::storage::validate_path(path).is_err()
        {
            return self.amoo_attachment_card(name, kind, mime, url, size);
        }
        let p = self.p();
        let icon = if kind == "image" || mime.starts_with("image/") {
            super::icons::UiIcon::Image
        } else {
            super::icons::UiIcon::File
        };
        let meta = [
            "手机附件".to_string(),
            mime.to_string(),
            size.map(stepx_companion::storage::format_bytes)
                .unwrap_or_default(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        self.panel(vec![
            self.hstack(
                S2,
                vec![
                    self.ui_icon(icon, 20., p.text2),
                    Expanded::new(1., self.heading(name)).finish(),
                ],
            ),
            self.text(meta, CAPTION, p.text3),
            self.text("来自手机共享存储，可以保存到电脑查看。", CAPTION, p.text2),
            self.wrap(vec![self.button(
                format!("amoo-phone-attachment-save-{path}"),
                "另存为",
                super::Action::AmooUi(super::AmooUiAction::DownloadPhoneAttachment {
                    path: path.into(),
                    name: name.into(),
                }),
                Btn::Secondary,
            )]),
        ])
    }

    pub(in super::super) fn amoo_attachment_card(
        &self,
        name: &str,
        kind: &str,
        mime: &str,
        url: &str,
        size: Option<u64>,
    ) -> El {
        let p = self.p();
        let is_image = kind == "image" || mime.starts_with("image/");
        let icon = if is_image {
            super::icons::UiIcon::Image
        } else {
            super::icons::UiIcon::File
        };
        let meta = [
            mime.to_string(),
            size.map(stepx_companion::storage::format_bytes)
                .unwrap_or_default(),
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        let mut body = vec![
            self.hstack(
                S2,
                vec![
                    self.ui_icon(icon, 20., p.text2),
                    Expanded::new(1., self.heading(name)).finish(),
                ],
            ),
            self.line(meta, CAPTION, p.text3),
        ];
        if url.starts_with("https://") || url.starts_with("http://") {
            if is_image {
                if let Some(path) = self
                    .work
                    .image_previews
                    .get(url)
                    .filter(|path| path.is_file())
                {
                    body.push(
                        ConstrainedBox::new(
                            Image::new(
                                facloud_ui::assets::asset_cache::AssetSource::LocalFile {
                                    path: path.to_string_lossy().into_owned(),
                                },
                                CacheOption::BySize,
                            )
                            .contain()
                            .finish(),
                        )
                        .with_max_width(440.)
                        .with_height(240.)
                        .finish(),
                    );
                } else {
                    body.push(self.button(
                        format!("attachment-preview-{url}"),
                        if self.work.preview_errors.contains_key(url) {
                            "重新载入图片"
                        } else {
                            "载入图片"
                        },
                        super::Action::Work(super::WorkAction::PreviewAttachment {
                            name: name.into(),
                            kind: kind.into(),
                            mime: mime.into(),
                            url: url.into(),
                        }),
                        Btn::Ghost,
                    ));
                    if let Some(error) = self.work.preview_errors.get(url) {
                        body.push(self.text(error, CAPTION, p.text3));
                    }
                }
            }
            let mut actions = vec![self.button(
                format!("attachment-save-{url}"),
                "另存为",
                super::Action::Work(super::WorkAction::SaveAttachment {
                    name: name.into(),
                    kind: kind.into(),
                    mime: mime.into(),
                    url: url.into(),
                }),
                Btn::Secondary,
            )];
            if self.work.image_previews.contains_key(url) {
                actions.push(self.button(
                    format!("attachment-view-{url}"),
                    "查看原图",
                    super::Action::Work(super::WorkAction::OpenPreview(url.into())),
                    Btn::Ghost,
                ));
            } else if !url.starts_with("http://127.0.0.1:") {
                actions.push(self.button(
                    format!("attachment-open-{url}"),
                    "打开附件",
                    super::Action::Work(super::WorkAction::OpenRemote(url.into())),
                    Btn::Ghost,
                ));
            }
            actions.push(self.button(
                format!("attachment-copy-{url}"),
                "复制链接",
                super::Action::CopyText(url.into(), "已复制手机附件链接".into()),
                Btn::Ghost,
            ));
            body.push(self.wrap(actions));
        } else {
            body.push(self.text(
                "该附件尚未返回可下载地址，可在原会话中继续处理。",
                CAPTION,
                p.text3,
            ));
        }
        self.panel(body)
    }

    pub(super) fn page_content(&self) -> El {
        match self.page {
            Page::Overview => self.overview_page(),
            Page::BackScreen => self.backscreen_page(),
            Page::Amoo => self.amoo_page(),
            Page::Account => self.account_usage_page(),
            Page::About => self.about_page(),
            Page::Work => self.work_page(),
            Page::Studio => self.studio_page(),
            Page::Sms => self.sms_page(),
            Page::Phone => self.phone_page(),
            Page::Files => self.files_page(),
            Page::Apps => self.apps_page(),
            Page::Tools => self.tools_page(),
            Page::Connect => self.connect_page(),
        }
    }

    /// Buttons on the right of the page header. At most one `Primary`.
    pub(super) fn page_actions(&self) -> Vec<El> {
        match self.page {
            Page::About => Vec::new(),
            Page::Overview => self.overview_actions(),
            Page::BackScreen => self.backscreen_actions(),
            Page::Amoo => self.amoo_actions(),
            Page::Account => vec![self.button(
                "account-refresh",
                "刷新用量",
                super::Action::AmooUi(super::AmooUiAction::RefreshUsage),
                Btn::Secondary,
            )],
            Page::Work => self.work_actions(),
            Page::Studio => self.studio_actions(),
            Page::Sms => self.sms_actions(),
            Page::Phone => self.phone_actions(),
            Page::Files => self.files_actions(),
            Page::Apps => self.apps_actions(),
            Page::Tools => self.tools_actions(),
            Page::Connect => self.connect_actions(),
        }
    }

    /// Chat bubble shared by Amoo and SMS. `mine` aligns right with the accent tint.
    pub(super) fn bubble(&self, mine: bool, meta: String, body: El, footer: Option<El>) -> El {
        let p = self.p();
        let mut inner = vec![body];
        if let Some(f) = footer {
            inner.push(f);
        }
        let bubble = Container::new(
            Flex::column()
                .with_spacing(S1 + 2.)
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_children(inner)
                .finish(),
        )
        .with_horizontal_padding(S3)
        .with_vertical_padding(S2 + 1.)
        .with_background_color(if mine { p.accent_soft } else { p.surface })
        .with_border(Border::all(1.).with_border_color(if mine { p.accent_soft } else { p.border }))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL + 2.)))
        .finish();
        let column = Flex::column()
            .with_spacing(S1)
            .with_cross_axis_alignment(if mine {
                CrossAxisAlignment::End
            } else {
                CrossAxisAlignment::Start
            })
            .with_child(self.text(meta, MICRO, p.text3))
            .with_child(ConstrainedBox::new(bubble).with_max_width(560.).finish())
            .finish();
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(if mine {
                MainAxisAlignment::End
            } else {
                MainAxisAlignment::Start
            })
            .with_child(column)
            .finish()
    }
}

/// "14:05" for today, "昨天 14:05", otherwise "10-18 14:05".
pub(super) fn format_time(ms: i64) -> String {
    use chrono::{Local, TimeZone};
    let Some(t) = Local.timestamp_millis_opt(ms).single() else {
        return String::new();
    };
    let today = Local::now().date_naive();
    let day = t.date_naive();
    if day == today {
        t.format("%H:%M").to_string()
    } else if today.pred_opt() == Some(day) {
        t.format("昨天 %H:%M").to_string()
    } else {
        t.format("%m-%d %H:%M").to_string()
    }
}
