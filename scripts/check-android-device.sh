#!/usr/bin/env bash
# Explicit serial prevents accidentally installing on a connected personal device.
set -euo pipefail
cd "$(dirname "$0")/.."
serial="${1:?Usage: check-android-device.sh emulator-SERIAL}"
case "$serial" in emulator-*) ;; *) echo 'This smoke test is restricted to emulators.' >&2; exit 2 ;; esac
: "${ANDROID_HOME:?Set ANDROID_HOME}"
adb="$ANDROID_HOME/platform-tools/adb"
packaging/android/gradlew -p packaging/android --no-daemon assemblePreviewAndroidTest
"$adb" -s "$serial" install -r packaging/android/app/build/outputs/apk/preview/app-preview.apk
"$adb" -s "$serial" install -r packaging/android/app/build/outputs/apk/androidTest/preview/app-preview-androidTest.apk
mkdir -p .build
"$adb" -s "$serial" shell am instrument -w dev.hui.markdown.preview.test/dev.hui.markdown.SmokeInstrumentation | tee .build/android-smoke.log
# Instrumentation can exit 0 on test failure; require the explicit success marker.
grep -q 'PASS: provider save' .build/android-smoke.log
! grep -q 'FAIL:' .build/android-smoke.log
"$adb" -s "$serial" uninstall dev.hui.markdown.preview.test
