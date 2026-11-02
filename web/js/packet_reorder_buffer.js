// pixfer web packet reorder buffer — buffer out-of-order chunks and emit
// them in sequence number order once the gap fills.
// Mirrors `pixfer-core/src/packet_reorder_buffer.rs`.

// A buffered chunk awaiting reorder.
export class BufferedPacket {
  constructor({ seq = 0, payload = [] } = {}) {
    this.seq = seq;
    this.payload = payload.slice();
  }
}

// Buffers out-of-order packets and emits them in order.
export class PacketReorderBuffer {
  constructor() {
    this.buffer = new Map();
    this.nextSeq = 0;
  }

  // The expected next sequence number.
  getNextSeq() {
    return this.nextSeq;
  }

  // The number of packets waiting in the buffer.
  pending() {
    return this.buffer.size;
  }

  // Insert a packet into the reorder buffer.
  insert(packet) {
    this.buffer.set(packet.seq, packet);
  }

  // Remove and return the next in-order packet, if one is ready.
  popNext() {
    const p = this.buffer.get(this.nextSeq);
    if (p !== undefined) {
      this.buffer.delete(this.nextSeq);
      this.nextSeq += 1;
      return p;
    }
    return null;
  }

  // Whether a specific sequence is buffered.
  contains(seq) {
    return this.buffer.has(seq);
  }

  // The smallest buffered sequence (null if empty).
  minSeq() {
    const it = this.buffer.keys();
    const f = it.next();
    return f.done ? null : f.value;
  }
}