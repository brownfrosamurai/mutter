//! Two independent capture modes sharing the same downstream pipeline: mic
//! dictation (default) and system-audio loopback (opt-in, Granola-style).
//! Only one is active at a time per recording. See
//! docs/mutter-project-plan.md Section 3.

pub mod mic;
pub mod system_audio;
