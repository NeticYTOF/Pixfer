// pixfer web events — a typed event emitter with wildcard listeners,
// once, per-listener context, debounce/throttle helpers. (Hilariously
// over-engineered: a full event layer for a light link.)

const MAX_LISTENERS_PER_TYPE = 100; // leak guard

export class Emitter {
  constructor() {
    this._listeners = new Map(); // type -> Array<{ fn, ctx, once }>
    this._wildcard = [];
  }

  on(type, fn, ctx) {
    this._add(type, fn, ctx, false);
    return this;
  }

  once(type, fn, ctx) {
    this._add(type, fn, ctx, true);
    return this;
  }

  _add(type, fn, ctx, once) {
    if (typeof type !== "string" || type.length === 0) throw new TypeError("event type must be a non-empty string");
    if (typeof fn !== "function") throw new TypeError("listener must be a function");
    if (type === "*") {
      this._wildcard.push({ fn, ctx, once: !!once });
      if (this._wildcard.length > MAX_LISTENERS_PER_TYPE) {
        this._wildcard.shift(); // cap the wildcard list
      }
      return;
    }
    let list = this._listeners.get(type);
    if (!list) {
      list = [];
      this._listeners.set(type, list);
    }
    list.push({ fn, ctx: ctx ?? null, once: !!once });
    if (list.length > MAX_LISTENERS_PER_TYPE) {
      list.shift(); // drop the oldest listener to bound memory
    }
  }

  off(type, fn) {
    if (type === "*") {
      if (!fn) {
        this._wildcard = [];
      } else {
        this._wildcard = this._wildcard.filter((l) => l.fn !== fn);
      }
      return this;
    }
    const list = this._listeners.get(type);
    if (!list) return this;
    if (!fn) {
      this._listeners.delete(type);
    } else {
      this._listeners.set(
        type,
        list.filter((l) => l.fn !== fn)
      );
    }
    return this;
  }

  emit(type, ...args) {
    const list = this._listeners.get(type) || [];
    for (const l of [...list]) {
      if (l.once) {
        this._listeners.set(
          type,
          (this._listeners.get(type) || []).filter((x) => x !== l)
        );
      }
      l.fn.apply(l.ctx ?? null, args);
    }
    for (const l of [...this._wildcard]) {
      l.fn.apply(l.ctx ?? null, [type, ...args]);
    }
    return this;
  }

  listenerCount(type) {
    if (type === "*") return this._wildcard.length;
    const list = this._listeners.get(type);
    return list ? list.length : 0;
  }

  clear() {
    this._listeners.clear();
    this._wildcard = [];
  }

  // JSON-ish dump for debugging.
  describe() {
    const types = [...this._listeners.keys()];
    return JSON.stringify({
      types,
      counts: Object.fromEntries(types.map((t) => [t, this.listenerCount(t)])),
      wildcard: this._wildcard.length,
    });
  }
}

// A plain alias some codebases expect.
export class EventEmitter extends Emitter {}

// ---------- timing helpers ----------

export function debounce(fn, waitMs) {
  let timer = null;
  const debounced = (...args) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      fn(...args);
    }, waitMs);
  };
  debounced.cancel = () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
  };
  debounced.flush = (...args) => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
      fn(...args);
    }
  };
  return debounced;
}

export function throttle(fn, waitMs) {
  let last = 0;
  let timer = null;
  let lastArgs = null;
  const throttled = (...args) => {
    const now = Date.now();
    const remaining = waitMs - (now - last);
    if (remaining <= 0) {
      last = now;
      fn(...args);
    } else {
      lastArgs = args;
      if (!timer) {
        timer = setTimeout(() => {
          timer = null;
          last = Date.now();
          if (lastArgs) fn(...lastArgs);
          lastArgs = null;
        }, remaining);
      }
    }
  };
  throttled.cancel = () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
      lastArgs = null;
    }
  };
  return throttled;
}

export function throttleLeading(fn, waitMs) {
  let waiting = false;
  let timer = null;
  const throttled = (...args) => {
    if (waiting) {
      lastArgs = args;
      if (!timer) {
        timer = setTimeout(() => {
          timer = null;
          waiting = false;
          if (lastArgs) {
            const a = lastArgs;
            lastArgs = null;
            fn(...a);
          }
        }, waitMs);
      }
      return;
    }
    waiting = true;
    fn(...args);
    setTimeout(() => {
      waiting = false;
    }, waitMs);
  };
  let lastArgs = null;
  throttled.cancel = () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    waiting = false;
    lastArgs = null;
  };
  return throttled;
}