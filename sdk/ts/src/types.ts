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

/** Challenge claim type — see §5.1 `Challenge.claim`. */
export const ChallengeClaim = {
  ModelMismatch: 0,
  OutputMismatch: 1,
  InputOmission: 2,
  Replay: 3,
  StaleVRF: 4,
} as const;
export type ChallengeClaim = (typeof ChallengeClaim)[keyof typeof ChallengeClaim];

// ---------------------------------------------------------------------------
// AgentProfile     PDA: ["agent", agent_pubkey]
// ---------------------------------------------------------------------------
//
// Bytes:
//   0 ..   8  discriminator
//   8 ..  40  operator: Pubkey                  (32)
//  40 ..  72  stake_vault: Pubkey               (32)
//  72 ..  80  stake_amount: u64                  (8)
//  80 ..  88  reputation: i64                    (8)
//  88 ..  92  active_thoughts: u32               (4)
//  92 .. 100  cooldown_until: u64                (8)
// 100 .. 104  bump: u8 + _pad: [u8; 3]           (4)
// Total: 104 bytes.
export interface AgentProfile {
  operator: PublicKey;
  stakeVault: PublicKey;
  stakeAmount: bigint;
  reputation: bigint;
  activeThoughts: number;
  cooldownUntil: bigint;
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
// ThoughtRecord    PDA: ["thought", agent, nonce]
// ---------------------------------------------------------------------------
//
// Spec §4.1, 264 bytes incl. discriminator. Layout (from spec):
//   0 ..   8  discriminator
//   8 ..  40  agent: Pubkey                     (32)
//  40 ..  72  model_id: [u8; 32]                (32)
//  72 .. 104  input_commitment: [u8; 32]        (32)
// 104 .. 136  output_commitment: [u8; 32]       (32)
// 136 .. 168  trace_uri_hash: [u8; 32]          (32)
// 168 .. 200  vrf_seed: [u8; 32]                (32)
// 200 .. 232  policy_id: [u8; 32]               (32)
// 232 .. 240  slot: u64                          (8)
// 240 .. 272  action_pda: Pubkey                (32)  -- spec is 256 + 8 disc; see note
// 272 .. 273  status: u8                         (1)
// 273 .. 280  _pad: [u8; 7]                      (7)
//
// NOTE: spec §4.1 reads "Total: 264 bytes incl. discriminator 8" but the
// listed fields sum to 256+8 = 264 ONLY if `_pad` is 7 bytes; we follow the
// spec layout exactly. Status enum is encoded as a `u8` to match.
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
  /** 7 bytes of alignment padding. Always zero on submit; never asserted on read. */
  pad: Uint8Array;
}

// ---------------------------------------------------------------------------
// Challenge        PDA: ["challenge", thought_pda, challenger]
// ---------------------------------------------------------------------------
//
// Bytes:
//   0 ..   8  discriminator
//   8 ..  40  challenger: Pubkey                (32)
//  40 ..  48  bond: u64                          (8)
//  48 ..  49  claim: u8 (ChallengeClaim)         (1)
//  49 ..  81  evidence_uri_hash: [u8; 32]       (32)
//  81 ..  89  opened_at_slot: u64                (8)
//  89 ..  90  resolved: bool (u8)                (1)
//  90 ..  91  bump: u8                           (1)
// Total: 91 bytes (program will pad to 96 for alignment).
export interface Challenge {
  challenger: PublicKey;
  bond: bigint;
  claim: ChallengeClaim;
  evidenceUriHash: Hash32;
  openedAtSlot: bigint;
  resolved: boolean;
  bump: number;
}

// ---------------------------------------------------------------------------
// Policy           PDA: ["policy", policy_id]
// ---------------------------------------------------------------------------
//
// Bytes (variable — `allowed_models` is a bounded Vec):
//   0 ..   8  discriminator
//   8 ..  40  policy_id: [u8; 32]               (32)
//  40 ..  72  schema_uri_hash: [u8; 32]         (32)
//  72 ..  73  equiv_class: u8 (EquivClass)       (1)
//  73 ..  77  max_inference_ms: u32              (4)
//  77 ..  85  challenge_window_slots: u64        (8)
//  85 ..  93  bond_min: u64                      (8)
//  93 ..  97  allowed_models len: u32            (4)
//  97 ..  ..  allowed_models: [u8;32] × len     (32 each)
//   ..        bump: u8                           (1)
// `allowed_models` is bounded by program-level constant (default 16).
export interface Policy {
  policyId: Hash32;
  schemaUriHash: Hash32;
  equivClass: EquivClass;
  maxInferenceMs: number;
  challengeWindowSlots: bigint;
  bondMin: bigint;
  allowedModels: Hash32[];
  bump: number;
}

// ---------------------------------------------------------------------------
// VrfRequest       PDA: ["vrf", agent, nonce_idx]
// ---------------------------------------------------------------------------
//
// Bytes:
//   0 ..   8  discriminator
//   8 ..  40  agent: Pubkey                     (32)
//  40 ..  48  nonce_idx: u64                     (8)
//  48 ..  80  seed: [u8; 32]                    (32)
//  80 ..  88  requested_slot: u64                (8)
//  88 ..  96  fulfilled_slot: u64                (8)   0 == not yet fulfilled
//  96 ..  97  consumed: bool (u8)                (1)
//  97 ..  98  bump: u8                           (1)
// Total: 98 bytes (program pads to 104).
export interface VrfRequest {
  agent: PublicKey;
  nonceIdx: bigint;
  seed: Hash32;
  requestedSlot: bigint;
  /** 0 if request is still pending. */
  fulfilledSlot: bigint;
  consumed: boolean;
  bump: number;
}
