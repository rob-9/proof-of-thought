/**
 * Determinism + schema test suite for the canonical layer.
 *
 * The locked regression vector is the load-bearing test in this file: any
 * silent change to the canonical encoder, hash function, or schema MUST
 * change the locked digest, breaking this test. Don't update the constants
 * without a spec amendment + a watcher migration plan.
 */

import { describe, expect, it } from "vitest";
import {
  buildThoughtCommitment,
  canonicalDecode,
  canonicalEncode,
  CanonicalEncodingError,
  CanonicalSchemaError,
  fromHex,
  hashCommitment,
  hashHex,
  toHex,
  validateCanonicalInput,
  type CanonicalInput,
  type CanonicalOutput,
} from "../canonical/index.js";

// ---------------------------------------------------------------------------
// Reference fixtures
// ---------------------------------------------------------------------------

function makeBytes(len: number, fill: (i: number) => number): Uint8Array {
  const arr = new Uint8Array(len);
  for (let i = 0; i < len; i++) arr[i] = fill(i) & 0xff;
  return arr;
}

const REF_INPUT: CanonicalInput = {
  system: "you are a careful market-making agent",
  messages: [{ role: "user", content: "should I buy SOL at 142.5?" }],
  tools: [],
  tool_calls: [
    {
      call: "get_price",
      response: { symbol: "SOL/USDC", px: 142.5, ts: 1740000000 },
    },
  ],
  memory_snap: new Uint8Array(32),
  context_t: 312000000n,
  vrf_seed: makeBytes(32, (i) => i),
  policy_id: makeBytes(32, (i) => 0xa0 + (i % 16)),
};

const REF_OUTPUT: CanonicalOutput = {
  decision: { action: "hold", confidence: 0.42 },
  reasoning: [
    "price within band",
    "volatility elevated",
    "wait for vrf next slot",
  ],
  tool_intents: [],
  self_score: 0.42,
  model_id: makeBytes(32, (i) => 0x10 + (i % 16)),
  sampling: {
    temperature: 0,
    top_p: 1,
    seed: 0x0123456789abcdefn,
    max_tokens: 256,
  },
};

// Locked regression vectors — computed once with the implementation in this
// repo at commit time. If you change the encoder or schema and these change,
// document the migration in the spec.
const REF_INPUT_HASH_HEX =
  "1d29e7a5b68674a6d8870e863926f1512fd1f62c16d45ff4c240eeabab813127";
const REF_OUTPUT_HASH_HEX =
  "596846348f46ac6af45dfb9ed5e7caa6a8a5ab88813177027e8294ed3323be18";

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

describe("canonicalEncode — determinism", () => {
  it("produces the same bytes when keys are reordered (top-level)", () => {
    const a = canonicalEncode(REF_INPUT);
    // Same fields, different insertion order.
    const reordered: CanonicalInput = {
      vrf_seed: REF_INPUT.vrf_seed,
      policy_id: REF_INPUT.policy_id,
      tool_calls: REF_INPUT.tool_calls,
      memory_snap: REF_INPUT.memory_snap,
      messages: REF_INPUT.messages,
      tools: REF_INPUT.tools,
      context_t: REF_INPUT.context_t,
      system: REF_INPUT.system,
    };
    const b = canonicalEncode(reordered);
    expect(toHex(a)).toBe(toHex(b));
  });

  it("produces the same bytes when keys are reordered (nested)", () => {
    const a = canonicalEncode({ a: 1, b: 2, c: { x: 1, y: 2 } });
    const b = canonicalEncode({ c: { y: 2, x: 1 }, b: 2, a: 1 });
    expect(toHex(a)).toBe(toHex(b));
  });

  it("orders {b:1,a:1} identically to {a:1,b:1}", () => {
    const a = canonicalEncode({ b: 1, a: 1 });
    const b = canonicalEncode({ a: 1, b: 1 });
    expect(toHex(a)).toBe(toHex(b));
  });

  it("encodes integer-valued floats as integers (smallest-int rule)", () => {
    // 1.0 must encode the same as 1 under CDE — both become MT 0 / value 1.
    const fromInt = canonicalEncode(1);
    const fromFloat = canonicalEncode(1.0);
    expect(toHex(fromInt)).toBe(toHex(fromFloat));
    expect(toHex(fromInt)).toBe("01");
  });

  it("encodes a known small map deterministically (length-first sort)", () => {
    // Keys "aa" (2 bytes) and "b" (1 byte) — length-first puts "b" first.
    //   a2          map(2)
    //   61 62       "b"
    //   02          unsigned 2
    //   62 61 61    "aa"
    //   01          unsigned 1
    expect(toHex(canonicalEncode({ aa: 1, b: 2 }))).toBe("a261620262616101");
    expect(toHex(canonicalEncode({ b: 2, aa: 1 }))).toBe("a261620262616101");
  });
});

// ---------------------------------------------------------------------------
// Locked regression vectors
// ---------------------------------------------------------------------------

describe("hashCommitment — locked regression vectors", () => {
  it("CanonicalInput → blake3-256 matches locked digest", () => {
    expect(hashHex(REF_INPUT)).toBe(REF_INPUT_HASH_HEX);
  });

  it("CanonicalOutput → blake3-256 matches locked digest", () => {
    expect(hashHex(REF_OUTPUT)).toBe(REF_OUTPUT_HASH_HEX);
  });

  it("hashCommitment returns 32 bytes", () => {
    expect(hashCommitment(REF_INPUT)).toHaveLength(32);
    expect(hashCommitment(REF_OUTPUT)).toHaveLength(32);
  });

  it("decoded round-trip preserves logical values", () => {
    const enc = canonicalEncode({ x: 1, y: "hi", z: [true, false, null] });
    const decoded = canonicalDecode(enc);
    expect(decoded).toEqual({ x: 1, y: "hi", z: [true, false, null] });
  });
});

// ---------------------------------------------------------------------------
// Cross-input independence
// ---------------------------------------------------------------------------

describe("canonical hash — sensitivity", () => {
  it("a 1-byte change in vrf_seed flips the input commitment", () => {
    const flipped = makeBytes(32, (i) => (i === 5 ? 0xff : i));
    const mutated: CanonicalInput = { ...REF_INPUT, vrf_seed: flipped };
    expect(hashHex(mutated)).not.toBe(REF_INPUT_HASH_HEX);
  });

  it("changing memory_snap changes the input commitment", () => {
    const mutated: CanonicalInput = {
      ...REF_INPUT,
      memory_snap: makeBytes(32, () => 0xab),
    };
    expect(hashHex(mutated)).not.toBe(REF_INPUT_HASH_HEX);
  });

  it("changing the seed inside sampling changes the output commitment", () => {
    const mutated: CanonicalOutput = {
      ...REF_OUTPUT,
      sampling: { ...REF_OUTPUT.sampling, seed: 0n },
    };
    expect(hashHex(mutated)).not.toBe(REF_OUTPUT_HASH_HEX);
  });

  it("VRF binding: different vrf_seed → different input commitment", () => {
    const seedA = makeBytes(32, (i) => i);
    const seedB = makeBytes(32, (i) => i + 1);
    const inputA: CanonicalInput = { ...REF_INPUT, vrf_seed: seedA };
    const inputB: CanonicalInput = { ...REF_INPUT, vrf_seed: seedB };
    expect(hashHex(inputA)).not.toBe(hashHex(inputB));
  });
});

// ---------------------------------------------------------------------------
// Schema validation
// ---------------------------------------------------------------------------

describe("validateCanonicalInput — schema", () => {
  it("throws CanonicalSchemaError with field path on missing field", () => {
    const bad = { ...REF_INPUT } as Partial<CanonicalInput>;
    delete bad.vrf_seed;
    expect(() => validateCanonicalInput(bad)).toThrow(CanonicalSchemaError);
    try {
      validateCanonicalInput(bad);
    } catch (e) {
      const err = e as CanonicalSchemaError;
      expect(err.path).toBe("/vrf_seed");
      expect(err.message).toContain("missing required field");
    }
  });

  it("throws on wrong-length 32-byte field", () => {
    const bad = { ...REF_INPUT, memory_snap: new Uint8Array(16) };
    expect(() => validateCanonicalInput(bad)).toThrow(/expected 32 bytes/);
  });

  it("throws on wrong type (string instead of number)", () => {
    const bad = { ...REF_INPUT, context_t: "nope" as unknown as bigint };
    expect(() => validateCanonicalInput(bad)).toThrow(CanonicalSchemaError);
  });

  it("path points into nested message field", () => {
    const bad = {
      ...REF_INPUT,
      messages: [{ role: 7 as unknown as string, content: "x" }],
    };
    try {
      validateCanonicalInput(bad);
      throw new Error("expected throw");
    } catch (e) {
      const err = e as CanonicalSchemaError;
      expect(err).toBeInstanceOf(CanonicalSchemaError);
      expect(err.path).toBe("/messages/0/role");
    }
  });
});

// ---------------------------------------------------------------------------
// Encoder rejections
// ---------------------------------------------------------------------------

describe("canonicalEncode — forbidden inputs", () => {
  it("rejects NaN", () => {
    expect(() => canonicalEncode({ x: Number.NaN })).toThrow(
      CanonicalEncodingError,
    );
  });

  it("rejects +Infinity", () => {
    expect(() => canonicalEncode({ x: Number.POSITIVE_INFINITY })).toThrow(
      CanonicalEncodingError,
    );
  });

  it("rejects -Infinity", () => {
    expect(() => canonicalEncode({ x: Number.NEGATIVE_INFINITY })).toThrow(
      CanonicalEncodingError,
    );
  });

  it("rejects undefined", () => {
    expect(() => canonicalEncode({ x: undefined })).toThrow(
      CanonicalEncodingError,
    );
  });

  it("rejects bigints out of u64 range", () => {
    expect(() => canonicalEncode({ x: 1n << 70n })).toThrow(
      CanonicalEncodingError,
    );
  });

  it("rejects functions", () => {
    expect(() =>
      canonicalEncode({ x: (() => 1) as unknown as number }),
    ).toThrow(CanonicalEncodingError);
  });

  it("rejects class instances", () => {
    class Foo {
      public a = 1;
    }
    expect(() => canonicalEncode({ x: new Foo() })).toThrow(
      CanonicalEncodingError,
    );
  });
});

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

describe("buildThoughtCommitment", () => {
  it("returns 32-byte commitments matching standalone hashCommitment", () => {
    const result = buildThoughtCommitment({
      input: REF_INPUT,
      output: REF_OUTPUT,
      traceParts: [
        { name: "canonical_input.cbor", hash: hashCommitment(REF_INPUT) },
        { name: "canonical_output.cbor", hash: hashCommitment(REF_OUTPUT) },
      ],
    });
    expect(toHex(result.inputCommitment)).toBe(REF_INPUT_HASH_HEX);
    expect(toHex(result.outputCommitment)).toBe(REF_OUTPUT_HASH_HEX);
    expect(result.traceManifest).toHaveLength(2);
    expect(result.traceManifestHash).toHaveLength(32);
  });

  it("manifest hash is deterministic across re-runs", () => {
    const args = {
      input: REF_INPUT,
      output: REF_OUTPUT,
      traceParts: [
        { name: "canonical_input.cbor", hash: hashCommitment(REF_INPUT) },
        { name: "canonical_output.cbor", hash: hashCommitment(REF_OUTPUT) },
      ],
    };
    const a = buildThoughtCommitment(args);
    const b = buildThoughtCommitment(args);
    expect(toHex(a.traceManifestHash)).toBe(toHex(b.traceManifestHash));
  });

  it("manifest hash is order-sensitive (matches spec §4.5 'ordered hashes')", () => {
    const h1 = hashCommitment(REF_INPUT);
    const h2 = hashCommitment(REF_OUTPUT);
    const a = buildThoughtCommitment({
      input: REF_INPUT,
      output: REF_OUTPUT,
      traceParts: [
        { name: "a.bin", hash: h1 },
        { name: "b.bin", hash: h2 },
      ],
    });
    const b = buildThoughtCommitment({
      input: REF_INPUT,
      output: REF_OUTPUT,
      traceParts: [
        { name: "b.bin", hash: h2 },
        { name: "a.bin", hash: h1 },
      ],
    });
    expect(toHex(a.traceManifestHash)).not.toBe(toHex(b.traceManifestHash));
  });

  it("rejects malformed trace part hashes", () => {
    expect(() =>
      buildThoughtCommitment({
        input: REF_INPUT,
        output: REF_OUTPUT,
        traceParts: [{ name: "x", hash: new Uint8Array(16) }],
      }),
    ).toThrow(/32 bytes/);
  });
});

// ---------------------------------------------------------------------------
// hex helpers
// ---------------------------------------------------------------------------

describe("hex helpers", () => {
  it("toHex/fromHex round-trip", () => {
    const bytes = makeBytes(32, (i) => i * 7);
    expect(toHex(fromHex(toHex(bytes)))).toBe(toHex(bytes));
  });

  it("fromHex strips 0x prefix", () => {
    expect(toHex(fromHex("0xdeadbeef"))).toBe("deadbeef");
  });
});
