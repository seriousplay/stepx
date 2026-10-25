//! A compact LED studio: edit locally, inspect the result, then play on the phone.
use super::super::{
    backscreen_state::BackScreenAction as BsAction, icons::UiIcon, theme::*, widgets::*, Action,
    Companion, F_BACK_MARQUEE, F_BACK_PROMPT,
};
use facloud_ui::{elements::*, fonts::Weight, platform::Cursor};
use std::time::Duration;
use stepx_companion::backscreen as bs;

impl Companion {
    fn bs_button(&self, id: &str, label: &str, action: BsAction, style: Btn) -> El {
        self.button(format!("bs-{id}"), label, Action::BackScreen(action), style)
    }
    fn bs_title(&self, icon: UiIcon, title: &str) -> El {
        self.hstack(
            S2,
            vec![self.ui_icon(icon, 16., self.p().text2), self.heading(title)],
        )
    }
    pub(in super::super) fn backscreen_actions(&self) -> Vec<El> {
        vec![
            self.bs_button(
                "inspect-top",
                "刷新手机状态",
                BsAction::CompanionInspect,
                Btn::Secondary,
            ),
            self.bs_button(
                "advanced-top",
                if self.back_screen.advanced {
                    "收起设置"
                } else {
                    "背屏设置"
                },
                BsAction::Advanced,
                Btn::Secondary,
            ),
        ]
    }
    fn bs_preview(&self) -> El {
        let p = self.p();
        let state = &self.back_screen;
        let game = state.tab == 4;
        let game_frame = game_preview_frame(state.elapsed_ms);
        let preview_frame = if game { &game_frame } else { &state.frame };
        let mut parts = vec![
            self.between(
                self.bs_title(UiIcon::Matrix, if game { "玩法预览" } else { "点阵预览" }),
                self.pill_badge(
                    if game || state.native_feature.is_some() {
                        "功能示意"
                    } else if state.has_unapplied_changes() {
                        "待应用"
                    } else {
                        "已应用"
                    },
                    if !game && !state.has_unapplied_changes() {
                        p.success
                    } else {
                        p.text2
                    },
                    p.surface,
                ),
            ),
            self.line_w(
                if game { "重力对齐" } else { &state.title },
                TITLE,
                p.text,
                Weight::Semibold,
            ),
            Align::new(self.hstack(
                S4,
                vec![
                        self.phone_rear_housing(100., 210., preview_frame, 3.6, 0.95),
                        ConstrainedBox::new(
                            self.vstack(
                                S2,
                                vec![
                                    self.line("点阵放大", MICRO, p.text3),
                                    Container::new(self.matrix_grid(
                                        preview_frame,
                                        3.6,
                                        0.95,
                                        false,
                                    ))
                                    .with_uniform_padding(S2)
                                    .with_background_color(rgb(24, 24, 30))
                                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                                        R_CONTROL,
                                    )))
                                    .finish(),
                                ],
                            ),
                        )
                        .with_width(100.)
                        .finish(),
                    ],
            ))
            .finish(),
            self.mono("17 × 23 · 379 灯珠", MICRO, p.text2),
            self.bs_button(
                "preview-toggle",
                if state.preview_running {
                    "暂停预览"
                } else {
                    "播放预览"
                },
                BsAction::TogglePreview,
                Btn::Secondary,
            ),
            self.text(
                if game {
                    "面对背屏，左倾向左、右倾向右。翻到正面结束本局。"
                } else {
                    &state.status
                },
                CAPTION,
                if state.error.is_some() && !game {
                    p.danger
                } else {
                    p.text2
                },
            ),
        ];
        if !state.applied_title.is_empty() {
            parts.push(self.divider());
            parts.push(self.line("手机当前", MICRO, p.text3));
            parts.push(self.line_w(&state.applied_title, BODY, p.text, Weight::Medium));
            parts.push(self.text(&state.applied_status, MICRO, p.text2));
        }
        if state
            .error
            .as_ref()
            .is_some_and(|error| error.contains("自定义背屏"))
        {
            parts.push(self.bs_button(
                "resolve-occupied",
                "处理旧背屏占用",
                BsAction::Advanced,
                Btn::Secondary,
            ));
        }
        let needs_companion = game || (state.native_feature.is_none() && !state.temporary_app());
        let primary = if self.selected.is_none() {
            self.disabled_button("请先连接手机")
        } else if state.converting_image {
            self.disabled_button("正在转换图片")
        } else if state.applying() {
            self.disabled_button("正在同步手机…")
        } else if state.generating {
            self.disabled_button("等待 Amoo 生成")
        } else if needs_companion
            && (state.companion_installed != Some(true) || !state.companion_permission)
        {
            self.bs_button(
                "install-companion",
                if state.companion_installed == Some(true) {
                    "修复 StepX伴侣"
                } else {
                    "安装 StepX伴侣"
                },
                BsAction::CompanionInstall,
                Btn::Primary,
            )
        } else if game {
            self.bs_button("arm-game", "启用三敲启动", BsAction::GameArm, Btn::Primary)
        } else {
            self.bs_button(
                "apply",
                if state.temporary_app() {
                    "应用临时联动"
                } else {
                    "应用到背屏"
                },
                BsAction::Play,
                Btn::Primary,
            )
        };
        let mut footer = vec![primary];
        if needs_companion {
            footer.push(self.text(&state.companion_status, MICRO, p.text2));
        }
        if state.playing {
            footer.push(self.bs_button("stop", "停止并恢复", BsAction::Stop, Btn::Secondary));
        } else if state.applied_to_phone && state.companion_mode != "native" {
            footer.push(self.bs_button(
                "stop-companion",
                "停止并恢复",
                BsAction::CompanionStop,
                Btn::Secondary,
            ));
        }
        footer.push(self.text(
            if game {
                "背屏朝向自己，轻敲三下开始；翻到正面结束并恢复原显示。"
            } else if state.native_feature.is_some() {
                "应用后，由手机系统管理显示。"
            } else if state.temporary_app() {
                "需保持桌面连接，计时结束后恢复原显示。"
            } else if matches!(state.source, bs::Source::Preset(bs::Preset::Countdown)) {
                "倒计时在手机运行，到时恢复原显示。"
            } else {
                "应用后由手机持续显示，选择其他图案不会立即更换。"
            },
            MICRO,
            p.text3,
        ));
        let preview = Container::new(self.vstack(S3, parts))
            .with_uniform_padding(S3)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish();
        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(S3)
            .with_child(
                Expanded::new(1., self.pane_scroll(self.scroll_aux.clone(), preview)).finish(),
            )
            .with_child(self.panel(footer))
            .finish()
    }
    fn bs_preset_card(&self, preset: bs::Preset) -> El {
        let p = self.p();
        let selected = self.back_screen.selection() == Some(preset);
        // Fixed gallery thumbnails are generated once on first opening the gallery.
        static THUMBNAILS: std::sync::OnceLock<Vec<(bs::Preset, Vec<u8>)>> =
            std::sync::OnceLock::new();
        let thumbnails = THUMBNAILS.get_or_init(|| {
            bs::Preset::ALL
                .into_iter()
                .map(|preset| {
                    (
                        preset,
                        bs::frame(
                            &bs::Source::Preset(preset),
                            &bs::PlaybackConfig::default(),
                            Duration::from_millis(700),
                            &bs::LiveData::default(),
                        ),
                    )
                })
                .collect()
        });
        let frame = &thumbnails.iter().find(|(key, _)| *key == preset).unwrap().1;
        let thumbnail = ConstrainedBox::new(
            Container::new(Align::new(self.matrix_grid(&frame, 1.1, 0.35, false)).finish())
                .with_background_color(rgb(0x16, 0x16, 0x1B))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish(),
        )
        .with_width(40.)
        .with_height(46.)
        .finish();
        let contents = self.hstack(
            S3,
            vec![
                thumbnail,
                Expanded::new(
                    1.,
                    self.vstack(
                        S1,
                        vec![
                            self.line_w(preset.label(), BODY, p.text, Weight::Semibold),
                            self.line(preset.description(), MICRO, p.text3),
                        ],
                    ),
                )
                .finish(),
                if selected {
                    self.ui_icon(UiIcon::Check, 16., p.text)
                } else {
                    self.ui_icon(UiIcon::ChevronRight, 14., p.text3)
                },
            ],
        );
        Hoverable::new(
            self.mouse_state(format!("bs-preset-{}", preset.key())),
            move |mouse| {
                Container::new(contents)
                    .with_uniform_padding(S3)
                    .with_background_color(if selected {
                        p.selected
                    } else if mouse.is_mouse_over_element() {
                        p.hover
                    } else {
                        p.bg
                    })
                    .with_border(Border::all(1.).with_border_color(if selected {
                        p.border_strong
                    } else {
                        p.border
                    }))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                    .finish()
            },
        )
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(Action::BackScreen(BsAction::SelectPreset(preset)))
        })
        .finish()
    }
    fn bs_gallery(&self) -> El {
        let p = self.p();
        let mut parts = vec![
            self.between(
                self.bs_title(UiIcon::Smile, "有趣，也实用"),
                self.line(
                    format!("{} 种效果", bs::Preset::ALL.len()),
                    CAPTION,
                    p.text3,
                ),
            ),
            self.text(
                "点击图案预览，满意后点击「应用到背屏」。时钟和电量由手机实时更新。",
                CAPTION,
                p.text2,
            ),
        ];
        for row in bs::Preset::ALL.chunks(2) {
            let mut cards: Vec<El> = row
                .iter()
                .map(|preset| Expanded::new(1., self.bs_preset_card(*preset)).finish())
                .collect();
            if cards.len() == 1 {
                cards.push(Expanded::new(1., Empty::new().finish()).finish());
            }
            parts.push(self.hstack(S2, cards));
        }
        parts.push(self.bs_recent_gallery(3));
        self.vstack(S2, parts)
    }
    fn bs_recent_gallery(&self, limit: usize) -> El {
        let p = self.p();
        let mut parts = vec![self.between(
            self.bs_title(UiIcon::Image, "Amoo 作品图库"),
            self.bs_button(
                "refresh-gallery",
                "刷新",
                BsAction::RefreshGallery,
                Btn::Ghost,
            ),
        )];
        if self.back_screen.recent_images.is_empty() {
            parts.push(self.text(
                "生成的图片会保存在这里，重新打开桌面后也能继续使用。",
                CAPTION,
                p.text3,
            ));
        } else {
            let images: Vec<_> = self
                .back_screen
                .recent_images
                .iter()
                .take(limit)
                .enumerate()
                .collect();
            for chunk in images.chunks(3) {
                let mut cards = Vec::new();
                for &(index, path) in chunk {
                    let image = ConstrainedBox::new(
                        Image::new(
                            facloud_ui::assets::asset_cache::AssetSource::LocalFile {
                                path: path.to_string_lossy().into_owned(),
                            },
                            CacheOption::BySize,
                        )
                        .contain()
                        .finish(),
                    )
                    .with_height(92.)
                    .finish();
                    let date = std::fs::metadata(path)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .map(|time| {
                            chrono::DateTime::<chrono::Local>::from(time)
                                .format("%m-%d %H:%M")
                                .to_string()
                        })
                        .unwrap_or_default();
                    cards.push(
                        Expanded::new(
                            1.,
                            self.panel(vec![
                                image,
                                self.line(date, MICRO, p.text3),
                                self.bs_button(
                                    &format!("recent-{index}"),
                                    "转为像素",
                                    BsAction::RecentImage(index),
                                    Btn::Secondary,
                                ),
                            ]),
                        )
                        .finish(),
                    );
                }
                while cards.len() < 3 {
                    cards.push(Expanded::new(1., Empty::new().finish()).finish());
                }
                parts.push(self.hstack(S2, cards));
            }
        }
        self.vstack(S2, parts)
    }
    fn bs_prompt_input(&self) -> El {
        let p = self.p();
        let field = &self.fields[F_BACK_PROMPT];
        let focused = self.focus == Some(F_BACK_PROMPT);
        let mut value = field.text.clone();
        let mut caret = 0;
        if focused {
            let cursor = field.cursor.min(value.len());
            caret = cursor + field.preedit.len();
            value.insert_str(cursor, &format!("{}│", field.preedit));
        }
        let empty = field.text.is_empty() && field.preedit.is_empty();
        if value.is_empty() {
            value = "例如：一只眨眼的猫咪，柔和亮度，播放 20 秒。也可以做番茄钟，或生成新的图片。"
                .into();
        }
        Hoverable::new(self.mouse_state("bs-amoo-prompt"), move |mouse| {
            ConstrainedBox::new(
                Container::new(
                    Text::new(value, self.font, BODY)
                        .with_saved_char_position(caret, format!("input-caret:{F_BACK_PROMPT}"))
                        .with_color(if empty && !focused { p.text3 } else { p.text })
                        .with_line_height_ratio(1.5)
                        .finish(),
                )
                .with_uniform_padding(S3)
                .with_background_color(p.surface)
                .with_border(Border::all(1.).with_border_color(if focused {
                    p.accent
                } else if mouse.is_mouse_over_element() {
                    p.border_strong
                } else {
                    p.border
                }))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish(),
            )
            .with_min_height(100.)
            .finish()
        })
        .with_cursor(Cursor::IBeam)
        .on_mouse_down(|ctx, _, _| ctx.dispatch_typed_action(Action::Focus(F_BACK_PROMPT)))
        .finish()
    }
    fn bs_amoo(&self) -> El {
        let p = self.p();
        let state = &self.back_screen;
        let modes = self.segmented(
            "bs-amoo-mode",
            vec![
                (
                    "智能编排".into(),
                    !state.amoo_image,
                    Action::BackScreen(BsAction::AmooMode(false)),
                ),
                (
                    "生成图片".into(),
                    state.amoo_image,
                    Action::BackScreen(BsAction::AmooMode(true)),
                ),
            ],
        );
        let mut parts = vec![
            self.bs_title(UiIcon::Star, "让 Amoo 理解你的想法"),
            self.text(if state.amoo_image {"调用手机 Amoo 实际生成图片，再转成适合背屏的单色像素。"} else {"描述图案或用途，Amoo 会选择动画、亮度、速度与时长，也能创作新图片。"}, CAPTION, p.text2),
            modes, self.bs_prompt_input(),
            self.wrap(vec![
                self.bs_button("prompt-cat", "猫咪陪伴", BsAction::Prompt("做一只会眨眼的猫咪，低亮度，播放 20 秒".into()), Btn::Secondary),
                self.bs_button("prompt-focus", "专注倒计时", BsAction::Prompt("我要专注 25 分钟，背屏显示倒计时，亮度柔和".into()), Btn::Secondary),
                self.bs_button("prompt-image", "宇航员像素画", BsAction::Prompt("生成一张宇航员抱着月亮的图片，单色，纯黑背景，白色轮廓，主体居中，适合 17 列 23 行的背屏".into()), Btn::Secondary),
            ]),
            if state.generating {self.bs_button("cancel-generation", "取消生成", BsAction::CancelGenerate, Btn::Secondary)} else {self.bs_button("amoo-generate", "生成背屏方案", BsAction::Generate, Btn::Secondary)},
        ];
        if !state.generator_status.is_empty() {
            parts.push(self.panel(vec![
                self.hstack(
                    S2,
                    vec![
                        self.ui_icon(
                            if state.generating {
                                UiIcon::Clock
                            } else {
                                UiIcon::Check
                            },
                            16.,
                            if state.error.is_some() {
                                p.danger
                            } else {
                                p.text2
                            },
                        ),
                        self.heading(if state.generating {
                            "Amoo 正在创作"
                        } else {
                            "创作结果"
                        }),
                    ],
                ),
                self.text(&state.generator_status, BODY, p.text2),
            ]));
        }
        if state.image_path.is_some() {
            parts.push(self.bs_source_image());
        }
        self.vstack(S3, parts)
    }
    fn bs_source_image(&self) -> El {
        let p = self.p();
        let Some(path) = &self.back_screen.image_path else {
            return Empty::new().finish();
        };
        let image = ConstrainedBox::new(
            Image::new(
                facloud_ui::assets::asset_cache::AssetSource::LocalFile {
                    path: path.to_string_lossy().into_owned(),
                },
                CacheOption::BySize,
            )
            .contain()
            .finish(),
        )
        .with_height(148.)
        .finish();
        self.panel(vec![
            self.between(
                self.bs_title(UiIcon::Image, "原图"),
                self.line(&self.back_screen.origin, MICRO, p.text3),
            ),
            image,
            self.text("左侧为实际灯珠布局下的灰度转换结果。", MICRO, p.text3),
        ])
    }
    fn bs_images(&self) -> El {
        let p = self.p();
        let state = &self.back_screen;
        let mut parts = vec![
            self.bs_title(UiIcon::Image, "把喜欢的图片装进背屏"),
            self.text(
                "支持 PNG、JPEG、WebP、GIF 首帧。简单轮廓和高对比度主体更适合 379 颗灯珠。",
                CAPTION,
                p.text2,
            ),
            self.bs_button(
                "choose-image",
                "选择图片…",
                BsAction::PickImage,
                Btn::Secondary,
            ),
        ];
        if state.image_path.is_some() {
            parts.push(self.bs_source_image());
            parts.push(self.wrap(vec![
                self.bs_button(
                    "image-fit",
                    match state.image_options.fit {
                        bs::ImageFit::Contain => "完整显示",
                        bs::ImageFit::Cover => "居中裁切",
                    },
                    BsAction::Fit,
                    Btn::Secondary,
                ),
                self.bs_button(
                    "image-dither",
                    if state.image_options.dither {
                        "抖动：开"
                    } else {
                        "抖动：关"
                    },
                    BsAction::Dither,
                    Btn::Secondary,
                ),
                self.bs_button(
                    "image-threshold",
                    if state.image_options.threshold.is_some() {
                        "黑白模式"
                    } else {
                        "灰度模式"
                    },
                    BsAction::Threshold,
                    Btn::Secondary,
                ),
                self.bs_button(
                    "image-invert",
                    if state.config.invert {
                        "反色：开"
                    } else {
                        "反色：关"
                    },
                    BsAction::Invert,
                    Btn::Secondary,
                ),
            ]));
            parts.push(self.text(
                "抖动保留明暗层次；黑白模式强化轮廓。反色适合白底图片。",
                CAPTION,
                p.text3,
            ));
        } else {
            parts.push(self.panel(vec![self.bs_title(UiIcon::Lightbulb, "试试这些画面"), self.text("简笔头像、宠物轮廓、像素角色、单色标志。可以先让 Amoo 生成图片，再在这里调整转换效果。", BODY, p.text2), self.bs_button("image-to-amoo", "用 Amoo 创作", BsAction::Tab(1), Btn::Secondary)]));
        }
        parts.push(self.bs_recent_gallery(24));
        self.vstack(S3, parts)
    }
    fn bs_android(&self) -> El {
        let p = self.p();
        let state = &self.back_screen;
        let mut parts = vec![
            self.bs_title(UiIcon::Smartphone, "让背屏跟着手机做事"),
            self.text(
                "系统功能由 Android 管理；应用动画由桌面监听当前手机前台应用。",
                CAPTION,
                p.text2,
            ),
        ];
        for feature in [
            bs::NativeFeature::Notifications,
            bs::NativeFeature::Clock,
            bs::NativeFeature::Camera,
        ] {
            let selected = state.native_feature == Some(feature);
            let icon = match feature {
                bs::NativeFeature::Notifications => UiIcon::Bell,
                bs::NativeFeature::Clock => UiIcon::Clock,
                bs::NativeFeature::Camera => UiIcon::Camera,
            };
            parts.push(self.panel(vec![
                self.between(
                    self.bs_title(icon, feature.label()),
                    self.bs_button(
                        &format!("native-{}", feature.key()),
                        if selected { "已选择" } else { "选择" },
                        BsAction::SelectNative(feature),
                        Btn::Secondary,
                    ),
                ),
                self.text(
                    super::super::backscreen_state::native_description(feature),
                    CAPTION,
                    p.text2,
                ),
            ]));
            if feature == bs::NativeFeature::Camera {
                parts.push(self.bs_button(
                    "camera-off",
                    "关闭相机背屏联动",
                    BsAction::CameraOff,
                    Btn::Secondary,
                ));
            }
        }
        parts.push(self.divider());
        parts.push(self.between(
            self.bs_title(UiIcon::Apps, "应用专属动画"),
            self.button("bs-refresh-apps", "读取应用", Action::Apps, Btn::Secondary),
        ));
        parts.push(self.text(format!("当前动画：{}。先在图库挑选图案，再选择应用；播放期间，进入该应用显示动画，离开后恢复原显示。", state.selection().unwrap_or(bs::Preset::Meteor).label()), CAPTION, p.text2));
        let apps: Vec<_> = self
            .apps
            .iter()
            .filter(|app| app.launchable && app.enabled)
            .collect();
        if apps.is_empty() {
            parts.push(self.text(
                "点击读取应用，获取手机中可启动的应用列表。",
                CAPTION,
                p.text3,
            ));
        } else {
            for app in apps {
                let selected = matches!(&state.source, bs::Source::App {package, ..} if package == &app.package);
                let icon = if let Some(path) = &app.icon_path {
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
                    .with_width(28.)
                    .with_height(28.)
                    .finish()
                } else {
                    self.ui_icon(UiIcon::Package, 28., p.text3)
                };
                parts.push(self.panel(vec![self.between(
                    self.hstack(
                        S2,
                        vec![
                                icon,
                                Expanded::new(
                                    1.,
                                    self.vstack(
                                        S1,
                                        vec![
                                            self.line_w(&app.label, BODY, p.text, Weight::Medium),
                                            self.line(&app.package, MICRO, p.text3),
                                        ],
                                    ),
                                )
                                .finish(),
                            ],
                    ),
                    self.bs_button(
                        &format!("app-{}", app.package),
                        if selected { "已选择" } else { "关联" },
                        BsAction::SelectApp(app.package.clone(), app.label.clone()),
                        Btn::Secondary,
                    ),
                )]));
            }
        }
        self.vstack(S3, parts)
    }
    fn bs_settings(&self) -> El {
        let p = self.p();
        let state = &self.back_screen;
        if state.native_feature.is_some() {
            return self.panel(vec![self.bs_title(UiIcon::Settings, "由手机系统管理"), self.text("系统联动采用手机现有的亮屏、亮度与勿扰设置。这里的动画速度和播放时长不影响手机通知、相机或原生时钟。", CAPTION, p.text2)]);
        }
        let brightness = self.segmented(
            "bs-brightness",
            (1..=8)
                .map(|level| {
                    (
                        level.to_string(),
                        level == state.config.brightness,
                        Action::BackScreen(BsAction::Brightness(level)),
                    )
                })
                .collect(),
        );
        let speed = self.segmented(
            "bs-speed",
            [1, 3, 6, 10]
                .into_iter()
                .map(|fps| {
                    (
                        format!("{}", fps),
                        fps == state.config.fps,
                        Action::BackScreen(BsAction::Fps(fps)),
                    )
                })
                .collect(),
        );
        let length = self.segmented(
            "bs-length",
            [5, 15, 30, 60, 300, 1500]
                .into_iter()
                .map(|s| {
                    (
                        if s < 60 {
                            format!("{s} 秒")
                        } else {
                            format!("{} 分", s / 60)
                        },
                        s == state.config.duration_secs,
                        Action::BackScreen(BsAction::Duration(s)),
                    )
                })
                .collect(),
        );
        let dynamic = matches!(
            state.source,
            bs::Source::Preset(bs::Preset::Clock | bs::Preset::Battery | bs::Preset::Countdown)
        );
        let mut settings = vec![self.between(self.bs_title(UiIcon::Sun, "亮度"), brightness)];
        if !dynamic {
            settings.push(self.between(
                self.hstack(
                    S2,
                    vec![
                        self.ui_icon(UiIcon::Wave, 16., p.text2),
                        self.line(
                            format!("帧率 · {} 帧/秒", state.config.fps),
                            CAPTION,
                            p.text2,
                        ),
                    ],
                ),
                speed,
            ));
        }
        if state.temporary_app()
            || matches!(state.source, bs::Source::Preset(bs::Preset::Countdown))
        {
            settings.push(self.between(
                self.bs_title(
                    UiIcon::Clock,
                    if state.temporary_app() {
                        "临时联动时长"
                    } else {
                        "倒计时时长"
                    },
                ),
                length,
            ));
        } else {
            settings.push(self.text("应用后持续显示；停止或应用新图案时更换。", MICRO, p.text3));
        }
        self.panel(settings)
    }
    fn bs_games(&self) -> El {
        let p = self.p();
        let state = &self.back_screen;
        let mut rounds: Vec<_> = (1..=5)
            .rev()
            .map(|width| self.pill_badge(format!("{width} 格"), p.text, p.surface))
            .collect();
        rounds.push(self.pill_badge("回到 5 格", p.text2, p.surface));
        let steps = self.wrap(rounds);
        let mut game = vec![
            self.between(
                self.bs_title(UiIcon::Play, "重力对齐"),
                self.pill_badge("体感小游戏", p.success, p.surface),
            ),
            self.text(
                "轻敲三下开始。面对背屏，向左倾斜时长条向左，向右倾斜时长条向右，让它接入底部槽位。",
                BODY,
                p.text2,
            ),
            self.divider(),
            self.line_w("循环挑战，累计积分", BODY, p.text, Weight::Semibold),
            steps,
            self.text(
                "成功对齐获得积分，宽度按 5 → 4 → 3 → 2 → 1 循环，完成 1 格后继续下一轮。",
                CAPTION,
                p.text2,
            ),
            self.text(
                "想结束时，将手机翻到正面，自动退出游戏并恢复原背屏。深度休眠时先拿起或唤醒手机。",
                CAPTION,
                p.text2,
            ),
        ];
        if state.companion_installed == Some(true)
            && state.companion_permission
            && !state.companion_busy
        {
            game.push(self.wrap(vec![
                self.bs_button(
                    "game-now",
                    "立即开始一局",
                    BsAction::GameStart,
                    Btn::Secondary,
                ),
                self.bs_button(
                    "game-inspect",
                    "刷新积分与状态",
                    BsAction::CompanionInspect,
                    Btn::Ghost,
                ),
            ]));
        }
        let mut companion = vec![
            self.between(
                self.bs_title(UiIcon::Phone, "StepX伴侣"),
                self.pill_badge(
                    if state.companion_installed == Some(true) {
                        "已安装"
                    } else if state.companion_installed == Some(false) {
                        "待安装"
                    } else {
                        "待检测"
                    },
                    if state.companion_installed == Some(true) {
                        p.success
                    } else {
                        p.text3
                    },
                    p.surface,
                ),
            ),
            self.text(&state.companion_status, CAPTION, p.text2),
            self.text(
                "游戏在手机本地运行，传感器控制与动画无需电脑往返。安装后可以断开 USB。",
                CAPTION,
                p.text3,
            ),
        ];
        if !state.companion_busy {
            companion.push(self.wrap(vec![
                self.bs_button(
                    "companion-refresh",
                    "检测伴侣",
                    BsAction::CompanionInspect,
                    Btn::Secondary,
                ),
                self.bs_button(
                    "companion-install",
                    if state.companion_installed == Some(true) {
                        "更新 StepX伴侣"
                    } else {
                        "安装 StepX伴侣"
                    },
                    BsAction::CompanionInstall,
                    Btn::Secondary,
                ),
            ]));
        }
        if state.companion_installed == Some(true) && !state.companion_permission {
            companion.push(self.text(
                "背屏权限尚未就绪，请更新伴侣后重新检测。",
                CAPTION,
                p.warning,
            ));
        }
        if let Some(error) = &state.error {
            companion.push(self.text(error, CAPTION, p.danger));
        }
        self.vstack(S4, vec![self.panel(game), self.panel(companion)])
    }
    fn bs_advanced(&self) -> El {
        let p = self.p();
        let mut settings = vec![
            self.bs_title(UiIcon::Settings, "手机默认设置"),
            self.text(
                "以下按钮直接保存到手机，即刻生效。要修改图案，请返回预览后点击「应用到背屏」。",
                CAPTION,
                p.text2,
            ),
            self.wrap(vec![
                self.bs_button(
                    "native-auto",
                    "自动亮屏",
                    BsAction::DisplayMode("auto".into()),
                    Btn::Secondary,
                ),
                self.bs_button(
                    "native-always",
                    "常亮",
                    BsAction::DisplayMode("always".into()),
                    Btn::Secondary,
                ),
                self.bs_button(
                    "native-schedule",
                    "按手机设置的时段",
                    BsAction::DisplayMode("custom".into()),
                    Btn::Secondary,
                ),
                self.bs_button(
                    "native-brightness",
                    &format!("保存 {} 级亮度", self.back_screen.config.brightness),
                    BsAction::SaveBrightness,
                    Btn::Secondary,
                ),
            ]),
            self.input(F_BACK_MARQUEE, "输入英文或数字个性文字", None),
            self.wrap(vec![
                self.bs_button(
                    "native-scroll",
                    if self.matrix_marquee_on {
                        "滚动文字：开"
                    } else {
                        "滚动文字：关"
                    },
                    BsAction::ToggleMarquee,
                    Btn::Secondary,
                ),
                self.bs_button(
                    "native-save-text",
                    "保存到手机背屏",
                    BsAction::SaveMarquee,
                    Btn::Secondary,
                ),
            ]),
            self.text(
                "手机原生字库支持英文、数字和简单标点；中文图案可通过 Amoo 图片创作。",
                MICRO,
                p.text3,
            ),
            self.divider(),
            self.bs_title(UiIcon::Matrix, "背屏控制说明"),
            self.text(
                "预览是桌面模拟。应用后由 StepX伴侣在手机上播放；应用专属临时联动需要保持桌面连接。",
                CAPTION,
                p.text2,
            ),
            self.text(
                "单色 8-bit 灰度 · 17 列 × 23 行 · 391 个地址 / 379 颗灯珠；四角缺口自动留空。",
                CAPTION,
                p.text3,
            ),
            self.text(
                "时钟、电量应用后在手机本地读取；预览时钟使用电脑时间。律动动画为内置视觉效果。",
                CAPTION,
                p.text3,
            ),
        ];
        settings.push(self.divider());
        if self.back_screen.clear_confirmation {
            settings.push(self.text(
                "将清除手机当前自定义背屏图案，恢复系统显示。固件无法导出原图，旧图案不能还原。",
                CAPTION,
                p.warning,
            ));
            settings.push(self.wrap(vec![
                self.bs_button(
                    "clear-cancel",
                    "取消",
                    BsAction::CancelClear,
                    Btn::Secondary,
                ),
                self.bs_button(
                    "clear-confirm",
                    "清除旧图案",
                    BsAction::ClearFrame,
                    Btn::Danger,
                ),
            ]));
        } else {
            settings.push(self.bs_button(
                "clear-prepare",
                "清除旧自定义图案…",
                BsAction::PrepareClear,
                Btn::Secondary,
            ));
        }
        self.panel(settings)
    }
    pub(in super::super) fn backscreen_page(&self) -> El {
        let state = &self.back_screen;
        let tabs = self.segmented(
            "bs-tabs",
            vec![
                (
                    "灵感图库".into(),
                    state.tab == 0,
                    Action::BackScreen(BsAction::Tab(0)),
                ),
                (
                    "Amoo 创作".into(),
                    state.tab == 1,
                    Action::BackScreen(BsAction::Tab(1)),
                ),
                (
                    "图片转像素".into(),
                    state.tab == 2,
                    Action::BackScreen(BsAction::Tab(2)),
                ),
                (
                    "手机联动".into(),
                    state.tab == 3,
                    Action::BackScreen(BsAction::Tab(3)),
                ),
                (
                    "小游戏".into(),
                    state.tab == 4,
                    Action::BackScreen(BsAction::Tab(4)),
                ),
            ],
        );
        let content = match state.tab {
            1 => self.bs_amoo(),
            2 => self.bs_images(),
            3 => self.bs_android(),
            4 => self.bs_games(),
            _ => self.bs_gallery(),
        };
        let mut creative = Vec::new();
        if state.advanced {
            creative.push(self.bs_advanced());
        }
        creative.push(content);
        let right = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(S3)
            .with_child(tabs)
            .with_child(
                Expanded::new(
                    1.,
                    self.pane_scroll(
                        state.scroll.clone(),
                        Container::new(self.vstack(S4, creative))
                            .with_padding_right(S1)
                            .finish(),
                    ),
                )
                .finish(),
            )
            .with_child(if state.tab == 4 {
                Empty::new().finish()
            } else {
                self.bs_settings()
            })
            .finish();
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_spacing(S4)
            .with_child(self.fixed_w(268., self.bs_preview()))
            .with_child(Expanded::new(1., right).finish())
            .finish()
    }
}

fn game_preview_frame(elapsed_ms: u64) -> Vec<u8> {
    let mut frame = vec![0; bs::SLOTS];
    let mut remaining = elapsed_ms % 50_000;
    let mut width = 5;
    let mut points = (elapsed_ms / 50_000) * 150;
    for (index, duration) in [12_000, 11_000, 10_000, 9_000, 8_000]
        .into_iter()
        .enumerate()
    {
        width = 5 - index;
        if remaining < duration {
            break;
        }
        remaining -= duration;
        points += width as u64 * 10;
    }
    let phase = remaining as f32 / ((7 + width) * 1000) as f32;
    const DIGITS: [&str; 10] = [
        "111101101101111",
        "010110010010111",
        "111001111100111",
        "111001111001111",
        "101101111001001",
        "111100111001111",
        "111100111101111",
        "111001001001001",
        "111101111101111",
        "111101111001111",
    ];
    for (column, digit) in format!("{:04}", points % 10_000).bytes().enumerate() {
        for (pixel, bit) in DIGITS[(digit - b'0') as usize].bytes().enumerate() {
            if bit == b'1' {
                frame[(1 + pixel / 3) * bs::WIDTH + 1 + column * 4 + pixel % 3] = 160;
            }
        }
    }
    let target_x = (bs::WIDTH - width) / 2;
    let bar_x = (target_x as f32 + (phase * std::f32::consts::TAU).sin() * 4.).round() as usize;
    let bar_y = 7 + (phase * 13.) as usize;
    for x in 1..16 {
        frame[21 * bs::WIDTH + x] = 50;
    }
    for x in target_x..target_x + width {
        frame[20 * bs::WIDTH + x] = 90;
        frame[21 * bs::WIDTH + x] = 180;
    }
    for x in bar_x..bar_x + width {
        frame[bar_y * bs::WIDTH + x] = 255;
    }
    frame
}
