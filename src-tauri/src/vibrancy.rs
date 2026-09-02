//! Real native "shell" material for a window's outer glass — "the shell
//! should be invisible, only the [content] visible" (2026-08-30,
//! user-directed) is approximated as closely as AppKit allows: a genuine
//! rounded-corner `NSGlassEffectView` (macOS 26+ Liquid Glass) or, on older
//! macOS, flat `NSVisualEffectView` vibrancy — applied once, at startup, to
//! both the dashboard and the pill (2026-09-01: the pill migrated onto this
//! same mechanism, replacing its own separate approach — see below).
//!
//! **Earlier approach, abandoned for both windows.** Before this, both
//! windows tried shaping vibrancy after the fact via a hand-built
//! `NSVisualEffectView.maskImage` (a small Objective-C shim finding the
//! vibrancy view Tauri's declarative `windowEffects` config created, and
//! reshaping only its visible region, re-run on every layout change). It
//! genuinely worked for the pill — a fixed-size window with real dead space
//! around a state-driven-width capsule — but never for the dashboard: the
//! native call reported success with mathematically-correct geometry, yet
//! the surrounding dead space kept rendering full, square-cornered vibrancy
//! regardless. Root-caused afterward via actual research: `maskImage` only
//! reliably masks when the effect view *is* the window's content view, not
//! a subview sitting alongside a WKWebView sibling — this app's setup in
//! every window. `window-vibrancy` 0.8's `apply_liquid_glass` fixes this at
//! the source via a `content_view` option that reparents the WebView *into*
//! the glass view instead of leaving it a sibling, and `NSGlassEffectView`
//! exposes a real, public, first-class `cornerRadius` — no private
//! selector, no hand-built CoreGraphics bitmap mask, no re-running on every
//! resize (`cornerRadius` is a CALayer-backed property that tracks the
//! view's own frame automatically).
//!
//! **Why the pill could migrate too.** The maskImage approach's one real
//! advantage for the pill was masking a *fixed-size* window down to a
//! *variable-width* capsule inside it. But `session::resize_pill_to_content`
//! already resizes the pill *window itself* to exactly fit `#pill` on every
//! state change (originally added to fix a separate blur-bleed "wisp" past
//! the mask's hard alpha edge) — meaning the window's content view is
//! always exactly the capsule shape already, at a *constant* height
//! (`session::PILL_HEIGHT`, the pill never changes height, only width). A
//! capsule's corner radius is `height / 2`, which is therefore also
//! constant — so `apply_glass_shell` covers the pill with no compromise at
//! all, applied once, same as the dashboard, with no dead-space trade-off
//! to make in the first place. This retired the maskImage shim, the
//! `ResizeObserver`-driven re-masking IPC round trip, and the "wisp" bleed
//! it used to leave behind entirely — see `session::apply_pill_layout` and
//! `Pill.tsx`'s content-width reporting for what's left (window resize
//! only, no vibrancy masking).
//!
//! The dashboard's own dead space (the dead-space gap around its floating
//! sidebar) is a deliberate layout element, not incidental, so it can't
//! shrink-to-fit the same way — its whole window content view (sidebar +
//! card + the gap between them) shares one glass shell, rounded at the
//! window's own outer radius. See `App.tsx`'s module doc for that trade.

/// Applies real native glass to `window`'s entire content view — macOS 26+
/// Liquid Glass (`NSGlassEffectView`, real public `cornerRadius`) via
/// `window-vibrancy`'s `apply_liquid_glass`, falling back to flat
/// `NSVisualEffectView` vibrancy on older macOS. Called once, at startup,
/// for the dashboard and pill (`DASHBOARD_SHELL_RADIUS`/`PILL_SHELL_RADIUS`,
/// both applied before those windows are ever shown) — see `lib.rs`'s
/// `setup()`. Does not need reapplying on resize. Call ordering DOES matter,
/// unlike this doc previously claimed: onboarding/recovery are shown from
/// inside `setup()` itself (not from a separate later code path the way
/// dashboard/pill are), and calling `.show()` before this produced a window
/// with no vibrancy at all — a real, live-found bug (pre-landing review,
/// 2026-09-01) fixed by moving each window's call to immediately before its
/// own `.show()`. Always call this before showing a window, never after.
pub fn apply_glass_shell<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, radius: f64) {
    let window_for_webview = window.clone();
    let result = window.with_webview(move |platform_webview| {
        apply_glass_shell_to_webview(&window_for_webview, platform_webview, radius);
    });
    if let Err(e) = result {
        tracing::warn!(error = %e, "apply_glass_shell: with_webview failed");
    }
}

#[cfg(target_os = "macos")]
fn apply_glass_shell_to_webview<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    platform_webview: tauri::webview::PlatformWebview,
    radius: f64,
) {
    use objc2_app_kit::NSView;

    let webview_ptr = platform_webview.inner();
    let webview_ns_view = unsafe { (webview_ptr as *mut NSView).as_ref() };
    let Some(webview_ns_view) = webview_ns_view else {
        tracing::warn!("apply_glass_shell: webview ns_view was null");
        return;
    };

    let options =
        window_vibrancy::LiquidGlassOptions::new(window_vibrancy::NSGlassEffectViewStyle::Regular)
            .radius(radius)
            .content_view(webview_ns_view);

    match window_vibrancy::apply_liquid_glass(window, options) {
        Ok(()) => {
            tracing::info!(
                window = window.label(),
                radius,
                "real Liquid Glass applied (macOS 26+)"
            );
        }
        Err(e) => {
            // Covers both UnsupportedPlatformVersion (< macOS 26 — the
            // expected, common case for now) and any other real failure —
            // either way, the flat vibrancy fallback is strictly better
            // than a window with no material applied at all.
            tracing::info!(
                window = window.label(),
                error = %e,
                "Liquid Glass unavailable, applying flat vibrancy fallback"
            );
            let _ = window_vibrancy::apply_vibrancy(
                window,
                window_vibrancy::NSVisualEffectMaterial::HudWindow,
                Some(window_vibrancy::NSVisualEffectState::Active),
                None,
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_glass_shell_to_webview<R: tauri::Runtime>(
    _window: &tauri::WebviewWindow<R>,
    _platform_webview: tauri::webview::PlatformWebview,
    _radius: f64,
) {
}
