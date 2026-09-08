#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if ! command -v cargo >/dev/null 2>&1 && [ -d /opt/homebrew/opt/rustup/bin ]; then
  export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fi
exec cargo run -p hui-app -- "$@"
