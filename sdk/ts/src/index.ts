/**
 * `@canteen/pot` — Proof of Thought TypeScript SDK.
 *
 * This package currently ships the canonicalisation + commitment layer.
 * The on-chain client wrapper (submit/challenge/resolve) ships in a
 * follow-up phase and will live alongside these exports.
 */

export * from "./canonical/index.js";
export * from "./types.js";
