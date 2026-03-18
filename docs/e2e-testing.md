# E2E Testing

## Overview

End-to-end tests for epubl use [WebdriverIO v8](https://webdriver.io/) with the
[`wdio-service-tauri`](https://github.com/nicholasgasior/wdio-service-tauri) service.
WebdriverIO launches `tauri-driver`, which in turn spawns the compiled Tauri binary and
exposes a WebDriver endpoint. Tests interact with the live application window using
standard CSS selectors.

```
npm run test:e2e
    └─ bash e2e/run.sh
           ├─ (optional) starts Xvfb for headless display
           ├─ cargo build --release --features e2e-mock,e2e-update-mock
           └─ npx wdio run wdio.conf.ts
                  └─ tauri-driver → epubl binary → WebDriver session
```

## Prerequisites

### tauri-driver

`tauri-driver` is a WebDriver server that wraps the Tauri application. Install it once:

```bash
cargo install tauri-driver
```

`tauri-driver` must be on `$PATH` when the tests run; `wdio-service-tauri` locates it
automatically.

### Node dependencies

```bash
npm install
```

### (Linux) Virtual display

On a headless Linux machine (e.g. CI), install Xvfb:

```bash
sudo apt-get install -y xvfb
```

`e2e/run.sh` starts Xvfb automatically when `$DISPLAY` is unset and `Xvfb` is available.

## Running locally

```bash
npm run test:e2e
```

This script:
1. Starts Xvfb on `:99` if needed.
2. Builds the app with mock Cargo features (`e2e-mock`, `e2e-update-mock`).
3. Invokes `npx wdio run wdio.conf.ts`.

Pass extra wdio flags after `--`:

```bash
npm run test:e2e -- --spec e2e/specs/sync-button.spec.ts
```

## Running against a pre-built binary

If you already have a binary (e.g. a release artifact or a specific mock build), skip
the cargo step by setting `EPUBL_BIN`:

```bash
EPUBL_BIN=./src-tauri/target/release/epubl npx wdio run wdio.conf.ts
```

### Mock device build

```bash
cargo build --release --features e2e-mock --manifest-path src-tauri/Cargo.toml
EPUBL_BIN=./src-tauri/target/release/epubl npx wdio run wdio.conf.ts
```

## CI integration

The `test:e2e:ci` script assumes Xvfb is already running on `:99` (set up by the CI
job before invoking npm):

```yaml
# Example GitHub Actions step
- name: Start Xvfb
  run: Xvfb :99 -screen 0 1280x800x24 &

- name: Run E2E tests
  run: npm run test:e2e:ci
  env:
    DISPLAY: ':99'
```

`EPUBL_BIN` can be set in CI to point at a pre-built artifact, skipping the cargo
build step entirely.

## What is (and is not) tested

| Area | Tested | Notes |
|---|---|---|
| No-device placeholder | Yes | Standard build, always runs |
| Connected-device badge | Yes (mock) | Requires `e2e-mock` feature |
| Device model / drive label | Yes (mock) | Requires `e2e-mock` feature |
| Eject button state | Yes | Disabled without device, enabled with mock |
| Eject error display | Yes (mock) | Mock build always returns an error |
| Update banner appearance | Yes (mock) | Requires `e2e-update-mock` feature |
| Update banner dismiss | Yes (mock) | Requires `e2e-update-mock` feature |
| Sync button loading state | Yes | Uses setTimeout stub |
| Sync button re-enable | Yes | Waits for stub to complete |
| Actual file transfer | No | Out of scope for E2E layer |
| Real device detection | No | Requires physical hardware |
| Real update download | No | Replaced by mock |

## Cargo feature flags

### `e2e-mock`

When this feature is compiled in, the `get_connected_ereaders` Tauri command returns a
hard-coded device list (e.g. `Kindle Paperwhite` at drive `E:`) instead of performing
real USB enumeration. The `eject_device` command always returns a
`"Mocked eject error"` to exercise the UI error path.

### `e2e-update-mock`

When this feature is compiled in, the `check_for_update` command immediately emits an
`update-available` event with version `"9.9.9"` instead of contacting the update
server. This makes the update banner tests deterministic and offline-friendly.

Both features are compiled together by `e2e/run.sh` and are never present in production
release builds.

## Generating test fixtures

A minimal valid EPUB3 file can be generated for use in future sync tests:

```bash
python3 e2e/fixtures/make_sample_epub.py
# Creates e2e/fixtures/sample.epub
```

See `e2e/fixtures/README.md` for details.
