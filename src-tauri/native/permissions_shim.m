#import <AVFoundation/AVFoundation.h>
#include "permissions_shim.h"

int mutter_mic_auth_status(void) {
  return (int)[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
}

int mutter_request_mic_access(void) {
  // Two real threading requirements, both handled here:
  //
  // 1. requestAccessForMediaType:completionHandler:'s completion handler
  //    fires on an arbitrary internal AVFoundation queue, not the calling
  //    thread — bridge it to a synchronous return via a semaphore, the
  //    standard pattern for wrapping a callback-based Cocoa API behind a
  //    blocking C ABI.
  // 2. The call itself must be MADE from the main thread. Rust's caller
  //    runs this on a background blocking-pool thread (never the main
  //    thread — see the header). Verified live: calling it directly from
  //    that background thread resolves instantly to `granted=NO` with no
  //    system prompt ever shown, even with NSMicrophoneUsageDescription
  //    present in Info.plist — TCC's UI-presentation path silently
  //    declines to show anything when the request doesn't originate on
  //    the main thread/run loop. Dispatching the actual AVFoundation call
  //    onto the main queue (while still blocking the ORIGINAL calling
  //    thread on the semaphore) fixes this without changing the function's
  //    "callable from any thread, blocks until answered" contract.
  dispatch_semaphore_t sema = dispatch_semaphore_create(0);
  __block BOOL result = NO;
  dispatch_async(dispatch_get_main_queue(), ^{
    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio
                              completionHandler:^(BOOL granted) {
                                result = granted;
                                dispatch_semaphore_signal(sema);
                              }];
  });
  dispatch_semaphore_wait(sema, DISPATCH_TIME_FOREVER);
  return result ? 1 : 0;
}
