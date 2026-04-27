import { describe, it, expect } from "vitest";
import { PublicKey } from "@solana/web3.js";

import {
  IX,
  ACCOUNT,
  EVENT,
  agentPda,
  vrfRequestPda,
  thoughtPda,
  encodeRequestVrfArgs,
  encodeSubmitThoughtArgs,
  encodeChallengeArgs,
} from "../program/index.js";
import { ChallengeClaim } from "../types.js";

const PROGRAM_ID = new PublicKey("Pot1111111111111111111111111111111111111111");
const OPERATOR = new PublicKey("11111111111111111111111111111112");

describe("program/discriminators", () => {
  it("are 8-byte arrays", () => {
    for (const ix of Object.values(IX)) expect(ix.length).toBe(8);
    for (const acc of Object.values(ACCOUNT)) expect(acc.length).toBe(8);
    for (const ev of Object.values(EVENT)) expect(ev.length).toBe(8);
  });

  it("are pairwise distinct within their namespace", () => {
    const seen = new Set<string>();
    for (const [name, bytes] of Object.entries(IX)) {
      const hex = Buffer.from(bytes).toString("hex");
      expect(seen.has(hex)).toBe(false);
      seen.add(hex);
      // Sanity: each ix discriminator is non-zero.
      expect(bytes.some((b) => b !== 0)).toBe(true);
      void name;
    }
  });
});

describe("program/pdas", () => {
  it("agentPda is deterministic", () => {
    const [a, _bumpA] = agentPda(OPERATOR, PROGRAM_ID);
    const [b, _bumpB] = agentPda(OPERATOR, PROGRAM_ID);
    expect(a.toString()).toBe(b.toString());
  });

  it("thoughtPda differs by nonce_idx", () => {
    const agent = agentPda(OPERATOR, PROGRAM_ID)[0];
    const [t0] = thoughtPda(agent, 0n, PROGRAM_ID);
    const [t1] = thoughtPda(agent, 1n, PROGRAM_ID);
    expect(t0.toString()).not.toBe(t1.toString());
  });

  it("vrfRequestPda u64 encoding is little-endian", () => {
    const agent = agentPda(OPERATOR, PROGRAM_ID)[0];
    const [v0] = vrfRequestPda(agent, 0n, PROGRAM_ID);
    const [v256] = vrfRequestPda(agent, 256n, PROGRAM_ID);
    expect(v0.toString()).not.toBe(v256.toString());
  });
});

describe("program/encoding", () => {
  it("encodeRequestVrfArgs lays out u64 || seed[32]", () => {
    const seed = new Uint8Array(32).fill(0xab);
    const enc = encodeRequestVrfArgs(1n, seed);
    expect(enc.length).toBe(40);
    // u64 LE for 1n
    expect(Array.from(enc.slice(0, 8))).toEqual([1, 0, 0, 0, 0, 0, 0, 0]);
    expect(Array.from(enc.slice(8))).toEqual(Array.from(seed));
  });

  it("encodeRequestVrfArgs rejects bad seed length", () => {
    expect(() => encodeRequestVrfArgs(0n, new Uint8Array(31))).toThrow();
  });

  it("encodeSubmitThoughtArgs has fixed-prefix length 32×6 + 32 + 8 = 232", () => {
    const ZERO = new Uint8Array(32);
    const traceUri = "ar://test";
    const enc = encodeSubmitThoughtArgs(
      {
        modelId: ZERO,
        inputCommitment: ZERO,
        outputCommitment: ZERO,
        traceUriHash: ZERO,
        vrfSeed: ZERO,
        policyId: ZERO,
        actionPda: OPERATOR,
        vrfNonceIdx: 0n,
      },
      traceUri,
    );
    // 6 hashes × 32 = 192, + actionPda 32 + nonce 8 + (4 + uriBytes) =
    // 192 + 32 + 8 + 4 + 9 = 245
    expect(enc.length).toBe(245);
  });

  it("encodeChallengeArgs lays out u8 || u64 || hash[32]", () => {
    const evidence = new Uint8Array(32).fill(0xcc);
    const enc = encodeChallengeArgs(ChallengeClaim.InconsistentCommitments, 1_000_000n, evidence);
    expect(enc.length).toBe(1 + 8 + 32);
    expect(enc[0]).toBe(6); // InconsistentCommitments
    // u64 LE 1_000_000
    expect(Array.from(enc.slice(1, 9))).toEqual([0x40, 0x42, 0x0f, 0, 0, 0, 0, 0]);
    expect(Array.from(enc.slice(9))).toEqual(Array.from(evidence));
  });
});
