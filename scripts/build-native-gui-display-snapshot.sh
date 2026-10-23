#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JDK="${STEPX_JDK:-${JAVA_HOME}}"
SDK="${ANDROID_SDK_ROOT:-${ANDROID_HOME}}"
BUILD="$PROJECT_ROOT/desktop/.runtime/native-gui-research"
mkdir -p "$BUILD"
SNAPSHOT_BUILD="$(mktemp -d "$BUILD/display-snapshot-build.XXXXXX")"
trap 'rm -rf -- "$SNAPSHOT_BUILD"' EXIT
mkdir -p "$SNAPSHOT_BUILD/classes" "$SNAPSHOT_BUILD/dex"
"$JDK/bin/javac" -encoding UTF-8 -source 11 -target 11 -cp "$SDK/platforms/android-36/android.jar" \
  -d "$SNAPSHOT_BUILD/classes" "$PROJECT_ROOT/desktop/gui-native/NativeGuiDisplaySnapshot.java"
"$JDK/bin/jar" cf "$SNAPSHOT_BUILD/classes.jar" -C "$SNAPSHOT_BUILD/classes" .
"$JDK/bin/java" -cp "$SDK/cmdline-tools/latest/lib/r8.jar" com.android.tools.r8.D8 --release --min-api 36 \
  --lib "$SDK/platforms/android-36/android.jar" --output "$SNAPSHOT_BUILD/dex" "$SNAPSHOT_BUILD/classes.jar"
cp "$SNAPSHOT_BUILD/dex/classes.dex" "$BUILD/stepx-native-gui-display-snapshot.dex"
sha256sum "$BUILD/stepx-native-gui-display-snapshot.dex"
