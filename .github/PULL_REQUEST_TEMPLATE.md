## What this changes and why

<!-- The "why" matters more than the "what" — link an issue if one exists. -->

## Checklist

- [ ] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` all pass (`src-tauri/`)
- [ ] `npm run build` passes with zero TypeScript errors (`frontend/`)
- [ ] New backend behavior has a test in the same PR
- [ ] If a `#[tauri::command]` signature changed, bindings were regenerated (`cargo test --lib export_bindings -- --ignored`)
- [ ] This doesn't violate any hard constraint in `CONTRIBUTING.md` (no Swift/AppKit, no new network calls, no paid resources, no accounts/payments)
- [ ] If this is UI-facing, it was actually run and screenshotted/verified in the real app, not just type-checked

## Testing

<!-- How did you verify this works? -->
