#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JDK="${STEPX_JDK:-${JAVA_HOME}}"
SDK="${ANDROID_SDK_ROOT:-${ANDROID_HOME}}"
BUILD="$PROJECT_ROOT/desktop/.runtime/native-gui-research"
mkdir -p "$BUILD"
TASK_BUILD="$(mktemp -d "$BUILD/task-host-build.XXXXXX")"
trap 'rm -rf -- "$TASK_BUILD"' EXIT
mkdir -p "$TASK_BUILD/classes" "$TASK_BUILD/dex" "$TASK_BUILD/stubs"
"$JDK/bin/javac" --release 11 -encoding UTF-8 -cp "$SDK/platforms/android-36/android.jar" -d "$TASK_BUILD/stubs" \
  "$PROJECT_ROOT/desktop/gui-native/compile-stubs/android/content/IContentProvider.java"
"$JDK/bin/jar" cf "$TASK_BUILD/stubs.jar" -C "$TASK_BUILD/stubs" .
"$JDK/bin/javac" -encoding UTF-8 -source 11 -target 11 -cp "$TASK_BUILD/stubs.jar:$SDK/platforms/android-36/android.jar" -d "$TASK_BUILD/classes" \
  "$PROJECT_ROOT/desktop/gui-native/NativeGuiTaskHost.java" \
  "$PROJECT_ROOT/desktop/gui-native/NativeGuiTaskAdapter.java" \
  "$PROJECT_ROOT/desktop/gui-native/NativeGuiSettingsLauncher.java" \
  "$PROJECT_ROOT/desktop/gui-native/NativeGuiTaskResources.java" \
  "$PROJECT_ROOT/desktop/gui-native/NativeGuiModelDiagnostics.java" \
  "$PROJECT_ROOT/desktop/gui-native/NativeGuiEventDiagnostics.java" \
  "$PROJECT_ROOT/desktop/gui-native/NativeGuiQueryCleanup.java" \
  "$PROJECT_ROOT/desktop/gui-native/NativeGuiContentResolver.java"
"$JDK/bin/jar" cf "$TASK_BUILD/classes.jar" -C "$TASK_BUILD/classes" .
"$JDK/bin/java" -cp "$SDK/cmdline-tools/latest/lib/r8.jar" com.android.tools.r8.D8 --release --min-api 36 \
  --lib "$SDK/platforms/android-36/android.jar" --classpath "$TASK_BUILD/stubs.jar" --output "$TASK_BUILD/dex" "$TASK_BUILD/classes.jar"
cp "$TASK_BUILD/dex/classes.dex" "$BUILD/stepx-native-gui-task-host.dex"
sha256sum "$BUILD/stepx-native-gui-task-host.dex"
