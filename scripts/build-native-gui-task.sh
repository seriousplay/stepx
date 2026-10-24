#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JDK="${STEPX_JDK:-${JAVA_HOME:?Set JAVA_HOME to JDK 17}}"
SDK="${ANDROID_SDK_ROOT:-${ANDROID_HOME:?Set ANDROID_HOME to Android SDK}}"
BUILD="$PROJECT_ROOT/desktop/.runtime/native-gui-task-build"
mkdir -p "$BUILD/classes" "$BUILD/dex" "$PROJECT_ROOT/desktop/assets"
"$JDK/bin/javac" -encoding UTF-8 -source 11 -target 11 -cp "$SDK/platforms/android-36/android.jar" -d "$BUILD/classes" "$PROJECT_ROOT/desktop/gui-native/NativeGuiTaskAdapter.java"
"$JDK/bin/jar" cf "$BUILD/classes.jar" -C "$BUILD/classes" .
"$JDK/bin/java" -cp "$SDK/cmdline-tools/latest/lib/r8.jar" com.android.tools.r8.D8 --release --min-api 36 --lib "$SDK/platforms/android-36/android.jar" --output "$BUILD/dex" "$BUILD/classes.jar"
cp "$BUILD/dex/classes.dex" "$PROJECT_ROOT/desktop/assets/stepx-native-gui-task.dex"
echo 'StepX original GUI fixed Settings/About test adapter built; no task starts on load.'
