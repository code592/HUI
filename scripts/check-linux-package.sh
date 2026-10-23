#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
arch="${1:?Usage: check-linux-package.sh x64|arm64}"
case "$arch" in x64|arm64) ;; *) exit 2 ;; esac
app="$PWD/dist/HUI-linux-$arch/bin/hui"
mkdir -p ".build/cross/smoke-$arch"
if command -v file >/dev/null 2>&1; then file "$app"; fi
ldd "$app" | tee ".build/cross/smoke-$arch/dependencies.txt"
if grep -q 'not found' ".build/cross/smoke-$arch/dependencies.txt"; then exit 1; fi
"$app" --render packaging/smoke/multilingual.md ".build/cross/smoke-$arch/document.png"
"$app" --render packaging/smoke/multilingual.md ".build/cross/smoke-$arch/document.pdf"

for format in png html pdf; do
    "$app" --render packaging/smoke/mermaid.md ".build/cross/smoke-$arch/mermaid.$format"
done
"$app" --render packaging/smoke/mermaid.md ".build/cross/smoke-$arch/mermaid-dark.png" dark
