FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Tauri system dependencies for Linux
# https://v2.tauri.app/start/prerequisites/#linux
RUN apt-get update && apt-get install -y --no-install-recommends \
    # WebKit/GTK (required by Tauri on Linux)
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    # Build tools
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libxdo-dev \
    # SVG icon rendering
    librsvg2-dev \
    # For tauri-plugin-dialog (file picker)
    libglib2.0-dev \
    # For running a display in CI / headless testing
    xvfb \
    # WebKitWebDriver — required by tauri-driver for E2E tests on Linux
    webkit2gtk-driver \
    # clang + lld — required by cargo-xwin to cross-compile for Windows MSVC
    clang \
    lld \
    llvm \
    && rm -rf /var/lib/apt/lists/*

# Ensure LLVM tools (clang-cl, lld-link, llvm-lib) are on PATH for cargo-xwin.
# clang-cl is clang invoked as the MSVC-compatible driver; symlink it explicitly.
RUN ln -s /usr/lib/llvm-14/bin/clang-14 /usr/lib/llvm-14/bin/clang-cl
ENV PATH="/usr/lib/llvm-14/bin:${PATH}"

# Node.js LTS (v22) — matches the developer's local version
ARG NODE_VERSION=22
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Rust stable toolchain (base image ships stable, but pin explicitly)
RUN rustup update stable && rustup default stable && rustup component add clippy

# Windows cross-compilation support via cargo-xwin
RUN rustup target add x86_64-pc-windows-msvc && \
    cargo install cargo-xwin --locked

# cargo-tauri CLI and tauri-driver (WebDriver server for E2E tests)
RUN cargo install tauri-cli --version "^2" --locked && \
    cargo install tauri-driver --locked

# uv + pre-commit for source-controlled git hooks
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:${PATH}"
RUN uv tool install pre-commit

# Pre-warm the Cargo registry cache so the first `cargo build` is faster.
# We copy only the dependency manifests to keep this layer cacheable.
WORKDIR /tmp/cache
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock ./src-tauri/
COPY src-tauri/build.rs ./src-tauri/
# Stub out src so Cargo.toml resolves without the real source
RUN mkdir -p src-tauri/src \
    && echo 'fn main() {}' > src-tauri/src/main.rs \
    && echo 'pub fn run() {}' > src-tauri/src/lib.rs \
    && touch src-tauri/src/config.rs src-tauri/src/epub.rs \
       src-tauri/src/usb.rs src-tauri/src/updater.rs \
    && cd src-tauri && cargo fetch \
    && cd .. && rm -rf /tmp/cache

WORKDIR /workspaces/epubl
