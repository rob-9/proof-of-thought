# Glossary

Terms used throughout the Proof of Thought protocol. Cross-references to the
[design spec](specs/2026-04-26-proof-of-thought-design.md) (cited as §N.M)
and ADRs (cited as ADR-NNNN).

---

**Agent.** Off-chain process that produces ThoughtRecords and posts on-chain
commitments. Has a registered identity (`AgentProfile` PDA) and locked
stake. Spec §3.

**AgentProfile.** On-chain account at PDA `["agent", operator_pubkey]`
holding the agent's stake, reputation, active-thought counter, and
cooldown. Spec §5.1.

**Attestation.** A hardware-signed quote from a Trusted Execution
Environment (Intel TDX, AMD SEV-SNP, NVIDIA H100 CC) proving that a specific
binary measurement ran with specific input. Skips the optimistic challenge
window when verified. Spec §6.2.

**Bond.** Lamports a challenger locks when filing a `challenge`
instruction. Returned with a share of slashed stake on a successful
challenge; partially forfeited on a failed challenge (griefing tax).
ADR-0007.

**Canonical input.** Deterministically serialized representation of
everything an agent's model saw before producing a decision: system prompt,
messages, tool schemas, tool outputs, memory snapshot Merkle root, slot,
VRF seed, policy ID. Hashed to `input_commitment`. Spec §4.2.

**Canonical output.** Deterministically serialized representation of the
agent's typed decision, reasoning trace, tool intents, claimed model_id,
and sampling parameters. Hashed to `output_commitment`. Spec §4.3.

**Canonical CBOR.** Deterministic CBOR encoding per RFC 8949 §4.2: definite
lengths, smallest-int encoding, deterministic float, sorted map keys.
ADR-0002.

**Challenge.** An on-chain `challenge` instruction filed by a watcher that
disputes a thought's validity. Locks the watcher's bond and pauses the
challenge window for the disputed thought. Spec §5.2, §5.3.

**Challenge window.** Policy-defined slot count after `submit_thought`
during which a challenge can be filed. After it expires with no challenge,
`resolve` finalizes the thought. Default 150 slots (~60s). Spec §5.3.

**ChallengeClaim.** Enum tagging the type of fraud asserted: `ModelMismatch`,
`OutputMismatch`, `InputOmission`, `Replay`, `StaleVRF`, `AttestationInvalid`.
Spec §5.1, §6.

**Challenger.** A watcher (or anyone with a bond) who has filed a
challenge instruction.

**Consumer.** A Solana program (or downstream user) that gates an action
on a finalized PoT, typically via CPI to `consume_thought`. Spec §9.

**Cooldown.** Delay (in slots) after `withdraw_stake` before the agent can
register again. Prevents stake-cycling exploits.

**Determinism.** Property required of an inference run for `StrictRegime`
verification. Achieved with temperature=0, fixed seed, deterministic CUDA
kernels, pinned engine version. Spec §6.1.

**Dispute resolver.** Pubkey authorized by the policy to issue a verdict on
challenged thoughts. MVP: single hardcoded operator. Future: decentralized
juror pool or meta-PoT optimistic resolver. Spec §5.3, future-work.md.

**Equivalence class.** Policy-declared rule for what counts as "matching"
output during verification. `Strict` (byte-exact), `StructuralJSON` (decision
field only), `SemanticCommittee` (k-of-n entailment vote), `AnyOfN` (Merkle
sample-set membership). Spec §6, ADR-0005.

**Finalized.** Terminal `ThoughtStatus` indicating the thought is valid and
consumable. Reached either by passing the challenge window unchallenged or
by an attestation verifying inline.

**Freshness binding.** The technique of including a fresh VRF seed in the
canonical input so that no input commitment can be constructed before the
seed exists. Defeats replay (T1), time-warp (T8), and prompt
front-running (T7). Spec §7.

**Governance.** Pubkey authorized to register new models in the
`ModelRegistry`. MVP: hardcoded constant. Future: SPL governance DAO with
operator multisig. Spec §5.2.

**Manifest.** Ordered list of `{name, blake3}` pairs for the trace bundle's
parts (canonical_input.cbor, canonical_output.cbor, raw_provider_response,
tool I/O, attestation). Hashed to `trace_uri_hash`. Spec §4.5.

**Memory snapshot.** Merkle root of an agent's long-term key-value store,
captured at the slot when inference begins. Included in canonical input so
challengers can prove omission of declared memory. Spec §4.2.

**Mismatch.** Verifier outcome indicating a thought's claimed output does
not match what the verifier reproduced (or, for `Soft` regime, what the
committee judges equivalent). Triggers a challenge filing if EV-positive.

**ModelRegistry.** On-chain account at PDA `["model", model_id]` holding
the model's class (`OpenWeights` / `HostedAttested` / `TeeSealed`),
verifier pubkey, and TEE root CA pointer.

**model_id.** 32-byte canonical digest identifying a model. For open weights:
blake3 of safetensors bytes. For hosted: `H(provider ∥ name ∥ snapshot ∥
hosted_pubkey)`. For TEE: `H(measurement ∥ image_id)`. Spec §4.4.

**Optimistic.** Verification posture where the commitment is assumed valid
and a challenge window allows watchers to dispute. Inverse of "enforced":
verification cost is paid only on disputes. Spec §6, ADR-0001.

**PDA.** Program-Derived Address. Solana account whose address is
deterministically derived from a seed and the program ID, so the program
can sign for it.

**Policy.** Permissionless on-chain document at PDA `["policy", policy_id]`
declaring schema, equivalence class, allowed models, challenge window,
bond minimum, max action age. Consumers choose which policies to trust.
Spec §5.1.

**policy_id.** 32-byte hash of the policy's normative JSON document.
Embedded in every ThoughtRecord and verified by consumers.

**PoT.** Proof of Thought. The protocol itself; also informally the
on-chain artifact (ThoughtRecord) plus the trace.

**Pyth Entropy.** Pyth Network's on-chain VRF used as the source of fresh
randomness for `vrf_seed`. ADR-0004.

**Recursive PoT.** Pattern where the SemanticCommittee members produce
their own PoTs under StrictRegime, closing the trust recursion at a
verifiable open-weights bottom. ADR-0008.

**Replay attack.** Adversary reuses a prior thought's commitment for a
new action. Defeated by VRF-bound freshness. T1 in spec §2.

**Resolve.** On-chain instruction that terminates a thought's lifecycle.
Two variants in our impl: `resolve_unchallenged` (permissionless crank,
finalizes after window) and `resolve_challenged` (resolver-only, applies
verdict). Spec §5.3.

**Slashing.** Confiscation of agent stake on a guilty verdict. Distribution:
60% to challenger, 30% locked-burn (vault remains; agent stake_amount
zeroed), 10% treasury. ADR-0007.

**Slot.** Solana's native time unit, ~400ms. Used throughout for
challenge windows and freshness checks.

**SoftRegime.** Verification regime for non-deterministic / hosted /
unattested models. Verifier doesn't byte-compare; uses an EquivalenceClass
(StructuralJSON / SemanticCommittee / AnyOfN). Spec §6.3.

**Stake.** SOL locked by an agent in a vault PDA, slashable on guilty
verdicts. Floor recommended at 10× the maximum loss per thought it gates.
ADR-0007.

**StrictRegime.** Verification regime for open-weights deterministic
models. Watcher re-executes byte-for-byte; mismatch is fraud. Spec §6.1.

**Sybil attack.** Adversary spawns many agents to amplify a fraudulent
position. Defeated by per-agent stake. T5 in spec §2.

**TEE measurement.** Hardware-rooted hash of the binary running inside a
confidential VM, included in the attestation quote. The verifier matches
this against a registered model's expected measurement.

**ThoughtRecord.** The on-chain commitment account: 256-byte fixed layout
holding agent, model_id, input/output commitments, trace_uri_hash, vrf_seed,
slot, action_pda, policy_id, status. Spec §4.1.

**ThoughtStatus.** Lifecycle enum: `Pending`, `Challenged`, `Finalized`,
`Slashed`. Spec §5.3.

**ThoughtSubmitted.** Anchor event emitted by `submit_thought`, the
primary signal watchers subscribe to. Carries `thought_pda`, `agent`,
`model_id`, `trace_uri`.

**Trace bundle.** Off-chain tarball stored on Arweave or Shadow Drive
containing the full trace: canonical input/output, raw provider response,
tool I/O with provenance, memory inclusion proofs, optional attestation.
Hashed to `trace_uri_hash`. Spec §4.5.

**Trace URI.** Address of the trace bundle in storage. Logged in
`ThoughtSubmitted` event; its hash is the on-chain `trace_uri_hash` so
swaps are detectable.

**Verifier.** Off-chain logic that takes a thought event + bundle and
returns `Match`, `Mismatch`, or `Inconclusive`. Concrete verifiers:
`StrictVerifier`, `AttestedVerifier`, `SoftVerifier`. Spec §8.

**VRF seed.** 32-byte freshness nonce produced by Pyth Entropy, requested
before inference and embedded in canonical input. Single-use,
per-(agent, nonce_idx). Spec §7.

**VrfRequest.** On-chain account at PDA `["vrf", agent, nonce_idx]`
holding the seed, the request slot, and a `consumed` flag.

**Watcher.** Permissionless off-chain daemon that subscribes to
`ThoughtSubmitted` events, fetches traces, runs verifiers, and files
challenges. Earns from slashed stake. Spec §3, §8.

**WatcherProfile.** Future on-chain account capturing watcher reputation
(successful/failed challenges, time active, slash-collected), used by
consumers and policies to weight challenge signals.
