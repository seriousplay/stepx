use super::super::{
    icons::UiIcon, is_wireless, theme::*, widgets::*, Action, Companion, Page, PairTab, ToolPage,
    F_CONNECT, F_PAIR_ADDR, F_PAIR_CODE,
};
use facloud_ui::{assets::asset_cache::AssetSource, elements::*};

fn guide_can_continue(
    step: u8,
    connected: bool,
    wireless: bool,
    companion: &stepx_companion::companion_app::CompanionStatus,
) -> bool {
    match step {
        0 => true,
        1 => connected,
        2 => connected && companion.installed && companion.pairing_permission_granted,
        3 => connected && companion.pairing_ready && companion.wireless_enabled == Some(true),
        4 => connected && wireless,
        _ => false,
    }
}

impl Companion {
    pub(in super::super) fn connect_actions(&self) -> Vec<El> {
        let mut actions = vec![self.button(
            "conn-guide",
            "连接引导",
            Action::ConnectGuide(Some(0)),
            Btn::Secondary,
        )];
        actions.push(self.button("conn-adb-help", "如何开启 ADB", Action::AdbGuide(Some(0)), Btn::Secondary));
        actions.push(if self.busy {
            self.disabled_button("检测中…")
        } else {
            self.button("conn-refresh", "重新扫描", Action::Refresh, Btn::Secondary)
        });
        if self.selected.is_some() {
            actions.push(self.button(
                "conn-disconnect",
                "断开当前设备",
                Action::Forget,
                Btn::Secondary,
            ));
        }
        actions
    }

    fn connection_guide(&self, step: u8) -> El {
        let p = self.p();
        let titles = [
            "开启 USB 调试",
            "连接并授权电脑",
            "安装 StepX伴侣",
            "开启无线调试",
            "手机扫码配对",
            "连接成功",
        ];
        let mut body = vec![self.between(
            self.heading(format!(
                "新手连接 · {} / 6 · {}",
                step + 1,
                titles[step as usize]
            )),
            self.button(
                "guide-hide",
                "收起引导",
                Action::ConnectGuide(None),
                Btn::Ghost,
            ),
        )];
        let connected = self.selected.is_some();
        let companion = &self.companion_status;
        let can_continue = guide_can_continue(
            step,
            connected,
            self.selected
                .as_ref()
                .is_some_and(|d| is_wireless(&d.transport)),
            companion,
        );
        match step {
            0 => {
                body.push(self.text("在手机「设置 → 关于手机 → 版本信息」连续点击「版本号」7 次，按系统提示完成验证。返回设置，搜索「开发者选项」，开启「USB 调试」。不同系统的入口名称可能略有不同。", BODY, p.text));
                body.push(self.text("首次安装伴侣需要已授权的 USB 连接。也可先用下方系统配对码建立无线连接使用现有功能，但安装与权限适配仍需 USB。未安装的伴侣无法直接扫码取得权限。", CAPTION, p.text2));
            }
            1 => {
                body.push(self.text("用支持数据传输的 USB 线连接手机和电脑，保持手机解锁。在「允许 USB 调试」弹窗核对这台电脑并允许。", BODY, p.text));
                let hint = if connected {
                    "已通过 ADB 验证当前手机，可继续安装。"
                } else if self.devices.iter().any(|d| d.state == "unauthorized") {
                    "已检测到手机，但尚未授权。请在手机确认调试弹窗；若没有弹窗，重插数据线或撤销调试授权后重试。"
                } else if self.devices.iter().any(|d| d.state == "offline") {
                    "手机通道离线。请解锁手机、重插数据线，再检测。"
                } else {
                    "尚未检测到可用连接。检查数据线是否支持传输、USB 调试是否开启；若显示多台设备，请在下面选择目标手机。"
                };
                body.push(self.text(hint, CAPTION, if connected { p.success } else { p.text2 }));
            }
            2 => {
                body.push(self.text("安装手机界面与当前会话的扫码系统服务，并配置文件收发、背屏所需权限。安装会核验签名、权限和手机的服务接收结果；熄屏任务另以真实连接回执确认。", BODY, p.text));
                body.push(self.text(
                    if companion.checked {
                        &companion.message
                    } else {
                        "正在等待伴侣状态检测。"
                    },
                    CAPTION,
                    p.text2,
                ));
                body.push(if !connected || self.busy {
                    self.disabled_button("安装／修复 StepX伴侣")
                } else {
                    self.button(
                        "guide-install",
                        if companion.installed {
                            "更新／修复 StepX伴侣"
                        } else {
                            "安装 StepX伴侣"
                        },
                        Action::CompanionInstall,
                        Btn::Primary,
                    )
                });
                body.push(self.text("安装时配置相机、通知、通话状态和电池优化豁免。手机重启后，扫码服务需通过 USB 连接后点击「更新／修复 StepX伴侣」恢复；当前未提供开机自启。", CAPTION, p.text3));
            }
            3 => {
                body.push(self.text(if companion.pairing_ready { "手机和电脑连接同一局域网。在手机 StepX伴侣「连接」页开启无线调试，确认 Android 的当前 Wi-Fi 信任提示。不要使用访客网络或阻止设备互访的网络。" } else { "手机和电脑连接同一局域网。在手机「设置 → 开发者选项 → 无线调试」手动开启，确认系统的 Wi-Fi 信任提示。伴侣扫码服务未通过核验时，可直接使用下方系统配对码完成连接。" }, BODY, p.text));
                if !companion.pairing_ready {
                    body.push(self.text(&companion.pairing_message, CAPTION, p.warning));
                    body.push(self.button(
                        "guide-system-pair",
                        "改用系统配对码",
                        Action::PairTab(PairTab::Code),
                        Btn::Secondary,
                    ));
                }
                body.push(self.text(
                    match companion.wireless_enabled {
                        Some(true) => "已检测到无线调试开启。请确认手机和电脑在同一网络后继续。",
                        Some(false) => "无线调试尚未开启；在手机确认后点击「检测伴侣状态」。",
                        None => "尚未读到无线调试状态，请先完成手机端操作并重新检测。",
                    },
                    CAPTION,
                    p.text2,
                ));
                body.push(self.wrap(vec![
                    if connected && companion.installed && !self.busy {
                        self.button(
                            "guide-open-phone",
                            "在手机打开 StepX伴侣",
                            Action::OpenApp(stepx_companion::companion_app::PACKAGE.into()),
                            Btn::Secondary,
                        )
                    } else {
                        self.disabled_button("在手机打开 StepX伴侣")
                    },
                    if connected && !self.busy {
                        self.button(
                            "guide-check",
                            "检测伴侣状态",
                            Action::CompanionInspect,
                            Btn::Secondary,
                        )
                    } else {
                        self.disabled_button("检测伴侣状态")
                    },
                ]));
            }
            4 => {
                body.push(self.text(if companion.pairing_ready { "在手机 StepX伴侣「连接」页点「扫码连接电脑」扫描下方二维码。也可以使用系统开发者选项中的「使用二维码配对设备」。" } else { "伴侣扫码服务尚未通过核验。请使用手机系统「开发者选项 → 无线调试 → 使用二维码配对设备」，或切换下方的 6 位配对码。" }, BODY, p.text));
                body.push(self.text("配对成功后，下方会出现真实无线通道。点击该通道的「连接」完成验证，再拔下 USB；不要扫描过期二维码。手机重启后如需再次扫码，请连接 USB 并更新／修复伴侣恢复服务。", CAPTION, p.text2));
            }
            _ => {
                let wireless = self
                    .selected
                    .as_ref()
                    .is_some_and(|d| is_wireless(&d.transport));
                body.push(self.text(if wireless { "手机已通过无线 ADB 验证。保持同一网络和无线调试开启，即可使用 Amoo、文件传输和背屏。" } else if connected { "手机已通过 USB 验证。无线连接尚未确认，请返回扫码步骤，连接真实无线通道后再拔线。" } else { "当前连接已断开，请重新连接后再继续。" }, BODY, p.text));
                body.push(self.wrap(vec![
                    self.button(
                        "guide-home",
                        "返回首页",
                        Action::Page(Page::Overview),
                        Btn::Primary,
                    ),
                    self.button(
                        "guide-tools",
                        "管理 StepX伴侣",
                        Action::OpenTool(Some(ToolPage::Companion)),
                        Btn::Secondary,
                    ),
                ]));
            }
        }
        if self.busy {
            let progress = self
                .google_progress
                .try_lock()
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| s.clone())
                .unwrap_or_else(|| self.status.clone());
            body.push(self.text(progress, CAPTION, p.accent));
        } else if self.error {
            body.push(self.text(&self.status, CAPTION, p.danger));
        }
        let mut navigation = vec![];
        if step > 0 {
            navigation.push(self.button(
                "guide-prev",
                "上一步",
                Action::ConnectGuide(Some(step - 1)),
                Btn::Secondary,
            ));
        }
        if step < 5 {
            navigation.push(if can_continue && !self.busy {
                self.button(
                    "guide-next",
                    if step == 0 {
                        "已开启，下一步"
                    } else {
                        "下一步"
                    },
                    Action::ConnectGuide(Some(step + 1)),
                    Btn::Primary,
                )
            } else {
                self.disabled_button("完成此步骤后继续")
            });
        }
        if !navigation.is_empty() {
            body.push(self.wrap(navigation));
        }
        self.panel(body)
    }

    fn connection_pairing_panel(&self) -> El {
        let p = self.p();
        let mut rows = vec![
            self.heading("无线配对"),
            self.text(
                "Android 11 及以上支持系统无线调试。手机与电脑需要能在同一局域网互相访问。",
                CAPTION,
                p.text2,
            ),
            self.segmented(
                "pair-mode",
                [
                    (PairTab::Qr, "扫码配对"),
                    (PairTab::Code, "6 位配对码"),
                    (PairTab::Manual, "IP 直连"),
                ]
                .into_iter()
                .map(|(tab, label)| (label.into(), self.pair_tab == tab, Action::PairTab(tab)))
                .collect(),
            ),
        ];
        match self.pair_tab {
            PairTab::Qr => {
                if let Some(qr) = self.qr.as_ref().filter(|q| !q.expired()) {
                    rows.push(
                        ConstrainedBox::new(
                            Image::new(
                                AssetSource::LocalFile {
                                    path: qr.path.to_string_lossy().into_owned(),
                                },
                                CacheOption::BySize,
                            )
                            .contain()
                            .finish(),
                        )
                        .with_width(210.)
                        .with_height(210.)
                        .finish(),
                    );
                    rows.push(self.text(
                        format!("等待手机扫描 · 还有 {} 秒", qr.remaining_secs()),
                        CAPTION,
                        p.text2,
                    ));
                    rows.push(self.text(format!("配对服务：{}", qr.service), MICRO, p.text3));
                } else {
                    rows.push(self.text(
                        "二维码尚未生成或已经过期，请点击生成二维码。",
                        BODY,
                        p.text2,
                    ));
                }
                rows.push(self.text(if self.companion_status.pairing_ready { "StepX伴侣 → 连接 → 扫码连接电脑，或使用 Android 系统二维码配对入口。" } else { "伴侣扫码服务尚未通过核验。请使用 Android 系统「无线调试 → 使用二维码配对设备」；没有此入口时请选择 6 位配对码。" }, CAPTION, p.text2));
                rows.push(self.button("qr-refresh", "生成新二维码", Action::Qr, Btn::Secondary));
            }
            PairTab::Code => {
                rows.push(self.text("手机「开发者选项 → 无线调试 → 使用配对码配对设备」，输入弹窗中的配对端口和 6 位数字。配对端口与连接端口不同。", CAPTION, p.text2));
                rows.push(self.input(
                    F_PAIR_ADDR,
                    "手机显示的 IP:配对端口（唯一设备可留空）",
                    None,
                ));
                rows.push(self.input(F_PAIR_CODE, "6 位配对码", None));
                rows.push(if self.busy {
                    self.disabled_button("配对")
                } else {
                    self.button("pair-code-btn", "配对", Action::PairCode, Btn::Primary)
                });
            }
            PairTab::Manual => {
                rows.push(self.text("仅用于已配对的手机。输入手机无线调试首页显示的 IP 和连接端口，无法用此入口跳过首次配对授权。", CAPTION, p.text2));
                rows.push(self.input(F_CONNECT, "IP:连接端口", None));
                rows.push(if self.busy {
                    self.disabled_button("连接")
                } else {
                    self.button("pair-ip-btn", "连接", Action::Connect, Btn::Primary)
                });
            }
        }
        rows.push(self.text("扫描无响应：确认同一网络、关闭访客隔离，检查电脑防火墙，刷新二维码；也可切换配对码填写手机显示的地址。", CAPTION, p.text3));
        self.panel(rows)
    }

    pub(in super::super) fn connect_page(&self) -> El {
        let p = self.p();
        let mut sections = vec![];
        if let Some(step) = self.connect_guide_step {
            sections.push(self.connection_guide(step));
        } else {
            sections.push(self.panel(vec![self.between(
                self.vstack(
                    S1,
                    vec![
                        self.heading("第一次连接？"),
                        self.text(
                            "从 USB 授权开始，完成伴侣安装与无线扫码连接。",
                            CAPTION,
                            p.text2,
                        ),
                    ],
                ),
                self.button(
                    "connect-guide-start",
                    "开始引导",
                    Action::ConnectGuide(Some(0)),
                    Btn::Primary,
                ),
            )]));
        }
        let mut channels = vec![self.heading(format!("检测到的设备 · {}", self.devices.len()))];
        if self.devices.is_empty() {
            channels.push(self.text(
                "尚未发现设备。连接 USB 并授权，或使用右侧无线配对。",
                BODY,
                p.text2,
            ));
        }
        for (i, device) in self.devices.iter().enumerate() {
            let selected = self
                .selected
                .as_ref()
                .is_some_and(|d| d.transport == device.id);
            let note = match device.state.as_str() {
                "device" if selected => "当前使用 · 已验证",
                "device" => "ADB 在线 · 可连接验证",
                "unauthorized" => "等待手机授权 USB 调试",
                "offline" => "通道离线，请解锁或重新连接",
                _ => "通道不可用",
            };
            channels.push(self.divider());
            channels.push(self.between(
                self.vstack(
                    S1,
                    vec![
                        self.hstack(
                            S2,
                            vec![
                                self.ui_icon(
                                    if is_wireless(&device.id) {
                                        UiIcon::Wifi
                                    } else {
                                        UiIcon::Smartphone
                                    },
                                    20.,
                                    p.text2,
                                ),
                                self.heading(if device.model.is_empty() {
                                    "Android 设备"
                                } else {
                                    &device.model
                                }),
                            ],
                        ),
                        self.text(&device.id, CAPTION, p.text3),
                        self.text(note, CAPTION, if selected { p.success } else { p.text2 }),
                    ],
                ),
                if selected {
                    self.pill_badge("已连接", p.success_text, p.success_soft)
                } else if device.state == "device" && !self.busy {
                    self.button(
                        format!("conn-select-{i}"),
                        "连接",
                        Action::Select(device.id.clone()),
                        Btn::Secondary,
                    )
                } else {
                    self.disabled_button("等待就绪")
                },
            ));
        }
        let mut remembered = vec![self.heading("已信任的手机")];
        if self.preferences.devices.is_empty() {
            remembered.push(self.text(
                "成功验证设备后，会在此保存型号和序列号。",
                CAPTION,
                p.text2,
            ));
        }
        for device in &self.preferences.devices {
            remembered.push(self.text(
                format!("{} · {}", device.model, device.serial),
                CAPTION,
                p.text2,
            ));
        }
        remembered.push(self.text(
            "相同网络内会尝试重连已配对的设备；手机关闭无线调试后需要重新开启。",
            CAPTION,
            p.text3,
        ));
        if self.error && self.connect_guide_step.is_none() {
            sections.push(self.panel(vec![self.text(&self.status, BODY, p.danger)]));
        }
        sections.push(
            Flex::row()
                .with_spacing(S4)
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(
                    Expanded::new(
                        1.,
                        self.vstack(S4, vec![self.panel(channels), self.panel(remembered)]),
                    )
                    .finish(),
                )
                .with_child(Expanded::new(1., self.connection_pairing_panel()).finish())
                .finish(),
        );
        self.vstack(S4, sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stepx_companion::companion_app::CompanionStatus;
    #[test]
    fn onboarding_requires_actual_install_permissions_and_wireless_connection() {
        let mut phone = CompanionStatus::default();
        assert!(guide_can_continue(0, false, false, &phone));
        assert!(!guide_can_continue(1, false, false, &phone));
        assert!(guide_can_continue(1, true, false, &phone));
        assert!(!guide_can_continue(2, true, false, &phone));
        phone.installed = true;
        assert!(!guide_can_continue(2, true, false, &phone));
        phone.pairing_permission_granted = true;
        assert!(guide_can_continue(2, true, false, &phone));
        assert!(!guide_can_continue(2, false, false, &phone));
        assert!(!guide_can_continue(3, true, false, &phone));
        phone.wireless_enabled = Some(false);
        assert!(!guide_can_continue(3, true, false, &phone));
        phone.wireless_enabled = Some(true);
        assert!(!guide_can_continue(3, true, false, &phone));
        phone.pairing_ready = true;
        assert!(guide_can_continue(3, true, false, &phone));
        assert!(!guide_can_continue(4, true, false, &phone));
        assert!(!guide_can_continue(4, false, true, &phone));
        assert!(guide_can_continue(4, true, true, &phone));
        assert!(!guide_can_continue(5, true, true, &phone));
    }
}
