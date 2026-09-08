#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if ! command -v cargo >/dev/null 2>&1 && [ -d /opt/homebrew/opt/rustup/bin ]; then
  export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fi
export MACOSX_DEPLOYMENT_TARGET=12.0
cargo build --release -p hui-app --locked
app="$PWD/dist/HUI.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/katex"
cp target/release/hui "$app/Contents/MacOS/HUI"
cp packaging/macos/Info.plist "$app/Contents/Info.plist"
cp packaging/macos/HUI.icns "$app/Contents/Resources/HUI.icns"
cp vendor/katex/katex.min.css "$app/Contents/Resources/katex/"
cp -R vendor/katex/fonts "$app/Contents/Resources/katex/"
cp vendor/katex/LICENSE "$app/Contents/Resources/katex/"
mkdir -p "$app/Contents/Resources/licenses"
cp vendor/mathjax/LICENSE "$app/Contents/Resources/licenses/MathJax-LICENSE"
cp vendor/slint/LICENSE.md "$app/Contents/Resources/licenses/Slint-LICENSE.md"
cp THIRD_PARTY.md "$app/Contents/Resources/licenses/THIRD_PARTY.md"
cp LICENSE "$app/Contents/Resources/licenses/HUI-LICENSE"
codesign --force --deep --sign "${HUI_SIGN_IDENTITY:--}" "$app"
# Finder watches the bundle directory, not just changed files inside Resources.
touch "$app/Contents" "$app"
if [ "${HUI_DMG:-0}" = 1 ]; then
  dmg_stage="$(mktemp -d "$PWD/dist/.dmg-stage.XXXXXX")"
  trap 'rm -rf "$dmg_stage"' EXIT
  ditto "$app" "$dmg_stage/HUI.app"
  ln -s /Applications "$dmg_stage/Applications"
  hdiutil create -volname HUI -srcfolder "$dmg_stage" -ov -format UDZO dist/HUI-macos.dmg
fi
printf 'Built %s\n' "$app"
