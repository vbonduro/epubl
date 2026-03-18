#!/usr/bin/env bash
set -euo pipefail

# Start a virtual display for headless Linux/CI environments
if command -v Xvfb &>/dev/null && [ -z "${DISPLAY:-}" ]; then
  Xvfb :99 -screen 0 1280x800x24 &
  XVFB_PID=$!
  export DISPLAY=:99
  trap "kill $XVFB_PID 2>/dev/null || true" EXIT
  echo "[e2e] started Xvfb on :99"
fi

# Build the app with e2e mock features unless EPUBL_BIN is already set
if [ -z "${EPUBL_BIN:-}" ]; then
  echo "[e2e] building frontend..."
  npm run build

  echo "[e2e] building epubl with e2e-mock features..."
  cargo build --release --features e2e-mock,e2e-update-mock \
    --manifest-path src-tauri/Cargo.toml
  export EPUBL_BIN="./src-tauri/target/release/epubl"
fi

echo "[e2e] running WebdriverIO..."
npx wdio run wdio.conf.ts "$@"
