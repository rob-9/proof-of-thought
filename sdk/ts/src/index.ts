/**
 * `@canteen/pot` — Proof of Thought TypeScript SDK.
 *
 * Canonicalisation (CBOR + blake3), on-chain account types, program
 * helpers (PDA derivation, instruction encoding, discriminators), and a
 * high-level `ProofOfThought` client are all exported from here.
 */

export * from "./canonical/index.js";
export * from "./types.js";
export * from "./program/index.js";
export * from "./client.js";
