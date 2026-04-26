/**
 * blake3 hashing primitives for PoT commitments.
 *
 * The protocol settles on blake3-256 (32-byte digests) for every commitment
 * field on-chain (input_commitment, output_commitment, trace_uri_hash,
 * model_id, policy_id, manifest hash). Rationale lives in
 * `docs/adr/0003-blake3-hash-choice.md`; tl;dr — fast on every architecture
 * a watcher might run on, parallelisable for trace-bundle hashing, and
 * widely available in Solana toolchains via `blake3` syscalls.
 */

import { blake3 } from "@noble/hashes/blake3";
import { canonicalEncode } from "./cbor.js";

/** Length of every commitment field on-chain. Matches `[u8; 32]` accounts. */
export const COMMITMENT_BYTES = 32;

/** Compute blake3-256 of arbitrary bytes. Always returns 32 bytes. */
export function blake3_256(data: Uint8Array): Uint8Array {
  // `dkLen: 32` is the default but pinning it here documents the protocol
  // invariant — every PoT commitment is exactly 32 bytes.
  return blake3(data, { dkLen: COMMITMENT_BYTES });
}

/**
 * Canonically encode `value` and return its blake3-256 digest. This is the
 * one-shot helper used to derive `input_commitment` and `output_commitment`
 * from CanonicalInput / CanonicalOutput per spec §4.2 / §4.3.
 */
export function hashCommitment(value: unknown): Uint8Array {
  return blake3_256(canonicalEncode(value));
}

/** Lowercase hex of a `Uint8Array`. No `0x` prefix. */
export function toHex(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i++) {
    const b = bytes[i] ?? 0;
    out += b.toString(16).padStart(2, "0");
  }
  return out;
}

/** Lowercase hex of `hashCommitment(value)`. Convenience for tests/logging. */
export function hashHex(value: unknown): string {
  return toHex(hashCommitment(value));
}

/** Decode lowercase or mixed-case hex into a `Uint8Array`. */
export function fromHex(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  if (clean.length % 2 !== 0) {
    throw new Error(`hex string has odd length: ${clean.length}`);
  }
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) {
    const byte = Number.parseInt(clean.slice(i * 2, i * 2 + 2), 16);
    if (Number.isNaN(byte)) {
      throw new Error(`invalid hex at offset ${i * 2}: ${clean.slice(i * 2, i * 2 + 2)}`);
    }
    out[i] = byte;
  }
  return out;
}
