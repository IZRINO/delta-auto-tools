# Deployment

Delta Auto Tools is a Windows desktop application distributed as an NSIS installer via GitHub Releases. There is no server deployment, no Docker, and no CI/CD pipeline that auto-builds. Releases are built locally and uploaded manually.

## Build

### Prerequisites

- The Tauri signing key pair must exist. Run `scripts/setup-update-key.ps1` once to generate it. The private key is saved to `$HOME/.tauri/delta-auto-tools.key` (not committed). The public key is written to `tauri.conf.json` `plugins.updater.pubkey`.

### Signed stable build

```bash
# Set the signing key (content, not path)
$env:TAURI_SIGNING_PRIVATE_KEY = "<private key content>"
# Optional: $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<password>"

bun run tauri build
```

Or use the one-click script:

```bash
scripts/build-release.ps1
```

This produces:
- `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe`
- `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe.sig`

### Beta build (unsigned)

Beta versions do not need signing:

```bash
bun run tauri build
```

Produces only the `.exe` (no `.sig`).

### latest.json

After a signed build, run `scripts/generate-latest-json.ps1` to produce `latest.json` from the `.sig` file. This is the Tauri updater manifest that the app fetches at runtime.

## Version numbering

Versions follow SemVer. Beta versions use `<major>.<minor>.<patch>-beta.<N>` (e.g. `0.17.0-beta.1`). The updater does SemVer full-order comparison: same-number stable > beta, higher number > lower, stable never downgrades to beta.

All three version sources must be in sync: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.

## Release process

1. Update version in all three files.
2. Build (signed for stable, unsigned for beta).
3. Commit with subject `发布 v<version>` and body containing `变更：` section with actual changes.
4. Tag: `git tag -a v<version> -m "发布 v<version>"` and push.
5. Create GitHub Release and upload assets:
   - **Stable**: 3 assets - `.exe`, `.sig`, `latest.json`
   - **Beta**: 1 asset - `.exe` only, with `--prerelease` flag
6. Verify with `gh release view v<version> --json tagName,isDraft,isPrerelease,assets`.

## Auto-update mechanism

The app checks `https://github.com/IZRINO/delta-auto-tools/releases/latest/download/latest.json` for updates. GitHub's `/releases/latest` endpoint only resolves non-prerelease releases, so beta users are not offered other betas; they update to the next stable when it ships.

Beta builds use the same stable endpoint. Because `0.17.0-beta.5 < 0.17.0` in SemVer, a beta user will be offered the update to `0.17.0` when it releases.

## Network and proxy

If `git push` or `gh release` fails with connection errors, set local proxy environment variables:

```bash
$env:HTTP_PROXY = "http://127.0.0.1:7897"
$env:HTTPS_PROXY = "http://127.0.0.1:7897"
```

Do not leave a trailing space after the value in `set` commands (Windows cmd includes it in the variable).
