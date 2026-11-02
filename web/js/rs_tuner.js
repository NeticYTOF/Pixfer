// pixfer web RS tuner — adaptively choose the Reed-Solomon parity count
// for the link based on observed error rate.
// Mirrors `pixfer-core/src/rs_tuner.rs`.

const RS_N = 255;
const RS_T = 16;

// Tunes the RS parity count based on observed link error rate.
export class RSTuner {
  constructor() {
    this.ber = 0.0;
    this.t = RS_T;
  }

  // The current parity count.
  parityCount() {
    return this.t;
  }

  // The observed BER.
  observedBer() {
    return this.ber;
  }

  // Update the tuner with a new BER observation.
  observe(ber) {
    this.ber = ber;
    this.t =
      ber < 0.001
        ? RS_T
        : ber < 0.01
          ? RS_T + 8
          : ber < 0.05
            ? 32
            : ber < 0.1
              ? 64
              : RS_N;
  }
}