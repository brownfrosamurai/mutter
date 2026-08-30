#import <AVFoundation/AVFoundation.h>
#include "permissions_shim.h"

int mutter_mic_auth_status(void) {
  return (int)[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
}
