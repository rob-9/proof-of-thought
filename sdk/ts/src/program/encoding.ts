/**
 * Borsh-compatible serialization for the small set of ix args the SDK emits.
 *
 * We intentionally do NOT pull in a Borsh library — the args are tiny and
 * fixed-shape, hand-rolling the encoders is cheaper than carrying a
 * dependency we'd have to keep in sync with the program's `AnchorSerialize`
 * derives. Every helper here matches a specific instruction's args struct.
 */

import { PublicKey } from "@solana/web3.js";

export function encodeU64Le(n: bigint): Uint8Array {
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigUint64(0, n, true);
  return out;
}

export function encodeU32Le(n: number): Uint8Array {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, n, true);
  return out;
}

export function encodeString(s: string): Uint8Array {
  const bytes = new TextEncoder().encode(s);
  const out = new Uint8Array(4 + bytes.length);
  new DataView(out.buffer).setUint32(0, bytes.length, true);
  out.set(bytes, 4);
  return out;
}

/** Concatenate byte arrays in order. */
export function concat(...parts: (Uint8Array | number[])[]): Uint8Array {
  let total = 0;
  for (const p of parts) total += p.length;
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p instanceof Uint8Array ? p : Uint8Array.from(p), off);
    off += p.length;
  }
  return out;
}

/** Args for `request_vrf(nonce_idx: u64, seed: [u8; 32])`. */
export function encodeRequestVrfArgs(nonceIdx: bigint, seed: Uint8Array): Uint8Array {
  if (seed.length !== 32) throw new Error("seed must be 32 bytes");
  return concat(encodeU64Le(nonceIdx), seed);
}

/** Args struct for `submit_thought` mirrors `ThoughtRecordArgs` in the program. */
export interface ThoughtRecordArgs {
  modelId: Uint8Array;
  inputCommitment: Uint8Array;
  outputCommitment: Uint8Array;
  traceUriHash: Uint8Array;
  vrfSeed: Uint8Array;
  policyId: Uint8Array;
  actionPda: PublicKey;
  vrfNonceIdx: bigint;
}

export function encodeSubmitThoughtArgs(
  args: ThoughtRecordArgs,
  traceUri: string,
): Uint8Array {
  const f32 = (b: Uint8Array, name: string) => {
    if (b.length !== 32) throw new Error(`${name} must be 32 bytes, got ${b.length}`);
    return b;
  };
  return concat(
    f32(args.modelId, "modelId"),
    f32(args.inputCommitment, "inputCommitment"),
    f32(args.outputCommitment, "outputCommitment"),
    f32(args.traceUriHash, "traceUriHash"),
    f32(args.vrfSeed, "vrfSeed"),
    f32(args.policyId, "policyId"),
    args.actionPda.toBytes(),
    encodeU64Le(args.vrfNonceIdx),
    encodeString(traceUri),
  );
}

/** Args for `challenge(claim: u8, bond: u64, evidence_uri_hash: [u8;32])`. */
export function encodeChallengeArgs(
  claim: number,
  bond: bigint,
  evidenceUriHash: Uint8Array,
): Uint8Array {
  if (evidenceUriHash.length !== 32) throw new Error("evidenceUriHash must be 32 bytes");
  return concat(Uint8Array.from([claim & 0xff]), encodeU64Le(bond), evidenceUriHash);
}
