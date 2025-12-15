import { DataEncoder } from "./encoding";
import { handleBinaryResponse, MessageType, sync_request_binary } from "./ipc";

/**
 * Rust function wrapper that can call back into Rust.
 */
class RustFunction {
  private fnId: number;

  constructor(fnId: number) {
    this.fnId = fnId;
  }

  call(): unknown {
    // Build Evaluate message: [0, fn_id]
    const encoder = new DataEncoder();
    encoder.pushU8(MessageType.Evaluate);
    encoder.pushU32(0); // Call argument function
    encoder.pushU64(this.fnId);

    const response = sync_request_binary("wry://handler", encoder.finalize());
    return handleBinaryResponse(response);
  }
}

export { RustFunction };