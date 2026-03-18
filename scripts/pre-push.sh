#!/usr/bin/env bash
# Full test suite runner — executed by pre-commit in the pre-push stage.
# All tests run inside the epubl-dev Docker image to match CI exactly.
set -euo pipefail

echo "[pre-push] Running all tests in devcontainer..."

if ! command -v docker &>/dev/null; then
  echo "[pre-push] ERROR: Docker not found. Cannot run tests." >&2
  exit 1
fi

if ! docker image inspect epubl-dev &>/dev/null; then
  echo "[pre-push] Building devcontainer image (first time setup)..."
  docker build \
    -f .devcontainer/Dockerfile \
    -t epubl-dev \
    . 2>&1 || {
    echo "[pre-push] ERROR: Failed to build devcontainer image." >&2
    exit 1
  }
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"

docker run --rm \
  -v "${REPO_ROOT}:/workspaces/epubl" \
  -w /workspaces/epubl \
  epubl-dev \
  bash -c '
    set -euo pipefail

    echo "=== Installing npm dependencies ==="
    npm install --prefer-offline 2>&1 | tail -3

    echo "=== Running Rust unit and integration tests ==="
    cargo test --manifest-path src-tauri/Cargo.toml 2>&1

    echo "=== Running clippy ==="
    cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings 2>&1

    echo "=== Building frontend ==="
    npm run build 2>&1 | tail -5

    echo "=== Building app with E2E mock features ==="
    cargo build --release --features e2e-mock,e2e-update-mock \
      --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5

    echo "=== Running E2E tests ==="
    Xvfb :99 -screen 0 1280x800x24 &
    XVFB_PID=$!
    trap "kill $XVFB_PID 2>/dev/null || true" EXIT
    sleep 1
    DISPLAY=:99 EPUBL_BIN=./src-tauri/target/release/epubl npx wdio run wdio.conf.ts 2>&1
  '

echo "[pre-push] All tests passed."
