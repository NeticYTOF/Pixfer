// pixfer web frame codec — encode/decode a payload into the 270x480
// color-block wire format (3 bits per block).
// Mirrors `pixfer-core/src/frame_codec.rs`.

const RS_K = 223;
const BLOCKS_PER_FRAME = 270 * 480;

// Encode a raw payload (up to RS_K bytes) into a minimal block
// representation (3 bits per block), skipping the header/parity layer.
export function encodePayload(payload) {
  if (payload.length > RS_K) {
    throw new Error(`payload too large: ${payload.length} > ${RS_K}`);
  }
  const totalBits = payload.length * 8;
  const blocksNeeded = Math.ceil(totalBits / 3);
  const out = new Array(blocksNeeded).fill(0);
  let g = 0;
  for (let i = 0; i < blocksNeeded; i++) {
    let v = 0;
    for (let k = 0; k < 3; k++) {
      const bitIdx = g + k;
      if (bitIdx < totalBits) {
        const byteIdx = Math.floor(bitIdx / 8);
        const bitInByte = bitIdx % 8;
        const bit = (payload[byteIdx] >> bitInByte) & 1;
        v |= bit << k;
      }
    }
    out[i] = v & 0b111;
    g += 3;
  }
  return out;
}

// Decode the minimal block representation back into a payload.
export function decodePayload(blocks) {
  const totalBits = blocks.length * 3;
  const bytesNeeded = Math.floor(totalBits / 8);
  const out = new Array(bytesNeeded).fill(0);
  for (let i = 0; i < out.length; i++) {
    let byteVal = 0;
    for (let bit = 0; bit < 8; bit++) {
      const bitIdx = i * 8 + bit;
      const bi = Math.floor(bitIdx / 3);
      if (bi < blocks.length) {
        const bitPos = bitIdx % 3;
        const blockVal = blocks[bi] & 0b111;
        if (((blockVal >> bitPos) & 1) === 1) {
          byteVal |= 1 << bit;
        }
      }
    }
    out[i] = byteVal;
  }
  return out;
}

// The number of color blocks in one frame.
export { BLOCKS_PER_FRAME, RS_K };