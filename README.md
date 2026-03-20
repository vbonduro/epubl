# epubl

A dead-simple Windows desktop app for copying epub books to a Kobo or similar eReader. Built with [Tauri](https://v2.tauri.app/) (Rust + Svelte).

## Development

All development happens inside a Docker devcontainer so the Linux host gets the exact same environment as CI. The devcontainer includes:

- Rust stable + clippy
- Node.js 22
- Tauri CLI + tauri-driver (E2E)
- cargo-xwin (Windows cross-compilation)
- pre-commit (source-controlled git hooks)

### Starting the devcontainer

**Option A — VS Code Dev Containers extension (recommended)**

Open the repo in VS Code, then *Reopen in Container* when prompted. The `postCreateCommand` runs `npm ci` and installs the pre-push hook automatically.

**Option B — manually via Docker**

```bash
# Build the image (only needed once, or after Dockerfile changes)
docker build -t epubl-dev -f .devcontainer/Dockerfile .

# Start a background container with the repo mounted
docker run --rm -d --name epubl-dev \
  -v "$(pwd):/workspaces/epubl" \
  -w /workspaces/epubl \
  epubl-dev sleep infinity

# Run commands inside it
docker exec epubl-dev bash -c "npm ci"
docker exec epubl-dev bash -c "cd src-tauri && cargo test"
docker exec epubl-dev bash -c "npm run build"

# Stop when done
docker stop epubl-dev
```

> **Why the devcontainer?** Node modules in the repo are sometimes written as root by Docker, so `npm run build` fails on the host. Always build and test inside the container.

### Running tests

```bash
# Inside the devcontainer:
cd src-tauri && cargo test          # Rust unit + integration tests
npm run build                       # Frontend type-check + build
scripts/ci-windows.sh               # Reproduce Windows CI (cargo check + clippy)
```

### Git hooks

The pre-push hook runs the full test suite before any push. It is installed automatically via `postCreateCommand`. To install it manually:

```bash
pre-commit install --hook-type pre-push
```

## Architecture

```
src/               Svelte frontend (TypeScript)
src-tauri/
  src/
    lib.rs         Tauri app entry point + command registration
    config.rs      Config read/write (JSON in app data dir)
    epub.rs        Epub folder scanning + diff logic
    usb.rs         USB eReader detection + safe eject (Windows)
    updater.rs     Auto-update check on startup
  tests/           Integration tests (real files, no mocks)
e2e/               WebdriverIO E2E tests (tauri-driver)
scripts/
  pre-push.sh      Full test suite (runs inside Docker)
  ci-windows.sh    Windows cross-compile check via cargo-xwin
.devcontainer/     Docker dev environment
.github/workflows/ CI (Linux build + Windows cross-compile check)
```

## Distribution

Releases are built for Windows (`x86_64-pc-windows-msvc`) via GitHub Actions and published to GitHub Releases as an NSIS installer.
