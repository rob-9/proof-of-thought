/**
 * TypeScript mirrors of the on-chain Anchor accounts.
 *
 * These types do NOT serialize/deserialize on their own — the full client
 * wrapper (Phase B) will own (de)coders generated from the Anchor IDL. The
 * shapes here are the load-bearing surface SDK consumers see and the source
 * of truth for byte-offset comments in the spec.
 *
 * Conventions:
 *  - `PublicKey` for any 32-byte Solana key.
 *  - `Uint8Array` (always exactly 32 bytes) for any opaque digest field
 *    (`model_id`, `*_commitment`, `*_hash`, `vrf_seed`).
 *  - `bigint` for `u64`/`i64` to avoid silent precision loss past 2^53.
 *  - String enums where the program uses Anchor enums; documented in JSDoc.
 *
 * Each account interface lists the byte layout immediately above the
 * declaration so a watcher / re-implementation can verify the wire format
 * matches. Anchor prepends an 8-byte discriminator to every account; the
 * "Total" line includes it.
 */

import type { PublicKey } from "@solana/web3.js";

// ---------------------------------------------------------------------------
// Primitive helpers
// ---------------------------------------------------------------------------

/** Branded `[u8; 32]` digest. The brand is purely advisory — no runtime cost. */
export type Hash32 = Uint8Array & { readonly __brand?: "Hash32" };

// ---------------------------------------------------------------------------
// Enums (numeric — must match `#[repr(u8)]` discriminants on-chain)
// ---------------------------------------------------------------------------

/** ThoughtRecord status — see §4.1 `status`. */
export const ThoughtStatus = {
  Pending: 0,
  Challenged: 1,
  Finalized: 2,
  Slashed: 3,
} as const;
export type ThoughtStatus = (typeof ThoughtStatus)[keyof typeof ThoughtStatus];

/** Model class — see §4.4. */
export const ModelClass = {
  /** `model_id = blake3(safetensors_bytes)`. */
  OpenWeights: 0,
  /** `model_id = H(provider ∥ model_name ∥ snapshot_date ∥ hosted_pubkey)`. */
  Hosted: 1,
  /** `model_id = H(measurement ∥ image_id)`. */
  TeeFronted: 2,
} as const;
export type ModelClass = (typeof ModelClass)[keyof typeof ModelClass];

/** Equivalence class for soft-comparison verification — see §6.3. */
export const EquivClass = {
  Strict: 0,
  StructuralJSON: 1,
  SemanticCommittee: 2,
  AnyOfN: 3,
} as const;
export type EquivClass = (typeof EquivClass)[keyof typeof EquivClass];

/** Challenge claim type — see §5.1 `Challenge.claim`.
 *
 * **MUST stay in sync with `programs/pot_program/src/state/challenge.rs`
 * `ChallengeClaim`.** The numeric values are the on-chain wire format.
 */
export const ChallengeClaim = {
  ModelMismatch: 0,
  OutputMismatch: 1,
  InputOmission: 2,
  Replay: 3,
  StaleVRF: 4,
  AttestationInvalid: 5,
  /** Agent's `output_commitment` does not match blake3 of canonical_output bytes. */
  InconsistentCommitments: 6,
} as const;
export type ChallengeClaim = (typeof ChallengeClaim)[keyof typeof ChallengeClaim];

// ---------------------------------------------------------------------------
// AgentProfile     PDA: ["agent", agent_pubkey]
// ---------------------------------------------------------------------------
//
// Mirrors `programs/pot_program/src/state/agent.rs` AgentProfile (LEN 109).
// Bytes:
//   0 ..   8  discriminator
//   8 ..  40  operator: Pubkey                  (32)
//  40 ..  72  stake_vault: Pubkey               (32)
//  72 ..  80  stake_amount: u64                  (8)
//  80 ..  88  reputation: i64                    (8)
//  88 ..  92  active_thoughts: u32               (4)
//  92 .. 100  cooldown_until: u64                (8)
// 100 .. 108  vrf_nonce: u64                     (8)
// 108 .. 109  bump: u8                           (1)
// Total: 109 bytes.
export interface AgentProfile {
  operator: PublicKey;
  stakeVault: PublicKey;
  stakeAmount: bigint;
  reputation: bigint;
  activeThoughts: number;
  cooldownUntil: bigint;
  /** Monotonic counter consumed by `request_vrf` to derive VrfRequest PDAs. */
  vrfNonce: bigint;
  bump: number;
}

// ---------------------------------------------------------------------------
// ModelRegistry    PDA: ["model", model_id]
// ---------------------------------------------------------------------------
//
// Bytes:
//   0 ..   8  discriminator
//   8 ..  40  model_id: [u8; 32]                (32)
//  40 ..  41  class: u8 (ModelClass)             (1)
//  41 ..  73  verifier_pubkey: Pubkey           (32)
//  73 .. 105  tee_root_ca: Pubkey               (32)
// 105 .. 137  registered_by: Pubkey             (32)
// 137 .. 138  bump: u8                           (1)
// Total: 138 bytes (program may pad to 144).
export interface ModelRegistry {
  modelId: Hash32;
  class: ModelClass;
  verifierPubkey: PublicKey;
  teeRootCa: PublicKey;
  registeredBy: PublicKey;
  bump: number;
}

// ---------------------------------------------------------------------------
// ThoughtRecord    PDA: ["thought", agent, vrf_nonce_idx_le]
// ---------------------------------------------------------------------------
//
// Spec §4.1 specifies 264 bytes including discriminator. The on-chain
// program adds 5 lifecycle-bookkeeping fields beyond the spec
// (`attestation_verified`, `challenge_deadline_slot`, `consumed_count`,
// `vrf_nonce_idx`, `bump`) that are load-bearing for §5.3 lifecycle
// correctness. Total grows to 302 bytes.
//
// Byte layout (mirrors `programs/pot_program/src/state/thought.rs`):
//   0 ..   8  discriminator
//   8 ..  40  agent: Pubkey                     (32)
//  40 ..  72  model_id: [u8; 32]                (32)
//  72 .. 104  input_commitment: [u8; 32]        (32)
// 104 .. 136  output_commitment: [u8; 32]       (32)
// 136 .. 168  trace_uri_hash: [u8; 32]          (32)
// 168 .. 200  vrf_seed: [u8; 32]                (32)
// 200 .. 232  policy_id: [u8; 32]               (32)
// 232 .. 240  slot: u64                          (8)
// 240 .. 272  action_pda: Pubkey                (32)
// 272 .. 273  status: u8                         (1)
// 273 .. 274  attestation_verified: bool (u8)    (1)
// 274 .. 282  challenge_deadline_slot: u64       (8)
// 282 .. 286  consumed_count: u32                (4)
// 286 .. 294  vrf_nonce_idx: u64                 (8)
// 294 .. 295  bump: u8                           (1)
// 295 .. 302  _pad: [u8; 7]                      (7)
export interface ThoughtRecord {
  agent: PublicKey;
  modelId: Hash32;
  inputCommitment: Hash32;
  outputCommitment: Hash32;
  traceUriHash: Hash32;
  vrfSeed: Hash32;
  policyId: Hash32;
  slot: bigint;
  actionPda: PublicKey;
  status: ThoughtStatus;
  /** True iff a TEE attestation skipped the challenge window. */
  attestationVerified: boolean;
  /** Slot after which `resolve_unchallenged` may finalize. */
  challengeDeadlineSlot: bigint;
  /** Number of consumer CPIs that have spent this thought. */
  consumedCount: number;
  /** Monotonic per-agent nonce; second component of the PDA seed. */
  vrfNonceIdx: bigint;
  /** Bump cached at submit time. */
  bump: number;
  /** 7 bytes of alignment padding. Always zero on submit; never asserted on read. */
  pad: Uint8Array;
}

// ---------------------------------------------------------------------------
// Challenge        PDA: ["challenge", thought_pda, challenger]
// ---------------------------------------------------------------------------
//
// Mirrors `programs/pot_program/src/state/challenge.rs` Challenge (LEN 124).
// Bytes:
//   0 ..   8  discriminator
//   8 ..  40  thought: Pubkey                   (32)
//  40 ..  72  challenger: Pubkey                (32)
//  72 ..  80  bond: u64                          (8)
//  80 ..  81  claim: u8 (ChallengeClaim)         (1)
//  81 .. 113  evidence_uri_hash: [u8; 32]       (32)
// 113 .. 121  opened_at_slot: u64                (8)
// 121 .. 122  resolved: bool (u8)                (1)
// 122 .. 123  verdict: bool (u8)                 (1)  meaningful when resolved
// 123 .. 124  bump: u8                           (1)
// Total: 124 bytes.
export interface Challenge {
  /** ThoughtRecord PDA this challenge disputes. */
  thought: PublicKey;
  challenger: PublicKey;
  bond: bigint;
  claim: ChallengeClaim;
  evidenceUriHash: Hash32;
  openedAtSlot: bigint;
  resolved: boolean;
  /** True iff challenger won (agent guilty). Only meaningful when resolved. */
  verdict: boolean;
  bump: number;
}

// ---------------------------------------------------------------------------
// Policy           PDA: ["policy", policy_id]
// ---------------------------------------------------------------------------
//
// Mirrors `programs/pot_program/src/state/policy.rs` Policy.
// `MAX_ALLOWED_MODELS = 16` (program constant).
// Bytes (fixed — Anchor reserves max length for the bounded Vec):
//   0 ..   8  discriminator
//   8 ..  40  policy_id: [u8; 32]               (32)
//  40 ..  72  schema_uri_hash: [u8; 32]         (32)
//  72 ..  73  equiv_class: u8 (EquivClass)       (1)
//  73 ..  77  max_inference_slots: u32           (4)
//  77 ..  85  max_action_age_slots: u64          (8)
//  85 ..  93  challenge_window_slots: u64        (8)
//  93 .. 101  bond_min: u64                      (8)
// 101 .. 133  resolver: Pubkey                  (32)
// 133 .. 165  treasury: Pubkey                  (32)
// 165 .. 169  allowed_models len: u32            (4)
// 169 .. 681  allowed_models: [u8;32] × 16     (32 × 16 = 512)
// 681 .. 682  bump: u8                           (1)
// Total: 682 bytes.
export interface Policy {
  policyId: Hash32;
  schemaUriHash: Hash32;
  equivClass: EquivClass;
  /** Cap on agent inference latency in slots — VRF freshness gate. */
  maxInferenceSlots: number;
  /** Slots a finalized thought stays valid for `consume_thought`. */
  maxActionAgeSlots: bigint;
  challengeWindowSlots: bigint;
  bondMin: bigint;
  /** Authorized resolver for challenged thoughts. Single-key MVP. */
  resolver: PublicKey;
  /** Treasury account that receives the protocol cut on slash/finalize. */
  treasury: PublicKey;
  /** Whitelist of model_ids accepted under this policy. Max 16 entries. */
  allowedModels: Hash32[];
  bump: number;
}

// ---------------------------------------------------------------------------
// VrfRequest       PDA: ["vrf", agent, nonce_idx_le_bytes]
// ---------------------------------------------------------------------------
//
// Mirrors `programs/pot_program/src/state/vrf.rs` VrfRequest (LEN 90).
// Bytes:
//   0 ..   8  discriminator
//   8 ..  40  agent: Pubkey                     (32)
//  40 ..  48  nonce_idx: u64                     (8)
//  48 ..  80  seed: [u8; 32]                    (32)
//  80 ..  88  request_slot: u64                  (8)
//  88 ..  89  consumed: bool (u8)                (1)
//  89 ..  90  bump: u8                           (1)
// Total: 90 bytes.
//
// Note: the program does not separate `requested_slot` / `fulfilled_slot`
// today — Pyth Entropy CPI is stubbed (caller supplies the seed). When the
// real callback flow lands, a `fulfilled_slot` field will be added.
export interface VrfRequest {
  agent: PublicKey;
  nonceIdx: bigint;
  seed: Hash32;
  requestSlot: bigint;
  consumed: boolean;
  bump: number;
}
