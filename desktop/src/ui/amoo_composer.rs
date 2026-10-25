//! One composer for full and compact Amoo: draft, attachments and actions share a surface.
use super::{icons::UiIcon, theme::*, widgets::*, *};
use facloud_ui::{
    assets::asset_cache::AssetSource, color::ColorU, fonts::Weight, platform::Cursor,
};

impl Companion {
    /// Keep room to read the conversation on short windows. The input and
    /// attachments scroll independently instead of consuming the entire pane.
    pub(super) fn composer_limits(&self, compact: bool) -> (f32, f32) {
        if self.workspace.viewport.y() < 520. {
            (48., 44.)
        } else if self.workspace.viewport.y() < 560. {
            (64., 52.)
        } else if self.workspace.viewport.y() < 720. {
            (96., 64.)
        } else if compact {
            (126., 88.)
        } else {
            (160., 104.)
        }
    }
    fn composer_icon(
        &self,
        id: impl Into<String>,
        label: &str,
        icon: UiIcon,
        action: Option<Action>,
        primary: bool,
    ) -> El {
        let id = id.into();
        let p = self.p();
        let enabled = action.is_some();
        let label = label.to_owned();
        let control = Hoverable::new(self.mouse_state(id.clone()), move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let fg = if primary && enabled {
                p.accent_fg
            } else if enabled {
                p.text2
            } else {
                p.text3
            };
            let bg = if primary && enabled {
                p.accent
            } else if primary || (hovered && enabled) {
                p.hover
            } else {
                ColorU::transparent_black()
            };
            let button = ConstrainedBox::new(
                Container::new(Align::new(self.ui_icon(icon, 18., fg)).finish())
                    .with_background_color(bg)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(if primary {
                        16.
                    } else {
                        R_CONTROL
                    })))
                    .finish(),
            )
            .with_width(32.)
            .with_height(32.)
            .finish();
            if !hovered {
                return button;
            }
            let tooltip = Container::new(self.line(&label, MICRO, p.text))
                .with_horizontal_padding(S2)
                .with_vertical_padding(S1)
                .with_background_color(p.bg)
                .with_border(Border::all(1.).with_border_color(p.border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish();
            Stack::new()
                .with_child(button)
                .with_positioned_child(
                    tooltip,
                    OffsetPositioning::offset_from_parent(
                        vec2f(0., -S1),
                        ParentOffsetBounds::WindowByPosition,
                        ParentAnchor::TopMiddle,
                        ChildAnchor::BottomMiddle,
                    ),
                )
                .finish()
        })
        .with_cursor(if enabled {
            Cursor::PointingHand
        } else {
            Cursor::Arrow
        })
        .on_click(move |ctx, _, _| {
            if let Some(action) = &action {
                ctx.dispatch_typed_action(action.clone());
            }
        })
        .finish();
        #[cfg(feature = "ui-capture")]
        let control = super::scenario_check::control(id, control);
        control
    }

    fn amoo_composer_editor(&self, phone_mode: bool, compact: bool) -> El {
        let p = self.p();
        let field = &self.fields[F_AMOO];
        let focused = self.focus == Some(F_AMOO);
        let empty = field.text.is_empty() && field.preedit.is_empty();
        let placeholder = if phone_mode {
            "描述要在手机上完成的操作…"
        } else {
            "发消息，也可以添加图片和文件…"
        };
        let mut value = field.text.clone();
        let mut caret = 0;
        if focused {
            let cursor = field.cursor.min(value.len());
            // Text's saved positions use Unicode character indices; Input uses
            // UTF-8 byte offsets for editing. Convert before positioning IME/scroll.
            caret = value[..cursor].chars().count() + field.preedit.chars().count();
            value.insert_str(cursor, &field.preedit);
        }
        if empty {
            value.push_str(placeholder);
        }
        let editor = self.pane_scroll(
            self.amoo_ui.composer_scroll.clone(),
            Container::new(self.multiline_field_text(
                &value,
                caret,
                F_AMOO,
                if empty { p.text3 } else { p.text },
            ))
            .with_padding_bottom(S1)
            .finish(),
        );
        // Use measured wrapped text height, not a character-count estimate. Long
        // drafts scroll independently, keeping both the conversation and tools visible.
        SavePosition::new(
            Hoverable::new(self.mouse_state("amoo-composer-editor"), move |_| {
                ConstrainedBox::new(
                    Container::new(editor)
                        .with_uniform_padding(S1)
                        .with_background_color(if field.all {
                            p.accent_soft
                        } else {
                            ColorU::transparent_black()
                        })
                        .finish(),
                )
                .with_min_height(
                    (if compact { 48.0_f32 } else { 68.0 }).min(self.composer_limits(compact).0),
                )
                .with_max_height(self.composer_limits(compact).0)
                .finish()
            })
            .with_cursor(Cursor::IBeam)
            .with_defer_events_to_children()
            .on_mouse_down(|ctx, _, _| ctx.dispatch_typed_action(Action::Focus(F_AMOO)))
            .on_right_click(|ctx, _, at| {
                ctx.dispatch_typed_action(Action::ContextOpen(
                    context_menu::Target::Input(F_AMOO),
                    at,
                ))
            })
            .finish(),
            "composer-editor",
        )
        .for_single_frame()
        .finish()
    }

    pub(super) fn amoo_message_composer(&self, compact: bool, phone_mode: bool) -> El {
        let p = self.p();
        let focused = self.focus == Some(F_AMOO);
        let pending = self
            .amoo_sessions
            .get(self.amoo_session_idx)
            .is_some_and(|s| self.amoo_pending_reply.contains_key(&s.session_key));
        let occupied = self.busy || self.workspace_busy();
        let connected = self.selected.is_some() || self.demo_mode;
        let gui_blocked = phone_mode
            && (self.amoo_ui.gui_checking
                || self.amoo_ui.gui_unconfirmed
                || (!self.amoo_ui.gui_desktop && self.amoo_ui.gui_unavailable_reason.is_some()));
        let has_content = !self.fields[F_AMOO].text.trim().is_empty()
            || (!phone_mode && !self.amoo_attachments.is_empty());
        let gui_has_attachments = phone_mode && !self.amoo_attachments.is_empty();
        let can_send = connected
            && has_content
            && !occupied
            && !gui_blocked
            && !gui_has_attachments
            && (phone_mode || !pending)
            && self.fields[F_AMOO].preedit.is_empty();
        let can_send = if phone_mode {
            can_send
        } else {
            self.workspace_can_send()
        };
        let can_stop = !phone_mode && !self.busy && self.workspace.running.is_some();
        let stop_label = if self.workspace.running.as_ref().is_some_and(|r| {
            matches!(
                r.executor,
                stepx_companion::amoo_workspace::Executor::Files
                    | stepx_companion::amoo_workspace::Executor::App
            )
        }) {
            "停止任务"
        } else {
            "停止等待，手机任务可能继续"
        };
        let mut rows = Vec::new();
        if !self.amoo_attachments.is_empty() && !phone_mode {
            if self.amoo_attachments.len() > 1 {
                rows.push(self.between(
                    self.line(
                        format!("已添加 {} 个附件", self.amoo_attachments.len()),
                        MICRO,
                        p.text2,
                    ),
                    self.line("滚动查看", MICRO, p.text3),
                ));
            }
            let chips = self
                .amoo_attachments
                .iter()
                .enumerate()
                .map(|(index, att)| {
                    let visual = if att.is_image {
                        match &att.local_path {
                            Some(path) => ConstrainedBox::new(
                                Image::new(
                                    AssetSource::LocalFile {
                                        path: path.to_string_lossy().into_owned(),
                                    },
                                    CacheOption::BySize,
                                )
                                .contain()
                                .finish(),
                            )
                            .with_width(32.)
                            .with_height(32.)
                            .finish(),
                            None => self.ui_icon(UiIcon::Image, 24., p.text2),
                        }
                    } else {
                        self.ui_icon(UiIcon::File, 24., p.text2)
                    };
                    Container::new(self.hstack(
                        S2,
                        vec![
                                visual,
                                ConstrainedBox::new(self.vstack(
                                    2.,
                                    vec![
                                        self.line_w(&att.name, CAPTION, p.text, Weight::Medium),
                                        self.line(&att.size_label, MICRO, p.text3),
                                    ],
                                ))
                                .with_max_width(if compact { 150. } else { 190. })
                                .finish(),
                                self.composer_icon(
                                    format!("amoo-composer-remove-{index}"),
                                    "移除附件",
                                    UiIcon::Close,
                                    (!self.busy).then_some(Action::AmooRemoveAttachment(index)),
                                    false,
                                ),
                            ],
                    ))
                    .with_padding_left(S2)
                    .with_padding_right(S1)
                    .with_vertical_padding(S1)
                    .with_background_color(p.surface)
                    .with_border(Border::all(1.).with_border_color(p.border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                    .finish()
                })
                .collect();
            rows.push(
                SavePosition::new(
                    ConstrainedBox::new(
                        self.pane_scroll(self.amoo_ui.attachment_scroll.clone(), self.wrap(chips)),
                    )
                    .with_max_height(self.composer_limits(compact).1)
                    .finish(),
                    "composer-attachments",
                )
                .for_single_frame()
                .finish(),
            );
        }
        rows.push(self.amoo_composer_editor(phone_mode, compact));
        let attachments_enabled = !self.busy && !self.workspace_busy();
        let tools = if phone_mode {
            self.line("手机界面操作", MICRO, p.text2)
        } else {
            self.hstack(
                S1,
                vec![
                    self.composer_icon(
                        "amoo-composer-image",
                        "添加图片",
                        UiIcon::Image,
                        attachments_enabled.then_some(Action::AmooAttachFile(true)),
                        false,
                    ),
                    self.composer_icon(
                        "amoo-composer-file",
                        "添加文件",
                        UiIcon::Attach,
                        attachments_enabled.then_some(Action::AmooAttachFile(false)),
                        false,
                    ),
                    self.composer_icon(
                        "amoo-composer-album",
                        "从手机相册选取",
                        UiIcon::Smartphone,
                        (attachments_enabled && connected).then_some(Action::AmooAttachPhoneAlbum),
                        false,
                    ),
                ],
            )
        };
        let hint = if !connected {
            "请连接手机"
        } else if self.busy {
            "正在处理…"
        } else if self.amoo_ui.gui_running {
            "手机正在执行…"
        } else if !phone_mode && pending {
            "Amoo 正在回复…"
        } else if gui_has_attachments {
            "请切回后台对话发送附件"
        } else if gui_blocked {
            "手机操作暂不可用"
        } else {
            "发送到手机 Amoo · Enter 发送"
        };
        let mut actions = vec![tools];
        if !phone_mode {
            actions.push(
                SavePosition::new(
                    self.button_icon(
                        "workspace-composer-intent",
                        self.workspace.intent.label(),
                        UiIcon::ChevronDown,
                        Action::Workspace(WorkspaceAction::Menu(7)),
                        Btn::Ghost,
                    ),
                    "workspace-intent-anchor",
                )
                .for_single_frame()
                .finish(),
            );
            if !compact || self.workspace.viewport.x() >= 480. {
                actions.push(self.composer_icon(
                    "workspace-composer-model",
                    "模型与账号",
                    UiIcon::Cpu,
                    Some(Action::Workspace(WorkspaceAction::Menu(1))),
                    false,
                ));
            }
            actions.push(self.composer_icon(
                "workspace-composer-resources",
                "更多：模型、技能和成果",
                UiIcon::Plus,
                Some(Action::Workspace(WorkspaceAction::Menu(2))),
                false,
            ));
        }
        rows.push(
            SavePosition::new(
                self.hstack(
                    S2,
                    vec![
                        self.hstack(S1, actions),
                        Expanded::new(
                            1.,
                            Align::new(self.line(
                                if !occupied && !pending {
                                    ""
                                } else if self.workspace.viewport.x() < 1000. {
                                    "…"
                                } else {
                                    hint
                                },
                                MICRO,
                                p.text3,
                            ))
                            .right()
                            .finish(),
                        )
                        .finish(),
                        SavePosition::new(
                            self.composer_icon(
                                "amoo-composer-send",
                                if can_stop {
                                    stop_label
                                } else if can_send {
                                    if phone_mode {
                                        "开始操作"
                                    } else {
                                        "发送消息"
                                    }
                                } else if has_content {
                                    hint
                                } else {
                                    "输入消息或添加附件后发送"
                                },
                                if can_stop {
                                    UiIcon::Close
                                } else {
                                    UiIcon::ArrowUp
                                },
                                if can_stop {
                                    Some(Action::Workspace(WorkspaceAction::Stop))
                                } else {
                                    can_send.then_some(if phone_mode {
                                        Action::AmooSend
                                    } else {
                                        Action::Workspace(WorkspaceAction::Prepare)
                                    })
                                },
                                true,
                            ),
                            "composer-send",
                        )
                        .for_single_frame()
                        .finish(),
                    ],
                ),
                "composer-toolbar",
            )
            .for_single_frame()
            .finish(),
        );
        SavePosition::new(
            Container::new(self.vstack(S2, rows))
                .with_uniform_padding(S3)
                .with_background_color(p.bg)
                .with_border(Border::all(1.).with_border_color(if focused {
                    p.border_strong
                } else {
                    p.border
                }))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(16.)))
                .finish(),
            "composer-surface",
        )
        .for_single_frame()
        .finish()
    }
}
