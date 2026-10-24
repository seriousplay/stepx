# Neo Agent Touch：手机权限与系统适配

> 2026-10-23 只读评估。当前集成方案使用单 APK `com.stepx.companion.app`，通过本机组合构建将 GUI Agent 合入 StepX伴侣；普通公开 APK 不包含上游执行引擎。早期独立适配包 `com.stepx.companion.neo` 的验收记录仅作历史依据；以 PZ261A、Android 16 / API 36、arm64 真机的当时状态为依据。本文不包含设备标识、账号、密钥或其他应用的数据。最初评估未安装 Neo、未改变手机权限、未切换界面、未重启，也没有执行 Android 权限注入。后续按用户授权完成安装适配，结果见“已实现与安装验证”；两阶段的状态分别记录。

## 结论

Neo 的核心 GUI Agent 调度与执行循环可以全部在手机执行：手机直接请求自己配置的视觉模型 API，通过 Android 无障碍服务观察、点击、滑动和输入。模型推理仍依赖该服务与网络，并非手机离线运行模型权重，也不默认复用 Amoo 账号额度。电脑只负责安装、配置与远程控制，不需要在任务执行期间保持连接。**核心 GUI 操作不依赖 root；需要的是可用的无障碍服务、正确的模型配置，以及任务与后台生命周期的适配。**

项目已有的 RootBridge 可用于经过校验的系统适配，但它不是授予 App 的通用 root。Google Play 也不是一个可复制的“权限等级”：安装权限、平台签名权限、特权应用身份、SELinux 域和 Linux UID 是不同机制。本机 Play 商店拥有 `INSTALL_PACKAGES`，但仍使用独立应用 UID，并非 UID 0；复制 Google 的全部权限不能直接提高 Neo 的看屏或规划能力。

当前实现分为两个模块并放在同一个 App 内：**Neo 使用 Android 原生 GUI 能力；StepX伴侣负责安装、受限调用、配置、状态和按功能划分的系统适配。** 不将任意 shell、任意权限名或 root 执行入口交给模型。

## 真机只读结果

| 检查项 | 本次读回 | 含义 |
| --- | --- | --- |
| Android / ABI | Android 16、API 36、arm64-v8a | 满足上游 minSdk 30；截图、手势和输入法 API 可用 |
| ADB 身份 | shell，UID 2000 | ADB 本身不是 root；未找到可执行的 `su` |
| SELinux | Enforcing | 未关闭或修改系统策略 |
| 现有 RootBridge | UID/EUID 0，`zygote` 域；当前命名空间不直接显示 Android `/system` | 已有容器内高权限后端可运行只读探针；不能解释为 Neo 拥有 Android root |
| RootBridge 完整性 | launcher 与仓库摘要一致；启动属性空闲 | 未发现当前安装运输链被其他操作占用 |
| 现有 Google 适配记录 | success、已分离，启动与 system_server 身份仍匹配 | 证明既有适配的记录与当前运行期相符；不是本次重新注入验证 |
| StepX伴侣 | 已安装于 `/data/app`，独立应用 UID；无障碍已启用 | 当前仍是应用沙箱内的服务 |
| 伴侣常规权限 | 网络、相机、通知、电话状态、唤醒锁和前台服务已授予 | 这些不能替代签名权限或无障碍能力 |
| 伴侣专用权限 | 背屏、CloudWake、MANAGE_DEBUGGING、WRITE_SECURE_SETTINGS 均未授予 | 不能显示“所有系统权限已就绪”；与 GUI 的无障碍状态分别报告 |
| 后台状态 | 伴侣在 Doze 白名单；不在所读 OEM 冻结白名单 | Doze 豁免不等于厂商冻结豁免 |
| Neo | 上游包和适配包均未安装；适配输入法未启用 | 不能宣称 Neo 已经通过真机操作验收 |

读取只使用被授权的 StepX 连接。ADB 原始输出中的设备标识和完整安装路径不应复制到公开文档。只读证据保存在开发机诊断目录，未放入仓库。

## 按能力配置权限

| 能力 | 上游实现 / 实际要求 | 安装与产品处理 | 是否需要 root |
| --- | --- | --- | --- |
| 看屏、读取控件 | `ControlService` 无障碍服务；XML 声明 `canRetrieveWindowContent`、`canTakeScreenshot`、窗口与控件 ID | 引导开启指定服务，并读取真实连接状态。`BIND_ACCESSIBILITY_SERVICE` 是系统绑定服务的保护权限，不是给 App 执行 `pm grant` 的普通权限 | 否 |
| 点击、滑动、返回 | `dispatchGesture`、控件动作、`performGlobalAction`；声明 `canPerformGestures` | 保留实际动作回执、截图刷新和完成校验；不申请 `INJECT_EVENTS` | 否 |
| 中文及长文本输入 | 优先无障碍 `ACTION_SET_TEXT`；可选专用 IME 对已授权输入框输入 | 可选引导启用 Agent 输入法；任务临时切换，暂停、停止、断连时恢复原输入法；用户手动改回后不能抢回 | 否 |
| 暂停、停止和接管胶囊 | `TYPE_ACCESSIBILITY_OVERLAY`、`RunOwnership` 代次机制 | 保留可见控制胶囊；暂停使旧模型响应失效，停止释放输入法租约与请求 | 否；不必申请普通悬浮窗权限 |
| 调用模型 | `INTERNET`；HTTPS API，模型名、协议、密钥 | 模型配置在手机本地保存，密钥存入 `Vault`；先做模型与图片输入验证。断开电脑仍由手机发请求 | 否 |
| 语音目标 | 发布版使用当前系统输入法转写 | 上游 `build.sh release` 会删除 `RECORD_AUDIO`，不能根据源 Manifest 将麦克风标成安装必需项；若以后恢复独立录音再按需请求 | 否 |
| 启动目标 App | Launcher intent 查询及显式启动 | `<queries>` 已声明 launcher；不需要 `QUERY_ALL_PACKAGES` 才能列出可启动应用。仍保留用户选择应用与发送画面的范围授权 | 否 |
| App 自更新 | 上游 `REQUEST_INSTALL_PACKAGES`、受保护 APK provider、系统安装器 | 仍需 Android 的安装来源许可与系统确认；校验包名、版本、摘要与签名。适配包不能沿用上游原包的升级下载源 | 否 |
| 电脑安装、更新伴侣内置 GUI Agent | 已授权 ADB 或伴侣的固定安装流程 | 校验适配包、签名和版本，复用已有安装基础设施；不能把 `INSTALL_PACKAGES` 当成看屏所需权限 | 仅现有 OEM 安装适配可能使用受限后端 |
| 手机长期运行 | 无障碍服务受系统绑定；可增加用户可见的任务通知 | 上游未声明独立前台任务服务/唤醒锁，不能只加权限而没有生命周期实现。必要时由伴侣管理有期限的任务保持与 OEM 单包冻结豁免 | OEM 适配视固件而定；核心执行不需要 |
| Amoo 委派任务 | 上游 `DeviceMcp` 是进程内 JSON-RPC，不是外部监听 MCP 服务 | 需要伴侣到内置 Neo 的固定调用适配；开放开始、状态、暂停、继续、停止，绑定调用者、任务 ID 和本轮授权范围 | 否 |
| 背屏、扫码、CloudWake | 是伴侣的业务，不是 Neo GUI 核心要求 | 原有专用权限留在伴侣；有证据表明 Neo 直接调用对应能力时再增加独立权限范围 | 不给 Neo 复制这些权限 |

Android 无障碍截图会拒绝部分受保护窗口；锁屏也不能靠增大权限当作已解锁。上游主动检查系统锁屏、密码输入框与画面范围，这些应继续显示为“等待用户接管”，不能伪报执行成功。[Android 无障碍 API](https://developer.android.com/reference/android/accessibilityservice/AccessibilityService)

输入法切换 API 在已启用的输入法之间工作，适合任务租约，不是自动强制永久更换默认键盘。[SoftKeyboardController](https://developer.android.com/reference/android/accessibilityservice/AccessibilityService.SoftKeyboardController)

## 现有安装链可以复用什么

现有 `scripts/companion-permissions.py`、`desktop/companion-permissions/CompanionGrant.java` 已具备：

- 固定包名、签名摘要、独立 UID、固件与原生文件摘要核验。
- 背屏、连接、CloudWake 三组固定权限；没有从模型接收任意权限名的入口。
- 修改前保存基线，失败保留恢复记录；撤销时检查是否被其他操作者改变，避免覆盖用户的新设置。
- OEM 单包冻结豁免与 Doze 分开处理；只恢复自己加入的条目，不清空全局列表。
- 有期限的 worker、启动属性恢复、launcher 摘要核对、system_server 身份核对、Google 权限基线核对。

**GUI Agent 的组件初始化与伴侣的系统适配分别记录。** Python 和 Java 的原有系统权限适配仍固定为 `com.stepx.companion.app`。GUI 安装器只启用内置 Neo 无障碍服务与用户选择的输入法，不再单独改动同包 Doze 或 OEM 冻结白名单；这些属于伴侣已有安装流程。不能把固定包名改为任意输入，也不复制 Google 权限集合。

现有受限 Android worker 会修改临时启动属性并切换执行上下文。本次只验证其外围运输链与已有状态，未运行该 worker。若未来真的需要新的系统权限组，必须独立核对固件、单包、签名、权限用途和回读结果；不能用容器 UID 0 的只读结果替代验收。

将 APK 放入 `priv-app` 本身也不会自动获得所有签名权限；Android 的特权权限还受系统权限清单及签名等规则约束。本方案不需要改系统分区、刷机、改 SELinux 或改 AI 安全中心。[Android 特权权限说明](https://source.android.com/docs/core/permissions/perms-allowlist)

## 单 APK 安装与恢复

1. **更新 StepX伴侣。** 用户选择本地完整 APK，包名必须为 `com.stepx.companion.app`，且与已安装伴侣同签名；首次安装用随安装包附带的伴侣 APK 作签名基线。先备份已有 APK，再使用保留数据的更新安装；不卸载降级。
2. **核验内置组件。** 安装前及安装后回读 Manifest，必须包含 `com.stepx.companion.neo.ControlService`、`com.stepx.companion.neo.AgentInputMethodService`、`com.stepx.companion.neo.CompanionBridgeProvider` 与 `.NeoSettingsActivity`。两个服务分别声明 Android 无障碍与输入法绑定权限；provider authority 必须为 `com.stepx.companion.app.neo.control`，不会与旧独立包冲突。无需在电脑安装 Android SDK；发布运行时使用 Android 官方 Java 二进制 XML 解析器完成检查。
3. **只初始化 GUI 组件。** 保存无障碍与输入法旧值，新增内置 Neo 无障碍服务；增强中文输入由用户选择，安装不会改变默认键盘。手机配置页保存模型 URL、协议、密钥以及允许操作的应用。电脑离线后仍由手机模型请求与执行循环完成任务。
4. **独立恢复记录。** 单包使用 `neo-agent-embedded` 私有状态目录和 `embedded-neo-v1` 范围标识；不读取或套用早期独立 Neo 安装记录。记录绑定设备身份、固件、伴侣签名与 UID，重复更新保留首次配置基线。
5. **只撤销本模块新增项。** 先确认没有活跃任务并关闭 Neo 路由，再移除本机添加的 Neo 无障碍/输入法组件。保留伴侣其他无障碍服务、文件服务、Doze/OEM 后台豁免、应用数据，以及用户后来添加的设置。正在使用临时 IME 时先恢复仍可用的原键盘；找不到原键盘则提示手动选择，不造成无键盘状态。
6. **失败保留可恢复状态。** 安装或激活失败会尝试恢复本次组件配置；如果 APK 已更新，保留新 APK 与数据，不用卸载重装宣称无损回滚。恢复失败保存记录并显示未完成原因。旧独立适配包由迁移验收处理，本安装器不自动卸载它。

如果最小权限已能完成任务，性能、失败重试和完成核验应在 Agent 调度层优化；提高权限不能修复模型判断错误、旧截图坐标或假完成状态。

## 验收清单

- APK 签名、适配包名、无障碍实际连接与模型图像输入全部有回读。
- 从手机正常对话委派多步骤任务，实际到达目标页面，返回的完成结论有最终画面/原生状态依据。
- 人工接管立即停止派发；旧请求返回不会重新控制手机；继续时重新观察。
- 中文输入使用真实输入框，结束恢复输入法，不更改用户剪贴板。
- 关闭电脑、拔掉 USB 后仍能由手机发起与结束任务；重启后的授权/状态未验证时明确显示待检查。
- 锁屏、受保护窗口、网络失败、模型无权限、服务被禁用均保留任务状态并给出可操作提示。
- 更新、安装失败、任务异常和撤销均不破坏 Google 服务、其他无障碍服务、全局省电策略或 AI 安全设置。

## 已实现与验证范围

桌面小工具已加入 **内置 GUI Agent**：选择完整伴侣 APK、可选增强中文输入、检测状态、打开手机配置、启用任务路由和撤销组件配置。仓库只包含集成与安装代码；上游无明确许可证的源码和本地适配 APK 不随公共仓库分发。

- `desktop/src/neo_agent.rs` 对接伴侣 `neo_enable`、`neo_status`、`neo_disable`，配置按钮打开同 App 的 `.NeoSettingsActivity`。激活失败会请求恢复组件配置。
- 普通“安装／修复伴侣”优先使用安装目录中的 `desktop/assets/stepx-companion.apk`，缺失才采用编译内置公开包；客户端会另外从编译内置 APK 创建独立签名基线，外置候选必须同证书；首次安装也不能仅凭“有签名”通过。已有手机同签名与设备核验继续保留。共同安装预检会读取当前 APK：若手机已有内置引擎而候选包不完整，拒绝覆盖并引导选择同签名完整包。检测发生在安装授权写入之前，保留已有引擎和数据。
- `scripts/neo-agent-install.py` 验证包名、签名、必需组件、独立 UID 和所选设备身份。支持 Android build-tools 旧版与 37 的签名输出格式；Windows/macOS/Linux 发布版使用便携 Java 解析组件，无硬编码 SDK 路径。
- 无障碍“已启用”与执行器“实际连接”分别报告；后者只采用伴侣返回的真实运行状态，避免从服务名称文本推测。电池豁免只读显示，不在 GUI 模块安装/撤销时改动。
- 21 个无需设备写入的测试覆盖包与证书拒绝、必需组件校验、新旧恢复记录隔离、原输入法恢复、伴侣其他服务和后台策略保留、重复更新基线及安装失败回滚。另有 14 项更新保护测试与 15 项 Rust 伴侣模块测试（含资源优先、快照固定和缺失回退检查）通过。Java 解析器已用 Android build-tools 36 编译，并核对现有 APK 元数据。开发者验证入口：`python3 scripts/test-neo-agent-install.py`。
- 早期独立适配版 `0.1.16-beta.1` 曾在指定真机通过同签名、独立 UID、无障碍、IME 与单包后台豁免回读；这不等于合并 APK 已通过同样验收。单 APK 已于 2026-10-24 在 PZ261A 真机完成保留数据更新；旧独立测试包移除，仅保留伴侣启动图标。桌面程序关闭时，由手机独立执行京东订单列表导航，以及设置中输入“电池”并显示搜索结果；从伴侣正常“新建手机任务”按钮发起的 Android 版本查看也到达关于手机页面，截图确认版本为 16；后者通过绑定到 finish 观察的屏幕校验，结束恢复原输入法。暂停与停止实测通过，Studio 官网登录与已有离线作品保留。USB 保持连接用于只读取证，未将其表述为拔线测试。普通 Amoo 对话仍只调用原厂开应用能力，未委派新执行器；官方测试技能安装接口尚未成功，不能将端侧 MCP 接口就绪表述为普通对话已打通。

手机操作脚本要求明确传入所选连接和物理设备身份，不自动选择 `adb devices` 的第一台设备。本文不把安装成功当作任务完成，也不把容器内 UID 0 当成 App 已获得 Android root。

## 源码依据

- 上游：[`neo-agent-touch/source`](https://github.com/JiayuStudio/stepx-neo-lab/tree/main/guides/neo-agent-touch/source) 的 `AndroidManifest.xml`、`res/xml/accessibility.xml`、`build.sh`、`ControlService.java`、`AgentImeLease.java`、`RunOwnership.java`、`Config.java`、`DeviceMcp.java`。
- 本项目：`scripts/companion-permissions.py`、`desktop/companion-permissions/CompanionGrant.java`、`desktop/src/companion_app.rs`、`handoff/google-play-stepx/tools/common.py`、`root_bridge.py`、`root_worker.py`。
- 只读检查：已选设备上的 `id`、系统属性、`getenforce`、包管理状态、无障碍/输入法启用状态、OEM 冻结白名单、Doze 白名单，以及现有 RootBridge 的最小身份与完整性探针。

## 2026-10-24：结合方式与 Android 交互复审（0.1.17）

现阶段应让伴侣成为唯一手机任务工作台：本机 UI、desktop 和 Amoo 端侧 MCP 共用 `GuiDelegation`，再由 `NeoAgent` 向同 APK 内的 `CompanionBridgeRuntime` 委派。Neo 负责视觉规划、动作、接管和输入法租约；伴侣负责明确目标、模型与应用授权、进度、结果、记录，以及文件/Studio/背屏入口。文件传输和作品运行继续走原生功能，不要求 GUI 模型绕到手机界面重复做一次。RootBridge 只处理固定的安装与系统适配，不向模型暴露通用 root 命令。

| 复审问题 | 本次处理 | 仍需注意 |
| --- | --- | --- |
| 任务、模型、权限、Studio 混在一页，背屏藏在设置中 | 首页、任务、文件、我的四个主入口；首页提供 Studio 与背屏快捷入口 | 更多背屏创作仍在 desktop 中 |
| 任务必须弹窗输入，切页难以续写 | 内联任务编辑器；切页、后台和 Activity 重建保留草稿；失败不清空，已接受任务才清空 | 系统清除应用数据不保留草稿 |
| 只显示截断的状态文本，看不到流程 | 当前目标、步骤、实际动作和看屏次数；模型报告、核验完成、接管、失败、停止分别呈现 | 模型完成陈述不是独立业务证明 |
| 暂停/继续/停止埋在长页面底部 | 运行和暂停期间固定控制栏；仅开放状态允许的动作，防重复提交 | Android 密码锁屏不能由此解除 |
| 结果被下一项任务覆盖 | 本机最近 20 项已读取的终态摘要，Keystore 加密，可查看和再次编辑或清空 | 不是常驻后台完整审计；未读取过的历史终态不会凭空恢复 |
| 示例完成没有客观条件 | “查看系统版本”绑定 Settings 包、可见 Android 版本标签与真实版本值，并核对总览或版本详情页的一组锚点；使用执行器 finish 时采集的观察证据 | 验收只对精确内置示例生效；自由目标仍可为待核实，不擅自放宽到所有任务 |
| 模型配置是大表单，应用范围不易查找 | 可搜索应用、显示选择数量、步数上限、未保存提醒、任务活跃时禁改 | 配置就绪不代表远端模型已经验证；首次实际任务仍可能被服务拒绝 |
| 更新后服务重连，权限页面停在旧提示 | 设置页前台定时回读；主页面按当前模块异步读取状态，离开停止轮询 | 固件授权可能在 APK 更新后需要重做，必须读取实际权限 |
| 扫码、Studio、文件按钮样式不同，系统边缘遮挡 | 共用线性图标与按钮样式，统一安全区域；扫码空态，Studio 返回刷新与 WebView 状态恢复 | 官网内部页面由官网控制，不伪造登录成功 |
| Studio 渲染进程退出后点击刷新会访问空 WebView | 检查 WebView 存活，失效时明确重新打开本地作品 | 作品仍不获得任意原生接口或网络权限 |
| README 仍声称没有网络/文件/安全设置权限 | 按实际组件与分组权限重写说明；MCP 版本号读取实际 APK 版本 | 公开 APK 与私有组合 APK 的能力边界仍需明确 |

### 权限评估

本次 UI 和任务整合 **没有增加 Android Manifest 权限**。核心任务需要网络、明确选择的应用、Neo 无障碍服务和可选的专用输入法；暂停/停止沿用原执行器的所有权和租约逻辑。扫码、背屏、CloudWake 的固定固件权限属于伴侣业务，不能推导出模型应拥有同等系统访问权。升级后继续使用签名、设备和固件校验过的现有安装器恢复所需权限，并保留原始权限基线。

### Amoo 与后续价值

- 手机本身可以发起任务，desktop 可查看和控制同一执行器，不依赖电脑来循环截图或调用模型。
- Amoo 的理想角色是提交目标、查询进度并给出结果；具体 GUI 操作不与原厂逐步控制器同时竞争。
- **普通 Amoo 对话自动路由到新 Agent 尚未通过。** 本次没有将页面“已就绪”、端侧 MCP 注册成功或直接手机任务成功当成普通对话已打通。继续接通需要官方工具发现/技能授权/模型选工具链路的证据，而不是提高 root 权限。
- Neo 模型账号独立配置；不宣称它消费手机 Amoo Token 权益。
- 上游尚无再分发许可，保留作者标识与私有组合构建；公开发布仍需取得授权或换用许可明确的执行器。


### 0.1.18：默认设置与浮钮

根据用户要求，应用范围默认全部。首次保存模型时一次确认截图接收端，应用列表不再要求逐个选择；已就绪旧配置第一次进入伴侣后迁移为全部。明确保存过的自定义范围保留，新安装的可启动应用在下一任务开始前刷新。全部模式仍绑定模型接收地址；更换地址不能沿用旧地址的截图授权。应用名单不是通配系统进程，也没有扩大 Manifest 权限、支付或密码处理能力。

默认悬浮控件由大条形改为 72 dp 圆形，黑白灰配色加绿色状态线，显示“观察、点击、滑动、输入、核验”或暂停。点击可展开，展开后显示真实进度及暂停/继续/停止；可拖动，位置持久保存。`CompanionControlBar` 仅替代展示，仍用原生控制回调，保留遮挡遮罩、触点/路径排除与自动避让。伴侣任务页使用自己的底部栏，5 秒宿主可见心跳抑制重复控件；离开或心跳失效恢复浮钮。

升级安装已保留原数据、Studio 本地作品、模型配置与输入法恢复记录。公共源码 APK 与用户本机私有合并 APK 继续分开，不把上游未许可再分发的引擎加入公共资源。

### 0.1.19：两端统一样式与页面验收

Android 颜色与圆角以 desktop 实际 `theme.rs` 为准，布局设计稿保存在 `docs/design/android/`。手机保留系统字体缩放和加粗偏好。全屏蓝色边缘动画停止重绘，运行反馈收拢到可拖动圆形浮钮；原生遮挡、窗口归属和暂停/停止检查不变。

真机发现版本任务可能进入“Android 版本”详情页而非“关于手机”总览，旧的三文字交集会将实际完成显示为待核实。精确内置目标增加两种页面的锚点组合，并要求可见的准确系统版本值；遮挡、错误窗口、搜索结果、缺少版本值和混杂的不完整锚点均不算完成。不中途改判旧任务，也不使用任务结束后的截图冒充结束时证据。


## 2026-10-24：Amoo 原厂 MCP 对话回归

- 手机 StepX伴侣 0.1.20 / code21，内置 Neo Agent Touch；desktop 0.2.7。
- 原厂 Amoo 普通 `chat.send` 对话中的 `ability exec stepx_gui_agent` 已真实经过本地 Binder 进入手机执行器（调用 UID 1000），不使用桌面解析动作协议。
- 验收目标为精确的“打开设置，查看 Android 版本”。Amoo 发起一次 start、六次 status；新手机任务经历 planning → running → completed，3 张截图、1 次界面动作。真实设置窗口的 Android 版本、精确系统版本值及相关锚点在模型结束时完成同任务核验；result.verified=true，result.phoneStopped=true。
- 此前 MCP 未附带 UI 入口的内置验收规则，导致同一目标仅 reported；现统一到 NeoAgent 委派入口，规则由伴侣生成，MCP 调用者仍不能提交自定义成功断言。其他目标不因此自动视为完成。
- **稳定自动发现仍未通过**：全新自然语言会话可能搜索不到已安装的本地工具并拒绝选择；明确直接调用的测试会话已通过。在已成功的会话内继续任务已通过，不代表所有新会话无需提示即可自动选中。
- Studio 私有技能源码更新为 0.6.0，提交 46e29fa；私有测试分发版本 0.9.0。官方安装接口仍返回 HTTP 400 / 非 JSON，ListSkills 仍为空，未将推送成功写成安装成功。未发布上架。
- 独立 Agent 使用伴侣内已配置的 Step Plan 视觉模型；Amoo 负责委派和读取结果，两者账号/额度不混同。
- 私有证据：diagnostics/amoo-followup-20261024/mcp-verified-followup/amoo-native-agent-chat.json、mcp-verified-followup.log。证据可能包含账号会话和屏幕数据，不公开上传。
