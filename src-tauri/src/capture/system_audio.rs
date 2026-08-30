//! System-audio (speaker) capture via `ScreenCaptureKit`, "like Granola" per
//! the original spec (docs/mutter-idea-dump.md), through the C ABI shim in
//! `native/system_audio_shim.{h,m}` (compiled by `build.rs`).
//!
//! **Phase 0 spike result: resolved in favor of a minimal build-time
//! Objective-C shim**, not an `objc2`-based crate bridge
//! (docs/mutter-project-plan.md Section 9/15). The shim compiles cleanly
//! against the real macOS SDK headers (`clang -fobjc-arc`, verified
//! 2026-08-29) and type-checks against ScreenCaptureKit's actual API
//! surface (`SCStream`/`SCStreamConfiguration`/`SCContentFilter`, the
//! documented `CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer`
//! path for pulling PCM out of an audio `CMSampleBuffer`).
//!
//! **What is NOT verified**: actual runtime behavior. Exercising this
//! requires the user to grant Screen Recording permission via a real macOS
//! system dialog — nothing but a human clicking through that can confirm
//! it. This environment also has no way to independently confirm the
//! audio-only capability shape claim in Section 9 (no video-frame capture
//! overhead) beyond configuring the smallest possible video surface
//! (2x2, 1fps ceiling) in the shim — that's a mitigation, not a measurement.
//!
//! Buffer cap: 300s (longer than mic's 120s, given meeting-length use).

use std::ffi::{c_char, c_void, CStr};
use std::os::raw::c_int;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub const MAX_DURATION_SECS: u64 = 300;
const SAMPLE_RATE: i32 = 16_000;
const CHANNEL_COUNT: i32 = 1;
const CAP_SAMPLES: usize =
    MAX_DURATION_SECS as usize * SAMPLE_RATE as usize * CHANNEL_COUNT as usize;

#[repr(C)]
struct MutterSckCapture {
    _private: [u8; 0],
}

type SamplesCallback = extern "C" fn(*const f32, usize, *mut c_void);
type StopCallback = extern "C" fn(*const c_char, *mut c_void);

extern "C" {
    fn mutter_sck_start(
        sample_rate: c_int,
        channel_count: c_int,
        on_samples: SamplesCallback,
        on_stop: StopCallback,
        user_data: *mut c_void,
        error_out: *mut c_char,
        error_out_len: usize,
    ) -> *mut MutterSckCapture;

    fn mutter_sck_stop(handle: *mut MutterSckCapture);
}

#[derive(Debug, thiserror::Error)]
pub enum SystemAudioCaptureError {
    #[error("ScreenCaptureKit capture failed to start: {0}")]
    StartFailed(String),
}

struct SharedState {
    samples: Mutex<Vec<f32>>,
    at_cap: AtomicBool,
}

extern "C" fn on_samples_trampoline(samples: *const f32, count: usize, user_data: *mut c_void) {
    if samples.is_null() || user_data.is_null() {
        return;
    }
    // SAFETY: `user_data` is a pointer from `Arc::into_raw` in `start()`,
    // kept alive by that leaked reference for as long as the native handle
    // exists — `mutter_sck_stop` guarantees no further callbacks after it
    // returns, and that's the only place this reference is reclaimed.
    let state = unsafe { &*(user_data as *const SharedState) };
    let slice = unsafe { std::slice::from_raw_parts(samples, count) };

    if state.at_cap.load(Ordering::Relaxed) {
        return;
    }
    let mut buf = state
        .samples
        .lock()
        .expect("system-audio buffer lock poisoned");
    if buf.len() >= CAP_SAMPLES {
        state.at_cap.store(true, Ordering::Relaxed);
        return;
    }
    buf.extend_from_slice(slice);
    if buf.len() >= CAP_SAMPLES {
        state.at_cap.store(true, Ordering::Relaxed);
    }
}

extern "C" fn on_stop_trampoline(error: *const c_char, _user_data: *mut c_void) {
    if error.is_null() {
        return;
    }
    // SAFETY: the shim guarantees a valid, NUL-terminated C string when
    // non-null (see system_audio_shim.m's mutter_sck_copy_error).
    let message = unsafe { CStr::from_ptr(error) }.to_string_lossy();
    tracing::error!(error = %message, "ScreenCaptureKit stream stopped with error");
}

pub struct SystemAudioCapture {
    handle: Option<*mut MutterSckCapture>,
    state: Option<Arc<SharedState>>,
    /// The exact pointer handed to the native side as `user_data` — kept
    /// separate from `state` (our own independent `Arc` clone) so
    /// start/stop's reference bookkeeping is unambiguous: exactly one
    /// `Arc::into_raw` here, paired with exactly one `Arc::from_raw` (in
    /// `stop()`, or in `start()`'s own failure path).
    user_data_ptr: Option<*mut c_void>,
}

// SAFETY: the raw `handle`/`user_data_ptr` pointers are only ever passed to
// `mutter_sck_start`/`mutter_sck_stop`, which don't rely on thread-local
// state — the shim's own doc contract is "callable from any thread, blocks
// until the async ScreenCaptureKit setup/teardown completes". Actual
// mutation of shared audio data happens through `SharedState`'s `Mutex`.
unsafe impl Send for SystemAudioCapture {}

impl SystemAudioCapture {
    pub fn new() -> Self {
        Self {
            handle: None,
            state: None,
            user_data_ptr: None,
        }
    }

    pub fn is_at_cap(&self) -> bool {
        self.state
            .as_ref()
            .map(|s| s.at_cap.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    /// Blocks the calling thread until ScreenCaptureKit's async setup
    /// completes or fails — including however long the Screen Recording
    /// permission prompt takes to resolve. Call from a dedicated thread,
    /// mirroring `capture::mic::MicCapture`'s design, never from the async
    /// runtime.
    pub fn start(&mut self) -> Result<(), SystemAudioCaptureError> {
        let state = Arc::new(SharedState {
            samples: Mutex::new(Vec::new()),
            at_cap: AtomicBool::new(false),
        });
        let user_data_ptr = Arc::into_raw(state.clone()) as *mut c_void;

        let mut error_buf = [0u8; 512];
        // Native FFI call into the Objective-C shim — CLAUDE.md requires
        // this guarded so a panic can never take down the whole app.
        // ScreenCaptureKit itself reports failures through `error_buf`, not
        // by panicking/throwing, so this is defense-in-depth.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            mutter_sck_start(
                SAMPLE_RATE,
                CHANNEL_COUNT,
                on_samples_trampoline,
                on_stop_trampoline,
                user_data_ptr,
                error_buf.as_mut_ptr() as *mut c_char,
                error_buf.len(),
            )
        }));

        let handle = match result {
            Ok(h) => h,
            Err(_) => {
                unsafe { drop(Arc::from_raw(user_data_ptr as *const SharedState)) };
                return Err(SystemAudioCaptureError::StartFailed(
                    "native shim panicked".into(),
                ));
            }
        };

        if handle.is_null() {
            unsafe { drop(Arc::from_raw(user_data_ptr as *const SharedState)) };
            let message = CStr::from_bytes_until_nul(&error_buf)
                .map(|c| c.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(SystemAudioCaptureError::StartFailed(message));
        }

        self.handle = Some(handle);
        self.state = Some(state);
        self.user_data_ptr = Some(user_data_ptr);
        Ok(())
    }

    /// Stop capturing and return 16kHz mono f32 PCM, ready for
    /// `TranscriptionEngine::transcribe` — same contract as
    /// `MicCapture::stop`.
    pub fn stop(&mut self) -> Vec<f32> {
        if let Some(handle) = self.handle.take() {
            // Blocks until ScreenCaptureKit confirms the stream stopped —
            // after this, no more callbacks will fire.
            unsafe { mutter_sck_stop(handle) };
        }
        if let Some(ptr) = self.user_data_ptr.take() {
            unsafe { drop(Arc::from_raw(ptr as *const SharedState)) };
        }
        let Some(state) = self.state.take() else {
            return Vec::new();
        };
        let samples = std::mem::take(
            &mut *state
                .samples
                .lock()
                .expect("system-audio buffer lock poisoned"),
        );
        samples
    }
}

impl Default for SystemAudioCapture {
    fn default() -> Self {
        Self::new()
    }
}
