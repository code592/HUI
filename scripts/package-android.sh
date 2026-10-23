#!/usr/bin/env bash
# Build an installable preview (debug certificate) or an unsigned release for external signing.
set -euo pipefail
cd "$(dirname "$0")/.."
variant="${1:-preview}"
case "$variant" in preview) task=assemblePreview; apk=app-preview.apk ;; release) task=assembleRelease; apk=app-release-unsigned.apk ;; *) echo 'Usage: package-android.sh [preview|release]' >&2; exit 2 ;; esac
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
: "${JAVA_HOME:?Set JAVA_HOME to JDK 17 or newer}"
export ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-$ANDROID_HOME/ndk/28.2.13676358}"
export ANDROID_NDK="$ANDROID_NDK_ROOT"
case "$(uname -s)" in Darwin) host=darwin-x86_64 ;; Linux) host=linux-x86_64 ;; *) echo 'Use macOS/Linux/WSL with the Android NDK.' >&2; exit 1 ;; esac
llvm="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/$host/bin"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$llvm/aarch64-linux-android29-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export CXX_aarch64_linux_android="$llvm/aarch64-linux-android29-clang++"
export AR_aarch64_linux_android="$llvm/llvm-ar"
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/$host/sysroot --target=aarch64-linux-android29"
# Android 15+ devices can use 16 KiB pages. Verify with check-android-package.py.
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384'
test -x "$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
rustup target add aarch64-linux-android
cargo rustc --locked --release -p hui-app --lib --crate-type cdylib --target aarch64-linux-android -j "${HUI_BUILD_JOBS:-4}"
generated=packaging/android/generated
mkdir -p "$generated/jniLibs/arm64-v8a" "$generated/assets/katex" "$generated/assets/licenses" dist
cp "${CARGO_TARGET_DIR:-target}/aarch64-linux-android/release/libhui_app.so" "$generated/jniLibs/arm64-v8a/"
cp vendor/katex/katex.min.css vendor/katex/LICENSE "$generated/assets/katex/"
cp -R vendor/katex/fonts "$generated/assets/katex/"
cp LICENSE THIRD_PARTY.md "$generated/assets/licenses/"
cp vendor/slint/LICENSE.md "$generated/assets/licenses/Slint-LICENSE.md"
cp vendor/mathjax/LICENSE "$generated/assets/licenses/MathJax-LICENSE"
packaging/android/gradlew -p packaging/android --no-daemon "$task"
output="dist/HUI-android-arm64-$variant.apk"
cp "packaging/android/app/build/outputs/apk/$variant/$apk" "$output"
python3 scripts/check-android-package.py "$output"
python3 - "$output" <<'PY'
import hashlib, pathlib, sys
p = pathlib.Path(sys.argv[1])
h = hashlib.sha256()
with p.open('rb') as f:
    for chunk in iter(lambda: f.read(1024 * 1024), b''): h.update(chunk)
p.with_suffix('.apk.sha256').write_text(h.hexdigest() + '  ' + p.name + '\n')
PY
printf 'Built %s\nPreview uses a development certificate; release requires your signing key.\n' "$output"
