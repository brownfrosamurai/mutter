// Minimal C ABI over ScreenCaptureKit's audio-only capture, for Rust FFI.
//
// Deliberately a plain C function-pointer interface, not exposed
// Objective-C classes/blocks — Rust can call C ABI directly without an
// Objective-C runtime bridge crate (objc2 etc.), matching the plan's
// "minimal build-time Objective-C shim" choice over an objc2-based crate
// bridge (docs/mutter-project-plan.md Section 9/15).
//
// `mutter_sck_start` blocks the calling thread until ScreenCaptureKit's
// async setup (shareable-content lookup, then stream start) completes or
// fails — including however long the Screen Recording permission prompt
// takes to resolve. Call it from a dedicated thread, never from the async
// runtime or a UI-handling thread, mirroring capture/mic.rs's own
// dedicated-thread design for the same reason (blocking + non-Send
// concerns).
#pragma once
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*mutter_sck_samples_cb)(const float *samples, size_t count, void *user_data);
typedef void (*mutter_sck_stop_cb)(const char *error_message_or_null, void *user_data);

typedef struct MutterSckCapture MutterSckCapture;

// Starts audio-only system capture (mono unless channel_count > 1).
// Returns NULL on failure and writes a UTF-8, NUL-terminated message into
// `error_out` (a caller-owned buffer of `error_out_len` bytes).
MutterSckCapture *mutter_sck_start(
    int sample_rate,
    int channel_count,
    mutter_sck_samples_cb on_samples,
    mutter_sck_stop_cb on_stop,
    void *user_data,
    char *error_out,
    size_t error_out_len);

// Stops capture and releases `handle`. Call exactly once; `handle` is
// invalid after this returns. Blocks until ScreenCaptureKit confirms the
// stream has stopped.
void mutter_sck_stop(MutterSckCapture *handle);

#ifdef __cplusplus
}
#endif
