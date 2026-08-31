// Masks a window's native vibrancy blur view to exactly one or two
// rounded-rect regions using AppKit's public NSVisualEffectView.maskImage
// API — so the vibrancy (blur + tint) is visible only behind a window's
// actual UI, and everywhere else in the window (gaps, outer margin) is
// fully transparent: real desktop, no blur, no tint. "The shell should be
// invisible, only the [content] visible" (2026-08-30, user-directed;
// applied first to the dashboard's #app card + #sidebar pill, then to the
// pill window's own single capsule shape for the same reason).
//
// tauri.conf.json's own `windowEffects.radius` looks like the obvious way
// to shape vibrancy, but it routes through window-vibrancy's
// `setCornerRadius:` — which that crate's own source admits is an
// undocumented, private AppKit selector ("might be private, but it works")
// — and calling it blanks the dashboard window's WKWebView content entirely
// (root-caused 2026-08-30, see ../../DESIGN.md's "Glass effect" section).
// This shim instead finds the NSVisualEffectView Tauri's declarative
// `windowEffects` config already created and reshapes only its visible
// region, via the one public, documented API Apple ships for exactly this.
#pragma once

#ifdef __cplusplus
extern "C" {
#endif

// `ns_view` is the window's content NSView, exactly what
// `WebviewWindow::ns_view()` returns on macOS. Both rects are in top-left-
// origin CSS/logical points — i.e. exactly what `getBoundingClientRect()`
// returns in the webview — matching AppKit points 1:1 (both are DPI-
// independent; the shim converts to AppKit's bottom-left origin
// internally). A window with only one visible shape (the pill) passes a
// zero-size `secondary` rect — filling a zero-area path is a harmless
// no-op. Returns 1 if a tagged vibrancy view was found and masked, 0
// otherwise (no panic either way — this is purely cosmetic and never worth
// failing over, the return value exists only so the Rust side can
// log/diagnose a miss).
int mutter_apply_vibrancy_mask_regions(const void *ns_view, double primary_x, double primary_y,
                                        double primary_width, double primary_height,
                                        double primary_radius, double secondary_x,
                                        double secondary_y, double secondary_width,
                                        double secondary_height, double secondary_radius);

#ifdef __cplusplus
}
#endif
