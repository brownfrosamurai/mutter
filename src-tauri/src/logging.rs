//! Local-only, metadata-only structured logging via `tracing`. No remote
//! transmission — there is no crash-reporting service in v1 (Sentry or
//! equivalent was in the first draft of the plan and was deliberately
//! removed; see docs/mutter-project-plan.md Section 11). Logs must never
//! contain audio, transcript text, or history content — only timing, error
//! codes, model/engine identifiers, and OS version.
//!
//! Native FFI/bridge calls (whisper-rs, the ScreenCaptureKit bridge) are
//! wrapped in `std::panic::catch_unwind` at their call sites (not here) —
//! this module just gives them somewhere safe to log the caught panic.

use tracing_subscriber::EnvFilter;

/// Initialize the local log file. Call once, at app startup.
pub fn init() {
    // TODO(Phase 1): write to a rotated file under
    // ~/Library/Application Support/Mutter/logs/, not just stdout.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}
