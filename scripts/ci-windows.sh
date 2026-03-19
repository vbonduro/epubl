#!/usr/bin/env bash
# Reproduce the CI Windows job locally using cargo-xwin.
# Run inside the devcontainer or via:
#   docker run --rm -v "$PWD:/workspaces/epubl" -w /workspaces/epubl epubl-dev bash scripts/ci-windows.sh
#
# Mirrors .github/workflows/ci.yml job: "Rust check & clippy (Windows target)"
set -euo pipefail

TARGET=x86_64-pc-windows-msvc
MANIFEST=src-tauri/Cargo.toml

echo "=== Windows CI (local) — target: $TARGET ==="

# tauri::generate_context!() checks that frontendDist exists at compile time.
# Create a stub so cargo check doesn't panic before the frontend is built.
mkdir -p dist
touch dist/index.html

echo "--- cargo check ---"
cargo xwin check --target "$TARGET" --manifest-path "$MANIFEST"

echo "--- cargo clippy ---"
cargo xwin clippy --target "$TARGET" --manifest-path "$MANIFEST" -- -D warnings

echo "=== All Windows CI checks passed ==="
echo ""
echo "NOTE: 'cargo test --lib' is not run locally because cross-compiled Windows"
echo "binaries require Wine to execute. Tests run natively in CI on windows-latest."
