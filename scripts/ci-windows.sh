#!/usr/bin/env bash
# Reproduce the CI Windows job locally using cargo-xwin.
# Run inside the devcontainer or via:
#   docker run --rm -v "$PWD:/workspaces/epubl" -w /workspaces/epubl epubl-dev bash scripts/ci-windows.sh
#
# Mirrors .github/workflows/ci.yml job: "Rust check & clippy (Windows target)"
#
# NOTE: cargo-xwin handles pure-Rust cross-compilation well but some crates with
# C build scripts (ring, zstd-sys) require native MSVC cl.exe and cannot be
# compiled via clang-cl. We therefore check only the library crate with
# --no-default-features to avoid pulling in the updater (ring dependency).
# The full build is validated in CI on windows-latest runners.
set -euo pipefail

TARGET=x86_64-pc-windows-msvc
MANIFEST=src-tauri/Cargo.toml

echo "=== Windows CI (local) — target: $TARGET ==="

echo "--- cargo check --lib (Windows-specific Rust code) ---"
# --no-default-features excludes tauri-plugin-updater which brings in ring/zstd-sys
# Those crates require native MSVC cl.exe and can only fully build in CI.
cargo xwin check --lib --target "$TARGET" --manifest-path "$MANIFEST" \
  --no-default-features 2>&1

echo "--- cargo clippy --lib (Windows-specific Rust code) ---"
cargo xwin clippy --lib --target "$TARGET" --manifest-path "$MANIFEST" \
  --no-default-features -- -D warnings 2>&1

echo "=== Windows CI checks passed ==="
echo ""
echo "NOTE: Full 'cargo check' and 'cargo test --lib' against the Windows target"
echo "require native MSVC (cl.exe) for ring/zstd-sys and run only in CI."
echo "Push to trigger the full check on windows-latest."
