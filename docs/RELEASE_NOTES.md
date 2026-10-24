# StepX Companion 0.2.25

从 GitHub 上的 0.2.1 升级：日常对话、处理资料、生成图片与制作应用，集中到一个 Amoo 入口。仍面向 STEPX Neo PZ261A / Step AOS；普通 Android 可使用部分设备管理能力。

## 这次值得升级的变化

- **Amoo，一个输入框**：默认就是对话。主动选择“处理文件 / 生成图片 / 制作应用”才进入工作流程；开始前核对模型、资料和保存位置。工作记录折叠，实际交付的图片、文档和应用集中预览。
- **让手机模型真正完成工作**：图片、文字和 CSV 混合问答；生成带公式的 Excel、Word 报告和 PPT；继续对原文档精准修改；手机模型生成离线小应用，电脑在隔离浏览器中验收和修复。
- **舒服的简洁模式**：自适应小窗、进入会话定位最新消息、记住窗口大小。图片和文件可拖入，预览留在小窗；切换模式和会话保留草稿与附件。修复 Enter 发送、中文输入、复制及右键菜单。
- **存储扩容**：连接电脑时按照片、视频、文档自动迁移，支持保留最近文件。先选择范围与保存位置，再确认开启；校验原件后才释放手机空间，手机保留入口。离线与恢复状态明确显示。
- **日常体验**：统一设置、可自定义简洁对话快捷键、托盘、一次性 Star 查询；GitHub 更新提示、下载进度、校验及回滚。加载进度固定在底栏，减少页面跳动。
- **手机端与创作**：StepX伴侣的任务、文件、作品与设置重新整理；Studio 官方项目同步和本地创作能力整合；GUI Agent 保留独立的模型配置与暂停/继续/停止。

## 安装与兼容性

选择对应平台的完整压缩包，解压后运行；不要只复制 EXE。Windows x64、macOS Apple Silicon、macOS Intel、Linux x64 分别原生编译。发布包含 ADB、scrcpy、Python、Java 和 APK 身份/签名校验工具，无需自行安装 Android SDK。

- Windows 后台工具隐藏命令行窗口；安装与用户数据目录支持中文和空格。
- macOS 使用 `.app/Contents/Resources` 寻址，分别提供 Intel/ARM 包。没有 Apple Developer 公证，系统可能要求在“隐私与安全性”中允许打开；电脑截图需屏幕录制权限。
- 制作应用和 Studio 官网独立会话需要 Chrome、Edge 或 Chromium；macOS 的 Safari 不能代替这个验收引擎。文字对话、文件处理和图片生成不依赖该浏览器。
- Windows/macOS 的朗读使用系统语音；需要在系统设置添加中文语音。Linux 需要可用的 espeak-ng/系统语音服务。
- Google Play 安装不等于获得 Play Protect 认证。GUI Agent 使用单独配置的视觉模型；Amoo 手机任务沿用手机账号权益。

## 验证范围

Linux + PZ261A 已从客户端界面完成：混合附件对话、Excel/Word/三页 PPT、文档修订、1024×1024 图片、离线待办应用、停止任务，以及小窗长草稿与附件切换。待办应用第二轮通过 28 项独立检查。

本版本的 Actions 会在四个平台执行核心测试、更新器测试、真实隔离浏览器交互测试和完整打包烟测；打包测试清空 SDK/Python 环境并使用中文空格路径。Windows/macOS 尚无连接真机的完整 GUI 操作验收，不将 CI 编译结果等同于真机验证。

## 更新视频

[60 秒微信竖版（720p，约 1.7 MB）](https://github.com/xydzf520/stepx/releases/download/v0.2.25/StepX-Companion-0.2.25-update-wechat.mp4) · [1080p 高清版](https://github.com/xydzf520/stepx/releases/download/v0.2.25/StepX-Companion-0.2.25-update-1080p.mp4)

实际测试截图与流程动画，展示从 0.2.1 到本版的主要变化；含中文配音、字幕和下载二维码。

完整变化见 [CHANGELOG.md](https://github.com/xydzf520/stepx/blob/main/CHANGELOG.md)。
