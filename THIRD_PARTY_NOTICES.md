# 第三方组件与素材

- **FaCloud / Warp 派生 UI**：`vendor/` 中保留独立构建所需的九个 crate。许可证见各 Cargo.toml 与 `licenses/`。FaCloud UI / UI core 使用 MIT；其余适用 AGPL-3.0-only 等原许可。保留 Denver Technologies / FaCloud 原版权说明。本快照将 workspace 继承清单展开为独立清单，并去除依赖私有工作区的开发依赖，未改变原许可。
- **Rust 依赖**：`desktop/Cargo.lock` 锁定来源与版本，各 crate 按原许可使用。
- **GELab-Zero**：GUI Agent 提示词格式参考其 MIT 许可项目；版本、原版权与许可见 `desktop/companion-app/third_party/gelab-zero/`。
- **ZXing**：扫码库遵循 Apache-2.0；许可与 JAR 位于 `desktop/companion-app/third_party/`。
- **scrcpy / ADB**：发布时从 Genymobile 官方发行包下载，经固定 SHA256 核验后随包分发。scrcpy 为 Apache-2.0；ADB 及其依赖沿用官方压缩包内的 NOTICE/LICENSE。源代码与对应发行见 https://github.com/Genymobile/scrcpy/releases/tag/v5.0.1 和 https://android.googlesource.com/platform/packages/modules/adb/ 。
- **图标与品牌素材**：StepX / Amoo 名称、Amoo 形象和手机原厂界面素材归各自权利人所有，仅用于识别所连接的产品；不授予商标或品牌形象的独立使用权。项目自有线性 SVG 图标随源码许可。StepX Companion 的应用图标为独立生成的手机与电脑连接图形，不使用 Amoo 星形角色作为产品标识；外观预览根据实机参考生成，用于设备识别。生成提示词见 `docs/ASSET_PROMPTS.json`。

项目整体采用 AGPL-3.0-only，以满足包含的 AGPL 组件要求；这不会改变第三方文件已有的 MIT、Apache 或其他许可。Google Play / Google 服务 APK、厂商系统固件不包含在本仓库或发布包中。

- **CPython**：Windows 使用 python.org 的 3.13.13 嵌入包，Linux/macOS 使用 Astral python-build-standalone 20261021 的 CPython 3.13.17；下载均固定 SHA256，保留包内许可证。https://www.python.org/ 和 https://github.com/astral-sh/python-build-standalone 。
- **OpenJDK / Android 签名校验工具**：运行时由 Eclipse Temurin 17 的 jlink 生成，保留 legal 目录；Android build-tools 36.0.0 的 aapt/apksigner 随带 NOTICE。源代码见 https://github.com/adoptium/temurin17-binaries 和 https://android.googlesource.com/platform/tools/apksig/ 。
