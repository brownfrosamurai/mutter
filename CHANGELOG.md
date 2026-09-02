# Changelog

All notable changes to Mutter are documented in this file.

## [0.3.0] - 2026-09-02

### Added
- The dashboard's Activity chart now renders as one smooth wave (a Catmull-Rom spline with a gradient fill) instead of 7 discrete bars, matching the app's voice/waveform identity. The dashboard window is also a bit wider and taller (640×460) so the Metrics panel's tiles, chart, and latency table fit without clipping.

### Fixed
- Fixed Whisper occasionally inserting non-speech artifacts like "[BLANK_AUDIO]" or "[MUSIC]" into transcripts during silence or background noise — these no longer appear in dictated text.
- Transcription is also faster: leading and trailing silence around a recording is now trimmed before it's sent for transcription.

## [0.2.0] - 2026-09-01

### Added
- Onboarding now automatically requests Microphone, Accessibility, and Screen Recording permissions in sequence when you reach the final step, showing live per-permission progress — no more manual "Grant" clicks needed to get through first-run setup.
- Settings' Permissions section now has a fallback "Open System Settings" link for Accessibility and Screen Recording, in case the native permission prompt stops reappearing after a denial.

### Fixed
- Fixed the microphone permission prompt never appearing on a fresh install — a missing macOS entitlement was silently blocking the request.
- Fixed a rare crash risk where a failed permission check could take down the whole app instead of failing safely.
- Fixed the onboarding "Open Dashboard" and "Quit" buttons being clickable while permission requests were still in progress, which could close the onboarding window (or quit the app) mid-request.
- Fixed onboarding getting stuck on "Setting things up" indefinitely if a permission request failed unexpectedly.
- Fixed the microphone row briefly flickering back to "Requesting…" on reopening the onboarding Ready screen, even when mic access was already granted.
- Fixed the Metrics panel not filling the full window height when the dashboard is resized.
- Fixed a visual glitch where the Onboarding or Recovery window could briefly show without its frosted-glass background applied.

### Changed
- The Onboarding and Recovery windows now render with the same native frosted-glass material as the Dashboard and pill HUD, and can be dragged like the rest of the app.
