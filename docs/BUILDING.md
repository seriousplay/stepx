# 构建与打包

需要当前 stable Rust、Git；高级功能需要 Python 3.10+（标准库即可运行内置 Office XML 工具）。可用 `STEPX_PYTHON` 指定解释器，`STEPX_ADB` 指定 ADB，`STEPX_RESOURCE_DIR` 指定资源目录。

## Linux（Ubuntu 24.04）

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config clang libclang-dev libssl-dev libfontconfig1-dev libfreetype-dev libxkbcommon-dev libwayland-dev libxcb1-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev libgl1-mesa-dev
cargo build --release --locked --manifest-path desktop/Cargo.toml
```

需要支持 Vulkan/OpenGL 的图形环境。其他 Linux 发行版可能需要对应动态库；便携包不是静态链接的 AppImage。

## macOS

安装 Xcode（含 Metal 工具链）和 Command Line Tools，运行相同的 Cargo 命令。使用目标架构的原生机器构建；发布流程分别在 Intel 和 Apple Silicon runner 执行。

## Windows

安装 Rust MSVC 工具链、Visual Studio C++ Build Tools 和 LLVM / libclang，运行相同的 Cargo 命令。Python 可通过官方安装器安装并加入 PATH。

## 验证

```bash
cargo test --locked --manifest-path desktop/Cargo.toml --no-default-features --lib --test contracts
python3 -m unittest discover -s desktop/tests -p 'test_*.py'
```

真机验证脚本不属于默认测试，不会在 CI 中连接手机。

## 发布包

```bash
python3 scripts/package-release.py --platform linux-x86_64 --version 0.2.0
```

平台可选 `linux-x86_64`、`macos-arm64`、`macos-x86_64`、`windows-x86_64`。脚本验证官方 scrcpy 工具包的 SHA256，将可执行文件与运行资源封装到 `dist/`。通过 `.github/workflows/release.yml` 的手动运行构建全部平台；先生成草稿 Release，检查产物后再发布。

## Android

Android 源码位于 `desktop/companion-app`，以及 bridge、权限与后台服务对应目录。`scripts/build-companion-app.sh` 等脚本需要 JDK、Android SDK、`ANDROID_HOME` 与 `JAVA_HOME`。发布包含本项目编译的伴侣 APK / DEX / JAR；私有签名密钥不在仓库中。自己构建的 APK 若签名不同，无法直接覆盖已安装版本；保留数据前先自行备份。

## UI 截图

`cargo build --features ui-capture --manifest-path desktop/Cargo.toml` 后可使用 `--demo --capture-ui /absolute/path.png --page overview`。演示数据仅用于截图构建；普通发布构建不启用演示模式，截图不代表连接真实设备的验收记录。
