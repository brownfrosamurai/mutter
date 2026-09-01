# How to build and sign a release

Two separate things share the word "build" here, and they're not interchangeable: fast local iteration, and the real tagged release CI publishes. This guide covers both, plus the one-time signing setup each depends on.

## Prerequisites

- macOS 14+, Rust + Cargo, Node.js, `cargo tauri` CLI (`cargo install tauri-cli` if you don't have it)
- For a real tagged release: push access to the GitHub repo, and the two `TAURI_SIGNING_PRIVATE_KEY`/`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets already configured on it (see [Ed25519 update-signing key](#one-time-setup-ed25519-update-signing-key) below if setting this up fresh)

## Local dev builds — day-to-day iteration

Plain `cargo build` or `cargo tauri dev` does **not** reliably re-embed a changed `frontend/dist/` — `frontend/dist/` isn't part of cargo's own dependency graph, so an incremental rebuild can silently serve a stale frontend. Two ways to force a real rebuild:

```bash
# Fast local iteration, frontend already built:
touch src-tauri/build.rs && cargo build --manifest-path src-tauri/Cargo.toml

# The reliable path — runs the full beforeBuildCommand pipeline:
cd src-tauri && cargo tauri build --debug
```

Both `cargo build` and `cargo tauri dev` produce a binary signed `adhoc,linker-signed` regardless of `tauri.conf.json`'s `signingIdentity` — that config only takes effect in `cargo tauri build`'s actual bundling step. Permissions granted to an ad-hoc-signed dev binary don't stick across rebuilds (ad-hoc signatures reset per build), so expect to re-grant mic/Accessibility/Screen Recording repeatedly during normal dev iteration. This is expected, not a bug to chase.

## A locally-installed build with a stable identity

For anything that needs permissions to *stick* across rebuilds (multi-day dogfooding, testing the recovery flow, anything permission-gated):

```bash
cd src-tauri
cargo tauri build --debug
cp -R target/debug/bundle/macos/Mutter.app /Applications/Mutter.app
```

This uses `tauri.conf.json`'s configured `signingIdentity` ("Mutter Dev Signing" locally — see [one-time setup](#one-time-setup-a-stable-local-signing-identity) below), which stays the same across rebuilds, so `/Applications/Mutter.app`'s permission grants survive reinstalling a newer build over it. A raw `cargo build` binary run directly does not have this property.

## A real tagged release (CI)

`.github/workflows/release.yml` builds and publishes a GitHub Release automatically whenever a `v*` tag is pushed:

```bash
git tag v0.2.0
git push origin v0.2.0
```

This runs on a GitHub-hosted `macos-latest` runner via `tauri-apps/tauri-action`, which:

1. Builds the app and bundles it (`.app` + `.dmg`).
2. Code-signs the bundle **ad-hoc** (`--config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'`) — the local "Mutter Dev Signing" certificate only exists in the maintainer's own keychain, not on a CI runner, and this project has no paid Apple Developer notarization (a deliberate "zero paid resources for v1" constraint — see `CLAUDE.md`).
3. **Separately**, Ed25519-signs the update payload itself using the `TAURI_SIGNING_PRIVATE_KEY` GitHub secret, and generates `latest.json` — the manifest the app's own in-app updater (Settings → Software update → Check for Updates) polls for. This signature is independent of the OS code signature, and is what actually lets the updater verify a downloaded update's authenticity regardless of ad-hoc signing.
4. Creates a draft GitHub Release named `Mutter <tag>` with the `.dmg` and `latest.json` attached.

The release is created as a **draft** — publish it manually from the GitHub Releases UI once you've reviewed it.

## One-time setup: a stable local signing identity

Only needed once per development machine, for the locally-installed-build workflow above:

1. Create a self-signed certificate in Keychain Access (Certificate Assistant → Create a Certificate → Code Signing type).
2. Set it to "Always Trust" for Code Signing — `security find-identity -v -p codesigning` won't list it as a valid identity until this trust step happens.
3. Reference it by name in `tauri.conf.json`'s `bundle.macOS.signingIdentity` (already done in this repo — "Mutter Dev Signing").

## One-time setup: Ed25519 update-signing key

```bash
cargo tauri signer generate
```

Keep the generated private key **outside the repo** (this project keeps it at `~/.mutter-signing/update-key.pem`, `chmod 600`). Pin the printed public key into `tauri.conf.json`'s `plugins.updater.pubkey`. For CI, base64-encode the private key file and store it as the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret (plus `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if you set a password on it — this repo's key deliberately has none, a simplicity trade-off for a single-maintainer project; add a password and move it to a proper secret store before any other automation touches it).

## Verification

Locally: `codesign -dv` on the built `.app`'s executable should show `Authority=Mutter Dev Signing` (for a `cargo tauri build --debug` output) or `adhoc` (for a plain `cargo build`/`cargo tauri dev` binary) — confirm you're checking the one you think you are.

For a tagged release: check the GitHub Actions run for the `release` workflow succeeded, then check the draft release's assets include both the `.dmg` and `latest.json` before publishing it.

## Troubleshooting

- **`cargo tauri build` reports "A public key has been found, but no private key"** — expected locally if `TAURI_SIGNING_PRIVATE_KEY` isn't set in your shell environment; the `.app`/`.dmg` bundles still build successfully, only the update-payload signing step fails. This is fine for local iteration; it must succeed in CI (where the secret is set) for a real release.
- **Local build serves stale frontend content** — see "Local dev builds" above; use `cargo tauri build --debug` or `touch build.rs`.
- **Permissions keep resetting between local test runs** — you're running a raw `cargo build`/`cargo tauri dev` binary (always ad-hoc-signed, resets per build), not the stable-identity `cargo tauri build --debug` output installed to `/Applications/`.

## Related

- [`reference-architecture.md`](reference-architecture.md)
