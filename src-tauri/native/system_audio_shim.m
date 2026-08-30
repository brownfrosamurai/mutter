// See system_audio_shim.h for the C ABI contract and rationale.
//
// Audio-only capture still requires an SCContentFilter built around a
// display (ScreenCaptureKit has no "audio with no content source at all"
// filter) — Section 9's named concern about video-frame capture overhead
// for an audio-only feature is addressed by configuring the smallest
// possible video surface (2x2, 1fps ceiling) rather than the display's
// real resolution, not by avoiding a display filter entirely (the API
// doesn't allow that).
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreAudio/CoreAudio.h>
#import <Foundation/Foundation.h>

#include "system_audio_shim.h"
#include <string.h>
#include <stdlib.h>

API_AVAILABLE(macos(13.0))
@interface MutterSckOutput : NSObject <SCStreamOutput, SCStreamDelegate>
@property(nonatomic, assign) mutter_sck_samples_cb onSamples;
@property(nonatomic, assign) mutter_sck_stop_cb onStop;
@property(nonatomic, assign) void *userData;
@end

@implementation MutterSckOutput

- (void)stream:(SCStream *)stream
    didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
                    ofType:(SCStreamOutputType)type {
  if (type != SCStreamOutputTypeAudio || !CMSampleBufferIsValid(sampleBuffer)) {
    return;
  }

  // ScreenCaptureKit hands audio back as a CMSampleBuffer wrapping an
  // AudioBufferList (SCStream.h's SCStreamOutputTypeAudio docs) — the
  // documented way to get PCM bytes out is
  // CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer, not treating
  // the sample buffer's raw block buffer as directly interleaved samples.
  AudioBufferList audioBufferList;
  CMBlockBufferRef blockBuffer = NULL;
  OSStatus status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
      sampleBuffer, NULL, &audioBufferList, sizeof(audioBufferList), NULL, NULL,
      kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, &blockBuffer);
  if (status != noErr || blockBuffer == NULL) {
    return;
  }

  if (self.onSamples != NULL) {
    for (UInt32 i = 0; i < audioBufferList.mNumberBuffers; i++) {
      AudioBuffer buffer = audioBufferList.mBuffers[i];
      if (buffer.mData == NULL || buffer.mDataByteSize == 0) {
        continue;
      }
      size_t sampleCount = buffer.mDataByteSize / sizeof(float);
      self.onSamples((const float *)buffer.mData, sampleCount, self.userData);
    }
  }

  CFRelease(blockBuffer);
}

- (void)stream:(SCStream *)stream didStopWithError:(NSError *)error {
  if (self.onStop != NULL) {
    const char *message = error != nil ? [[error localizedDescription] UTF8String] : NULL;
    self.onStop(message, self.userData);
  }
}

@end

// Plain C struct fields can't hold ARC-managed Objective-C pointers
// directly — `stream`/`output` are CFBridgingRetain'd and stored as opaque
// `void *`, bridged back with `__bridge`/`CFBridgingRelease` where used.
struct MutterSckCapture {
  void *stream;
  void *output;
};

static void mutter_sck_copy_error(NSString *message, char *error_out, size_t error_out_len) {
  if (error_out == NULL || error_out_len == 0) {
    return;
  }
  const char *utf8 = message != nil ? [message UTF8String] : "unknown error";
  strncpy(error_out, utf8, error_out_len - 1);
  error_out[error_out_len - 1] = '\0';
}

MutterSckCapture *mutter_sck_start(int sample_rate, int channel_count,
                                    mutter_sck_samples_cb on_samples, mutter_sck_stop_cb on_stop,
                                    void *user_data, char *error_out, size_t error_out_len) {
  if (@available(macOS 13.0, *)) {
    __block SCDisplay *targetDisplay = nil;
    __block NSError *contentError = nil;
    dispatch_semaphore_t contentSem = dispatch_semaphore_create(0);

    [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *content,
                                                                    NSError *error) {
      if (content != nil && content.displays.count > 0) {
        targetDisplay = content.displays.firstObject;
      }
      contentError = error;
      dispatch_semaphore_signal(contentSem);
    }];
    // Blocks for however long Screen Recording consent takes to resolve —
    // see the header's contract on this.
    dispatch_semaphore_wait(contentSem, DISPATCH_TIME_FOREVER);

    if (targetDisplay == nil) {
      NSString *message = contentError != nil
                               ? contentError.localizedDescription
                               : @"no shareable display found (Screen Recording permission denied?)";
      mutter_sck_copy_error(message, error_out, error_out_len);
      return NULL;
    }

    SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:targetDisplay
                                                       excludingWindows:@[]];

    SCStreamConfiguration *config = [[SCStreamConfiguration alloc] init];
    // Minimal, unused video surface — this app only wants audio. See the
    // file header comment on why a display filter is still required.
    config.width = 2;
    config.height = 2;
    config.minimumFrameInterval = CMTimeMake(1, 1);
    config.capturesAudio = YES;
    config.sampleRate = sample_rate;
    config.channelCount = channel_count;
    config.excludesCurrentProcessAudio = YES;

    MutterSckOutput *output = [[MutterSckOutput alloc] init];
    output.onSamples = on_samples;
    output.onStop = on_stop;
    output.userData = user_data;

    SCStream *stream = [[SCStream alloc] initWithFilter:filter
                                           configuration:config
                                                delegate:output];

    NSError *addOutputError = nil;
    dispatch_queue_t queue =
        dispatch_queue_create("com.femimeduna.mutter.sck-audio", DISPATCH_QUEUE_SERIAL);
    BOOL added = [stream addStreamOutput:output
                                     type:SCStreamOutputTypeAudio
                       sampleHandlerQueue:queue
                                    error:&addOutputError];
    if (!added) {
      mutter_sck_copy_error(addOutputError.localizedDescription, error_out, error_out_len);
      return NULL;
    }

    __block BOOL startSucceeded = NO;
    __block NSError *startError = nil;
    dispatch_semaphore_t startSem = dispatch_semaphore_create(0);
    [stream startCaptureWithCompletionHandler:^(NSError *error) {
      startSucceeded = (error == nil);
      startError = error;
      dispatch_semaphore_signal(startSem);
    }];
    dispatch_semaphore_wait(startSem, DISPATCH_TIME_FOREVER);

    if (!startSucceeded) {
      mutter_sck_copy_error(startError.localizedDescription, error_out, error_out_len);
      return NULL;
    }

    MutterSckCapture *handle = (MutterSckCapture *)malloc(sizeof(MutterSckCapture));
    // Retain across the C boundary — ARC would otherwise release these
    // Objective-C objects the moment this function returns.
    handle->stream = (void *)CFBridgingRetain(stream);
    handle->output = (void *)CFBridgingRetain(output);
    return handle;
  } else {
    mutter_sck_copy_error(@"ScreenCaptureKit requires macOS 13.0+", error_out, error_out_len);
    return NULL;
  }
}

void mutter_sck_stop(MutterSckCapture *handle) {
  if (handle == NULL) {
    return;
  }
  SCStream *stream = (__bridge SCStream *)handle->stream;

  dispatch_semaphore_t stopSem = dispatch_semaphore_create(0);
  [stream stopCaptureWithCompletionHandler:^(NSError *_Nullable stopError) {
    (void)stopError;
    dispatch_semaphore_signal(stopSem);
  }];
  dispatch_semaphore_wait(stopSem, DISPATCH_TIME_FOREVER);

  // Transfers ownership from the CFBridgingRetain in mutter_sck_start back
  // to ARC, which releases immediately since nothing else holds a
  // reference — the standard idiom for undoing CFBridgingRetain.
  CFBridgingRelease(handle->output);
  CFBridgingRelease(handle->stream);
  free(handle);
}
