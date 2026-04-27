/**
 * Canonical CBOR encoder for PoT.
 *
 * Determinism is the entire point. We layer the following on top of `cbor2`:
 *
 * 1. **CBOR Common Deterministic Encoding (CDE)** — `cde: true` enables
 *    definite-length only, no streaming, smallest-int / smallest-float
 *    encoding, and rejects non-preferred forms. Per RFC 8949 §4.2.
 * 2. **Length-first map key sort** — the spec asks for length-first sort
 *    (RFC 8949 §4.2.3), not lexicographic. We pass `sortLengthFirstDeterministic`
 *    explicitly so map ordering is deterministic regardless of the JS object's
 *    insertion order.
 * 3. **No `undefined` / no NaN / no ±Infinity** — these have multiple valid
 *    representations and so are explicitly rejected before reaching the
 *    encoder.
 * 4. **Pre-walk validation** — we recursively check the value tree for
 *    forbidden inputs (NaN, ±Infinity, out-of-range bigints) and throw a
 *    typed error with a path so commit-builder mistakes are localised.
 *
 * Library choice: `cbor2` (over `cbor`, `cborg`, `borc`). cbor2 ships
 * first-class deterministic-encoding flags (`cde`, `dcbor`) and exposes
 * the §4.2.1 / §4.2.3 sort comparators, which is exactly what the spec
 * asks for. It's also actively maintained, ESM-native, and zero-dep.
 */

import { encode, decode } from "cbor2";
import { sortLengthFirstDeterministic } from "cbor2/sorts";

const MAX_U64 = (1n << 64n) - 1n;
const MIN_I64 = -(1n << 63n);

/** Thrown when a value cannot be canonically CBOR-encoded. */
export class CanonicalEncodingError extends Error {
  public readonly path: string;
  public constructor(path: string, message: string) {
    super(`canonical encoding error at ${path || "/"}: ${message}`);
    this.name = "CanonicalEncodingError";
    this.path = path;
  }
}

/**
 * Walk the value, rejecting anything that cannot be canonically encoded:
 *  - `undefined` (multiple representations / cbor simple value)
 *  - `NaN`, `±Infinity`
 *  - bigints outside the u64 range (the protocol's only integer width)
 *  - functions / symbols
 *  - typed arrays other than `Uint8Array` (encoder-specific behaviour)
 *
 * Plain objects with a `null`-prototype are accepted; class instances
 * (other than `Uint8Array`) are rejected — they would otherwise land via
 * `toCBOR` hooks and break determinism guarantees from outside this module.
 */
function assertCanonicalValue(v: unknown, path: string): void {
  if (v === null) return;
  switch (typeof v) {
    case "string":
    case "boolean":
      return;
    case "undefined":
      throw new CanonicalEncodingError(path, "undefined is not canonically encodable");
    case "number": {
      if (!Number.isFinite(v)) {
        throw new CanonicalEncodingError(
          path,
          `non-finite number forbidden (got ${String(v)})`,
        );
      }
      return;
    }
    case "bigint": {
      // The PoT protocol uses u64 for slots/seeds; reject anything that
      // would not round-trip cleanly through Anchor's `u64`/`i64`.
      if (v < MIN_I64 || v > MAX_U64) {
        throw new CanonicalEncodingError(
          path,
          `bigint out of [i64.min, u64.max] range: ${v.toString()}`,
        );
      }
      return;
    }
    case "function":
    case "symbol":
      throw new CanonicalEncodingError(path, `${typeof v} is not canonically encodable`);
    case "object": {
      if (v instanceof Uint8Array) return;
      if (ArrayBuffer.isView(v)) {
        throw new CanonicalEncodingError(
          path,
          "typed arrays other than Uint8Array are forbidden",
        );
      }
      if (Array.isArray(v)) {
        for (let i = 0; i < v.length; i++) {
          assertCanonicalValue(v[i], `${path}/${i}`);
        }
        return;
      }
      if (v instanceof Map) {
        // Maps are accepted by cbor2; require all keys to be canonical
        // primitives and all values canonical.
        let i = 0;
        for (const [k, val] of v) {
          assertCanonicalValue(k, `${path}/<key:${i}>`);
          assertCanonicalValue(val, `${path}/${String(k)}`);
          i++;
        }
        return;
      }
      // Plain object: reject class instances to keep encoding stable.
      const proto = Object.getPrototypeOf(v);
      if (proto !== Object.prototype && proto !== null) {
        throw new CanonicalEncodingError(
          path,
          `non-plain object (proto=${proto?.constructor?.name ?? "unknown"}) forbidden`,
        );
      }
      for (const [k, val] of Object.entries(v as Record<string, unknown>)) {
        assertCanonicalValue(val, `${path}/${k}`);
      }
      return;
    }
    default:
      throw new CanonicalEncodingError(path, `unsupported type ${typeof v}`);
  }
}

/**
 * Encode a value to canonical CBOR bytes.
 *
 * Guarantees:
 *  - Same input ⇒ same bytes, regardless of object key insertion order.
 *  - Map keys sorted length-first then lexicographically (RFC 8949 §4.2.3).
 *  - Definite-length items only; no indefinite streaming items.
 *  - Integers use the smallest legal encoding.
 *  - Floats use the shortest legal encoding (cbor2 CDE handles this).
 *  - NaN, Infinity, undefined, and out-of-range bigints throw.
 */
export function canonicalEncode(value: unknown): Uint8Array {
  assertCanonicalValue(value, "");
  return encode(value, {
    cde: true, // RFC 8949 §4.2 Core Deterministic Encoding
    sortKeys: sortLengthFirstDeterministic, // §4.2.3 length-first variant
    rejectDuplicateKeys: true,
    rejectUndefined: true,
    collapseBigInts: true,
  });
}

/**
 * Decode canonical CBOR bytes back into a JS value. Provided for
 * round-trip testing only — production consumers should rely on the
 * commitment hash, not on re-decoding.
 */
export function canonicalDecode(bytes: Uint8Array): unknown {
  return decode(bytes, {
    cde: true,
    rejectDuplicateKeys: true,
    rejectStreaming: true,
  });
}
