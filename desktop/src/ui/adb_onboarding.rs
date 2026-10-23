//! First-launch guidance; Android grants USB debugging on the phone itself.
use super::{icons::UiIcon, theme::*, widgets::*, *};

impl Companion {
    pub(super) fn adb_guide_modal(&self, step: u8) -> El {
        let p = self.p();
        let connected = self.selected.is_some();
        let (title, detail) = match step {
            0 => ("打开开发者选项", "在手机上打开「设置 → 关于手机 → 版本信息」，连续点击「版本号」7 次，直到提示已进入开发者模式。按手机提示完成本人验证。"),
            1 => ("开启 USB 调试", "返回手机设置，搜索「开发者选项」。进入后找到「USB 调试」并开启，在手机弹窗中确认。"),
            _ => ("连接并授权这台电脑", "用支持数据传输的 USB 线连接手机与电脑，保持手机解锁。在手机的「允许 USB 调试」弹窗中，核对并允许这台电脑。"),
        };
        let steps = ["开发者选项", "USB 调试", "连接并授权"].iter().enumerate().map(|(i, label)| {
            Container::new(self.line(format!("{}  {label}", i + 1), CAPTION,
                if i == step as usize { p.text } else { p.text3 }))
                .with_uniform_padding(S2)
                .with_background_color(if i == step as usize { p.surface } else { p.bg })
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(R_CONTROL)))
                .finish()
        }).collect();
        let mut body = vec![
            self.between(self.hstack(S2, vec![self.ui_icon(UiIcon::Smartphone, 22., p.text),
                self.heading("欢迎使用 StepX Companion")]),
                self.button("adb-guide-close", "关闭", Action::AdbGuide(None), Btn::Ghost)),
            self.text("先开启 ADB，让电脑连接你的手机。只需完成一次设置。", BODY, p.text2),
            self.hstack(S2, steps),
            self.divider(),
            self.heading(if connected { "手机已连接" } else { title }),
            self.text(if connected { "已验证手机的 ADB 连接。现在可以使用桌面功能，也可以继续安装 StepX伴侣，设置无线扫码和文件互传。" } else { detail }, BODY, p.text),
        ];
        if !connected {
            body.push(self.text(match step {
                0 => "找不到版本号？通常在「关于手机」的「软件信息」或「版本信息」里；也可以在设置中搜索。不同系统名称可能略有差异。",
                1 => "找不到开发者选项？确认上一步已提示开启成功，再在设置中搜索；部分系统把它放在「系统」或「更多设置」内。",
                _ => "仅在自己的可信电脑上勾选「一律允许使用这台计算机进行调试」。首次连接需要在手机上确认。",
            }, CAPTION, p.text2));
        }
        if step == 2 || connected {
            let (state, color) = if connected {
                ("已连接并通过授权".to_string(), p.success)
            } else if self.devices.iter().any(|d| d.state == "unauthorized") {
                ("已发现手机 · 请在手机上允许 USB 调试".into(), p.warning)
            } else if self.devices.iter().any(|d| d.state == "offline") {
                ("手机通道离线 · 请解锁手机并重新插拔数据线".into(), p.warning)
            } else if self.devices.iter().any(|d| d.state == "device") {
                ("已发现授权设备 · 等待识别；多台设备可稍后在连接页选择".into(), p.text2)
            } else if self.busy {
                ("正在检测设备…".into(), p.text2)
            } else {
                ("尚未发现手机 · 请检查 USB 调试和数据线".into(), p.text2)
            };
            body.push(self.hstack(S2, vec![self.dot(color), self.text(state, CAPTION, color)]));
            if !connected {
                body.push(self.text("没有授权弹窗：先解锁并重插数据线。仍找不到设备：换一根数据线或 USB 接口；Windows 请检查手机厂商的 USB 驱动。", CAPTION, p.text3));
                if self.error {
                    body.push(self.text(&self.status, CAPTION, p.danger));
                }
            }
        }
        body.push(self.divider());
        let mut actions = vec![];
        if connected {
            actions.push(self.button("adb-guide-done", "完成", Action::AdbGuide(None), Btn::Secondary));
            actions.push(self.button("adb-guide-companion", "继续安装伴侣", Action::AdbGuideContinue, Btn::Primary));
        } else {
            if step > 0 {
                actions.push(self.button("adb-guide-back", "上一步", Action::AdbGuide(Some(step - 1)), Btn::Secondary));
            }
            actions.push(if step < 2 {
                self.button("adb-guide-next", "已完成，下一步", Action::AdbGuide(Some(step + 1)), Btn::Primary)
            } else if self.busy {
                self.disabled_button("检测中…")
            } else {
                self.button("adb-guide-detect", "已允许，重新检测", Action::Refresh, Btn::Primary)
            });
        }
        body.push(self.between(
            self.link_action("adb-guide-skip", "稍后设置", Action::AdbGuide(None)),
            self.hstack(S2, actions),
        ));
        body.push(self.text("之后可在「连接设备 → 如何开启 ADB」重新查看。", CAPTION, p.text3));
        Container::new(Align::new(ConstrainedBox::new(self.panel(body)).with_max_width(600.).finish()).finish())
            .with_uniform_padding(S5)
            .with_background_color(rgba(0, 0, 0, 92))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.)))
            .finish()
    }
}

/// Keep the app visible under a modal without dispatching input to its controls.
pub(super) struct Inert(pub El);
impl Element for Inert {
    fn layout(&mut self, c: SizeConstraint, ctx: &mut LayoutContext, app: &AppContext) -> Vector2F {
        self.0.layout(c, ctx, app)
    }
    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) { self.0.after_layout(ctx, app); }
    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) { self.0.paint(origin, ctx, app); }
    fn size(&self) -> Option<Vector2F> { self.0.size() }
    fn origin(&self) -> Option<Point> { self.0.origin() }
    fn dispatch_event(&mut self, _: &DispatchedEvent, _: &mut EventContext, _: &AppContext) -> bool { false }
}
