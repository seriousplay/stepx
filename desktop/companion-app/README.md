# StepX伴侣 · Android

StepX Companion 的手机端。首页连接电脑，任务页使用内置 GUI Agent 看屏执行，文件页收发并校验文件，我的页面管理模型、授权和 Amoo 后台协作。Studio 作品、背屏动画与重力游戏均在手机运行。

## 手机界面（0.1.19）

- **首页**：真实电脑连接状态、扫码、任务工作台、文件、Studio 和背屏入口。
- **任务**：直接输入目标、示例任务、逐步进度、执行/看屏计数，以及固定在底部的暂停、继续、停止。成功提交后返回手机界面，由 Agent 继续；提交失败保留输入。切页或 Activity 重建保留草稿。
- **结果**：严格区分已核验完成、模型报告但待核实、等待接管、失败与停止。当前内置“查看系统版本”示例有独立的 Settings 页面验收条件；自由目标不凭模型文本自动宣布成功。最近 20 条已读取的终态摘要用 Android Keystore 加密保存在本机，可清空。不保存截图、模型原文或密钥。
- **模型与应用范围**：OpenAI / Anthropic / Gemini 协议、模型、密钥，默认全部可启动应用。新安装应用在任务开始时自动刷新；高级设置可切换为自定义范围、搜索应用和调整 4–40 步上限。运行或暂停中的任务不允许修改配置。改变服务地址或协议须重新提供密钥，并由用户确认截图接收端。
- **悬浮控制**：应用外默认圆形浮钮，显示下一动作类别或当前观察/暂停状态；点击展开、拖动移动，展开后可暂停、继续、停止。伴侣任务页使用固定底部栏，退出页面恢复浮钮。展示层继续向执行器提供真实遮挡边界，不能穿过控件执行点击。
- **文件**：Android 系统选择器授予单项文件访问；单文件最多 256 MB、每次最多 10 个，传输完整性校验、打开/保存/移除缓存。等待电脑确认不显示为已接收。
- **扫码与作品**：相机只在主动扫描时启动；相册识别与扫码走同一安全配对逻辑。作品库区分离线作品与官网链接，回到作品库会重新读取内容。官网使用原有登录会话，不拦截密码、不跳过证书校验。

## 构建与 GUI Agent 集成

`bash scripts/build-companion-app.sh` 编译公开的伴侣源码和离线策略测试，依赖 Android SDK 36 / 本机 JDK，不用 Gradle。签名密钥位于仓库外 `~/.config/stepx-companion/signing/`；保留签名才能无损更新。

内置 Neo Agent Touch 的构建方式见 [适配层说明](../neo-agent-adapter/README.md)。上游当前无再分发许可，因此组合 APK 仅本机验证，**不能直接放入公共 Release**。普通公开 APK 不包含 Neo 引擎；升级不能用普通 APK 覆盖已有内置版本。Neo 使用独立视觉模型账号，不默认复用 Amoo Token 权益。

手机页面、desktop 和端侧 MCP 都通过 `GuiDelegation → NeoAgent → CompanionBridgeRuntime` 使用同一执行器和任务归属；模型不能通过 MCP 修改密钥或应用范围。普通 Amoo 对话自动委派仍待官方接入验收，接口可调用不等于正常对话已打通。

## Android lifecycle and permissions

Package: `com.stepx.companion.app`; launcher label: **StepX伴侣**. The application now includes networking, pairing, file exchange, Studio, optional GUI execution and Amoo coordination; it is no longer only the original rear-display runtime.

GUI execution needs the explicitly enabled accessibility service, an image-capable model service, and approved applications. Chinese input uses the optional Agent IME lease and restores the previous IME after execution. Core GUI actions do **not** require root or a platform UID. Camera is for QR scanning, notifications explain active work, and phone state protects rear-display operation during calls. Scoped file selection does not request full storage access.

StepOS backscreen, secure pairing and CloudWake use separate fixed, signature-checked firmware permission groups in the desktop installer. An APK update can require re-adaptation; configuration must be read back rather than inferred from installation success. RootBridge is a bounded installation backend, not an arbitrary model shell. No boot receiver automatically starts GUI tasks. Public providers enforce DUMP or the StepOS MCP permission plus task ownership; the embedded Neo provider checks exact UID/signature identity.

The app reads/writes the observed `android.hardware.display.IStepOSDisplayManager` Binder interface. It checks its descriptor and reports failures rather than changing system security policy. It keeps an original display mode, native type, brightness mode/value and auto-rotation baseline in private preferences, then verifies restoration. The pending baseline is recovered on a later status/start/stop after a process crash; Android does not guarantee `onDestroy` after a force kill. It is not a system-integrated crash watchdog.

Nothing starts at boot. Standby is only armed by the user; it holds no wake lock. This device's accelerometer/gravity/gyro are **non-wakeup** sensors, so triple knocks cannot guarantee waking a deeply suspended processor. Pick the phone up first if necessary. During an active game or multi-frame/dynamic animation, a 60-second partial wake lock is renewed only while posture, call, power and thermal checks allow output. It is released on pause/stop. Real games continue until a stable flip to the front or an explicit stop; each individual CPU lease remains bounded. A static image needs no app wake lock. Animation duration 0 means continue until explicitly stopped (subject to system policies).

## Command protocol (version 1)

Use a fresh bounded request identifier for every command:

```
adb -s SERIAL shell am start-foreground-service \
  -n com.stepx.companion.app/.CompanionService \
  --es op status --es request_id REQUEST_ID
```

Supported operations:

* `status`: creates the external files directory, recovers any pending baseline, writes status, and stops itself if otherwise idle.
* `standby`: arms triple-knock entry. If an animation is playing, it stays visible.
* `game`: starts a real sensor-controlled game and enables standby. The current applied animation resumes when the game exits.
* `stop`: disables standby/output and restores original phone settings.
* `apply --es config_name incoming.json`: reads only the fixed app-owned `files/incoming.json`, validates it before applying, and stores the accepted configuration internally.
* `test --ez test_mode true`: explicitly synthetic alignment through five levels while sending frames to the real display. This does not prove physical knock/tilt correctness. It stops/restores after the bounded test and reports `lastTest` separately from real sensor counters.

The app-owned paths are:

```
/sdcard/Android/data/com.stepx.companion.app/files/status.json
/sdcard/Android/data/com.stepx.companion.app/files/incoming.json
```

`status.json` is atomically replaced no more often than every 250 ms. Check `requestId` equality and `commandReady`, then `lastError`. It includes service instance creation time, start count/id and the actual foreground service type for lifecycle diagnosis, plus mode, phase, pausedReason, jobId, appliedTitle, settingsRestored/restorePending, live sensor counts, actual tap trigger count, raw/calibrated tilt, game bar/slot positions and equal widths, score and submitted frame count. `outputAccepted` means Binder submission; `outputVerified` remains false because the app cannot independently read the protected HAL dump. Desktop must verify actual HAL power/frame ownership when claiming real output.

Frame configuration, maximum 256 KiB, 1–120 frames, 391 integer bytes per frame, all 12 absent corner slots zero:

```json
{"requestId":"REQUEST_ID","title":"旋转太空人","kind":"frames","fps":6,"brightness":5,"durationSecs":0,"frames":[[0]]}
```

The `[0]` above is schematic: every actual frame must contain exactly 391 values. Provide full-range pixels; brightness is applied once by the phone driver. `fps` is 1–12, `brightness` 1–8, and `durationSecs` 0–3600. `requestId` in the config, if supplied, must match the command. Only a regular non-symlink file at the fixed path is accepted.

Dynamic presets use `kind: "preset"`, `key: "clock" | "battery" | "countdown"` instead of frames. Phone time and battery are read locally; countdown requires a positive duration and displays minutes/seconds above 99 seconds. Other unsupported dynamic presets are rejected rather than silently frozen.

## Game and validation

Both falling bar and bottom target slot progress through widths 5, 4, 3, 2, 1. The first two levels allow one column of landing tolerance and snap to the slot; later levels require exact alignment. Positive Android gravity X maps to increasing back-view LED columns, with hypot(Y,Z) normalization; the firmware preserves column order when rotation is locked. Gravity is remapped for the back view, the game's output is locked to natural portrait, a three-second neutral calibration precedes play, and movement uses smoothing, a dead zone and column hysteresis. Each drop takes 12, 11, 10, 9 and 8 seconds respectively; later rounds repeat these speeds. Completing width 1 starts another round. Misses restart at width 5 and preserve cumulative score. The actual backscreen shows the total in four 3×5 digits at the top (K/M abbreviation for large values), with the playfield below. Best and last-session scores are kept locally. During play knocks are ignored; turning to the front for 600 ms ends the session and restores the applied animation/standby. A game-rotation-vector normal detects upright 180° turns; its reference follows gradual heading changes within 45° with a 1.5-second low-pass filter, while fast flips freeze that reference. A rapid whole-body turn is physically indistinguishable from turning only the phone using these sensors. front-up gravity above 5.5 m/s² is a fallback. Exit arms only after calibration and a back-facing/vertical pose (gravity Z below 2), so a desktop-started game initially lying front-up waits for the user.

Knock recognition uses separated Z-axis impulses relative to gravity, lateral/gyro rejection, a 4.5 m/s² minimum normal impulse, 120 ms debounce, 650 ms maximum inter-knock gap, 1600 ms total window and post-trigger cooldown. Front-up gravity above 7.5 m/s² prevents entry and shows a posture hint; held angled poses are accepted. Status includes the last ten bounded impulse candidates and rejection reasons. These tuning values do not imply universal recognition accuracy. Physically test bare phone/case, deliberate triple knocks, single/double knocks, placing on a table, walking and opposite tilt directions.

`GameCoreTest` covers knock counting/debounce/rejection, synthetic five-level completion, continuous real-input multi-round progression beyond five minutes, score LED rendering, 12-to-8-second drop timing, miss recovery, direction mapping, upright rotation/gravity flip debounce and 379 LED masking. Real sensor streaming, real output, restoration, foreground-service behavior, unplugged operation and physical input must be validated separately on the paired phone.

视觉规范与生成设计稿见 [Android 设计说明](../../docs/design/android/README.md)，以 desktop 当前主题实现为准。
