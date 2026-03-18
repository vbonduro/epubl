# Release Process

This document explains how to cut a release of **epubl** and how to set up the
signing keypair used to sign Windows NSIS installers.

---

## Prerequisites

- Write access to the GitHub repository
- `@tauri-apps/cli` v2 installed locally (`npm install -g @tauri-apps/cli` or
  use the project-local copy via `npx tauri`)
- The `TAURI_SIGNING_PRIVATE_KEY` (and optionally
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) repository secrets already configured
  (see [Generating the signing keypair](#generating-the-signing-keypair) below)

---

## Generating the signing keypair

Run once, then store the output in GitHub repository secrets.

```bash
# Generates a new Ed25519 keypair.
# You will be prompted for an optional password to protect the private key.
npx tauri signer generate -w ~/.tauri/epubl.key
```

The command prints both keys to stdout and writes the private key to
`~/.tauri/epubl.key` (and the public key to `~/.tauri/epubl.key.pub`).

### Store secrets in GitHub

| Secret name                          | Value                                      |
| ------------------------------------ | ------------------------------------------ |
| `TAURI_SIGNING_PRIVATE_KEY`          | Full content of `~/.tauri/epubl.key`       |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password chosen during key generation (leave empty if none was set) |

Go to **Settings > Secrets and variables > Actions > New repository secret** in
the GitHub UI, or use the GitHub CLI:

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/epubl.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD  # paste password when prompted
```

### Embed the public key in tauri.conf.json

The **public** key must be embedded in `src-tauri/tauri.conf.json` under
`plugins.updater.pubkey` so that the built app can verify update signatures:

```bash
cat ~/.tauri/epubl.key.pub
```

Copy the output and paste it as the value of `plugins.updater.pubkey` in
`src-tauri/tauri.conf.json`, replacing the `PLACEHOLDER_PUBLIC_KEY` string.
Commit and push that change before creating a release.

---

## Cutting a release

Releases are triggered automatically when a tag matching `v*` is pushed to the
`main` branch.

### Steps

1. **Update the version** in both `src-tauri/tauri.conf.json` and
   `src-tauri/Cargo.toml` (they should stay in sync):

   ```bash
   # Example: bump to 0.2.0
   # Edit "version": "0.1.0" → "version": "0.2.0" in both files.
   ```

2. **Commit the version bump:**

   ```bash
   git add src-tauri/tauri.conf.json src-tauri/Cargo.toml
   git commit -m "chore: bump version to 0.2.0"
   git push origin main
   ```

3. **Create and push an annotated tag:**

   ```bash
   git tag -a v0.2.0 -m "Release v0.2.0"
   git push origin v0.2.0
   ```

4. **Watch the workflow run** on the
   [Actions tab](https://github.com/vbonduro/epubl/actions). The
   `release.yml` workflow will:
   - Build the Windows NSIS installer on a `windows-latest` runner
   - Sign the installer with `TAURI_SIGNING_PRIVATE_KEY`
   - Create a GitHub Release named `epubl v0.2.0`
   - Upload the `.exe` installer and `latest.json` update manifest as release
     assets

5. **Verify the release** at
   [https://github.com/vbonduro/epubl/releases](https://github.com/vbonduro/epubl/releases).

---

## CI workflows

| Workflow | File | Trigger |
| -------- | ---- | ------- |
| CI checks | `.github/workflows/ci.yml` | Every push / PR to `main` |
| Release build | `.github/workflows/release.yml` | Push of a `v*` tag |

### CI jobs

- **check-rust** — `cargo check` + `cargo clippy` + `cargo test --lib` targeting
  `x86_64-pc-windows-msvc` on a `windows-latest` runner
- **check-frontend** — `npm ci` + `npm run build` (Vite) on `ubuntu-latest`

### Caching

Both workflows cache the Cargo registry/target directory and `node_modules`
using `actions/cache@v4`, keyed on the relevant lockfiles
(`Cargo.lock` / `package-lock.json`).

---

## Troubleshooting

| Problem | Likely cause | Fix |
| ------- | ------------ | --- |
| Installer signature verification fails at runtime | Public key in `tauri.conf.json` does not match `TAURI_SIGNING_PRIVATE_KEY` secret | Regenerate the keypair and update both the secret and `tauri.conf.json` |
| `TAURI_SIGNING_PRIVATE_KEY` secret is empty | Secret was not set before tagging | Set the secret, then re-run the release workflow or push the tag again |
| `cargo clippy` fails with `-D warnings` | A new Clippy lint was introduced | Fix the lint warning locally and push |
