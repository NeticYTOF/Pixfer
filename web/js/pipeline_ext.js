// pixfer web extended transfer pipeline — a pluggable, instrumented pipeline
// that chains validate -> dedup -> compress -> encode -> send with
// per-stage metrics.
// Mirrors `pixfer-core/src/pipeline_ext.rs`.

// A pipeline error with a stage name for diagnostics.
export class PipelineErrorExt {
  constructor(stage, message) {
    this.stage = stage;
    this.message = message;
  }
}

// A single pipeline stage: names itself and transforms a payload.
export class PipelineStageExt {
  name() {
    throw new Error("abstract");
  }

  run(payload) {
    throw new Error("abstract");
  }
}

// The extended transfer pipeline: validate -> dedup -> compress -> encode
// -> send, with per-stage timing.
export class PipelineExt {
  constructor() {
    this.stages = [];
    this.stageMs = [];
  }

  // Register a stage (appended in execution order).
  addStage(stage) {
    this.stages.push(stage);
    this.stageMs.push(0);
  }

  // The number of registered stages.
  stageCount() {
    return this.stages.length;
  }

  // Run the whole pipeline over `payload`, recording per-stage timing.
  run(payload) {
    let data = Array.from(payload);
    for (let i = 0; i < this.stages.length; i++) {
      const start = Date.now();
      const stage = this.stages[i];
      const result = stage.run(data);
      if (result instanceof PipelineErrorExt) {
        this.stageMs[i] = Date.now() - start;
        throw result;
      }
      data = result;
      this.stageMs[i] = Date.now() - start;
    }
    return data;
  }

  // The recorded per-stage timing (ms), aligned with `stages`.
  stageTimingsMs() {
    return this.stageMs;
  }
}

// A validation stage: reject payloads that exceed the max chunk size.
export class ValidateStageExt extends PipelineStageExt {
  constructor(maxLen) {
    super();
    this.maxLen = maxLen;
  }

  name() {
    return "validate";
  }

  run(payload) {
    if (payload.length > this.maxLen) {
      throw new PipelineErrorExt("validate", `payload ${payload.length} > max ${this.maxLen}`);
    }
    return Array.from(payload);
  }
}

// A dedup stage: advisory pass-through (the web mirror is stateless).
export class DedupStageExt extends PipelineStageExt {
  name() {
    return "dedup";
  }

  run(payload) {
    return Array.from(payload);
  }
}

// A compress stage: zip payloads larger than the threshold.
export class CompressStageExt extends PipelineStageExt {
  constructor(threshold) {
    super();
    this.threshold = threshold;
  }

  name() {
    return "compress";
  }

  run(payload) {
    // In the browser this would call the project's zip helper; here the
    // stage is a structural placeholder that passes through.
    return Array.from(payload);
  }
}

// An encode stage: placeholder for the on-screen frame encoding.
export class EncodeStageExt extends PipelineStageExt {
  name() {
    return "encode";
  }

  run(payload) {
    return Array.from(payload);
  }
}

// A send stage: hand the payload to a caller-provided sender.
export class SendStageExt extends PipelineStageExt {
  constructor(sender) {
    super();
    this.sender = sender;
  }

  name() {
    return "send";
  }

  run(payload) {
    const r = this.sender(payload);
    if (r && r.error) throw new PipelineErrorExt("send", r.error);
    return Array.from(payload);
  }
}