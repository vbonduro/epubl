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
    && rm -rf /var/lib/apt/lists/*

# Node.js LTS (v22) — matches the developer's local version
ARG NODE_VERSION=22
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Rust stable toolchain (base image ships stable, but pin explicitly)
RUN rustup update stable && rustup default stable && rustup component add clippy

# cargo-tauri CLI and tauri-driver (WebDriver server for E2E tests)
RUN cargo install tauri-cli --version "^2" --locked && \
    cargo install tauri-driver --locked

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
