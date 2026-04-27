/**
 * PoT canonical schema validators.
 *
 * Mirrors spec sections §4.2 (canonical input) and §4.3 (canonical output).
 *
 * Hand-rolled validators (no zod) — strict shape checks, throw
 * `CanonicalSchemaError` with a JSON-pointer-ish field path so callers
 * can pinpoint mismatches in the trace bundle. Keep this file dep-free
 * so it can be imported into both the encoder path and runtime guards.
 */

// ---------------------------------------------------------------------------
// Types — TS mirror of CBOR shapes, exported as the canonical TS surface.
// ---------------------------------------------------------------------------

/** A single chat message — see §4.2 `messages[]`. */
export interface Message {
  /** Conversation role; e.g. `system` | `user` | `assistant` | `tool`. */
  role: string;
  /** Free-form content the model saw. */
  content: string;
  /**
   * Optional attachments — opaque references (CIDs / URIs / digests) the
   * agent presented to the model. Order is significant.
   */
  attachments?: string[];
}

/** Tool I/O the model actually consumed — see §4.2 `tool_calls`. */
export interface ToolCall {
  /** Tool name as advertised in the `tools` schema. */
  call: string;
  /**
   * Arbitrary JSON-serialisable response from the tool. Hash collisions
   * across structurally-equivalent values are prevented by canonical CBOR.
   */
  response: unknown;
}

/** Sampling parameters — see §4.3 `sampling`. */
export interface Sampling {
  /** 0 in the strict regime; declared in canonical_output. */
  temperature: number;
  /** 1.0 in the strict regime. */
  top_p: number;
  /**
   * REQUIRED. Derived from `vrf_seed`; binds inference RNG to the freshness
   * proof. u64 — accepted as `bigint` or `number` in [0, 2^53-1].
   */
  seed: bigint;
  /** Hard cap on output length; integer >= 0. */
  max_tokens: number;
}

/**
 * Canonical input — exactly what the model saw, hashed to give
 * `input_commitment`. Order of keys does NOT matter at the TS level
 * because canonical CBOR sorts them — but presence/types are strict.
 */
export interface CanonicalInput {
  /** System prompt. Empty string allowed; `undefined` not allowed. */
  system: string;
  /** Conversation transcript; ordered. */
  messages: Message[];
  /**
   * Tool schemas advertised to the model (JSON schema fragments). Order
   * matters because some providers honour tool-order in routing.
   */
  tools: unknown[];
  /** Tool calls + responses the model consumed during this thought. */
  tool_calls: ToolCall[];
  /**
   * Merkle root of the long-term memory KV store as of `context_t`.
   * 32 bytes. If the agent has no memory, supply 32 zero bytes.
   */
  memory_snap: Uint8Array;
  /** Solana slot at start of inference. u64. */
  context_t: bigint;
  /** Pyth Entropy seed pulled BEFORE inference. 32 bytes. */
  vrf_seed: Uint8Array;
  /** Hash of the policy doc this thought claims to satisfy. 32 bytes. */
  policy_id: Uint8Array;
}

/** Tool the model intended to call after thinking — see §4.3 `tool_intents`. */
export interface ToolIntent {
  tool: string;
  args: unknown;
}

/**
 * Canonical output — the structured "thought" the agent emits, hashed to
 * give `output_commitment`.
 */
export interface CanonicalOutput {
  /** Schema-validated decision payload. Type is policy-defined. */
  decision: unknown;
  /** Free-form reasoning trace; string OR ordered list of step strings. */
  reasoning: string | string[];
  /** Tools the agent intends to invoke as a result of this thought. */
  tool_intents: ToolIntent[];
  /** Optional self-confidence in [0, 1]. */
  self_score?: number;
  /** Claimed model digest — must match registered ModelRegistry entry. */
  model_id: Uint8Array;
  /** Sampling settings — must satisfy strict-regime rules per policy. */
  sampling: Sampling;
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/**
 * Thrown when a value does not conform to the canonical shape. The `path`
 * is a slash-delimited JSON pointer-ish string, e.g. `/messages/2/role`.
 */
export class CanonicalSchemaError extends Error {
  public readonly path: string;
  public readonly received: unknown;

  public constructor(path: string, message: string, received?: unknown) {
    super(`canonical schema error at ${path}: ${message}`);
    this.name = "CanonicalSchemaError";
    this.path = path;
    this.received = received;
  }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

const MAX_U64 = (1n << 64n) - 1n;

function isPlainObject(v: unknown): v is Record<string, unknown> {
  if (v === null || typeof v !== "object") return false;
  if (Array.isArray(v)) return false;
  if (v instanceof Uint8Array) return false;
  const proto = Object.getPrototypeOf(v);
  return proto === Object.prototype || proto === null;
}

function requireField<T extends Record<string, unknown>>(
  obj: T,
  key: string,
  path: string,
): unknown {
  if (!(key in obj)) {
    throw new CanonicalSchemaError(`${path}/${key}`, "missing required field");
  }
  return obj[key];
}

function requireString(v: unknown, path: string): string {
  if (typeof v !== "string") {
    throw new CanonicalSchemaError(path, `expected string, got ${typeOf(v)}`, v);
  }
  return v;
}

function requireBytes(v: unknown, path: string, len?: number): Uint8Array {
  if (!(v instanceof Uint8Array)) {
    throw new CanonicalSchemaError(path, `expected Uint8Array, got ${typeOf(v)}`, v);
  }
  if (len !== undefined && v.length !== len) {
    throw new CanonicalSchemaError(
      path,
      `expected ${len} bytes, got ${v.length}`,
      v.length,
    );
  }
  return v;
}

function requireU64(v: unknown, path: string): bigint {
  let asBig: bigint;
  if (typeof v === "bigint") {
    asBig = v;
  } else if (typeof v === "number") {
    if (!Number.isInteger(v)) {
      throw new CanonicalSchemaError(path, "expected integer u64, got non-integer number", v);
    }
    if (v < 0) {
      throw new CanonicalSchemaError(path, "expected non-negative u64", v);
    }
    asBig = BigInt(v);
  } else {
    throw new CanonicalSchemaError(path, `expected u64 (bigint/number), got ${typeOf(v)}`, v);
  }
  if (asBig < 0n || asBig > MAX_U64) {
    throw new CanonicalSchemaError(path, `u64 out of range: ${asBig.toString()}`);
  }
  return asBig;
}

function requireNumber(v: unknown, path: string): number {
  if (typeof v !== "number") {
    throw new CanonicalSchemaError(path, `expected number, got ${typeOf(v)}`, v);
  }
  if (!Number.isFinite(v)) {
    throw new CanonicalSchemaError(path, "non-finite numbers (NaN/±Infinity) are forbidden", v);
  }
  return v;
}

function requireInteger(v: unknown, path: string): number {
  const n = requireNumber(v, path);
  if (!Number.isInteger(n)) {
    throw new CanonicalSchemaError(path, "expected integer", v);
  }
  return n;
}

function requireArray(v: unknown, path: string): unknown[] {
  if (!Array.isArray(v)) {
    throw new CanonicalSchemaError(path, `expected array, got ${typeOf(v)}`, v);
  }
  return v;
}

function requireObject(v: unknown, path: string): Record<string, unknown> {
  if (!isPlainObject(v)) {
    throw new CanonicalSchemaError(path, `expected object, got ${typeOf(v)}`, v);
  }
  return v;
}

function typeOf(v: unknown): string {
  if (v === null) return "null";
  if (Array.isArray(v)) return "array";
  if (v instanceof Uint8Array) return "Uint8Array";
  return typeof v;
}

// ---------------------------------------------------------------------------
// Validators
// ---------------------------------------------------------------------------

export function validateMessage(v: unknown, path: string): Message {
  const o = requireObject(v, path);
  const role = requireString(requireField(o, "role", path), `${path}/role`);
  const content = requireString(requireField(o, "content", path), `${path}/content`);
  let attachments: string[] | undefined;
  if ("attachments" in o && o.attachments !== undefined) {
    const arr = requireArray(o.attachments, `${path}/attachments`);
    attachments = arr.map((a, i) =>
      requireString(a, `${path}/attachments/${i}`),
    );
  }
  const out: Message = { role, content };
  if (attachments !== undefined) out.attachments = attachments;
  return out;
}

export function validateToolCall(v: unknown, path: string): ToolCall {
  const o = requireObject(v, path);
  const call = requireString(requireField(o, "call", path), `${path}/call`);
  // `response` is intentionally `unknown`: the tool determines its shape.
  // Presence-only check; canonical CBOR will reject non-encodable values.
  if (!("response" in o)) {
    throw new CanonicalSchemaError(`${path}/response`, "missing required field");
  }
  return { call, response: o.response };
}

export function validateToolIntent(v: unknown, path: string): ToolIntent {
  const o = requireObject(v, path);
  const tool = requireString(requireField(o, "tool", path), `${path}/tool`);
  if (!("args" in o)) {
    throw new CanonicalSchemaError(`${path}/args`, "missing required field");
  }
  return { tool, args: o.args };
}

export function validateSampling(v: unknown, path: string): Sampling {
  const o = requireObject(v, path);
  const temperature = requireNumber(
    requireField(o, "temperature", path),
    `${path}/temperature`,
  );
  if (temperature < 0) {
    throw new CanonicalSchemaError(`${path}/temperature`, "must be >= 0", temperature);
  }
  const top_p = requireNumber(requireField(o, "top_p", path), `${path}/top_p`);
  if (top_p < 0 || top_p > 1) {
    throw new CanonicalSchemaError(`${path}/top_p`, "must be in [0, 1]", top_p);
  }
  const seed = requireU64(requireField(o, "seed", path), `${path}/seed`);
  const max_tokens = requireInteger(
    requireField(o, "max_tokens", path),
    `${path}/max_tokens`,
  );
  if (max_tokens < 0) {
    throw new CanonicalSchemaError(`${path}/max_tokens`, "must be >= 0", max_tokens);
  }
  return { temperature, top_p, seed, max_tokens };
}

export function validateCanonicalInput(v: unknown): CanonicalInput {
  const path = "";
  const o = requireObject(v, "/");
  const system = requireString(requireField(o, "system", path), "/system");
  const messages = requireArray(requireField(o, "messages", path), "/messages")
    .map((m, i) => validateMessage(m, `/messages/${i}`));
  const tools = requireArray(requireField(o, "tools", path), "/tools");
  // Tool schemas are arbitrary JSON; we only confirm array-ness here. The
  // canonical CBOR encoder will reject non-encodable substructures.
  const tool_calls = requireArray(
    requireField(o, "tool_calls", path),
    "/tool_calls",
  ).map((tc, i) => validateToolCall(tc, `/tool_calls/${i}`));
  const memory_snap = requireBytes(
    requireField(o, "memory_snap", path),
    "/memory_snap",
    32,
  );
  const context_t = requireU64(
    requireField(o, "context_t", path),
    "/context_t",
  );
  const vrf_seed = requireBytes(
    requireField(o, "vrf_seed", path),
    "/vrf_seed",
    32,
  );
  const policy_id = requireBytes(
    requireField(o, "policy_id", path),
    "/policy_id",
    32,
  );
  return {
    system,
    messages,
    tools,
    tool_calls,
    memory_snap,
    context_t,
    vrf_seed,
    policy_id,
  };
}

export function validateCanonicalOutput(v: unknown): CanonicalOutput {
  const path = "";
  const o = requireObject(v, "/");
  if (!("decision" in o)) {
    throw new CanonicalSchemaError("/decision", "missing required field");
  }
  const decision = o.decision;
  const reasoningRaw = requireField(o, "reasoning", path);
  let reasoning: string | string[];
  if (typeof reasoningRaw === "string") {
    reasoning = reasoningRaw;
  } else if (Array.isArray(reasoningRaw)) {
    reasoning = reasoningRaw.map((s, i) =>
      requireString(s, `/reasoning/${i}`),
    );
  } else {
    throw new CanonicalSchemaError(
      "/reasoning",
      `expected string or string[], got ${typeOf(reasoningRaw)}`,
      reasoningRaw,
    );
  }
  const tool_intents = requireArray(
    requireField(o, "tool_intents", path),
    "/tool_intents",
  ).map((ti, i) => validateToolIntent(ti, `/tool_intents/${i}`));

  let self_score: number | undefined;
  if ("self_score" in o && o.self_score !== undefined) {
    const s = requireNumber(o.self_score, "/self_score");
    if (s < 0 || s > 1) {
      throw new CanonicalSchemaError("/self_score", "must be in [0, 1]", s);
    }
    self_score = s;
  }
  const model_id = requireBytes(
    requireField(o, "model_id", path),
    "/model_id",
    32,
  );
  const sampling = validateSampling(
    requireField(o, "sampling", path),
    "/sampling",
  );
  const out: CanonicalOutput = {
    decision,
    reasoning,
    tool_intents,
    model_id,
    sampling,
  };
  if (self_score !== undefined) out.self_score = self_score;
  return out;
}
