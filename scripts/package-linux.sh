#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Linux ]] || { echo 'Run this script inside Linux (native machine or container).' >&2; exit 1; }
case "$(uname -m)" in
  x86_64) arch=x64; target=x86_64-unknown-linux-gnu ;;
  aarch64) arch=arm64; target=aarch64-unknown-linux-gnu ;;
  *) echo 'Unsupported architecture' >&2; exit 1 ;;
esac
cargo build --release -p hui-app --locked --target "$target" -j "${HUI_BUILD_JOBS:-2}"
bundle="$PWD/dist/HUI-linux-$arch"
mkdir -p "$bundle/bin/assets/katex" "$bundle/share/applications" "$bundle/share/icons" "$bundle/share/licenses/HUI"
cp "${CARGO_TARGET_DIR:-target}/$target/release/hui" "$bundle/bin/hui"
cp vendor/katex/katex.min.css vendor/katex/LICENSE "$bundle/bin/assets/katex/"
cp -R vendor/katex/fonts "$bundle/bin/assets/katex/"
cp packaging/linux/hui.desktop "$bundle/share/applications/"
cp -R packaging/linux/icons/hicolor "$bundle/share/icons/"
cp LICENSE "$bundle/share/licenses/HUI/LICENSE"
cp vendor/mathjax/LICENSE "$bundle/share/licenses/HUI/MathJax-LICENSE"
cp vendor/slint/LICENSE.md "$bundle/share/licenses/HUI/Slint-LICENSE.md"
cp THIRD_PARTY.md "$bundle/share/licenses/HUI/THIRD_PARTY.md"
printf 'HUI Linux %s\nRun: ./bin/hui [document.md]\nUbuntu runtime dependencies: sudo apt install libfontconfig1 libxkbcommon0 libxkbcommon-x11-0 libxcb-shape0 libxcb-xfixes0 libudev1 libgl1 libegl1 fonts-noto-cjk
Requires a working X11 or Wayland desktop.\nInstall bin/ and share/ together into one prefix for desktop integration.\n' "$arch" > "$bundle/README.txt"
tar -C dist -czf "dist/HUI-linux-$arch.tar.gz" "HUI-linux-$arch"
(cd dist && sha256sum "HUI-linux-$arch.tar.gz") > "dist/HUI-linux-$arch.tar.gz.sha256"
printf 'Built %s\n' "$bundle"
