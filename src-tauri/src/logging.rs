//! Local-only, metadata-only structured logging via `tracing`. No remote
//! transmission — there is no crash-reporting service in v1 (Sentry or
//! equivalent was in the first draft of the plan and was deliberately
//! removed; see docs/mutter-project-plan.md Section 11). Logs must never
//! contain audio, transcript text, or history content — only timing, error
//! codes, model/engine identifiers, and OS version. Every `tracing::*!` call
//! elsewhere in this crate was written with that constraint in mind — grep
//! for ones that log `text`/`audio` fields before adding new call sites.
//!
//! Writes to both stdout (so `cargo tauri dev` output stays useful) and a
//! daily-rotating file under
//! `~/Library/Application Support/Mutter/logs/mutter.log.<date>`, via a
//! non-blocking writer so logging I/O never stalls the hotkey/session/
//! capture threads that call into it.
//!
//! Native FFI/bridge calls (whisper-rs, the ScreenCaptureKit bridge) are
//! wrapped in `std::panic::catch_unwind` at their call sites (not here) —
//! this module just gives them somewhere safe to log the caught panic.

use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::EnvFilter;

/// Holds the non-blocking writer's flush guard for the process lifetime —
/// dropping it would stop the background writer thread and lose buffered
/// log lines. `init()` is called once at startup and never again, so a
/// `OnceLock` (rather than leaking the guard) is enough to keep it alive
/// without `unsafe`.
static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Initialize logging. Call once, at app startup.
pub fn init() {
    let log_dir = match crate::paths::app_support_subdir("logs") {
        Ok(dir) => dir,
        Err(e) => {
            // Nowhere better to report this than stderr directly — the
            // logging subsystem itself is what failed to set up.
            eprintln!("mutter: could not create logs directory ({e}), logging to stdout only");
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .init();
            return;
        }
    };

    let file_appender = tracing_appender::rolling::daily(log_dir, "mutter.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    // If `init()` is ever accidentally called twice, keep the first
    // guard/subscriber rather than panicking or silently swapping loggers.
    let _ = LOG_GUARD.set(guard);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(non_blocking.and(std::io::stdout))
        .init();
}
