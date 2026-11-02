// pixfer web ui_state — a state machine for the tx/rx views (idle,
// transferring, done, error). Mirrors `pixfer-core/src/ui_state.rs`.

export const UiState = Object.freeze({
  IDLE: "idle",
  TRANSFERRING: "transferring",
  DONE: "done",
  ERROR: "error",
});

export class UiStateMachine {
  constructor() {
    this.state = UiState.IDLE;
    this.progress = 0;
    this.errorCode = null;
  }

  startTransfer() {
    this.state = UiState.TRANSFERRING;
    this.progress = 0;
    this.errorCode = null;
    return this;
  }

  setProgress(fraction) {
    this.progress = Math.min(1, Math.max(0, fraction));
    return this;
  }

  complete() {
    this.state = UiState.DONE;
    this.progress = 1;
    return this;
  }

  fail(code) {
    this.state = UiState.ERROR;
    this.errorCode = code;
    return this;
  }

  getState() {
    return this.state;
  }

  getProgress() {
    return this.progress;
  }

  getErrorCode() {
    return this.errorCode;
  }
}