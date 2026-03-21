# epubl 👵🍵

A dead-simple Windows desktop app for loading epub books onto a Kobo, Kindle, or any USB eReader. Built for non-technical users who just want to read their books.

> *"This will help ya load your books bud."*

---

## 📖 What it does

1. **Download books** — click a link to your ebook store and download epub files
2. **Plug in your eReader** — the app detects it automatically
3. **Load your books** — select which books to copy and hit the button
4. **Eject safely** — hit eject, unplug the USB cable, and start reading 🍵

No fussing with folders. No drag and drop. No confusing menus.

---

## 🍵 How to use it

### Step 1 — Download Books

When you open the app you'll see a link to your ebook store. Click it, find a book you want, and choose **Download: EPUB**.

The epub file will land in your downloads folder (or wherever your library folder is set to).

### Step 2 — Connect your eReader

Plug your eReader into your computer with the USB cable. Look at your eReader screen and press the **Connect** button when it appears.

The app will show a green "eReader connected" message along with your device name.

### Step 3 — Load your books

The app shows all the epub books waiting to be loaded. They're all ticked by default — just hit the big **Load Books** button.

A progress bar shows how things are going. When it's done, you'll see a message saying all books have been loaded.

### Step 4 — Eject and unplug

Hit the **Eject eReader** button, then unplug the USB cable. Your eReader is ready to go — put the kettle on 👵🍵

---

## 💻 Installing

Download the latest installer from the [Releases page](https://github.com/vbonduro/epubl/releases/latest).

Run the `.exe` file and follow the prompts. The app installs for your user only (no admin password needed) and adds a shortcut to your Start Menu.

### Updating

The app checks for updates automatically when it starts. If a new version is available, a banner appears at the top — just click **Download** and run the new installer.

---

## 🔧 First-time setup

The first time you open the app, a setup screen appears asking for:

- **Epub Folder** — the folder on your computer where epub files are saved (usually your Downloads folder or a dedicated Books folder)
- **Support Email** — optional. If something goes wrong, the app can send a problem report to this address

Click **Save Configuration** and you're done. You won't need to do this again.

---

## 👩‍💻 Development

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

### Dev mode (browser)

Run `npm run dev` and open `http://localhost:1420` in your browser. A purple **Testing Controls** panel appears — use it to simulate connecting an eReader, adding books, and running a transfer without needing a real device or Tauri backend.

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
    copy.rs        File copy with per-file progress events
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

Releases are built for Windows (`x86_64-pc-windows-msvc`) via GitHub Actions on every `v*` tag push, and published to GitHub Releases as an NSIS installer. The auto-updater checks `releases/latest/download/latest.json` on startup.

To ship a new release:

```bash
git tag v0.2.0
git push origin v0.2.0
```
