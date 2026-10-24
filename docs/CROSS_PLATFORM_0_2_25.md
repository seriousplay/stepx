# 0.2.25 跨平台发布验证

范围：从 GitHub v0.2.1 到 0.2.25 的新增 Amoo 工作流、Office 处理、本机浏览器验收、图片缓存、更新器和系统路径。源码审查与平台 CI 配合；不把编译成功等同于手机真机验收。

## 已修复

| 位置 | 问题 | 修复 |
| --- | --- | --- |
| Windows Amoo | 模型权限记录、Token 用量及图片缓存依赖 HOME | 使用 Windows 用户数据目录；各账号继续隔离 |
| 文件处理 | 寻找系统 Python，且脚本通过命令行传入可能超出 Windows 长度限制 | 使用随包 Python；临时脚本文件以 UTF-8 执行 |
| Windows 文件 / 链接 | 部分入口仅有 macOS / Linux 分支 | 补齐 Windows 系统打开方式 |
| macOS Amoo | 菜单和响应式布局访问仅在 Windows / Linux 存在的窗口接口 | 使用通用原生内容尺寸接口 |
| macOS 测试路径 | /var 与 /private/var 指向相同临时目录却比较失败 | 按规范化路径验证 |
| 本机浏览器 | 新安装环境冷启动超过短超时 | 独立延长首次启动等待，保留后续操作超时与实际结果断言 |
| 旧设备工具 | 开发机绝对路径 | 使用安装资源目录、用户目录或已配置环境 |
| 电脑截图 | 不支持 / 失败时仍给出示例结果 | 按系统截屏；失败真实报错，传输成功才返回结果 |

## 原生 CI 与安装包

[本轮构建记录](https://github.com/xydzf520/stepx/actions/runs/40103974265)。四种目标均通过，使用各自原生 runner，执行核心及契约测试、更新器、运行环境、Chrome / Edge 真实交互检查，然后编译正式包。

| 平台 | 原生构建 | 更新器 / 浏览器 | 便携包运行环境 |
| --- | --- | --- | --- |
| Windows x64 | 通过 | 通过 | 通过，PE GUI 子系统 |
| macOS Apple Silicon | 通过 | 通过 | 通过，签名检查 |
| macOS Intel | 通过 | 通过 | 通过，签名检查 |
| Linux x64 | 通过 | 通过 | 通过，本机下载后复核 |

包内 ADB、scrcpy、Python、Java 和 APK 校验工具同时检查；清空 SDK / Python 环境，并复制到带中文和空格的路径执行。APK 验证包含正常签名、篡改内容、错误包名的对照。Windows 额外校验 PE GUI 子系统；macOS 验证 .app 资源寻址与 ad-hoc 签名。每个归档附 SHA-256。

## 明确的适用范围

- Windows x64：CI 为 Windows Server 2022；没有 Windows 10 / 11 连接 PZ261A 的手工 GUI 回归。本项目没有商业代码签名，系统可能显示 SmartScreen 提示。
- macOS ARM / Intel：CI 为 macOS 15，未作旧系统或手机连接的手工 GUI 回归。应用为 ad-hoc 签名，没有 Apple Developer 公证。首次启动需按系统正常授权流程操作。电脑截图需要屏幕录制权限。简洁窗口置顶按钮暂不在 Mac 提供。
- Linux x64：使用 Ubuntu 24.04 构建，二进制需要 glibc 2.39 或更新版本；Ubuntu 22.04 等旧环境请从源码构建。已在当前 Linux 桌面与 PZ261A 做过界面场景验收。
- 应用生成的本机验收、Studio 独立官网会话需要 Chrome / Edge / Chromium。Safari 不能替代这一引擎；普通 Amoo 对话、Office 处理和图片任务不依赖它。
- Windows / Mac 中文朗读取决于安装的系统中文语音。部分电脑仍需手机厂家 USB 驱动；随包 ADB 无法代替 USB 驱动或手机授权。
- Google Play 安装、手机 ROOT 权限和 Play Protect 认证是不同条件；本次桌面打包不改变手机认证状态。

Linux 真机对话、附件、图片、文件和应用场景的具体记录见 [Amoo 场景验收](AMOO_SCENARIO_VERIFICATION.md)。

## 发布后真实更新复核

2026-10-24，在本机使用客户端同一更新器读取 GitHub latest，确认四种平台均能匹配 0.2.25 安装包和 SHA-256。实际下载 Linux 发布包、解压和复核后，将已安装 0.2.24 更新为 0.2.25。旧窗口正常退出，新版原生窗口启动并返回版本与 nonce 匹配的回执，安装结果为 installed；完整旧目录保留用于回滚。这是更新器与原生启动链路测试，不是点击更新按钮的 UI 自动化测试。
