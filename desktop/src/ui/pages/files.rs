use super::super::{
    icons::UiIcon, theme::*, widgets::*, Action, Companion, FileModal, FileSort, Page, F_FILE_NAME,
    F_FILE_SEARCH,
};
use facloud_ui::{color::ColorU, elements::*, fonts::Weight, platform::Cursor};

const PLACES: [(&str, &str); 6] = [
    ("主目录", "/sdcard"),
    ("下载", "/sdcard/Download"),
    ("相册", "/sdcard/DCIM"),
    ("截屏", "/sdcard/DCIM/Screenshots"),
    ("文档", "/sdcard/Documents"),
    ("回收站", "/sdcard/.stepx-trash"),
];

struct FileMock {
    name: &'static str,
    date: &'static str,
    kind: &'static str,
    size: &'static str,
    is_dir: bool,
    selected: bool,
}

const FILE_MOCKS: [FileMock; 7] = [
    FileMock {
        name: "Camera",
        date: "今天 14:10",
        kind: "文件夹",
        size: "--",
        is_dir: true,
        selected: false,
    },
    FileMock {
        name: "ScreenRecorder",
        date: "昨天 19:42",
        kind: "文件夹",
        size: "--",
        is_dir: true,
        selected: false,
    },
    FileMock {
        name: "Screenshot_20261021_142809.png",
        date: "今天 14:28",
        kind: "PNG 图像",
        size: "2.4 MB",
        is_dir: false,
        selected: true,
    },
    FileMock {
        name: "Screenshot_20261021_110522.png",
        date: "今天 11:05",
        kind: "PNG 图像",
        size: "1.8 MB",
        is_dir: false,
        selected: false,
    },
    FileMock {
        name: "StepX_Product_Demo_1080p.mp4",
        date: "10-19 16:30",
        kind: "MP4 视频",
        size: "48.2 MB",
        is_dir: false,
        selected: false,
    },
    FileMock {
        name: "StepX_Design_Spec_v2.pdf",
        date: "10-18 09:15",
        kind: "PDF 文档",
        size: "4.1 MB",
        is_dir: false,
        selected: false,
    },
    FileMock {
        name: "device_log_20261018.txt",
        date: "10-18 08:40",
        kind: "纯文本",
        size: "128 KB",
        is_dir: false,
        selected: false,
    },
];

/// Common Android media directories keep their names and gain a semantic icon.
fn file_symbol(entry: &stepx_companion::storage::Entry) -> (UiIcon, &'static str) {
    if entry.directory {
        let name = entry.name.to_ascii_lowercase();
        return match name.as_str() {
            "music" => (UiIcon::Music, "音乐文件夹"),
            "audiobooks" => (UiIcon::Music, "有声书文件夹"),
            "podcasts" => (UiIcon::Mic, "播客文件夹"),
            "recordings" | "recording" | "soundrecorder" => (UiIcon::Mic, "录音文件夹"),
            "movies" | "screenrecorder" => (UiIcon::Video, "视频文件夹"),
            "dcim" | "camera" => (UiIcon::Camera, "相机文件夹"),
            "pictures" | "screenshots" => (UiIcon::Image, "图片文件夹"),
            "download" | "downloads" => (UiIcon::Download, "下载文件夹"),
            "documents" => (UiIcon::File, "文档文件夹"),
            "alarms" => (UiIcon::Clock, "闹钟铃声"),
            "ringtones" | "notifications" => (UiIcon::Bell, "铃声文件夹"),
            "android" => (UiIcon::Settings, "应用数据目录"),
            ".stepx-trash" => (UiIcon::Trash, "回收站"),
            ".stepaossafebox" => (UiIcon::Lock, "保险箱目录"),
            _ => (UiIcon::Folder, entry.kind_badge()),
        };
    }
    (
        match entry.kind_badge() {
            "图片" => UiIcon::Image,
            "视频" => UiIcon::Video,
            "音频" => UiIcon::Music,
            "安装包" | "压缩包" => UiIcon::Package,
            _ => UiIcon::File,
        },
        entry.kind_badge(),
    )
}

impl Companion {
    pub(in super::super) fn files_actions(&self) -> Vec<El> {
        vec![
            self.button(
                "files-mkdir",
                "＋ 新建文件夹",
                Action::OpenNewFolder,
                Btn::Secondary,
            ),
            self.button(
                "files-refresh",
                "刷新",
                Action::Browse(self.directory.clone()),
                Btn::Secondary,
            ),
            self.button("files-upload", "↑ 上传文件", Action::Upload, Btn::Primary),
        ]
    }

    pub(in super::super) fn storage_mode_bar(&self) -> El {
        let p = self.p();
        let mode_seg = self.segmented(
            "storage-mode-bar",
            vec![
                (
                    "手机文件管理".into(),
                    self.page == Page::Files,
                    Action::Page(Page::Files),
                ),
                (
                    "电脑存储扩容".into(),
                    self.page == Page::Expansion,
                    Action::Page(Page::Expansion),
                ),
            ],
        );
        let archived_count = self.expansion.snapshot.status["count"]
            .as_u64()
            .unwrap_or_else(|| {
                self.expansion
                    .snapshot
                    .files
                    .iter()
                    .filter(|j| j["state"] == "archived")
                    .count() as u64
            });
        let right_info = self.hstack(
            S2,
            vec![
                if archived_count > 0 {
                    self.pill_badge(
                        format!("电脑已归档 {archived_count} 项"),
                        p.success_text,
                        p.success_soft,
                    )
                } else {
                    self.pill_badge("双向文件与无感扩容", p.text2, p.tag_bg)
                },
                self.line("手机文件管理 · 电脑存储扩容", MICRO, p.text3),
            ],
        );
        Container::new(self.between(mode_seg, right_info))
            .with_horizontal_padding(S3)
            .with_vertical_padding(6.)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
    }

    fn file_grid_row(
        &self,
        col_name: El,
        col_date: El,
        col_type: El,
        col_size: El,
        col_actions: El,
    ) -> El {
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(8.)
            .with_child(Expanded::new(5., col_name).finish())
            .with_child(Expanded::new(2., col_date).finish())
            .with_child(Expanded::new(2., col_type).finish())
            .with_child(
                Expanded::new(
                    1.,
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::End)
                        .with_child(col_size)
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                ConstrainedBox::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::End)
                        .with_child(col_actions)
                        .finish(),
                )
                .with_width(230.)
                .finish(),
            )
            .finish()
    }

    pub(in super::super) fn files_page(&self) -> El {
        let p = self.p();

        // 1. Top Bar: Quick Places + Breadcrumbs + Sort + Filter
        let place_btns: Vec<El> = PLACES
            .iter()
            .enumerate()
            .map(|(i, (label, path))| {
                let active = self.directory == *path;
                let state = self.mouse_state(format!("files-place:{i}"));
                let act = Action::Browse((*path).to_string());
                Hoverable::new(state, move |mouse| {
                    let hovered = mouse.is_mouse_over_element();
                    let bg = if active {
                        if p.dark {
                            rgb(0x27, 0x27, 0x2A)
                        } else {
                            rgb(0x18, 0x18, 0x1B)
                        }
                    } else if hovered {
                        p.hover
                    } else {
                        ColorU::transparent_black()
                    };
                    let fg = if active { ColorU::white() } else { p.text2 };
                    Container::new(self.line_w(*label, CAPTION, fg, Weight::Medium))
                        .with_horizontal_padding(12.)
                        .with_vertical_padding(5.)
                        .with_background_color(bg)
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                        .finish()
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(act.clone()))
                .finish()
            })
            .collect();

        let places_bar = Container::new(self.hstack(4., place_btns))
            .with_uniform_padding(4.)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish();

        let sort_seg = self.segmented(
            "files-sort-seg",
            vec![
                (
                    "按名称".into(),
                    self.file_sort == FileSort::Name,
                    Action::FileSort(FileSort::Name),
                ),
                (
                    "按大小".into(),
                    self.file_sort == FileSort::Size,
                    Action::FileSort(FileSort::Size),
                ),
                (
                    "按类型".into(),
                    self.file_sort == FileSort::Kind,
                    Action::FileSort(FileSort::Kind),
                ),
            ],
        );

        let breadcrumb = self.wrap(
            breadcrumbs(&self.directory)
                .into_iter()
                .enumerate()
                .flat_map(|(i, (label, path))| {
                    let mut nodes = Vec::new();
                    if i > 0 {
                        nodes.push(self.ui_icon(UiIcon::ChevronRight, 12., p.text3));
                    }
                    nodes.push(self.button(
                        format!("files-crumb-{i}"),
                        &label,
                        Action::Browse(path.clone()),
                        if path == self.directory {
                            Btn::Secondary
                        } else {
                            Btn::Ghost
                        },
                    ));
                    nodes
                })
                .collect(),
        );
        let navigation = self.hstack(
            S2,
            vec![
                if self.directory == "/sdcard" {
                    self.disabled_button("↑ 上一级")
                } else {
                    self.button_icon(
                        "files-up-inline",
                        "上一级",
                        UiIcon::ArrowUp,
                        Action::ParentDirectory,
                        Btn::Secondary,
                    )
                },
                breadcrumb,
            ],
        );
        let top_bar = self.vstack(
            S2,
            vec![
                places_bar,
                self.between(
                    navigation,
                    self.hstack(
                        S2,
                        vec![
                            sort_seg,
                            self.input(F_FILE_SEARCH, "筛选当前目录…", Some(180.)),
                        ],
                    ),
                ),
            ],
        );

        let mut modal_banners: Vec<El> = Vec::new();
        if let Some(modal) = &self.file_modal {
            let title = match modal {
                FileModal::NewFolder => "新建文件夹：",
                FileModal::Rename(_) => "重命名所选项目：",
            };
            modal_banners.push(
                Container::new(self.between(
                    self.hstack(
                        8.,
                        vec![
                            self.line_w(title, CAPTION, p.text, Weight::Semibold),
                            self.input(F_FILE_NAME, "输入新名称…", Some(260.)),
                        ],
                    ),
                    self.hstack(
                        8.,
                        vec![
                            self.button(
                                "file-modal-cancel",
                                "取消",
                                Action::CancelFileModal,
                                Btn::Secondary,
                            ),
                            self.button(
                                "file-modal-ok",
                                "确定",
                                Action::SubmitFileModal,
                                Btn::Primary,
                            ),
                        ],
                    ),
                ))
                .with_horizontal_padding(14.)
                .with_vertical_padding(8.)
                .with_background_color(p.bg)
                .with_border(Border::all(1.).with_border_color(p.accent))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish(),
            );
        }

        if self.trash_confirm {
            modal_banners.push(
                Container::new(self.between(
                    self.hstack(
                        8.,
                        vec![
                            self.dot(p.danger),
                            self.line_w(
                                if self.permanent_delete {
                                    "确认永久删除所选文件？此操作不可撤销。"
                                } else {
                                    "确认将所选文件移至 StepX 安全回收站 (/sdcard/.StepXTrash)？"
                                },
                                CAPTION,
                                p.text,
                                Weight::Medium,
                            ),
                        ],
                    ),
                    self.hstack(
                        8.,
                        vec![
                            self.button(
                                "trash-cancel",
                                "取消",
                                Action::CancelTrash,
                                Btn::Secondary,
                            ),
                            self.button(
                                "trash-confirm",
                                "确认执行",
                                Action::ConfirmTrash,
                                Btn::DangerSolid,
                            ),
                        ],
                    ),
                ))
                .with_horizontal_padding(14.)
                .with_vertical_padding(8.)
                .with_background_color(p.warning_soft)
                .with_border(Border::all(1.).with_border_color(p.warning_border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish(),
            );
        } else if let Some(record) = &self.trashed {
            modal_banners.push(
                Container::new(self.between(
                    self.line(
                        format!("已将「{}」移至安全回收站", record.original),
                        CAPTION,
                        p.text,
                    ),
                    self.button(
                        "trash-restore",
                        "↺ 撤销并恢复",
                        Action::Restore,
                        Btn::Secondary,
                    ),
                ))
                .with_horizontal_padding(14.)
                .with_vertical_padding(8.)
                .with_background_color(p.surface)
                .with_border(Border::all(1.).with_border_color(p.border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish(),
            );
        }

        // 2. File Table Container
        let chk_box = |checked: bool| -> El {
            ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(if checked {
                            self.ui_icon(UiIcon::Check, 11., p.accent_fg)
                        } else {
                            Empty::new().finish()
                        })
                        .finish(),
                )
                .with_background_color(if checked { p.accent } else { p.bg })
                .with_border(Border::all(1.).with_border_color(if checked {
                    p.accent
                } else {
                    p.border_strong
                }))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(3.)))
                .finish(),
            )
            .with_width(15.)
            .with_height(15.)
            .finish()
        };

        let table_header = ConstrainedBox::new(
            Container::new(self.file_grid_row(
                self.hstack(
                    12.,
                    vec![
                        chk_box(false),
                        self.line_w("名称 ↑", CAPTION, p.text2, Weight::Medium),
                    ],
                ),
                self.line_w("位置", CAPTION, p.text2, Weight::Medium),
                self.line_w("类型", CAPTION, p.text2, Weight::Medium),
                self.line_w("大小", CAPTION, p.text2, Weight::Medium),
                self.line_w("操作", CAPTION, p.text2, Weight::Medium),
            ))
            .with_horizontal_padding(S4)
            .with_vertical_padding(8.)
            .with_background_color(if p.dark {
                rgb(0x20, 0x20, 0x26)
            } else {
                p.surface
            })
            .with_border(Border::bottom(1.).with_border_color(p.border))
            .finish(),
        )
        .with_height(36.)
        .finish();

        let real_files = self.filtered_files();
        let mut table_rows: Vec<El> = Vec::new();
        if !real_files.is_empty() {
            for (row_idx, &(orig_idx, entry)) in real_files.iter().enumerate() {
                if row_idx > 0 {
                    table_rows.push(self.divider());
                }
                let selected = self.selected_file == Some(orig_idx);
                let (symbol, kind_label) = file_symbol(entry);
                let icon =
                    self.ui_icon(symbol, 18., if entry.directory { p.blue } else { p.text2 });
                let name_col = self.hstack(
                    12.,
                    vec![
                        chk_box(selected),
                        icon,
                        self.line_w(
                            &entry.name,
                            CAPTION,
                            p.text,
                            if selected {
                                Weight::Semibold
                            } else {
                                Weight::Medium
                            },
                        ),
                    ],
                );
                let is_img = entry.name.ends_with(".png")
                    || entry.name.ends_with(".jpg")
                    || entry.name.ends_with(".jpeg");
                let mut act_links = vec![self.link(
                    format!("f-real-open-{orig_idx}"),
                    "打开",
                    Action::ActivateFile(orig_idx),
                )];
                if !entry.directory {
                    act_links.push(self.link(
                        format!("f-real-save-{orig_idx}"),
                        "下载",
                        Action::DownloadFile(orig_idx),
                    ));
                }
                if is_img {
                    act_links.push(self.link(
                        format!("f-real-mat-{orig_idx}"),
                        "背屏预览",
                        Action::FilePushToBackScreen(entry.path.clone()),
                    ));
                }
                act_links.push(self.link_muted(
                    format!("f-real-ren-{orig_idx}"),
                    "重命名",
                    Action::RenameFile(orig_idx),
                ));
                act_links.push(self.link(
                    format!("f-real-del-{orig_idx}"),
                    "删除",
                    Action::TrashFile(orig_idx),
                ));
                let actions_col = self.hstack(8., act_links);
                let size_str = if entry.directory {
                    "--".to_string()
                } else if entry.bytes >= 1_048_576 {
                    format!("{:.1} MB", entry.bytes as f64 / 1_048_576.0)
                } else if entry.bytes >= 1024 {
                    format!("{} KB", entry.bytes / 1024)
                } else {
                    format!("{} B", entry.bytes)
                };
                let row_inner = self.file_grid_row(
                    name_col,
                    self.mono("手机存储", CAPTION, p.text2),
                    self.line(kind_label, CAPTION, p.text2),
                    self.mono(size_str, CAPTION, p.text2),
                    actions_col,
                );
                let menu_path = entry.path.clone();
                let state = self.mouse_state(format!("file-real-row:{orig_idx}"));
                let row_el = Hoverable::new(state, move |mouse| {
                    let hovered = mouse.is_mouse_over_element();
                    let bg = if selected {
                        if p.dark {
                            rgb(0x25, 0x25, 0x2D)
                        } else {
                            rgb(0xF4, 0xF4, 0xF8)
                        }
                    } else if hovered {
                        p.hover
                    } else {
                        ColorU::transparent_black()
                    };
                    ConstrainedBox::new(
                        Container::new(row_inner)
                            .with_horizontal_padding(S4)
                            .with_vertical_padding(9.)
                            .with_background_color(bg)
                            .finish(),
                    )
                    .with_height(42.)
                    .finish()
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(Action::SelectFile(orig_idx)))
                .on_double_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(Action::ActivateFile(orig_idx))
                })
                .on_right_click(move |ctx, _, at| {
                    ctx.dispatch_typed_action(Action::ContextOpen(
                        crate::ui::context_menu::Target::File(menu_path.clone()),
                        at,
                    ))
                })
                .finish();
                table_rows.push(row_el);
            }
        } else if self.demo_mode {
            for (i, r) in FILE_MOCKS.iter().enumerate() {
                if i > 0 {
                    table_rows.push(self.divider());
                }
                let selected = match self.selected_file {
                    Some(sel) => sel == i,
                    None => r.selected,
                };
                let icon = if r.is_dir {
                    self.line("📁", CAPTION, p.warning)
                } else {
                    self.line("📄", CAPTION, p.text2)
                };
                let name_col = self.hstack(
                    12.,
                    vec![
                        chk_box(selected),
                        icon,
                        self.line_w(
                            r.name,
                            CAPTION,
                            p.text,
                            if selected {
                                Weight::Semibold
                            } else {
                                Weight::Medium
                            },
                        ),
                    ],
                );
                let actions_col = self.hstack(
                    10.,
                    vec![
                        self.link(
                            format!("f-act-open-{i}"),
                            if r.is_dir { "打开" } else { "下载" },
                            Action::SelectFile(i),
                        ),
                        self.link_muted(format!("f-act-ren-{i}"), "重命名", Action::OpenRename),
                        self.link(format!("f-act-del-{i}"), "删除", Action::Trash),
                    ],
                );
                let row_inner = self.file_grid_row(
                    name_col,
                    self.mono(r.date, CAPTION, p.text2),
                    self.line(r.kind, CAPTION, p.text2),
                    self.mono(r.size, CAPTION, p.text2),
                    actions_col,
                );
                let state = self.mouse_state(format!("file-row:{i}"));
                let row_el = Hoverable::new(state, move |mouse| {
                    let hovered = mouse.is_mouse_over_element();
                    let bg = if selected {
                        if p.dark {
                            rgb(0x25, 0x25, 0x2D)
                        } else {
                            rgb(0xF4, 0xF4, 0xF8)
                        }
                    } else if hovered {
                        p.hover
                    } else {
                        ColorU::transparent_black()
                    };
                    ConstrainedBox::new(
                        Container::new(row_inner)
                            .with_horizontal_padding(S4)
                            .with_vertical_padding(10.)
                            .with_background_color(bg)
                            .finish(),
                    )
                    .with_height(44.)
                    .finish()
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(Action::SelectFile(i)))
                .finish();
                table_rows.push(row_el);
            }
        }

        if table_rows.is_empty() {
            table_rows.push(
                Container::new(self.text("当前目录没有匹配的文件", BODY, p.text2))
                    .with_uniform_padding(S4)
                    .finish(),
            );
        }
        let total_items = if real_files.is_empty() && self.demo_mode {
            FILE_MOCKS.len()
        } else {
            real_files.len()
        };

        let table_footer = Container::new(self.between(
            self.hstack(
                10.,
                vec![
                    self.line(
                        format!("{total_items} 项 · 单击选择，双击打开"),
                        CAPTION,
                        p.text2,
                    ),
                    self.line("·", CAPTION, p.text2),
                    self.line_w(
                        format!("路径：{}", self.directory),
                        CAPTION,
                        p.text,
                        Weight::Medium,
                    ),
                ],
            ),
            self.hstack(
                8.,
                vec![
                    self.button(
                        "files-rename-sel",
                        "重命名",
                        Action::OpenRename,
                        Btn::Secondary,
                    ),
                    self.button("files-dl-sel", "下载所选", Action::Download, Btn::Secondary),
                    self.button("files-trash-sel", "移至回收站", Action::Trash, Btn::Danger),
                ],
            ),
        ))
        .with_horizontal_padding(S4)
        .with_vertical_padding(8.)
        .with_background_color(if p.dark {
            rgb(0x1C, 0x1C, 0x21)
        } else {
            p.workspace
        })
        .with_border(Border::top(1.).with_border_color(p.border))
        .finish();

        let rows_scroll = self.pane_scroll(self.scroll.clone(), self.vstack(0., table_rows));

        let table_card = Container::new(
            Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(table_header)
                .with_child(Expanded::new(1., rows_scroll).finish())
                .with_child(table_footer)
                .finish(),
        )
        .with_background_color(p.bg)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish();

        let selected_entry: Option<stepx_companion::storage::Entry> = self
            .selected_file
            .and_then(|idx| self.files.get(idx).cloned())
            .or_else(|| {
                if !real_files.is_empty() {
                    real_files.first().map(|&(_, e)| e.clone())
                } else if self.demo_mode {
                    let idx = self
                        .selected_file
                        .unwrap_or(2)
                        .min(FILE_MOCKS.len().saturating_sub(1));
                    let m = &FILE_MOCKS[idx];
                    Some(stepx_companion::storage::Entry {
                        name: m.name.to_string(),
                        path: format!("{}/{}", self.directory.trim_end_matches('/'), m.name),
                        bytes: 2_516_582,
                        directory: m.is_dir,
                        symlink: false,
                    })
                } else {
                    None
                }
            });

        let detail_panel = if let Some(entry) = selected_entry.as_ref() {
            let (symbol, kind_label) = file_symbol(entry);
            let size_label = if entry.directory {
                "文件夹目录".to_string()
            } else {
                stepx_companion::storage::format_bytes(entry.bytes)
            };
            let mut cross_actions: Vec<El> = Vec::new();
            if !entry.directory {
                if matches!(entry.kind_badge(), "图片" | "视频") {
                    cross_actions.push(self.button(
                        "file-detail-push-bs",
                        "推送到背屏创作",
                        Action::FilePushToBackScreen(entry.path.clone()),
                        Btn::Primary,
                    ));
                }
                cross_actions.push(self.button(
                    "file-detail-copy-path",
                    "复制手机路径",
                    Action::CopyText("文件手机路径".into(), entry.path.clone()),
                    Btn::Secondary,
                ));
                cross_actions.push(self.button(
                    "file-detail-to-amoo",
                    "去 Amoo 分析",
                    Action::Page(Page::Amoo),
                    Btn::Secondary,
                ));
            } else {
                cross_actions.push(self.button(
                    "file-detail-open-dir",
                    "进入此文件夹",
                    Action::Browse(entry.path.clone()),
                    Btn::Primary,
                ));
                cross_actions.push(self.button(
                    "file-detail-copy-path",
                    "复制手机路径",
                    Action::CopyText("文件手机路径".into(), entry.path.clone()),
                    Btn::Secondary,
                ));
            }

            self.panel(vec![
                self.between(
                    self.heading("所选文件详情"),
                    self.pill_badge(kind_label, p.text2, p.tag_bg),
                ),
                self.hstack(
                    S2,
                    vec![
                        self.ui_icon(symbol, 22., if entry.directory { p.blue } else { p.text }),
                        Expanded::new(
                            1.,
                            self.vstack(
                                2.,
                                vec![
                                    self.line_w(&entry.name, BODY, p.text, Weight::Semibold),
                                    self.mono(size_label, MICRO, p.text2),
                                ],
                            ),
                        )
                        .finish(),
                    ],
                ),
                self.divider(),
                self.vstack(
                    4.,
                    vec![
                        self.line("手机存储路径", MICRO, p.text3),
                        self.mono(&entry.path, CAPTION, p.text2),
                    ],
                ),
                self.divider(),
                self.line_w("跨页快捷流转", CAPTION, p.text, Weight::Medium),
                self.wrap(cross_actions),
                self.divider(),
                self.between(
                    self.line("需要自动释放手机空间？", MICRO, p.text3),
                    self.link_action(
                        "file-detail-go-exp",
                        "电脑存储扩容 →",
                        Action::Page(Page::Expansion),
                    ),
                ),
            ])
        } else {
            self.panel(vec![
                self.heading("所选文件详情"),
                self.text(
                    "单击左侧列表中的任意文件，即可复制手机路径、一键推送到背屏点阵创作，或交由 Amoo 分析。",
                    CAPTION,
                    p.text2,
                ),
                self.button(
                    "file-detail-empty-exp",
                    "前往电脑存储扩容",
                    Action::Page(Page::Expansion),
                    Btn::Secondary,
                ),
            ])
        };

        let main_split = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(S3)
            .with_child(Expanded::new(1., table_card).finish())
            .with_child(self.fixed_w(276., detail_panel))
            .finish();

        let mut page_children = vec![self.storage_mode_bar(), top_bar];
        page_children.extend(modal_banners);
        page_children.push(Expanded::new(1., main_split).finish());

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(S3)
            .with_children(page_children)
            .finish()
    }
}

/// Stable cumulative paths, with a human-readable root. No substring replacement.
fn breadcrumbs(path: &str) -> Vec<(String, String)> {
    let mut result = vec![("手机存储".into(), "/sdcard".into())];
    let mut current = String::from("/sdcard");
    for part in path
        .strip_prefix("/sdcard/")
        .unwrap_or("")
        .split('/')
        .filter(|p| !p.is_empty())
    {
        current.push('/');
        current.push_str(part);
        result.push((part.to_owned(), current.clone()));
    }
    result
}
#[cfg(test)]
mod navigation_tests {
    use super::*;
    #[test]
    fn breadcrumbs_keep_unicode_and_repeated_names() {
        assert_eq!(breadcrumbs("/sdcard").len(), 1);
        assert_eq!(
            breadcrumbs("/sdcard/DCIM/旅行/DCIM").last().unwrap(),
            &("DCIM".into(), "/sdcard/DCIM/旅行/DCIM".into())
        );
    }
}
