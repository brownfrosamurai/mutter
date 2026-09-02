//! One generic permission state machine, shared by mic, Accessibility, and
//! system-audio permissions — not three hand-rolled implementations. A DRY
//! finding from eng review (docs/mutter-project-plan.md Section 11, Code
//! Quality Issue 4): three near-identical state machines is a violation
//! waiting to happen, and a future fourth permission family should cost one
//! instantiation, not a new implementation.

use std::marker::PhantomData;

/// State of a single permission. `Unavailable` covers device-level problems
/// distinct from a user denial: mic not present, disconnected mid-capture,
/// held exclusively by another app (mic); or the equivalent for other
/// permission kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    NotRequested,
    Denied,
    Granted,
    Unavailable,
}

/// Marker types for each permission family this app cares about. Adding a
/// fourth family is one new zero-sized marker type plus one
/// `PermissionGate<NewKind>` instantiation — not a new state machine.
pub struct Mic;
pub struct Accessibility;
pub struct SystemAudio;

/// A permission gate for one permission kind `T`. `T` carries no data — it
/// exists only to make `PermissionGate<Mic>` and `PermissionGate<Accessibility>`
/// distinct types at compile time, so they can't be mixed up by accident.
pub struct PermissionGate<T> {
    state: PermissionState,
    _kind: PhantomData<T>,
}

impl<T> PermissionGate<T> {
    pub fn new() -> Self {
        Self {
            state: PermissionState::NotRequested,
            _kind: PhantomData,
        }
    }

    pub fn state(&self) -> PermissionState {
        self.state
    }

    pub fn set_state(&mut self, state: PermissionState) {
        self.state = state;
    }

    pub fn is_granted(&self) -> bool {
        matches!(self.state, PermissionState::Granted)
    }

    /// True for any state that should surface a recoverable UI path (deep
    /// link to System Settings, or a clear "unavailable" message) rather
    /// than a silent failure.
    pub fn needs_recovery_ui(&self) -> bool {
        matches!(
            self.state,
            PermissionState::Denied | PermissionState::Unavailable
        )
    }
}

impl<T> Default for PermissionGate<T> {
    fn default() -> Self {
        Self::new()
    }
}

// --- Real OS-backed status checks ---
//
// One `refresh()` per kind rather than a generic dispatch — `T` carries no
// data to dispatch on (it's a zero-sized marker), and each permission
// family's underlying OS API is genuinely different (AVFoundation,
// ApplicationServices' AX API, CoreGraphics' screen-capture preflight).
// Adding a fourth family per the module doc's own framing is still "one new
// marker type plus one `PermissionGate<NewKind>` instantiation" — this is
// just where that instantiation's OS-specific edge lives.

extern "C" {
    /// Defined in native/permissions_shim.m. Mirrors `AVAuthorizationStatus`
    /// exactly: 0 = NotDetermined, 1 = Restricted, 2 = Denied, 3 = Authorized.
    fn mutter_mic_auth_status() -> i32;

    /// Defined in native/permissions_shim.m. Shows the real native mic
    /// permission prompt and blocks until answered (or returns immediately
    /// if already answered). Returns 1 if granted, 0 otherwise. Callers
    /// MUST NOT invoke this on the main thread — see the shim header.
    fn mutter_request_mic_access() -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    /// Plain C entry point — no Objective-C shim needed for this one.
    fn CGPreflightScreenCaptureAccess() -> bool;

    /// Plain C entry point, same as its preflight sibling above — shows a
    /// real system alert on a never-before-asked app and registers it in
    /// System Settings' Screen Recording list either way. Real-world
    /// caveat, verified via web search against Apple dev forum reports and
    /// a real open-source TCC/ScreenCaptureKit bug tracker (not assumed):
    /// on modern macOS this often does NOT show an interactive dialog at
    /// all — it mostly just ensures the app is listed (off by default),
    /// which is still real value on top of the "never appears in Settings"
    /// bug this exact permission family already had for mic. Must run on
    /// the main thread (dispatched via `run_on_main_thread` below) — same
    /// defensive posture as Accessibility's request below, since this is
    /// otherwise a plain synchronous call with no completion-handler
    /// bridge to force main-thread affinity.
    fn CGRequestScreenCaptureAccess() -> bool;
}

// Explicit framework link (adversarial review, 2026-09-01): this symbol
// currently resolves only because `accessibility_sys` (a separate crate)
// transitively links ApplicationServices as a side effect of its own
// AXIsProcessTrusted binding — real today, but fragile if that dependency
// ever changes. Declared directly here, matching the explicit
// `#[link(name = "CoreGraphics", ...)]` already used for the
// CGPreflightScreenCaptureAccess/CGRequestScreenCaptureAccess block above.
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    #[link_name = "AXIsProcessTrustedWithOptions"]
    fn ax_is_process_trusted_with_options(
        options: core_foundation::dictionary::CFDictionaryRef,
    ) -> bool;
}

/// Runs `f` on the main thread/run loop and returns its result, blocking the
/// calling thread until it completes. Required for Accessibility's and
/// Screen Recording's active-request calls for the same reason mic's
/// request needed main-thread dispatch (`native/permissions_shim.m`'s own
/// module doc): TCC's UI-presentation path silently declines to show
/// anything when a permission request doesn't originate on the main
/// thread/run loop — confirmed live for AVFoundation's callback-based API,
/// applied defensively here too since neither of these two has a
/// completion-handler bridge to force it through. Simpler than mic's own
/// `dispatch_async` + semaphore bridge (`mutter_request_mic_access`)
/// specifically because these two are plain synchronous calls with no
/// callback to wait on — `exec_sync` alone gives a genuinely synchronous
/// round trip.
///
/// SAFETY INVARIANT: MUST NEVER be called from the main thread itself —
/// `exec_sync` onto the main queue from the main thread is a deadlock
/// (dispatch_sync waiting on the queue it's already running on). Every
/// caller in this file already runs off the main thread via
/// `tauri::async_runtime::spawn_blocking` in `lib.rs`'s `request_permission`
/// command — never call `PermissionGate::request()` directly from a
/// synchronous Tauri command or any other main-thread context.
///
/// PANIC SAFETY: `f` is run inside its own `catch_unwind`, entirely on the
/// main thread, before `exec_sync` returns — not left to unwind out of this
/// function. This matters because `dispatch2` 0.3.1's `exec_sync` invokes
/// the dispatched closure through `extern "C" fn function_wrapper` (verified
/// by reading `dispatch2-0.3.1/src/utils.rs`/`queue.rs` directly), which has
/// no `catch_unwind` of its own; a panic that tried to unwind out of `f`
/// across that boundary would cross a non-unwind-safe FFI edge and abort the
/// whole process, silently defeating every caller's outer
/// `catch_native_panic()` (which runs on the *calling* thread and would
/// never get a chance to catch anything, since the process aborts on the
/// main thread first) — a real gap caught by review, not a hypothetical
/// one. Catching here and `resume_unwind`ing back on the calling thread
/// (after `exec_sync` has returned, so no FFI boundary is involved) hands
/// the panic to `catch_native_panic()` as originally intended.
fn run_on_main_thread<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send + std::panic::UnwindSafe,
    R: Send + Default,
{
    let mut result: Option<std::thread::Result<R>> = None;
    let result_ref = &mut result;
    dispatch2::DispatchQueue::main().exec_sync(move || {
        *result_ref = Some(std::panic::catch_unwind(f));
    });
    match result {
        Some(Ok(value)) => value,
        Some(Err(payload)) => std::panic::resume_unwind(payload),
        None => unreachable!("exec_sync is synchronous — the closure above always runs"),
    }
}

/// Wraps a native FFI/bridge call in `catch_unwind`, matching this
/// project's hard constraint (CLAUDE.md: "a panic must never take down the
/// whole app") and this file's siblings (`injection.rs`, `system_audio.rs`,
/// `whisper.rs`, `llm_cleanup.rs` all already do this) — a gap specific to
/// `permissions.rs` until this pass (grep confirmed zero prior
/// `catch_unwind` usage here, the only native-FFI-touching file in the
/// codebase without it). On panic, logs and returns `R::default()` rather
/// than propagating — a permission check/request failing safe (denied/
/// unavailable) is always the right fallback, never a crash.
fn catch_native_panic<F, R>(f: F) -> R
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
    R: Default,
{
    match std::panic::catch_unwind(f) {
        Ok(value) => value,
        Err(_) => {
            tracing::error!("native permission call panicked, treating as failed");
            R::default()
        }
    }
}

impl PermissionGate<Mic> {
    /// Query the real microphone authorization status from AVFoundation and
    /// update `self` to match. `Restricted` (parental controls/MDM, not a
    /// user choice) maps to `Unavailable` per this module's own
    /// distinction between "device-level problem" and "user denial".
    pub fn refresh(&mut self) {
        let status = catch_native_panic(|| unsafe { mutter_mic_auth_status() });
        self.state = match status {
            0 => PermissionState::NotRequested,
            1 => PermissionState::Unavailable,
            2 => PermissionState::Denied,
            3 => PermissionState::Granted,
            other => {
                tracing::warn!(status = other, "unexpected AVAuthorizationStatus value");
                PermissionState::Unavailable
            }
        };
    }

    /// Shows the real native mic permission prompt (a one-time system
    /// dialog — a no-op returning the existing status if already answered)
    /// and updates `self` to match the real post-prompt status. Blocking —
    /// callers MUST run this off the main/UI thread (see
    /// `lib.rs`'s `request_permission` command, which wraps it in
    /// `tauri::async_runtime::spawn_blocking`).
    ///
    /// Deliberately untested, unlike its siblings' `_does_not_panic` smoke
    /// tests below (review finding): a real test would trigger the actual
    /// system dialog and hang CI waiting for a human to answer it. See
    /// `TODOS.md`'s "Onboarding's mic native-prompt path" entry — this
    /// exact path is already flagged as needing live human verification,
    /// which is a stronger check than a CI smoke test could give anyway.
    pub fn request(&mut self) -> bool {
        let granted = catch_native_panic(|| unsafe { mutter_request_mic_access() != 0 });
        self.refresh();
        granted
    }
}

impl PermissionGate<Accessibility> {
    /// `AXIsProcessTrusted` has no third "not yet asked" state the way mic
    /// permission does — untrusted is untrusted regardless of whether the
    /// user has ever seen the prompt (the first `AXUIElement` call made
    /// while untrusted, in injection.rs, is itself what triggers it).
    pub fn refresh(&mut self) {
        let trusted = catch_native_panic(|| unsafe { accessibility_sys::AXIsProcessTrusted() });
        self.state = if trusted {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        };
    }

    /// Shows the real native "trust this app" alert via
    /// `AXIsProcessTrustedWithOptions` with `kAXTrustedCheckOptionPrompt`
    /// set — unlike mic, this alert CAN reappear on repeat calls even after
    /// a prior denial (Accessibility has no true one-shot semantics), which
    /// is exactly why the frontend's re-entry guard checks real permission
    /// status before calling this rather than relying on component-local
    /// state (see `Ready.tsx`'s module doc). Main-thread dispatched — see
    /// `run_on_main_thread`'s doc for why.
    pub fn request(&mut self) -> bool {
        catch_native_panic(|| {
            run_on_main_thread(|| unsafe {
                use core_foundation::{
                    base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, string::CFString,
                };
                let prompt_key =
                    CFString::wrap_under_get_rule(accessibility_sys::kAXTrustedCheckOptionPrompt);
                let prompt_value = CFBoolean::true_value();
                let options = CFDictionary::from_CFType_pairs(&[(prompt_key, prompt_value)]);
                ax_is_process_trusted_with_options(options.as_concrete_TypeRef())
            })
        });
        // AXIsProcessTrustedWithOptions's own return value mirrors
        // AXIsProcessTrusted's current (pre-prompt-resolution) status, not
        // the user's eventual answer — the alert, if shown, resolves
        // asynchronously outside this call. Always refresh from the real
        // API afterward rather than trusting the return value, matching
        // mic's own established pattern above.
        self.refresh();
        self.is_granted()
    }
}

impl PermissionGate<SystemAudio> {
    pub fn refresh(&mut self) {
        let granted = catch_native_panic(|| unsafe { CGPreflightScreenCaptureAccess() });
        self.state = if granted {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        };
    }

    /// Registers the app in System Settings' Screen Recording list and
    /// returns current status — real-world caveat documented on the extern
    /// declaration above: on modern macOS this often does not show an
    /// interactive dialog, but it does fix the same "app never appears in
    /// Settings at all" bug this permission family already had for mic.
    /// Main-thread dispatched defensively, same reasoning as Accessibility's
    /// request above — see `run_on_main_thread`'s doc.
    pub fn request(&mut self) -> bool {
        catch_native_panic(|| run_on_main_thread(|| unsafe { CGRequestScreenCaptureAccess() }));
        self.refresh();
        self.is_granted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `catch_native_panic` itself (new this session, wraps every native
    // FFI/bridge call in this file per CLAUDE.md's hard constraint "a panic
    // must never take down the whole app") is pure Rust logic with no FFI
    // or main-thread dispatch involved — unlike `run_on_main_thread`, it
    // doesn't need a real run loop, so it's safe to exercise directly here,
    // not just live-verified. Regression-guards the fail-safe contract: a
    // panicking native call must degrade to `R::default()` (denied/
    // unavailable), never propagate and crash the app.
    #[test]
    fn catch_native_panic_returns_default_on_panic() {
        let result: bool = catch_native_panic(|| panic!("simulated native panic"));
        assert!(
            !result,
            "a panicking native call must fail safe to false, not propagate"
        );
    }

    #[test]
    fn catch_native_panic_passes_through_the_real_value_on_success() {
        let result: bool = catch_native_panic(|| true);
        assert!(result);
    }

    // Regression-guards `run_on_main_thread`'s panic-safety fix (pre-landing
    // review, 2026-09-01): a panic caught via `catch_unwind` and re-raised
    // via `resume_unwind` on a *different* stack frame must still be
    // catchable by an outer `catch_unwind` there — this is exactly the
    // catch-inside/resume-outside roundtrip `run_on_main_thread` relies on
    // to hand a panic from the main-thread-dispatched closure back to the
    // calling thread's `catch_native_panic()`, without ever letting it
    // unwind across `dispatch2`'s non-unwind-safe `extern "C"` boundary.
    // Doesn't exercise `dispatch2`/a real main-thread dispatch (that stays
    // live-verification-only, same as before, per the doc comments above)
    // — this isolates and proves just the unwind mechanism itself.
    #[test]
    fn resume_unwind_after_catch_unwind_is_still_catchable_by_an_outer_catch_unwind() {
        let inner: std::thread::Result<()> = std::panic::catch_unwind(|| panic!("simulated"));
        let payload = inner.expect_err("the inner closure panicked");

        let outer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            std::panic::resume_unwind(payload)
        }));

        assert!(
            outer.is_err(),
            "a panic caught then resume_unwound must still be catchable by an outer catch_unwind"
        );
    }

    #[test]
    fn starts_not_requested() {
        let gate: PermissionGate<Mic> = PermissionGate::new();
        assert_eq!(gate.state(), PermissionState::NotRequested);
        assert!(!gate.is_granted());
        assert!(!gate.needs_recovery_ui());
    }

    #[test]
    fn granted_is_granted_and_not_recoverable() {
        let mut gate: PermissionGate<Accessibility> = PermissionGate::new();
        gate.set_state(PermissionState::Granted);
        assert!(gate.is_granted());
        assert!(!gate.needs_recovery_ui());
    }

    #[test]
    fn denied_and_unavailable_both_need_recovery_ui() {
        let mut gate: PermissionGate<SystemAudio> = PermissionGate::new();
        gate.set_state(PermissionState::Denied);
        assert!(gate.needs_recovery_ui());

        gate.set_state(PermissionState::Unavailable);
        assert!(gate.needs_recovery_ui());
    }

    // Smoke tests only — the actual `PermissionState` these produce depends
    // on the CI/dev machine's real TCC state, which these tests
    // deliberately don't assert a specific value for. What they do verify:
    // the FFI calls into the native shim/frameworks don't crash and always
    // leave the gate in a valid (non-panicking-to-construct) state.
    #[test]
    fn mic_refresh_does_not_panic() {
        let mut gate: PermissionGate<Mic> = PermissionGate::new();
        gate.refresh();
        let _ = gate.state();
    }

    #[test]
    fn accessibility_refresh_does_not_panic() {
        let mut gate: PermissionGate<Accessibility> = PermissionGate::new();
        gate.refresh();
        let _ = gate.state();
    }

    #[test]
    fn system_audio_refresh_does_not_panic() {
        let mut gate: PermissionGate<SystemAudio> = PermissionGate::new();
        gate.refresh();
        let _ = gate.state();
    }

    // No `accessibility_request_does_not_panic` test: unlike mic's
    // `request()` (a true one-shot that either shows a prompt once ever or
    // silently no-ops), `AXIsProcessTrustedWithOptions` with the prompt
    // option actively triggers a real "trust this app" system alert on an
    // untrusted process — including an unsigned/untrusted `cargo test`
    // binary. A CI/local test run popping an unexpected system dialog is
    // disruptive even though the call itself doesn't block, so this path
    // gets the same live-verification-only treatment as mic's `request()`
    // above, not a CI smoke test.
    //
    // Also no `system_audio_request_does_not_panic`: a real deadlock was
    // found writing this test, not just assumed — `run_on_main_thread`'s
    // `dispatch2::DispatchQueue::main().exec_sync()` blocks until something
    // actually services the main dispatch queue, which requires a real run
    // loop (`NSApplication`/`CFRunLoopRun`). The shipped Tauri app has one;
    // a plain `cargo test` CLI binary does not, so this call hangs forever
    // there regardless of whether it would show any UI — the same
    // underlying reason mic's `request()` above was already untestable,
    // just via a different mechanism than "triggers a dialog". Both of
    // `PermissionGate<Accessibility>`'s and `PermissionGate<SystemAudio>`'s
    // `request()` methods route through this same main-thread dispatch and
    // are therefore both live-verification-only, not CI-testable — only
    // their `refresh()` methods (no main-thread dispatch, plain synchronous
    // FFI calls) are safe to exercise automatically, per the three
    // `_refresh_does_not_panic` tests above.
}
