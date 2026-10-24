//! Conventional context actions, sharing the same validation as keyboard/buttons.
use super::{theme::*, widgets::*, *};
use facloud_ui::{color::ColorU, platform::Cursor, TypedActionView};
#[derive(Clone, Debug)]
pub(super) enum Target {
    Input(usize),
    Selection,
    File(String),
    WorkspaceIntent(bool),
}
pub(super) struct Menu {
    target: Target,
    at: Vector2F,
    text: Option<String>,
    selected: Option<usize>,
}
impl Companion {
    pub(super) fn open_workspace_intent_menu(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(anchor) = ctx.element_position_by_id("workspace-intent-anchor".to_string()) else {
            return;
        };
        let Some(size) = ctx.windows().platform_window(ctx.window_id()).map(|w| w.size()) else {
            return;
        };
        let (x, y) = intent_menu_position(
            anchor.origin().x(),
            anchor.origin().y(),
            anchor.height(),
            size.x(),
            size.y(),
        );
        #[cfg(feature = "ui-capture")]
        if std::env::var_os("STEPX_CHAT_FIRST_TEST").is_some() {
            eprintln!(
                "MENU_GEOMETRY anchor={anchor:?} client={}x{} above={}",
                size.x(),
                size.y(),
                y < anchor.origin().y()
            );
        }
        use stepx_companion::amoo_workspace::Intent;
        let selected = [Intent::Chat, Intent::Files, Intent::Image, Intent::App]
            .iter()
            .position(|i| *i == self.workspace.intent)
            .unwrap_or(0);
        self.context_menu = Some(Menu {
            target: Target::WorkspaceIntent(y < anchor.origin().y()),
            at: Vector2F::new(x, y),
            text: None,
            selected: Some(selected),
        });
        ctx.notify();
    }
    pub(super) fn handle_context(&mut self, action: &Action, ctx: &mut ViewContext<Self>) -> bool {
        match action {
            Action::ContextOpen(target, at) => {
                if let Target::Input(i) = target {
                    self.focus = Some(*i);
                    self.selected_text = None;
                }
                if let Target::File(path) = target {
                    self.selected_file = self.files.iter().position(|f| &f.path == path);
                    self.focus = None;
                }
                self.context_menu = Some(Menu {
                    target: target.clone(),
                    at: *at,
                    text: self.selected_text.clone(),
                    selected: None,
                });
                ctx.notify();
                true
            }
            Action::ContextClose | Action::Escape if self.context_menu.is_some() => {
                self.context_menu = None;
                ctx.notify();
                true
            }
            Action::ContextNavigate(direction) => {
                if let Some(menu) = &self.context_menu {
                    let entries = self.context_entries(menu);
                    let count = entries.len();
                    if count > 0 {
                        let mut i = menu
                            .selected
                            .map(|i| i as isize)
                            .unwrap_or(if *direction > 0 { -1 } else { count as isize });
                        for _ in 0..count {
                            i = (i + *direction as isize).rem_euclid(count as isize);
                            if entries[i as usize].2.is_some() {
                                break;
                            }
                        }
                        self.context_menu.as_mut().unwrap().selected = Some(i as usize);
                    }
                }
                ctx.notify();
                true
            }
            Action::ContextActivate => {
                let next = self.context_menu.as_ref().and_then(|m| {
                    m.selected
                        .and_then(|i| self.context_entries(m).get(i).and_then(|e| e.2.clone()))
                });
                if let Some(next) = next {
                    self.run_context_action(&next, ctx);
                } else {
                    self.context_menu = None;
                }
                ctx.notify();
                true
            }
            Action::ContextRun(next) => {
                self.run_context_action(next, ctx);
                ctx.notify();
                true
            }
            _ => {
                if matches!(
                    action,
                    Action::Copy
                        | Action::CopyInput
                        | Action::CutInput
                        | Action::Paste
                        | Action::SelectAll
                        | Action::Insert(_)
                        | Action::UndoInput
                        | Action::RedoInput
                        | Action::Page(_)
                        | Action::CompactChat
                ) {
                    self.context_menu = None;
                }
                false
            }
        }
    }
    fn run_context_action(&mut self, next: &Action, ctx: &mut ViewContext<Self>) {
        let mut next = next.clone();
        if let Some(Menu {
            target: Target::File(path),
            ..
        }) = &self.context_menu
        {
            if let Some(i) = self.files.iter().position(|f| &f.path == path) {
                self.selected_file = Some(i);
                next = match next {
                    Action::ActivateFile(_) => Action::ActivateFile(i),
                    Action::DownloadFile(_) => Action::DownloadFile(i),
                    a => a,
                };
            } else {
                self.context_menu = None;
                self.status = "文件列表已变化，请重新选择文件".into();
                ctx.notify();
                return;
            }
        }
        self.context_menu = None;
        self.handle_action(&next, ctx);
    }
    fn context_entries(&self, menu: &Menu) -> Vec<(String, String, Option<Action>)> {
        let mut entries: Vec<(&str, &str, Option<Action>)> = Vec::new();
        let command = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl"
        };
        let copy_key = format!("{command}+C");
        let paste_key = format!("{command}+V");
        let cut_key = format!("{command}+X");
        let all_key = format!("{command}+A");
        match &menu.target {
            Target::WorkspaceIntent(_) => {
                use stepx_companion::amoo_workspace::Intent;
                return [Intent::Chat, Intent::Files, Intent::Image, Intent::App]
                    .into_iter()
                    .map(|intent| {
                        (
                            intent.label().into(),
                            String::new(),
                            Some(Action::Workspace(WorkspaceAction::Intent(intent))),
                        )
                    })
                    .collect();
            }
            Target::Input(i) => {
                let f = &self.fields[*i];
                let selected =
                    f.selection_range().is_some_and(|r| !r.is_empty()) && *i != F_LAB_KEY;
                entries.push((
                    "撤销",
                    "",
                    (!f.undo.is_empty()).then_some(Action::UndoInput),
                ));
                entries.push((
                    "重做",
                    "",
                    (!f.redo.is_empty()).then_some(Action::RedoInput),
                ));
                entries.push(("剪切", &cut_key, selected.then_some(Action::CutInput)));
                entries.push(("复制", &copy_key, selected.then_some(Action::CopyInput)));
                entries.push(("粘贴", &paste_key, Some(Action::Paste)));
                entries.push((
                    "全选",
                    &all_key,
                    (!f.text.is_empty()).then_some(Action::SelectAll),
                ));
            }
            Target::Selection => {
                let text = menu.text.clone().unwrap_or_default();
                entries.push((
                    "复制",
                    &copy_key,
                    (!text.is_empty())
                        .then(|| Action::CopyText(text.clone(), "已复制所选文字".into())),
                ));
                entries.push((
                    "朗读所选文字",
                    "",
                    (!text.is_empty()).then(|| Action::Speak("selected-text".into(), text)),
                ));
            }
            Target::File(path) => {
                if let Some((i, f)) = self.files.iter().enumerate().find(|(_, f)| &f.path == path) {
                    entries.push((
                        if f.directory {
                            "打开文件夹"
                        } else {
                            "打开"
                        },
                        "Enter",
                        Some(Action::ActivateFile(i)),
                    ));
                    if !f.directory {
                        entries.push(("下载到电脑…", "", Some(Action::DownloadFile(i))));
                    }
                    entries.push((
                        "复制路径",
                        "",
                        Some(Action::CopyText(
                            f.path.clone(),
                            "已复制手机文件路径".into(),
                        )),
                    ));
                    entries.push(("重命名…", "F2", Some(Action::OpenRename)));
                    entries.push(("移至回收站…", "Delete", Some(Action::Trash)));
                }
            }
        }
        entries
            .into_iter()
            .map(|(a, b, c)| (a.to_owned(), b.to_owned(), c))
            .collect()
    }
    pub(super) fn context_overlay(&self) -> Option<El> {
        let menu = self.context_menu.as_ref()?;
        let p = self.p();
        let entries = self.context_entries(menu);
        let is_intent = matches!(menu.target, Target::WorkspaceIntent(_));
        let rows = entries
            .into_iter()
            .enumerate()
            .map(|(i, (label, key, action))| {
                let enabled = action.is_some();
                let label = label.to_owned();
                let key = key.to_owned();
                let row = Hoverable::new(self.mouse_state(format!("context-{i}")), move |mouse| {
                    let content = if is_intent {
                        use stepx_companion::amoo_workspace::Intent;
                        let description = [
                            "日常聊天、看图、解读附件",
                            "整理资料，生成可编辑文件",
                            "描述画面，生成图片",
                            "制作并验证手机小工具",
                        ][i];
                        let selected = [Intent::Chat, Intent::Files, Intent::Image, Intent::App][i]
                            == self.workspace.intent;
                        self.between(
                            self.vstack(
                                2.,
                                vec![
                                    self.line(&label, BODY, p.text),
                                    self.line(description, MICRO, p.text3),
                                ],
                            ),
                            if selected {
                                self.ui_icon(icons::UiIcon::Check, 16., p.text)
                            } else {
                                self.line("", MICRO, p.text3)
                            },
                        )
                    } else {
                        self.between(
                            self.line(&label, BODY, if enabled { p.text } else { p.text3 }),
                            self.line(&key, MICRO, p.text3),
                        )
                    };
                    Container::new(content)
                        .with_horizontal_padding(S3)
                        .with_vertical_padding(S2)
                        .with_background_color(
                            if enabled
                                && (mouse.is_mouse_over_element() || menu.selected == Some(i))
                            {
                                p.hover
                            } else {
                                ColorU::transparent_black()
                            },
                        )
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
                        .finish()
                })
                .with_cursor(if enabled {
                    Cursor::PointingHand
                } else {
                    Cursor::Arrow
                })
                .on_click(move |ctx, _, _| {
                    if let Some(a) = &action {
                        ctx.dispatch_typed_action(Action::ContextRun(Box::new(a.clone())));
                    }
                })
                .finish();
                SavePosition::new(row, &format!("context-choice-{i}"))
                    .for_single_frame()
                    .finish()
            })
            .collect();
        let surface = self.fixed_w(
            if is_intent { 280. } else { 218. },
            Container::new(self.vstack(2., rows))
                .with_uniform_padding(S1)
                .with_background_color(p.bg)
                .with_border(Border::all(1.).with_border_color(p.border_strong))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
                .finish(),
        );
        let surface = if is_intent {
            motion::enter(
                surface,
                self.workspace.panel_entered,
                !self.settings.value.reduced_motion,
            )
        } else {
            surface
        };
        let dismiss = Hoverable::new(self.mouse_state("context-dismiss"), |_| {
            Container::new(Empty::new().finish())
                .with_background_color(ColorU::transparent_black())
                .finish()
        })
        .on_mouse_down(|ctx, _, _| ctx.dispatch_typed_action(Action::ContextClose))
        .finish();
        Some(
            Stack::new()
                .with_child(dismiss)
                .with_positioned_child(
                    SavePosition::new(surface, "context-surface")
                        .for_single_frame()
                        .finish(),
                    if let Target::WorkspaceIntent(above) = menu.target {
                        OffsetPositioning::offset_from_save_position_element(
                            "workspace-intent-anchor",
                            Vector2F::new(0., if above { -6. } else { 6. }),
                            PositionedElementOffsetBounds::WindowByPosition,
                            if above {
                                PositionedElementAnchor::TopLeft
                            } else {
                                PositionedElementAnchor::BottomLeft
                            },
                            if above {
                                ChildAnchor::BottomLeft
                            } else {
                                ChildAnchor::TopLeft
                            },
                        )
                    } else {
                        OffsetPositioning::offset_from_parent(
                            menu.at,
                            ParentOffsetBounds::WindowByPosition,
                            ParentAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        )
                    },
                )
                .finish(),
        )
    }
}

// Window-local coordinates. Prefer the button's lower edge; only flip for the bottom composer.
fn intent_menu_position(x: f32, y: f32, anchor_height: f32, width: f32, height: f32) -> (f32, f32) {
    let menu_height = 240.;
    let below = y + anchor_height + 6.;
    (
        x.clamp(8., (width - 288.).max(8.)),
        if below + menu_height <= height - 8. {
            below
        } else {
            (y - menu_height - 6.).max(8.)
        },
    )
}
#[cfg(test)]
mod intent_tests {
    use super::*;
    #[test]
    fn opens_below_when_room() {
        assert_eq!(
            intent_menu_position(120., 200., 36., 1000., 800.),
            (120., 242.)
        );
    }
    #[test]
    fn flips_above_bottom_composer() {
        assert_eq!(
            intent_menu_position(120., 700., 36., 1000., 800.),
            (120., 454.)
        );
    }
    #[test]
    fn stays_inside_narrow_window() {
        assert_eq!(
            intent_menu_position(480., 500., 36., 600., 650.),
            (312., 254.)
        );
    }
}
