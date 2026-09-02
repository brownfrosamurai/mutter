# Contributing to Mutter

Thanks for wanting to work on this. Mutter is a solo-maintained project so
far — outside contributions are welcome, but please open an issue before
starting non-trivial work so we can agree on approach first.

## Before you write code

Read these two, in order:

1. **[`docs/tutorial-getting-started.md`](docs/tutorial-getting-started.md)** — clone, build, run, dictate something.
2. **[`docs/reference-architecture.md`](docs/reference-architecture.md)** — the module layout and the core abstractions (`TranscriptionEngine`, `TextProcessor`, `PermissionGate<T>`).

`CLAUDE.md` is the project's full dated history of *why* things are the way
they are — useful when you're wondering "why is it built like this," not a
place to start.

## Hard constraints (a PR that violates these will be closed, not debated)

- **No Swift, no AppKit code.** Everything UI-facing is a Tauri webview.
  Native macOS APIs are called from Rust, never hand-written Swift/Obj-C —
  the one exception is a build-time Objective-C shim for ScreenCaptureKit,
  and it stays that way, not a precedent for more.
- **Frontend is React + TypeScript** (`frontend/`), no other framework.
- **Zero network calls after the model download.** No telemetry, no remote
  crash reporting, ever. This is the whole pitch — don't add a "just this
  once" exception.
- **Zero paid resources.** No paid Apple Developer notarization, no paid
  third-party services, in any dependency you add.
- **No payment, account, or licensing system.** Ever.
- Toggle hotkey, not push-to-talk — press once to start, press again to stop.
- Language is auto-detected from audio, never manually selected in settings.

See `CLAUDE.md`'s "Hard constraints" section for the canonical, currently-true
version of this list if this file ever drifts.

## Branching & PR workflow

`develop` is the default branch and where all work lands first. `main` only
ever receives tagged, release-ready code — never a direct feature merge.

```
main      ─●──────────────●───────────────●──────  tagged releases only
            \             / \             /
release/x.y  ●───●───●───●   ●───●───●───●          only when stabilizing a release
                            \
develop   ──●──●──●──●──●──●──●──●──●──●──●──●───  default branch, integration
              \    /    \    /    \    /
feature/*      ●──●      ●──●      ●──●             branch from develop, PR back to develop
```

| Branch prefix | Branch from | PR target | Use |
|---|---|---|---|
| `feature/<slug>` | `develop` | `develop` | new functionality, enhancements, visual/design work |
| `fix/<slug>` | `develop` | `develop` | non-urgent bug fixes |
| `chore/<slug>` | `develop` | `develop` | deps, config, docs-only changes |
| `release/<version>` | `develop` | `main` (then back-merge to `develop`) | stabilizing a release; skip for a trivial version bump — merge `develop` → `main` directly instead |
| `hotfix/<slug>` | `main` | `main` (then back-merge to `develop`) | urgent production fix that can't wait for `develop`'s current state |

Naming: `type/short-kebab-description`, GitHub issue number prefix
encouraged where one exists (`feature/42-fts-search`).

**Merge strategy** — GitHub doesn't enforce this per-branch, so it's a
convention, not a technical gate: **squash merge** `feature/`/`fix/`/`chore/`
PRs into `develop` (one clean commit per change). Use a real **merge commit**
(not squash) for **any PR into `main`** — `release/`/`hotfix/` branches and
the trivial-version-bump `develop` → `main` direct merge alike — so the
release branch's own history is preserved on `main` for later reference.

This isn't just a history-tidiness preference: `main`'s branch protection
requires the PR branch be up to date with `main` before merging
(`required_status_checks.strict`). A **squash** merge into `main` creates a
brand-new commit that only exists on `main` — it never becomes an ancestor
of `develop` — so the *next* `develop` → `main` PR gets blocked as
"out of date" even though the content already matches. A real merge commit
keeps `develop`'s tip as a direct parent of `main`'s new tip, which avoids
this. If it happens anyway, fix it by merging `main`'s tip back into
`develop` (verify zero content diff first with
`git merge --no-commit --no-ff origin/main`) — but note `develop`'s own
`required_linear_history` protection hides "create a merge commit" in
GitHub's PR UI, so that specific sync has to be a direct
`git push origin <local-branch>:develop` of a manually-built merge commit,
not a PR merge button.

**`release/`/`hotfix/` branches merge into two targets** (`main` and
`develop`) — don't let GitHub's auto-delete-on-merge remove the branch after
the *first* merge, or the second merge has nothing to point to. Merge into
`develop` first, confirm it landed, then merge (or cherry-pick equivalent
commits) into `main`, tag the release, and delete the branch last.

Both `main` and `develop` require: a PR (no direct pushes), the `check` and
`frontend` CI status checks passing, the branch up to date with its target,
and all review conversations resolved. As the repo owner you can merge your
own PRs once CI is green without waiting on a second approver; PRs from
anyone else need your explicit approval first.

## Development

```bash
npm install --prefix frontend
cd src-tauri && cargo tauri dev
```

Full walkthrough (permissions, first dictation): `docs/tutorial-getting-started.md`.

## Before opening a PR

```bash
cd src-tauri
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

```bash
cd frontend
npm run build   # tsc -b && vite build — must be zero TS errors
```

All four are required CI gates (`.github/workflows/ci.yml`) — a PR that
doesn't pass them won't merge. If you add backend behavior, add a test for
it in the same PR; this codebase's convention is real unit tests per
module, not a smoke-test-only suite.

If you change a `#[tauri::command]` signature, regenerate the frontend
bindings before committing:

```bash
cd src-tauri && cargo test --lib export_bindings -- --ignored
```

## Scope

Check [`TODOS.md`](TODOS.md) for known open items before starting something
new — several are human-verification-gated (need someone to click through a
real permission dialog) rather than code problems, and are good first
contributions if you have a Mac to test on. Features outside the current
plan of record (`docs/mutter-project-plan.md`) are worth an issue/discussion
before a PR — this keeps the project's scope intentional rather than
accretive.

## Reporting bugs / requesting features

Open a GitHub issue. For anything security-relevant (a permission bypass, a
way to exfiltrate dictated text, etc.), see [`SECURITY.md`](SECURITY.md)
instead of a public issue.
