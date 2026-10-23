//! Reusable components. Every visual value comes from `theme`.

use super::{
    icons::{self, UiIcon},
    theme::*,
    Action, Companion,
};
use facloud_ui::{
    color::ColorU,
    elements::*,
    fonts::{Properties, Weight},
    platform::Cursor,
    text_layout::ClipConfig,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    Primary,
    Secondary,
    Ghost,
    Danger,
    DangerSolid,
}

pub type El = Box<dyn Element>;

pub(super) fn ui_label(
    text: String,
    font: facloud_ui::fonts::FamilyId,
    size: f32,
    color: ColorU,
    weight: Weight,
) -> El {
    let Some((icon, rest)) = icons::label_icon(&text) else {
        return Text::new(text, font, size)
            .with_color(color)
            .with_style(Properties::default().weight(weight))
            .soft_wrap(false)
            .with_clip(ClipConfig::ellipsis())
            .finish();
    };
    let icon_size = size.max(14.);
    let image = icons::draw_icon(icon, icon_size, color);
    if rest.is_empty() {
        return image;
    }
    // A non-flex Text inside a row is measured with infinite width, losing its
    // ellipsis. Padding preserves the parent's width constraint while a
    // positioned icon still allows buttons to measure their intrinsic width.
    let label = Container::new(
        Text::new(rest.to_string(), font, size)
            .with_color(color)
            .with_style(Properties::default().weight(weight))
            .soft_wrap(false)
            .with_clip(ClipConfig::ellipsis())
            .finish(),
    )
    .with_padding_left(icon_size + 6.)
    .finish();
    Stack::new()
        .with_child(
            ConstrainedBox::new(label)
                .with_min_height(icon_size)
                .finish(),
        )
        .with_positioned_child(
            image,
            OffsetPositioning::offset_from_parent(
                pathfinder_geometry::vector::vec2f(0., 0.),
                ParentOffsetBounds::Unbounded,
                ParentAnchor::MiddleLeft,
                ChildAnchor::MiddleLeft,
            ),
        )
        .finish()
}

impl Companion {
    pub fn p(&self) -> Palette {
        self.palette.get()
    }

    // ---- Text ----

    pub fn text(&self, text: impl Into<String>, size: f32, color: ColorU) -> El {
        Text::new(text.into(), self.font, size)
            .with_color(color)
            .with_line_height_ratio(1.45)
            .finish()
    }

    pub fn text_w(&self, text: impl Into<String>, size: f32, color: ColorU, weight: Weight) -> El {
        Text::new(text.into(), self.font, size)
            .with_color(color)
            .with_style(Properties::default().weight(weight))
            .finish()
    }

    /// Single line, truncated with an ellipsis when it does not fit.
    pub fn line(&self, text: impl Into<String>, size: f32, color: ColorU) -> El {
        ui_label(text.into(), self.font, size, color, Weight::Normal)
    }

    pub fn line_w(&self, text: impl Into<String>, size: f32, color: ColorU, weight: Weight) -> El {
        ui_label(text.into(), self.font, size, color, weight)
    }

    pub fn mono(&self, text: impl Into<String>, size: f32, color: ColorU) -> El {
        Text::new(text.into(), self.mono_font, size)
            .with_color(color)
            .soft_wrap(false)
            .with_clip(ClipConfig::ellipsis())
            .finish()
    }

    pub fn heading(&self, text: impl Into<String>) -> El {
        let p = self.p();
        self.line_w(text, HEADING, p.text, Weight::Semibold)
    }

    pub fn caption(&self, text: impl Into<String>) -> El {
        let p = self.p();
        self.text(text, CAPTION, p.text2)
    }

    // ---- Layout ----

    pub fn hstack(&self, spacing: f32, children: Vec<El>) -> El {
        Flex::row()
            .with_spacing(spacing)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_children(children)
            .finish()
    }

    pub fn vstack(&self, spacing: f32, children: Vec<El>) -> El {
        Flex::column()
            .with_spacing(spacing)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_children(children)
            .finish()
    }

    pub fn wrap(&self, children: Vec<El>) -> El {
        Wrap::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(S2)
            .with_run_spacing(S2)
            .with_children(children)
            .finish()
    }

    /// Left side takes the remaining width (and truncates/wraps); right side keeps its size.
    pub fn between(&self, left: El, right: El) -> El {
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_spacing(S3)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Expanded::new(1., left).finish())
            .with_child(right)
            .finish()
    }

    pub fn fixed_w(&self, width: f32, child: El) -> El {
        ConstrainedBox::new(child).with_width(width).finish()
    }

    /// Fixed leading element followed by content side-by-side.
    pub fn lead(&self, spacing: f32, leading: El, rest: El) -> El {
        self.hstack(spacing, vec![leading, rest])
    }

    pub fn spacer(&self) -> El {
        Expanded::new(1., Empty::new().finish()).finish()
    }

    pub fn divider(&self) -> El {
        let p = self.p();
        ConstrainedBox::new(Rect::new().with_background_color(p.border).finish())
            .with_height(1.)
            .finish()
    }

    #[allow(dead_code)]
    pub fn vdivider(&self, height: f32) -> El {
        let p = self.p();
        ConstrainedBox::new(Rect::new().with_background_color(p.border).finish())
            .with_width(1.)
            .with_height(height)
            .finish()
    }

    // ---- Containers ----

    pub fn panel(&self, children: Vec<El>) -> El {
        let p = self.p();
        Container::new(self.vstack(S3, children))
            .with_uniform_padding(S4)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
    }

    /// Bordered list: rows separated by 1px dividers, no inner padding.
    pub fn list(&self, rows: Vec<El>) -> El {
        let p = self.p();
        let mut items = Vec::with_capacity(rows.len() * 2);
        for (i, row) in rows.into_iter().enumerate() {
            if i > 0 {
                items.push(self.divider());
            }
            items.push(row);
        }
        Container::new(self.vstack(0., items))
            .with_uniform_padding(1.)
            .with_background_color(p.bg)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
    }

    /// Section: heading row (title + optional trailing control) above its body.
    pub fn section(&self, title: impl Into<String>, trailing: Option<El>, body: El) -> El {
        let header = match trailing {
            Some(t) => self.between(self.heading(title), t),
            None => self.heading(title),
        };
        self.vstack(S2 + 2., vec![header, body])
    }

    pub fn list_row(
        &self,
        id: impl Into<String>,
        content: El,
        selected: bool,
        on_click: Option<Action>,
        on_double: Option<Action>,
    ) -> El {
        let p = self.p();
        let state = self.mouse_state(format!("row:{}", id.into()));
        let interactive = on_click.is_some() || on_double.is_some();
        let mut row = Hoverable::new(state, move |mouse| {
            let bg = if selected {
                p.selected
            } else if interactive && mouse.is_mouse_over_element() {
                p.hover
            } else {
                p.bg
            };
            Container::new(content)
                .with_horizontal_padding(S3)
                .with_vertical_padding(S2 + 2.)
                .with_background_color(bg)
                .finish()
        });
        if interactive {
            row = row.with_cursor(Cursor::PointingHand);
        }
        if let Some(action) = on_click {
            row = row.on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()));
        }
        if let Some(action) = on_double {
            row = row.on_double_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()));
        }
        row.finish()
    }

    pub fn empty(
        &self,
        title: impl Into<String>,
        detail: impl Into<String>,
        action: Option<El>,
    ) -> El {
        let p = self.p();
        let mut items = vec![
            Align::new(self.text_w(title, HEADING, p.text, Weight::Medium)).finish(),
            Align::new(self.text(detail, CAPTION, p.text3)).finish(),
        ];
        if let Some(a) = action {
            items.push(Align::new(Container::new(a).with_margin_top(S2).finish()).finish());
        }
        Container::new(self.vstack(S1 + 2., items))
            .with_vertical_padding(S6)
            .with_horizontal_padding(S4)
            .finish()
    }

    /// Inline confirmation used for irreversible actions.
    pub fn confirm_bar(&self, message: impl Into<String>, danger: bool, buttons: Vec<El>) -> El {
        let p = self.p();
        Container::new(self.between(
            self.text(message, BODY, if danger { p.danger } else { p.text }),
            self.hstack(S2, buttons),
        ))
        .with_horizontal_padding(S3)
        .with_vertical_padding(S2)
        .with_background_color(if danger { p.danger_soft } else { p.accent_soft })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
        .finish()
    }

    // ---- Small parts ----

    pub fn tag(&self, text: impl Into<String>, fg: ColorU) -> El {
        let p = self.p();
        Container::new(
            Text::new(text.into(), self.font, MICRO)
                .with_color(fg)
                .with_style(Properties::default().weight(Weight::Medium))
                .soft_wrap(false)
                .finish(),
        )
        .with_horizontal_padding(6.)
        .with_vertical_padding(2.)
        .with_background_color(p.tag_bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_TAG)))
        .finish()
    }

    pub fn dot(&self, color: ColorU) -> El {
        ConstrainedBox::new(
            Rect::new()
                .with_background_color(color)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .finish(),
        )
        .with_width(8.)
        .with_height(8.)
        .finish()
    }

    pub fn progress(&self, percent: Option<u8>, color: ColorU) -> El {
        let p = self.p();
        let pct = percent.unwrap_or(0).min(100) as f32;
        let bar = |c: ColorU| {
            ConstrainedBox::new(Rect::new().with_background_color(c).finish())
                .with_height(6.)
                .finish()
        };
        let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        if pct > 0. {
            row = row.with_child(Expanded::new(pct, bar(color)).finish());
        }
        if pct < 100. {
            row = row.with_child(Expanded::new(100. - pct, bar(p.tag_bg)).finish());
        }
        Clipped::new(
            Container::new(row.finish())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(3.)))
                .finish(),
        )
        .finish()
    }

    // ---- Controls ----

    pub fn button(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        action: Action,
        kind: Btn,
    ) -> El {
        let p = self.p();
        let id = id.into();
        let label = label.into();
        let state = self.mouse_state(format!("btn:{id}"));
        let font = self.font;
        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let (bg, border, fg) = match kind {
                Btn::Primary => {
                    let c = if hovered { p.accent_hover } else { p.accent };
                    (c, c, p.on_accent)
                }
                Btn::Secondary => (
                    if hovered { p.hover } else { p.bg },
                    if hovered { p.border_strong } else { p.border },
                    p.text,
                ),
                Btn::Ghost => (
                    if hovered {
                        p.hover
                    } else {
                        ColorU::transparent_black()
                    },
                    ColorU::transparent_black(),
                    if hovered { p.text } else { p.text2 },
                ),
                Btn::Danger => (
                    if hovered { p.danger_soft } else { p.bg },
                    p.border,
                    p.danger,
                ),
                Btn::DangerSolid => (p.danger, p.danger, p.on_accent),
            };
            Container::new(
                ConstrainedBox::new(
                    Align::new(ui_label(
                        label.clone(),
                        font,
                        BODY,
                        fg,
                        if matches!(kind, Btn::Primary | Btn::DangerSolid) {
                            Weight::Medium
                        } else {
                            Weight::Normal
                        },
                    ))
                    .finish(),
                )
                .with_height(28.)
                .finish(),
            )
            .with_horizontal_padding(S3)
            .with_background_color(bg)
            .with_border(Border::all(1.).with_border_color(border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    /// Text-style link button (used inside rows and breadcrumbs).
    pub fn link(&self, id: impl Into<String>, label: impl Into<String>, action: Action) -> El {
        let p = self.p();
        let state = self.mouse_state(format!("link:{}", id.into()));
        let label = label.into();
        let font = self.font;
        Hoverable::new(state, move |mouse| {
            ui_label(
                label.clone(),
                font,
                BODY,
                if mouse.is_mouse_over_element() {
                    p.accent_hover
                } else {
                    p.accent
                },
                Weight::Normal,
            )
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    /// Quiet inline action (e.g. under a message). Muted until hovered.
    pub fn link_muted(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        action: Action,
    ) -> El {
        let p = self.p();
        let state = self.mouse_state(format!("mlink:{}", id.into()));
        let label = label.into();
        let font = self.font;
        Hoverable::new(state, move |mouse| {
            ui_label(
                label.clone(),
                font,
                CAPTION,
                if mouse.is_mouse_over_element() {
                    p.accent
                } else {
                    p.text3
                },
                Weight::Normal,
            )
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    pub fn disabled_button(&self, label: impl Into<String>) -> El {
        let p = self.p();
        Container::new(
            ConstrainedBox::new(Align::new(self.line(label, BODY, p.text3)).finish())
                .with_height(28.)
                .finish(),
        )
        .with_horizontal_padding(S3)
        .with_border(Border::all(1.).with_border_color(p.border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
        .finish()
    }

    /// Segmented control for 2–4 mutually exclusive options.
    pub fn segmented(&self, id: &str, items: Vec<(String, bool, Action)>) -> El {
        let p = self.p();
        let font = self.font;
        let children: Vec<El> = items
            .into_iter()
            .enumerate()
            .map(|(i, (label, active, action))| {
                let state = self.mouse_state(format!("seg:{id}:{i}"));
                Hoverable::new(state, move |mouse| {
                    let hovered = mouse.is_mouse_over_element();
                    let mut c = Container::new(
                        Text::new(label.clone(), font, BODY)
                            .with_color(if active || hovered { p.text } else { p.text2 })
                            .with_style(Properties::default().weight(if active {
                                Weight::Medium
                            } else {
                                Weight::Normal
                            }))
                            .soft_wrap(false)
                            .finish(),
                    )
                    .with_horizontal_padding(S3)
                    .with_vertical_padding(4.)
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL - 1.)));
                    if active {
                        c = c
                            .with_background_color(p.bg)
                            .with_border(Border::all(1.).with_border_color(p.border));
                    } else {
                        c = c.with_border(
                            Border::all(1.).with_border_color(ColorU::transparent_black()),
                        );
                    }
                    c.finish()
                })
                .with_cursor(Cursor::PointingHand)
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
                .finish()
            })
            .collect();
        Container::new(self.hstack(2., children))
            .with_uniform_padding(2.)
            .with_background_color(p.sidebar)
            .with_border(Border::all(1.).with_border_color(p.border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL + 1.)))
            .finish()
    }

    /// Text field bound to `self.fields[index]`. Without a width it fills the parent.
    pub fn input(&self, index: usize, placeholder: &str, width: Option<f32>) -> El {
        let p = self.p();
        let field = &self.fields[index];
        let focused = self.focus == Some(index);
        let masked = index == super::F_PAIR_CODE && !self.show_secret;
        let mut value = if masked {
            "•".repeat(field.text.chars().count())
        } else {
            field.text.clone()
        };
        let mut caret_index = 0;
        if focused {
            let safe_cursor = field.cursor.min(field.text.len());
            let cursor = if masked {
                field.text[..safe_cursor].chars().count() * "•".len()
            } else {
                safe_cursor
            };
            caret_index = cursor + field.preedit.len();
            value.insert_str(cursor.min(value.len()), &format!("{}│", field.preedit));
        }
        let is_empty = field.text.is_empty() && field.preedit.is_empty();
        if value.is_empty() {
            value = placeholder.into();
        }
        let text_color = if is_empty && !focused {
            p.text3
        } else {
            p.text
        };
        let selected_all = field.all;
        let font = self.font;
        let state = self.mouse_state(format!("input:{index}"));
        let field_box = Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let border = if focused {
                p.accent
            } else if hovered {
                p.border_strong
            } else {
                p.surface_border
            };
            Container::new(
                Text::new(value.clone(), font, BODY)
                    .with_saved_char_position(caret_index, format!("input-caret:{index}"))
                    .with_color(text_color)
                    .soft_wrap(false)
                    .with_clip(ClipConfig::ellipsis())
                    .finish(),
            )
            .with_horizontal_padding(10.)
            .with_vertical_padding(6.)
            .with_background_color(if selected_all {
                p.accent_soft
            } else {
                p.surface
            })
            .with_border(Border::all(1.).with_border_color(border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
            .finish()
        })
        .with_cursor(Cursor::IBeam)
        .on_mouse_down(move |ctx, _, _| ctx.dispatch_typed_action(Action::Focus(index)))
        .finish();

        match width {
            Some(w) => self.fixed_w(w, field_box),
            None => field_box,
        }
    }

    pub fn checkbox_row(
        &self,
        id: impl Into<String>,
        checked: bool,
        label: impl Into<String>,
        action: Action,
    ) -> El {
        let p = self.p();
        let state = self.mouse_state(format!("chk:{}", id.into()));
        let label = label.into();
        let font = self.font;
        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let box_bg = if checked {
                p.accent
            } else if hovered {
                p.hover
            } else {
                p.bg
            };
            let box_border = if checked { p.accent } else { p.border_strong };
            let mark = ConstrainedBox::new(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_main_axis_alignment(MainAxisAlignment::Center)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(if checked {
                            icons::draw_icon(UiIcon::Check, 11., p.on_accent)
                        } else {
                            Empty::new().finish()
                        })
                        .finish(),
                )
                .with_background_color(box_bg)
                .with_border(Border::all(1.).with_border_color(box_border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .finish(),
            )
            .with_width(16.)
            .with_height(16.)
            .finish();
            Flex::row()
                .with_spacing(S2)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(mark)
                .with_child(
                    Text::new(label.clone(), font, BODY)
                        .with_color(if hovered { p.text } else { p.text2 })
                        .soft_wrap(false)
                        .finish(),
                )
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    pub fn icon_tile_btn(
        &self,
        id: impl Into<String>,
        icon: &str,
        title: &str,
        subtitle: Option<&str>,
        active: bool,
        action: Action,
    ) -> El {
        let p = self.p();
        let state = self.mouse_state(format!("tile:{}", id.into()));
        let icon = icon.to_string();
        let title = title.to_string();
        let subtitle = subtitle.map(str::to_string);
        let font = self.font;
        Hoverable::new(state, move |mouse| {
            let hovered = mouse.is_mouse_over_element();
            let bg = if active {
                p.selected
            } else if hovered {
                p.hover
            } else {
                p.surface
            };
            let border = if active {
                p.accent
            } else if hovered {
                p.border_strong
            } else {
                p.border
            };
            let mut col = vec![
                ui_label(
                    icon.clone(),
                    font,
                    HEADING + 1.,
                    if active { p.accent } else { p.text },
                    Weight::Normal,
                ),
                Text::new(title.clone(), font, CAPTION)
                    .with_color(if active { p.accent } else { p.text })
                    .with_style(Properties::default().weight(Weight::Medium))
                    .soft_wrap(false)
                    .finish(),
            ];
            if let Some(sub) = &subtitle {
                col.push(
                    Text::new(sub.clone(), font, MICRO)
                        .with_color(p.text3)
                        .soft_wrap(false)
                        .finish(),
                );
            }
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Min)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_spacing(3.)
                    .with_children(col)
                    .finish(),
            )
            .with_horizontal_padding(S2)
            .with_vertical_padding(S2 + 2.)
            .with_background_color(bg)
            .with_border(Border::all(1.).with_border_color(border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_PANEL)))
            .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    pub fn pill_badge(&self, text: impl Into<String>, fg: ColorU, bg: ColorU) -> El {
        Container::new(
            Text::new(text.into(), self.font, MICRO)
                .with_color(fg)
                .with_style(Properties::default().weight(Weight::Medium))
                .soft_wrap(false)
                .finish(),
        )
        .with_horizontal_padding(8.)
        .with_vertical_padding(2.)
        .with_background_color(bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(9.)))
        .finish()
    }

    pub fn link_action(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        action: Action,
    ) -> El {
        self.link(id, label, action)
    }

    pub fn icon_box(
        &self,
        symbol: &str,
        size: f32,
        bg: ColorU,
        fg: ColorU,
        border: Option<ColorU>,
    ) -> El {
        let mut c = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    icons::symbol(symbol)
                        .map(|icon| self.ui_icon(icon, 18., fg))
                        .unwrap_or_else(|| self.line_w(symbol, BODY, fg, Weight::Semibold)),
                )
                .finish(),
        )
        .with_background_color(bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)));
        if let Some(b) = border {
            c = c.with_border(Border::all(1.).with_border_color(b));
        }
        ConstrainedBox::new(c.finish())
            .with_width(size)
            .with_height(size)
            .finish()
    }

    /// Renders a 17×23 (391-pixel, 379 active LEDs) dot-matrix preview matching the white/silver LED hardware.
    pub fn matrix_grid(&self, frame: &[u8], dot_size: f32, gap: f32, dark_chassis: bool) -> El {
        let grid = super::led_grid::LedGrid::new(frame, dot_size, gap).finish();
        if !dark_chassis {
            return grid;
        }
        self.phone_rear_housing(182., 312., frame, dot_size, gap)
    }

    /// Orthographic device artwork. Screen/LED content is composited separately.
    fn phone_art(&self, name: &'static str, width: f32, height: f32) -> El {
        ConstrainedBox::new(Image::new(
            facloud_ui::assets::asset_cache::AssetSource::Bundled { path: name },
            CacheOption::BySize,
        ).contain().finish()).with_width(width).with_height(height).finish()
    }

    pub fn phone_rear_housing(&self, width: f32, height: f32, frame: &[u8], _dot_size: f32, _gap: f32) -> El {
        let scale = (width / 866.).min(height / 1817.);
        let w = 866. * scale;
        let h = 1817. * scale;
        // The real Neo display sits to the right of the camera lenses.
        Stack::new()
            .with_child(self.phone_art("neo_rear_housing.png", w, h))
            .with_positioned_child(
                self.matrix_grid(frame, 7. * scale, 2.5 * scale, false),
                OffsetPositioning::offset_from_parent(
                    pathfinder_geometry::vector::vec2f(562. * scale, 180. * scale),
                    ParentOffsetBounds::Unbounded, ParentAnchor::TopLeft, ChildAnchor::TopLeft,
                ),
            ).finish()
    }

    pub fn phone_front_housing(&self, width: f32, height: f32) -> El {
        let scale = (width / 866.).min(height / 1817.);
        let w = 866. * scale;
        let h = 1817. * scale;
        let source = if let Some((_, path)) = &self.screenshot {
            facloud_ui::assets::asset_cache::AssetSource::LocalFile { path: path.to_string_lossy().into_owned() }
        } else {
            facloud_ui::assets::asset_cache::AssetSource::Bundled { path: "stepx_home_screen.png" }
        };
        // Fit under the generated bezel, preserving screenshot aspect ratio.
        let screen = ConstrainedBox::new(Image::new(source, CacheOption::BySize)
            .cover().with_corner_radius(CornerRadius::with_all(Radius::Pixels(77. * scale)))
            .finish()).with_width(738. * scale).with_height(1660. * scale).finish();
        Stack::new()
            .with_child(ConstrainedBox::new(Rect::new().finish()).with_width(w).with_height(h).finish())
            .with_positioned_child(screen, OffsetPositioning::offset_from_parent(
                pathfinder_geometry::vector::vec2f(64. * scale, 80. * scale),
                ParentOffsetBounds::Unbounded, ParentAnchor::TopLeft, ChildAnchor::TopLeft,
            ))
            .with_positioned_child(self.phone_art("neo_front_frame.png", w, h), OffsetPositioning::offset_from_parent(
                pathfinder_geometry::vector::vec2f(0., 0.),
                ParentOffsetBounds::Unbounded, ParentAnchor::TopLeft, ChildAnchor::TopLeft,
            )).finish()
    }
}
