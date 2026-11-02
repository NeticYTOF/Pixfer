// pixfer web receiver camera — capture + decode pipeline. Mirrors
// `pixfer-core/src/rx_camera.rs`.

import { decodeBlocks } from "./core.js";

// A receiver-side camera capture + decode pipeline.
export class RxCamera {
  // hal: an object with { open(), close(), pollFrame(), lockExposure() }.
  constructor(hal) {
    this.hal = hal;
  }

  // Open the camera and poll for a frame, then decode it.
  // Returns the decoded frame on success, or throws on failure.
  captureAndDecode() {
    if (this.hal.lockExposure && this.hal.lockExposure()) {
      // exposure locked; good.
    }
    this.hal.open();
    const frame = this.hal.pollFrame();
    if (!frame) {
      throw new Error("no frame available");
    }
    const result = decodeBlocks(frame.blocks);
    if (!result.ok) {
      throw new Error(`decode failed: ${result.error}`);
    }
    return result.frame;
  }
}