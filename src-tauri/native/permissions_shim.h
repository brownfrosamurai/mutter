// Minimal C ABI for checking microphone authorization status.
//
// Accessibility (AXIsProcessTrusted) and Screen Recording
// (CGPreflightScreenCaptureAccess) already have plain C entry points —
// declared directly in Rust (see ../src/permissions.rs), no shim needed.
// Only microphone status requires an Objective-C call
// (AVCaptureDevice.authorizationStatusForMediaType:), hence this file.
#pragma once

#ifdef __cplusplus
extern "C" {
#endif

// Mirrors AVAuthorizationStatus's raw values exactly (see
// AVCaptureDevice.h): 0 = NotDetermined, 1 = Restricted, 2 = Denied,
// 3 = Authorized. Returned as a plain int rather than redefining the enum
// on the Rust side so the two can never drift silently out of sync.
int mutter_mic_auth_status(void);

#ifdef __cplusplus
}
#endif
