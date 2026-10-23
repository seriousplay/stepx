#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JDK="${STEPX_JDK:-${JAVA_HOME}}"
SDK="${ANDROID_SDK_ROOT:-${ANDROID_HOME}}"
BUILD="$PROJECT_ROOT/desktop/.runtime/native-gui-isolated-probe-build"
mkdir -p "$BUILD/classes" "$BUILD/dex" "$PROJECT_ROOT/desktop/assets"
"$JDK/bin/javac" -encoding UTF-8 -source 11 -target 11 -cp "$SDK/platforms/android-36/android.jar" -d "$BUILD/classes" "$PROJECT_ROOT/desktop/gui-native/NativeGuiIsolatedProbe.java"
"$JDK/bin/jar" cf "$BUILD/classes.jar" -C "$BUILD/classes" .
"$JDK/bin/java" -cp "$SDK/cmdline-tools/latest/lib/r8.jar" com.android.tools.r8.D8 --release --min-api 36 --lib "$SDK/platforms/android-36/android.jar" --output "$BUILD/dex" "$BUILD/classes.jar"
cp "$BUILD/dex/classes.dex" "$PROJECT_ROOT/desktop/assets/stepx-native-gui-isolated-probe.dex"
echo 'StepX isolated original-GUI classloader probe built; no SDK initialization or account access.'
