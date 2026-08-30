//! Text insertion at the cursor. STUB — Phase 1/2 (core loop + terminal
//! validation) work.
//!
//! Primary path: macOS Accessibility API (`AXUIElement`), setting `AXValue`
//! directly where the target element supports it. Fallback: clipboard swap +
//! synthetic paste for apps that don't expose a settable `AXValue` — notably
//! terminal emulators (Terminal.app, iTerm2), which render text via custom
//! drawing rather than a standard AX text value. The fallback MUST save and
//! restore the user's original clipboard contents.
//!
//! Terminal injection is the single most load-bearing integration point for
//! this app's primary named use case (dictating to an AI-agent REPL) — it is
//! validated in Phase 2, not deferred to a later manual QA pass. See
//! docs/mutter-project-plan.md Section 15, Phase 2.
//!
//! "Ready to be pasted" in the original spec is interpreted here as direct
//! auto-insertion, with clipboard-paste as the literal fallback path — a
//! flagged interpretive choice, see docs/mutter-project-plan.md Section 2.

#[derive(Debug, thiserror::Error)]
pub enum InjectionError {
    #[error("no focused element to insert into")]
    NoFocusedElement,
    #[error("AXValue not settable on target, and clipboard fallback failed: {0}")]
    FallbackFailed(String),
}

pub enum InjectionMethod {
    /// Direct `AXValue` set on the focused element.
    Accessibility,
    /// Clipboard swap + synthetic Cmd+V, original clipboard restored after.
    ClipboardFallback,
}

/// Insert `text` at the current cursor position. Empty/whitespace-only text
/// is the caller's responsibility to filter before calling this (see
/// docs/mutter-project-plan.md Section 3 — nothing is pasted, pill clears
/// silently, on an empty transcription result).
pub fn insert_at_cursor(_text: &str) -> Result<InjectionMethod, InjectionError> {
    unimplemented!("insert_at_cursor — Phase 1/2 core loop work")
}
