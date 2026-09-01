# Why permissions are one generic `PermissionGate<T>`, not three

Mutter needs three independent macOS permissions — microphone (for dictation), Accessibility (for text injection), and Screen Recording (for system-audio capture, which macOS gates behind the screen-capture permission even though no video is ever captured). `permissions.rs` implements all three as instantiations of one generic type, `PermissionGate<T>`, rather than three separate hand-rolled state machines.

## The problem this avoids

An early draft of this module (per an eng-review finding) had three near-identical structs — `MicPermission`, `AccessibilityPermission`, `SystemAudioPermission` — each with its own copy of the same four-state enum and the same `is_granted()`/`needs_recovery_ui()` logic. Three copies of near-identical logic is exactly the shape of bug that shows up later as a fix applied to one copy and forgotten in the other two. The generic fix costs nothing at the call site — `PermissionGate<Mic>` reads identically to a dedicated `MicPermission` would have — but a fourth permission family, if this app ever needs one, costs one new zero-sized marker type plus one instantiation, not a new state machine.

```rust
pub struct Mic;
pub struct Accessibility;
pub struct SystemAudio;

pub struct PermissionGate<T> {
    state: PermissionState,
    _kind: PhantomData<T>,
}
```

`T` carries no data — `Mic`/`Accessibility`/`SystemAudio` exist only to make `PermissionGate<Mic>` and `PermissionGate<Accessibility>` distinct types at compile time, so a caller can never accidentally pass one kind's gate where another's was expected. The shared state (`PermissionState::{NotRequested, Denied, Granted, Unavailable}`) and its two derived queries (`is_granted()`, `needs_recovery_ui()`) live once, on the generic type.

## Why `refresh()` is not generic

Each permission family's underlying OS query is genuinely different — AVFoundation's `AVAuthorizationStatus` for mic, `AXIsProcessTrusted()` for Accessibility, `CGPreflightScreenCaptureAccess()` for Screen Recording — so `refresh()` is implemented once per concrete instantiation (`impl PermissionGate<Mic>`, `impl PermissionGate<Accessibility>`, `impl PermissionGate<SystemAudio>`), not as one generic method dispatching on `T`. This isn't a compromise on the DRY goal above: the *state machine* (what the four states mean, when recovery UI is needed) is what was actually duplicated before; the OS-specific query was always going to differ per permission and gains nothing from being forced through one shared code path.

## Why `Unavailable` exists separately from `Denied`

`PermissionState::Unavailable` covers device-level problems distinct from a user's deliberate denial: no microphone present, or (for Accessibility) `Restricted` status from parental controls or MDM policy — situations a "click Grant" UI can't actually fix, unlike a plain `Denied`, which System Settings can resolve. `needs_recovery_ui()` treats both the same way today (both surface a recovery path), but keeping them as distinct states leaves room for the UI to eventually say something more specific than "denied" when the real answer is "this Mac has no microphone."

## Why mic gets an active `request()` and the other two don't

Mic is the only one of the three with an OS-level "show me the permission prompt" API (`AVCaptureDevice.requestAccessForMediaType:completionHandler:`, wrapped by the `permissions_shim.m` native shim). Accessibility and Screen Recording have no equivalent active-request call on macOS — the only way to grant either is a deep link to System Settings' matching pane (`open_permission_settings` in `lib.rs`), which the user has to act on manually. `PermissionGate<Mic>::request()` exists as a genuinely separate capability from `refresh()` for exactly this reason; the other two gates only ever `refresh()`.

## Related

- [`reference-architecture.md`](reference-architecture.md)
- [`reference-commands.md`](reference-commands.md) — `get_permission_status`, `open_permission_settings`, `request_mic_access`
