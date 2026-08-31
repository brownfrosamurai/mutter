#import <AppKit/AppKit.h>
#include "vibrancy_mask_shim.h"

// Must match window-vibrancy's own NS_VIEW_TAG_BLUR_VIEW exactly
// (window-vibrancy-0.6.0/src/macos/internal.rs) — this is how we find the
// NSVisualEffectView Tauri's `windowEffects` config already created via that
// crate, without reimplementing vibrancy application ourselves. If
// window-vibrancy ever changes this constant, this shim silently stops
// finding the view and becomes a no-op (see the header's contract) rather
// than crashing.
static const NSInteger kVibrancyViewTag = 91376254;

static void MutterAddRoundedRectPath(CGContextRef ctx, CGRect rect, CGFloat radius) {
  CGPathRef path = CGPathCreateWithRoundedRect(rect, radius, radius, NULL);
  CGContextAddPath(ctx, path);
  CGPathRelease(path);
}

int mutter_apply_vibrancy_mask_regions(const void *ns_view, double primary_x, double primary_y,
                                        double primary_width, double primary_height,
                                        double primary_radius, double secondary_x,
                                        double secondary_y, double secondary_width,
                                        double secondary_height, double secondary_radius) {
  if (ns_view == NULL) {
    return 0;
  }

  NSView *contentView = (__bridge NSView *)ns_view;
  NSView *tagged = [contentView viewWithTag:kVibrancyViewTag];
  if (![tagged isKindOfClass:[NSVisualEffectView class]]) {
    return 0;
  }
  NSVisualEffectView *effectView = (NSVisualEffectView *)tagged;

  CGFloat viewWidth = effectView.bounds.size.width;
  CGFloat viewHeight = effectView.bounds.size.height;
  if (viewWidth <= 0 || viewHeight <= 0) {
    return 0;
  }

  // A full-size mask, redrawn on every call (once per layout report from
  // the frontend, including on resize/state changes) rather than a small
  // stretchable capInsets image — these regions aren't a uniform border
  // around one shape, they're one or two independent, asymmetrically-placed
  // shapes, which a nine-patch stretch can't represent.
  //
  // Built via a manual alpha-enabled CGBitmapContext, not
  // `+[NSImage imageWithSize:flipped:drawingHandler:]` — that convenience
  // API was tried first (for the single-rect predecessor of this function)
  // and silently produced an image with no alpha channel at all (confirmed
  // via its own description: `Alpha=NO`), so `maskImage` masked nothing.
  // `kCGImageAlphaPremultipliedLast` guarantees a real alpha channel:
  // opaque (black, alpha 1) inside the two rounded rects, fully transparent
  // everywhere else.
  size_t pixelWidth = (size_t)ceil(viewWidth);
  size_t pixelHeight = (size_t)ceil(viewHeight);

  CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
  CGContextRef ctx = CGBitmapContextCreate(NULL, pixelWidth, pixelHeight, 8, 0, colorSpace,
                                            (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
  CGColorSpaceRelease(colorSpace);
  if (ctx == NULL) {
    return 0;
  }

  CGRect canvas = CGRectMake(0, 0, viewWidth, viewHeight);
  CGContextClearRect(ctx, canvas);
  CGContextSetFillColorWithColor(ctx, [NSColor blackColor].CGColor);

  // Both input rects are top-left-origin CSS/logical points (exactly what
  // `getBoundingClientRect()` returns) — this bitmap, like the effect
  // view's own `bounds`, is bottom-left-origin AppKit points, so flip Y:
  // appKitY = viewHeight - cssY - height.
  CGRect primaryRect =
      CGRectMake(primary_x, viewHeight - primary_y - primary_height, primary_width, primary_height);
  MutterAddRoundedRectPath(ctx, primaryRect, primary_radius);

  // A zero-size secondary rect (the pill window's single-shape case) adds a
  // degenerate, zero-area path — filling it is a harmless no-op.
  CGRect secondaryRect = CGRectMake(secondary_x, viewHeight - secondary_y - secondary_height,
                                     secondary_width, secondary_height);
  MutterAddRoundedRectPath(ctx, secondaryRect, secondary_radius);

  // Both paths were added above; one fill covers the union of both shapes.
  CGContextFillPath(ctx);

  CGImageRef cgImage = CGBitmapContextCreateImage(ctx);
  CGContextRelease(ctx);
  if (cgImage == NULL) {
    return 0;
  }

  NSImage *maskImage = [[NSImage alloc] initWithCGImage:cgImage
                                                    size:NSMakeSize(viewWidth, viewHeight)];
  CGImageRelease(cgImage);

  effectView.maskImage = maskImage;
  return 1;
}
