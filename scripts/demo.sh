#!/usr/bin/env bash
# Build, generate both scenarios, and serve the report UI.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "▸ building"
cargo build --release -q

echo "▸ generating reports (reports/ + web/src/demo/)"
cargo run --release -q -p leakbench-cli -- demo --out reports --web web/src/demo

if [ ! -f web/dist/index.html ]; then
  echo
  echo "▸ web/dist is missing. Build it once with:"
  echo "    (cd web && pnpm install && pnpm build)"
  echo
fi

exec cargo run --release -q -p leakbench-cli -- serve --dir reports --static web/dist
