// Minimal C ABI for checking and requesting microphone authorization.
//
// Accessibility (AXIsProcessTrusted) and Screen Recording
// (CGPreflightScreenCaptureAccess) already have plain C entry points —
// declared directly in Rust (see ../src/permissions.rs), no shim needed.
// Only microphone status/request requires an Objective-C call
// (AVCaptureDevice), hence this file.
#pragma once

#ifdef __cplusplus
extern "C" {
#endif

// Mirrors AVAuthorizationStatus's raw values exactly (see
// AVCaptureDevice.h): 0 = NotDetermined, 1 = Restricted, 2 = Denied,
// 3 = Authorized. Returned as a plain int rather than redefining the enum
// on the Rust side so the two can never drift silently out of sync.
int mutter_mic_auth_status(void);

// Shows the real, native macOS microphone permission prompt (once per
// install — a no-op that just returns the current status if the user has
// already answered it) and blocks the calling thread until the user
// responds. Returns 1 if access is granted, 0 otherwise.
//
// Must NEVER be called from the main/UI thread: internally this dispatches
// the actual AVFoundation call onto the main queue (verified live — TCC
// silently declines to present the prompt at all if the request doesn't
// originate on the main thread/run loop) and then blocks the CALLING
// thread on a semaphore until that main-queue block signals it — calling
// this from the main thread would deadlock against its own dispatch. The
// Rust side wraps it in `tauri::async_runtime::spawn_blocking` (see
// onboarding's `request_mic_access` command in lib.rs).
int mutter_request_mic_access(void);

#ifdef __cplusplus
}
#endif
