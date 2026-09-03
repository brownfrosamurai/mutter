# How to cut a release

This covers the branch/PR process for shipping a new version — getting `develop`'s content onto `main` and tagging it. For the build/signing mechanics that happen once you tag (what CI actually does, the two signing keys involved), see [`howto-build-and-sign-a-release.md`](howto-build-and-sign-a-release.md).

## Prerequisites

- `develop` has everything you want to ship, CI green on it
- Push access to the repo; `gh` CLI authenticated

## Steps

1. **Bump the version and changelog on `develop`.** Three files carry the version number — keep them in sync:

   ```bash
   # frontend/package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json
   # each has a "version" field — bump all three to the same value
   cd frontend && npm install --package-lock-only  # regenerates package-lock.json's version fields too
   cd ../src-tauri && cargo build --lib             # regenerates Cargo.lock's version field
   ```

   Add a new `## [x.y.z] - YYYY-MM-DD` section at the top of `CHANGELOG.md`, above the previous release — **never edit or delete an existing entry**, only add new ones. Commit this as its own `chore/bump-version-x.y.z` branch, PR into `develop`, merge (squash is fine — this is a `develop`-target PR, not `main`).

2. **Open a PR from `develop` into `main`:**

   ```bash
   git checkout develop && git pull origin develop
   gh pr create --base main --head develop --title "Release: vX.Y.Z"
   ```

3. **Wait for CI, then merge.** Prefer a real merge commit over squash (see [`CONTRIBUTING.md`](../CONTRIBUTING.md)'s "Merge strategy" section for why, and why it's a preference rather than a hard requirement now):

   ```bash
   gh pr merge <number> --merge
   ```

   If GitHub reports the PR needs review approval you can't wait on and you're the repo owner, `--admin` bypasses it: `gh pr merge <number> --merge --admin`.

4. **Tag `main` and push the tag:**

   ```bash
   git checkout main && git pull origin main
   git tag -a vX.Y.Z -m "Mutter vX.Y.Z"
   git push origin vX.Y.Z
   ```

   This triggers `.github/workflows/release.yml` — a real GitHub Actions build on a `macos-latest` runner (several minutes), producing a **draft** GitHub Release with the `.dmg` and `latest.json` attached. See [`howto-build-and-sign-a-release.md`](howto-build-and-sign-a-release.md) for what that workflow actually does.

5. **Publish the release** once you've reviewed the draft's assets:

   ```bash
   gh release edit vX.Y.Z --draft=false
   ```

## Verification

```bash
gh release view vX.Y.Z --json isDraft,publishedAt,assets
```

`isDraft` should be `false`, and `assets` should include the `.dmg`, `latest.json`, `.app.tar.gz`, and its `.sig`.

Confirm `main` and `develop` agree on content (they don't need identical commit graphs, just matching files):

```bash
git fetch origin main develop
git diff origin/main origin/develop --stat   # empty output = in sync
```

## Troubleshooting

- **"This branch is out-of-date with the base branch" on the `develop` → `main` PR, even though content matches** — a known GitHub quirk with zero-diff merges (see `CONTRIBUTING.md`'s "History note" in the Merge strategy section). Since `main`'s `required_status_checks.strict` is now off, this is cosmetic — you can merge anyway. If you want to close the gap for a clean ahead/behind count: `git checkout develop && git pull && git merge --no-commit --no-ff origin/main`, confirm `git diff --cached --stat` is empty, commit, then `git push origin <local-branch>:develop`.
- **`gh pr merge` refuses with "the base branch policy prohibits the merge"** — needs review approval. Either get a review, or (as repo owner) add `--admin` to bypass.
- **The merge commit GitHub produces has only one parent** (check with `git log --format="%P" -1 <sha>`) — this happens when there's genuinely nothing new to merge (a zero-diff sync). Harmless; `main` and `develop` still match content-wise. Not worth chasing unless you specifically want a clean ancestor graph.
- **`.dmg` download requires a Gatekeeper right-click → Open** — expected, this project isn't notarized (see [`TODOS.md`](../TODOS.md)'s "OS permissions reset on every new install/update" entry for the deeper mechanism and why that's a deliberate trade-off, not a bug).

## Related

- [`howto-build-and-sign-a-release.md`](howto-build-and-sign-a-release.md) — what actually happens once you tag
- [`CONTRIBUTING.md`](../CONTRIBUTING.md) — the full branching model and merge-strategy conventions
- `TODOS.md`'s "branch-sync process gap" and "OS permissions reset" entries — the full incident history behind several of the troubleshooting notes above
