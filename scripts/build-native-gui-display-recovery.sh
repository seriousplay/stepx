#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JDK="${STEPX_JDK:-${JAVA_HOME}}"
SDK="${ANDROID_SDK_ROOT:-${ANDROID_HOME}}"
BUILD="$PROJECT_ROOT/desktop/.runtime/native-gui-research"
mkdir -p "$BUILD"
RECOVERY_BUILD="$(mktemp -d "$BUILD/display-recovery-build.XXXXXX")"
trap 'rm -rf -- "$RECOVERY_BUILD"' EXIT
mkdir -p "$RECOVERY_BUILD/classes" "$RECOVERY_BUILD/dex"
"$JDK/bin/javac" -encoding UTF-8 -source 11 -target 11 -cp "$SDK/platforms/android-36/android.jar" \
  -d "$RECOVERY_BUILD/classes" "$PROJECT_ROOT/desktop/gui-native/NativeGuiDisplayRecovery.java"
"$JDK/bin/jar" cf "$RECOVERY_BUILD/classes.jar" -C "$RECOVERY_BUILD/classes" .
"$JDK/bin/java" -cp "$SDK/cmdline-tools/latest/lib/r8.jar" com.android.tools.r8.D8 --release --min-api 36 \
  --lib "$SDK/platforms/android-36/android.jar" --output "$RECOVERY_BUILD/dex" "$RECOVERY_BUILD/classes.jar"
cp "$RECOVERY_BUILD/dex/classes.dex" "$BUILD/stepx-native-gui-display-recovery.dex"
sha256sum "$BUILD/stepx-native-gui-display-recovery.dex"
