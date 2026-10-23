//! Sample data for design review screenshots (`--features ui-capture -- --demo`).
//! Never compiled into normal builds.

use super::*;
use stepx_companion::bridge::{AmooTurn, SimSubscription};

impl Companion {
    pub(super) fn load_demo(&mut self) {
        let now = chrono::Local::now().timestamp_millis();
        let min = 60_000;
        self.selected = if std::env::var_os("STEPX_EMPTY").is_some() {
            None
        } else {
            Some(Device {
                transport: "STEPX_DEVICE_SERIAL".into(),
                serial: "STEPX_DEVICE_SERIAL".into(),
                model: "StepX PZ261A".into(),
                android: "16".into(),
                product: "pandora".into(),
            })
        };
        self.devices = vec![Transport {
            id: "STEPX_DEVICE_SERIAL".into(),
            state: "device".into(),
            model: "StepX_PZ261A".into(),
        }];
        self.summary = "电量  85%    ·    正在充电\n电池温度  30.8 °C\n\n手机存储  256G\n已用  96.4G    ·    可用  159.6G    ·    使用率  38%".into();
        self.capabilities = Some(Capabilities {
            send_sms: true,
            locked: false,
            default_subscription: 1,
            subscriptions: vec![
                SimSubscription {
                    id: 1,
                    slot: 0,
                    label: "中国移动".into(),
                },
                SimSubscription {
                    id: 2,
                    slot: 1,
                    label: "中国联通".into(),
                },
            ],
            protocol: 1,
        });
        self.selected_sim = Some(1);
        self.amoo_snapshot = Some(AmooSnapshot {
            turns: vec![
                AmooTurn {
                    speaker: "user".into(),
                    is_user: true,
                    text: "明天上午十点的会议改到几点了？".into(),
                },
                AmooTurn {
                    speaker: "amoo".into(),
                    is_user: false,
                    text: "根据今天 16:20 收到的短信，会议改到明天下午 2 点，地点不变，仍在 3 楼大会议室。".into(),
                },
                AmooTurn {
                    speaker: "user".into(),
                    is_user: true,
                    text: "帮我回复一下，说我会准时到。".into(),
                },
            ],
            sessions: Vec::new(),
            selected_title: String::new(),
            prepared: false,
        });
        let msg = |address: &str, body: &str, ago: i64, outgoing: bool| Message {
            address: address.into(),
            body: body.into(),
            date_ms: now - ago * min,
            outgoing,
            kind: if outgoing { 2 } else { 1 },
            read: true,
            subscription: None,
        };
        self.sms = vec![
            msg(
                "10086",
                "您本月套餐内流量已使用 80%，剩余 4.0GB。",
                30,
                false,
            ),
            msg(
                "138 0013 8000",
                "会议改到明天下午 2 点，地点不变。",
                125,
                false,
            ),
            msg("138 0013 8000", "收到，我会准时到。", 110, true),
            msg("138 0013 8000", "好的，记得带上周的数据。", 95, false),
            msg(
                "95588",
                "您尾号 1234 的账户于 10-20 14:02 支出 128.00 元。",
                1500,
                false,
            ),
            msg(
                "106980005678",
                "【顺丰速运】您的快件已到达小区驿站，取件码 5-2-1103。",
                2900,
                false,
            ),
        ];
        self.selected_thread = Some("138 0013 8000".into());
        self.fields[F_SMS_TO].set("138 0013 8000");
        let entry = |name: &str, bytes: u64, directory: bool| Entry {
            name: name.into(),
            path: format!("/sdcard/{name}"),
            bytes,
            directory,
            symlink: false,
        };
        self.files = vec![
            entry("Alarms", 0, true),
            entry("DCIM", 0, true),
            entry("Documents", 0, true),
            entry("Download", 0, true),
            entry("Music", 0, true),
            entry("Pictures", 0, true),
            entry("会议纪要-1020.pdf", 1_482_000, false),
            entry("IMG_20261019_183205.jpg", 4_215_000, false),
            entry("stepx-release.apk", 58_330_000, false),
            entry("录音 2026-10-18.m4a", 7_820_000, false),
        ];
        self.selected_file = Some(6);
        self.apps = [
            "com.tencent.mm",
            "com.eg.android.AlipayGphone",
            "com.stepos.ailauncher",
            "com.sankuai.meituan",
            "com.ss.android.ugc.aweme",
            "com.autonavi.minimap",
            "tv.danmaku.bili",
        ]
        .iter()
        .map(|s| stepx_companion::apps::AppInfo {
            package: (*s).into(),
            label: stepx_companion::device::friendly_app_name(s)
                .unwrap_or(s)
                .into(),
            enabled: true,
            launchable: true,
            ..stepx_companion::apps::AppInfo::default()
        })
        .collect();
        self.apps_loaded = true;
        self.apps_error = None;
        self.preferences.devices = vec![stepx_companion::pairing::RememberedDevice {
            serial: "PZ261A0001".into(),
            model: "StepX PZ261A".into(),
        }];
        self.preferences.selected_serial = Some("PZ261A0001".into());
        self.status = "已连接 StepX PZ261A".into();
        self.error = false;
        // Alternate state set: confirmations, transient panels.
        if std::env::var_os("STEPX_DEMO_ALT").is_some() {
            self.trash_confirm = true;
            self.fields[F_SMS_BODY].set("收到，我会准时到。");
            self.pending_sms_plan = Some(SmsPlan {
                request_id: "demo".into(),
                number: "138 0013 8000".into(),
                body: "收到，我会准时到。".into(),
                subscription: 1,
                subscription_label: "卡 1 · 中国移动".into(),
                parts: 1,
            });
            self.content = "会议通知\n时间：10 月 22 日 14:00\n地点：3 楼大会议室\n参会人：产品组、设计组\n议题：第四季度版本规划\n请提前准备上周数据\n联系人：王磊".into();
            self.output_page = Page::Amoo;
            self.amoo_poll_remaining = 2;
            self.fields[F_AMOO].set("总结一下这条通知");
        }
    }
}
