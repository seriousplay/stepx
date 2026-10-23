#!/usr/bin/env bash
# Build and install StepX Companion for the current user on Linux or macOS.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
cd "$ROOT/desktop"
cargo build --locked

OS="$(uname -s)"
if [ "$OS" = "Linux" ] && [ "$(uname -m)" = "x86_64" ]; then
    "$ROOT/scripts/setup-desktop-tools.sh"
fi
if [ "$OS" = "Darwin" ]; then
    APP_DIR="${STEPX_MAC_APP_DIR:-$HOME/Applications/StepX Companion.app}"
    mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources/tools"
    install -m755 target/debug/stepx-companion "$APP_DIR/Contents/MacOS/stepx-companion.new"
    mv "$APP_DIR/Contents/MacOS/stepx-companion.new" "$APP_DIR/Contents/MacOS/stepx-companion"
    if [ -d "$ROOT/.tools/bin" ]; then
        cp -a "$ROOT/.tools/bin" "$APP_DIR/Contents/Resources/tools/"
        [ -d "$ROOT/.tools/root" ] && cp -a "$ROOT/.tools/root" "$APP_DIR/Contents/Resources/tools/" || true
    fi
    install -m644 assets/stepx-companion.svg "$APP_DIR/Contents/Resources/stepx-companion.svg"
    STEPX_APP_VERSION="$(cargo pkgid --offline -p stepx-companion)"
    STEPX_APP_VERSION="${STEPX_APP_VERSION##*@}"
    cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDisplayName</key>
    <string>阶跃伴侣</string>
    <key>CFBundleName</key>
    <string>StepX Companion</string>
    <key>CFBundleIdentifier</key>
    <string>com.stepx.companion</string>
    <key>CFBundleVersion</key>
    <string>$STEPX_APP_VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$STEPX_APP_VERSION</string>
    <key>CFBundleExecutable</key>
    <string>stepx-companion</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST
    echo "已安装 macOS 应用：$APP_DIR"
    exit 0
fi

STEPX_INSTALL_DIR="${STEPX_INSTALL_DIR:-$HOME/.local/opt/stepx-companion}"
STEPX_DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$STEPX_INSTALL_DIR/tools" "$STEPX_DATA_DIR/applications" "$STEPX_DATA_DIR/icons"
install -m755 target/debug/stepx-companion "$STEPX_INSTALL_DIR/stepx-companion.new"
mv "$STEPX_INSTALL_DIR/stepx-companion.new" "$STEPX_INSTALL_DIR/stepx-companion"
if [ -d "$ROOT/.tools/bin" ]; then
    cp -a "$ROOT/.tools/bin" "$ROOT/.tools/root" "$STEPX_INSTALL_DIR/tools/"
fi
install -m644 assets/amoo_desktop_icon.png "$STEPX_DATA_DIR/icons/stepx-companion-amoo-v2.png"
python3 - "$STEPX_INSTALL_DIR/stepx-companion" "$STEPX_DATA_DIR" <<'PY'
import pathlib, sys
binary = sys.argv[1].replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%')
data_dir = pathlib.Path(sys.argv[2]).resolve()
entry = data_dir/'applications/stepx-companion.desktop'
icon = str(data_dir/'icons/stepx-companion-amoo-v2.png').replace('\\', '\\\\')
entry.write_text(f'''[Desktop Entry]
Type=Application
Name=阶跃伴侣
Name[en]=StepX Companion
Comment=连接 StepX，协同手机与电脑
Exec="{binary}"
Icon={icon}
Terminal=false
Categories=Utility;
StartupWMClass=stepx-companion
''')
PY
if command -v update-desktop-database >/dev/null; then update-desktop-database "$STEPX_DATA_DIR/applications"; fi
echo "已安装：$STEPX_INSTALL_DIR/stepx-companion"
