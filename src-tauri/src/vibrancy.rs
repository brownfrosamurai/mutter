//! Masks a window's native vibrancy layer to exactly the shapes of its
//! visible UI, on macOS only — "the shell should be invisible, only the
//! [content] visible" (2026-08-30, user-directed; applied first to the
//! dashboard, then to the pill for the same reason). Anywhere the window's
//! actual UI doesn't cover shows real desktop with no blur or tint; only
//! the UI shapes themselves get the frosted vibrancy look.
//!
//! **As of 2026-08-31, only the pill uses this** (`set_pill_vibrancy_layout`,
//! single shape via `mask_to_shape`). The dashboard tried this twice the same
//! day (`set_dashboard_vibrancy_layout`, two shapes: `#app` card + `#sidebar`
//! pill — see `lib.rs`'s `run()` setup() comment for the full round trip)
//! and both times hit the same wall: the native call succeeded
//! (`mask_to_shapes` returned `true`, confirmed live with correct geometry),
//! but the mask never actually constrained vibrancy to just the reported
//! shapes — the surrounding dead space still showed filled, tinted vibrancy
//! rather than real desktop. Root cause not found; ruled out premature
//! measurement and a self-inflicted duplicate-call race along the way, but
//! not the underlying mechanism. This is an inherently fragile technique
//! regardless of caller: an undocumented private AppKit view tag to even
//! find the vibrancy view, a hand-built CoreGraphics mask that has to
//! exactly agree with layout reported across two IPC hops (WebKit -> Rust ->
//! AppKit), and (per the "wisp" bleed noted below) a material blur radius
//! with no public API to control at all. The pill fully solved its version
//! of this by resizing the *window itself* to eliminate dead space entirely
//! (see `session::apply_pill_layout`) — the dashboard couldn't do the same,
//! since its dead space (the gap around the floating sidebar) is a
//! deliberate design element, not incidental. The dashboard now has no
//! vibrancy at all (plain `transparent: true`, no `windowEffects`) rather
//! than keep fighting this.
//!
//! `tauri.conf.json`'s own `windowEffects.radius` looks like the obvious
//! way to shape vibrancy, but it's unusable: it routes through
//! `window-vibrancy`'s `setCornerRadius:`, which that crate's own source
//! admits is an undocumented, private AppKit selector ("might be private,
//! but it works") — and calling it makes the dashboard window render
//! completely blank (vibrancy only, zero WKWebView content), root-caused
//! 2026-08-30 (see `../../DESIGN.md`'s "Glass effect" section for the full
//! trace, including the corroborating upstream Tauri issue). This module
//! calls a small Objective-C shim (`native/vibrancy_mask_shim.m`) instead,
//! which finds the vibrancy view Tauri's declarative `windowEffects` config
//! already created and reshapes only its visible region, via the one
//! public, documented API Apple ships for exactly this
//! (`NSVisualEffectView.maskImage`).
//!
//! **2026-09-01: the dashboard's shell got real, working corner rounding —
//! see `apply_dashboard_glass` below.** Not another maskImage attempt: real
//! research (not guessing) found the actual root cause of the failures
//! above — `NSVisualEffectView.maskImage` only reliably masks when the
//! effect view *is* the window's content view, not a subview alongside a
//! WKWebView sibling — and the actual fix, `window-vibrancy` 0.8's
//! `apply_liquid_glass` (Apple's real macOS 26+ `NSGlassEffectView`, with a
//! `content_view` option that reparents the WebView into the glass view
//! instead of leaving it a sibling). Falls back to the same flat vibrancy
//! this module used to apply nothing in place of, on macOS < 26.
//!
//! The layout itself (where a window's real UI actually sits) is reported
//! live by the frontend — via `getBoundingClientRect()`, on load and
//! whenever it changes — rather than hardcoded here from CSS constants, so
//! this stays correct if the CSS layout ever changes without needing a
//! matching Rust edit.

/// A rect in top-left-origin CSS/logical points — exactly what
/// `getBoundingClientRect()` reports, and (both being DPI-independent)
/// exactly what AppKit points are too; the native shim handles the
/// top-left-vs-bottom-left origin conversion internally.
#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One region to keep vibrancy visible behind, with its own corner radius —
/// a card's fixed radius and a capsule/pill's `min(width, height) / 2` are
/// both just "a radius" to the native shim, so this is the one type both
/// call sites (dashboard, pill) build.
#[derive(Clone, Copy)]
pub struct Shape {
    pub rect: Rect,
    pub radius: f64,
}

const ZERO_SHAPE: Shape = Shape {
    rect: Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    },
    radius: 0.0,
};

/// `min(width, height) / 2` — what CSS's `--radius-pill: 999px` (an
/// intentionally oversized value AppKit/the browser both just clamp to the
/// shape's own half-dimension) actually resolves to at render time for a
/// given rendered rect. Used for both the dashboard's floating sidebar and
/// the pill window's own capsule — computed from the live-reported rect
/// rather than assumed, so it's correct at any size either one renders at.
pub fn capsule_radius(rect: Rect) -> f64 {
    rect.width.min(rect.height) / 2.0
}

#[cfg(target_os = "macos")]
extern "C" {
    #[allow(clippy::too_many_arguments)]
    fn mutter_apply_vibrancy_mask_regions(
        ns_view: *const std::ffi::c_void,
        primary_x: f64,
        primary_y: f64,
        primary_width: f64,
        primary_height: f64,
        primary_radius: f64,
        secondary_x: f64,
        secondary_y: f64,
        secondary_width: f64,
        secondary_height: f64,
        secondary_radius: f64,
    ) -> i32;
}

/// Masks `window`'s vibrancy layer to `primary` and `secondary` (both
/// reported live by the frontend) — general enough for two independent
/// shapes at once, though as of 2026-08-31 the only real caller is
/// `mask_to_shape` below (the pill's single-shape case) via a zero-size
/// `secondary`; the dashboard's two-real-shapes case (`#app` card +
/// `#sidebar` pill) was tried and reverted — see this module's own header
/// docs. Does nothing harmful if the window's native view handle isn't
/// available or
/// no tagged vibrancy view is found — purely cosmetic, never worth failing
/// a command over — but logs a warning either way so a silent miss is
/// diagnosable.
#[cfg(target_os = "macos")]
pub fn mask_to_shapes<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    primary: Shape,
    secondary: Shape,
) -> bool {
    let applied = match window.ns_view() {
        Ok(ns_view) => unsafe {
            mutter_apply_vibrancy_mask_regions(
                ns_view as *const _,
                primary.rect.x,
                primary.rect.y,
                primary.rect.width,
                primary.rect.height,
                primary.radius,
                secondary.rect.x,
                secondary.rect.y,
                secondary.rect.width,
                secondary.rect.height,
                secondary.radius,
            ) == 1
        },
        Err(_) => false,
    };
    if !applied {
        tracing::warn!(
            window = window.label(),
            "mask_to_shapes: no tagged vibrancy view found to mask"
        );
    }
    applied
}

#[cfg(not(target_os = "macos"))]
pub fn mask_to_shapes<R: tauri::Runtime>(
    _window: &tauri::WebviewWindow<R>,
    _primary: Shape,
    _secondary: Shape,
) -> bool {
    false
}

/// Convenience for a window with only one visible vibrancy shape — the
/// pill, which has no second floating element to mask separately.
pub fn mask_to_shape<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, shape: Shape) -> bool {
    mask_to_shapes(window, shape, ZERO_SHAPE)
}

/// The dashboard's outer "shell" — real `NSGlassEffectView` corner rounding
/// on macOS 26+ (Tahoe's actual native Liquid Glass material, applied via
/// `window-vibrancy`'s `apply_liquid_glass`), falling back to the same flat
/// `NSVisualEffectView` vibrancy the window used to get declaratively via
/// `windowEffects` in `tauri.conf.json` on older macOS.
///
/// **Replaces the maskImage-based masking attempt (2026-09-01, reverted the
/// same day it was tried).** That attempt live-verified the reported mask
/// rect and the vibrancy view's own bounds matched exactly, yet the render
/// stayed square regardless — root-caused afterward via actual research,
/// not just abandoned again: `NSVisualEffectView.maskImage` only reliably
/// masks when the effect view *is* the window's content view, not when it's
/// a subview sitting alongside a WKWebView sibling (this app's exact setup,
/// same as every window here). `apply_liquid_glass`'s `content_view` option
/// fixes this at the actual source, not around it — it reparents the
/// WebView *into* the glass view instead of leaving it a sibling, so there's
/// no compositing-order mismatch left to fight. `NSGlassEffectView` also
/// exposes a real, public, first-class `cornerRadius` — no private
/// selector, no hand-built CoreGraphics bitmap mask.
///
/// Called once, at startup — unlike the old per-resize `mask_to_shape`
/// approach, `NSGlassEffectView.cornerRadius` is a real CALayer-backed
/// property that tracks the view's own frame automatically on resize;
/// there's nothing to re-report on every layout change.
pub fn apply_dashboard_glass<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, radius: f64) {
    let window_for_webview = window.clone();
    let result = window.with_webview(move |platform_webview| {
        apply_dashboard_glass_to_webview(&window_for_webview, platform_webview, radius);
    });
    if let Err(e) = result {
        tracing::warn!(error = %e, "apply_dashboard_glass: with_webview failed");
    }
}

#[cfg(target_os = "macos")]
fn apply_dashboard_glass_to_webview<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    platform_webview: tauri::webview::PlatformWebview,
    radius: f64,
) {
    use objc2_app_kit::NSView;

    let webview_ptr = platform_webview.inner();
    let webview_ns_view = unsafe { (webview_ptr as *mut NSView).as_ref() };
    let Some(webview_ns_view) = webview_ns_view else {
        tracing::warn!("apply_dashboard_glass: webview ns_view was null");
        return;
    };

    let options =
        window_vibrancy::LiquidGlassOptions::new(window_vibrancy::NSGlassEffectViewStyle::Regular)
            .radius(radius)
            .content_view(webview_ns_view);

    match window_vibrancy::apply_liquid_glass(window, options) {
        Ok(()) => {
            tracing::info!(radius, "dashboard: real Liquid Glass applied (macOS 26+)");
        }
        Err(e) => {
            // Covers both UnsupportedPlatformVersion (< macOS 26 — the
            // expected, common case for now) and any other real failure —
            // either way, the flat vibrancy fallback is strictly better
            // than a window with no material applied at all.
            tracing::info!(error = %e, "dashboard: Liquid Glass unavailable, applying flat vibrancy fallback");
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
fn apply_dashboard_glass_to_webview<R: tauri::Runtime>(
    _window: &tauri::WebviewWindow<R>,
    _platform_webview: tauri::webview::PlatformWebview,
    _radius: f64,
) {
}
