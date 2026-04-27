/**
 * Anchor discriminators for `pot_program` instructions and accounts.
 *
 * Anchor derives the 8-byte discriminator as `sha256(prefix)[..8]` where
 * `prefix` is `"global:<ix_name>"` for instructions and `"account:<Name>"`
 * for accounts. We compute these at module-load using the Web Crypto API
 * (Node 20+ ships SubtleCrypto on `globalThis.crypto`).
 *
 * **MUST stay in sync with `programs/pot_program/src/lib.rs` and the
 * watcher's compile-time discriminators in `watcher/src/types.rs`.**
 */

import { sha256 } from "@noble/hashes/sha256";

function discriminator(prefix: string): Uint8Array {
  return sha256(new TextEncoder().encode(prefix)).slice(0, 8);
}

// Instruction discriminators
export const IX = {
  registerAgent: discriminator("global:register_agent"),
  registerModel: discriminator("global:register_model"),
  registerPolicy: discriminator("global:register_policy"),
  requestVrf: discriminator("global:request_vrf"),
  submitThought: discriminator("global:submit_thought"),
  consumeThought: discriminator("global:consume_thought"),
  challenge: discriminator("global:challenge"),
  resolveUnchallenged: discriminator("global:resolve_unchallenged"),
  resolveChallenged: discriminator("global:resolve_challenged"),
  stake: discriminator("global:stake"),
  withdrawStake: discriminator("global:withdraw_stake"),
} as const;

// Account discriminators
export const ACCOUNT = {
  agentProfile: discriminator("account:AgentProfile"),
  modelRegistry: discriminator("account:ModelRegistry"),
  policy: discriminator("account:Policy"),
  thoughtRecord: discriminator("account:ThoughtRecord"),
  challenge: discriminator("account:Challenge"),
  vrfRequest: discriminator("account:VrfRequest"),
} as const;

// Event discriminators (sha256("event:<EventName>")[..8])
export const EVENT = {
  thoughtSubmitted: discriminator("event:ThoughtSubmitted"),
  thoughtFinalized: discriminator("event:ThoughtFinalized"),
  thoughtConsumed: discriminator("event:ThoughtConsumed"),
  challengeOpened: discriminator("event:ChallengeOpened"),
  challengeResolved: discriminator("event:ChallengeResolved"),
} as const;
