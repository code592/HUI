#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .build/mathjax
npm install --prefix .build/mathjax --no-audit --no-fund --save-exact mathjax-full@3.2.2 esbuild@0.25.5
NODE_PATH="$PWD/.build/mathjax/node_modules" .build/mathjax/node_modules/.bin/esbuild scripts/mathjax-entry.mjs --bundle --format=iife --platform=browser --minify --outfile=vendor/mathjax/runtime.js
cp .build/mathjax/node_modules/mathjax-full/LICENSE vendor/mathjax/LICENSE
