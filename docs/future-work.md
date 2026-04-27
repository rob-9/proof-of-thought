# Future Work

Concrete roadmap items to take Proof of Thought from hackathon MVP to
production-grade primitive. Each entry has motivation, a design sketch,
dependencies, and a T-shirt size estimate.

T-shirt sizes (assume one strong protocol engineer, modulo external blockers):
**S** ≤ 1 week, **M** 2–4 weeks, **L** 1–2 months, **XL** 3+ months,
**XXL** open-ended research.

---

## Live Pyth Entropy integration — replace VRF stub

**Motivation.** The MVP `request_vrf` instruction accepts a caller-supplied
seed. That's a placeholder. Real freshness binding (spec §7) requires Pyth
Entropy as the on-chain VRF, with the seed delivered via a Pyth-signed
callback.

**Sketch.** Replace `request_vrf` with a CPI to Pyth Entropy's
`request_v2`. Add a `vrf_callback` ix that Pyth's keeper invokes; store the
delivered randomness in the existing `VrfRequest` PDA, gated to that PDA's
lamport-funded request only. Update the SDK's `requestVRF()` to await the
callback before returning.

**Dependencies.** Pyth Entropy on devnet (live). Anchor IDL stability for
Pyth's CPI types.

**Effort.** S.

---

## Real Arweave + Shadow Drive uploaders

**Motivation.** The trace bundle (spec §4.5) is the audit record. Today's
SDK builds the bundle in memory but doesn't ship it anywhere. Watchers can't
verify what they can't fetch.

**Sketch.** Add `sdk/ts/src/storage/{arweave.ts, shadow.ts}` with a common
`TraceUploader` interface. Arweave: bundle via `arbundles` and pay via the
SOL→AR bridge or held AR balance. Shadow Drive: native Solana program calls.
Both return a content-addressed URI. The SDK's `submit()` flow uploads
synchronously before posting the on-chain commit. Watcher's `TraceFetcher`
already has the trait; just needs verified hash retrievals.

**Dependencies.** Arweave wallet funding strategy (sponsorship vs.
per-trace SOL→AR swap). Shadow Drive program upgrade history (account size
limits).

**Effort.** M (size driven by funding/billing UX, not protocol code).

---

## Real Intel TDX quote verification with DCAP cert chain

**Motivation.** The watcher's `AttestedVerifier` parses a TDX quote
structure but does not validate the signature against Intel's root CA.
Without this, the attested regime (spec §6.2) is theatre.

**Sketch.** Pull Intel's DCAP/QvE quote-verification library (or its Rust
port — `dcap-quote-verifier` / `intel-tee-attestation-services`). The
verifier flow: parse → fetch PCK certificate from Intel PCS → verify QE
identity against TCB info → validate quote signature → check TCB level for
revocations. Cache the certs aggressively; PCS calls are slow.

**Dependencies.** Intel PCS endpoint reliability. Trusted source for the
SGX/TDX TCB JSON.

**Effort.** M.

---

## AMD SEV-SNP and NVIDIA H100 CC attestation

**Motivation.** Production agents will run on heterogeneous hardware. Locking
to Intel TDX is a single-vendor risk and excludes the GPUs frontier models
actually run on (NVIDIA).

**Sketch.** Add new variants to the on-chain `ModelRegistry` model class
enum: `TeeAmdSnp`, `TeeNvidiaCc`. Each gets its own verifier in the watcher
(`watcher/src/verify/attested_amd.rs`, `attested_nvidia.rs`). NVIDIA's CC
attestation chain goes via NVIDIA's RIM and NRAS services; AMD SNP via the
ASK/ARK/VCEK chain. Each path produces a normalized `AttestationOutcome` that
plugs into the existing dispute-skip machinery.

**Dependencies.** Hardware access for testing. NVIDIA NRAS API stability.

**Effort.** L.

---

## Real LLM committee members

**Motivation.** `SoftRegime → SemanticCommittee` (spec §6.3) currently
ships with `NoopMember` impls. The protocol promises that closed-API agents
are kept honest by an independent committee; today, that promise is unfulfilled.

**Sketch.** Start with one open-weights local member (Llama-3.3-70B-Instruct
via vLLM) and two closed-API members (gpt-4o-mini via Confidential API,
claude-haiku-4.5 via Confidential API). Wrap each in a
`CommitteeMember` impl that produces its own PoT under `StrictRegime`
(open-weights member) or `AttestedRegime` (closed-API members). Quorum logic
already exists; the missing part is the actual model-call wrappers + judgement
prompt + canonical output normalization.

**Dependencies.** OpenAI / Anthropic Confidential API GA (in progress). vLLM
deployment infra. Cost budget per committee judgement.

**Effort.** L.

---

## Memory Merkleization library

**Motivation.** Spec §4.2 requires `memory_snap` to be the Merkle root of
the agent's KV store as of the inference slot. Open question §13.4 flagged
the per-write cost. Without an efficient library, agents either skip memory
inclusion (defeats T3) or pay O(n) per write.

**Sketch.** A small Rust + TS library (`pot-merkle-kv`) implementing a
sparse Merkle tree over a fixed key space (BLAKE3-keyed). O(log n) updates,
inclusion proofs, non-inclusion proofs. Persistence via RocksDB (Rust) or
SQLite (TS). Watcher uses the same library to verify policy-required keys
were included.

**Dependencies.** None.

**Effort.** M.

---

## zkML for verifier-model paths

**Motivation.** [ADR-0008](adr/0008-recursive-pot-committee.md) closes the
trust recursion only if the bottom layer is verifiable. Open-weights small
models under StrictRegime are byte-exact verifiable today, but only if a
watcher actually re-executes. Adding a zkML proof option lets the *agent*
ship the proof inline and skip the challenge window for the committee
member's PoT.

**Sketch.** Use EZKL to prove a 100M-class verifier model. The proof is
posted alongside the committee member's `ThoughtRecord` and verified
on-chain. Replaces the StrictRegime byte-compare path for that specific
model class.

**Dependencies.** EZKL maturity for transformer-style models. Solana
program-side verifier (likely Halo2 verifier port — ongoing work in the
Solana ecosystem).

**Effort.** XL.

---

## Cross-chain PoT (Wormhole verifiers on EVM)

**Motivation.** The agent economy is multi-chain. An agent on Solana making
a decision that triggers an action on Ethereum should carry its PoT across
the bridge.

**Sketch.** Wormhole VAA emission on `consume_thought` containing a
canonicalized PoT receipt. EVM-side `PoTReceiptVerifier` contract validates
the VAA and exposes a `requirePoT(receipt) → bool` modifier for downstream
contracts. Same for the other direction (LayerZero, Hyperlane).

**Dependencies.** Wormhole guardian set, EVM gas budget for verification.

**Effort.** L.

---

## Streaming / multi-step thought chains

**Motivation.** Real agents reason in loops: plan → tool call → observe →
revise → act. PoT today commits one inference per ThoughtRecord. A multi-step
plan needs N PoTs and a structural integrity proof that they form a single
chain.

**Sketch.** Add `parent_thought` and `step_idx` fields to ThoughtRecord.
Define a `ThoughtChain` aggregator on-chain: a Merkle accumulator of step
ThoughtRecords with a final `terminal_thought` that gates the action.
SDK gains `pot.thinkChain(seed, steps)` that pipelines per-step commits.
Policy schema gains `max_chain_depth` and `chain_integrity_required`.

**Dependencies.** Memory Merkleization library (above).

**Effort.** L.

---

## Privacy-preserving traces (selective disclosure)

**Motivation.** Some traces contain user PII or proprietary tool outputs.
Today, traces are public. A privacy mode would let the agent encrypt the
trace to a recipient set and disclose only enough to satisfy verification.

**Sketch.** Trace bundle is encrypted to a threshold-key composed of the
consumer + a watcher quorum. Watchers receive a redacted view (input
commitment, output commitment, sampling, attestation) sufficient to detect
fraud, but not the prompt content. Consumer alone holds the full key.
Composes with TEE attestation: the TEE seals the key.

**Dependencies.** Threshold encryption library (BLS or Pedersen-based),
key-management UX.

**Effort.** XL.

---

## Formal threat model + audit

**Motivation.** Spec §2 is an informal threat model. Production deployment
requires a formal model and an external audit.

**Sketch.** Engage Trail of Bits, OtterSec, or Sec3 for a 4–6 week audit.
Pre-audit: produce a Coq/TLA+ model of the slashing flow's safety and
liveness properties (no double-slash, no fund-loss, no challenge bypass).
Post-audit: bug bounty (Immunefi).

**Dependencies.** Auditor availability. Budget.

**Effort.** L (audit) + M (formal model) + ongoing (bounty).

---

## Mainnet governance: multisig → DAO transition

**Motivation.** Spec §5.2 has `register_model` gated to a hardcoded
governance pubkey. That's fine for a hackathon; mainnet needs progressive
decentralization.

**Sketch.** Phase 0: multi-sig (3-of-5) operator. Phase 1: SPL governance
DAO with veto from the operator multisig. Phase 2: full DAO with
operator's veto sunset. Migration plan locks each transition behind a
public timelock.

**Dependencies.** SPL governance program. DAO charter drafted with legal
review.

**Effort.** M (phase 0) + L (phase 1) + ongoing.

---

## Decentralized dispute resolver

**Motivation.** Spec §5.3 documents that `resolve` for challenged thoughts
needs an authorized resolver — a single pubkey in the MVP. Production needs
a permissionless or multi-party resolver.

**Sketch.** Two designs to evaluate:
- **Kleros-style juror pool.** Jurors stake, are randomly selected per
  dispute, vote, and earn fees on the majority side. Schelling point
  enforcement.
- **Meta-PoT optimistic resolver.** The resolver itself is an agent emitting
  a PoT on the dispute decision. Recursive optimistic challenge — turtles all
  the way down, but composes cleanly with the rest of the protocol.

We'd prototype both and ship the simpler.

**Dependencies.** Stake/bond calibration; juror selection RNG (Pyth
Entropy again).

**Effort.** XL.

---

## Watcher reputation system

**Motivation.** Soft-regime mismatches (spec §6.3) are inherently fuzzy. A
malicious watcher can grief honest agents by submitting EquivalenceClass
challenges in marginal cases. Slashing the watcher on a failed challenge
helps but isn't perfect — repeated near-misses by the same watcher are a
red flag the protocol can't currently capture.

**Sketch.** Maintain on-chain `WatcherProfile { successful_challenges,
failed_challenges, slash_collected, time_active }`. Expose decay-weighted
reputation in events. Consumers and policy authors weight watcher
challenges by reputation when deciding finalization. Reputation is
non-transferable; sybil-resistant via tied stake.

**Dependencies.** Reputation aggregation primitive (Sybil-resistance is the
hard part).

**Effort.** M.

---

## Economic simulation suite (agent-based)

**Motivation.** Stake floor, bond size, slashing distribution, challenge
window — all are calibrated by intuition in the MVP
([ADR-0007](adr/0007-stake-bond-economics.md)). Production deployment
should be backed by an agent-based simulation that explores the parameter
space against a panel of adversaries.

**Sketch.** A simulator (Python or Rust) that models: agent populations
with mixed honest/lazy/malicious behaviors, watcher populations with
varying activity rates, challenge-window distributions, and external
shocks (price gaps, oracle failures). Outputs: expected loss per parameter
set, time-to-fraud-detection, watcher break-even revenue. Dashboards
parameter sweeps over (stake floor × challenge window × slash
distribution).

**Dependencies.** Adversary library — published exemplar attacks against
optimistic rollup designs are a starting point.

**Effort.** L.

---

## Smaller-but-important items

A grab bag of smaller follow-ups that don't merit their own section:

- **Replace `slash`-as-internal with explicit dispatch** so a future
  multi-resolver can call it (program TODO in `instructions/slash.rs`).
- **Replace governance hardcoded pubkey** with SPL governance derive
  (program TODO in `lib.rs`).
- **SDK Anchor IDL bindings** — generate strict TS types from the program
  IDL once `anchor build` is wired in CI.
- **Watcher metrics exporter** — Prometheus endpoint for thoughts/sec,
  challenge filings/sec, verdicts, EV-rejection counts.
- **CI watchers as a fallback** funded by the protocol treasury, so a thin
  open-watcher network in early days doesn't leave fraud undetected.
- **`AnyOfNEquiv` Merkle non-membership proof verifier** — currently a
  skeleton. Useful for high-temperature creative agents.
- **Policy schema docs** — a normative JSON Schema for the policy doc
  hashed into `policy_id`.
- **CLI tool `pot-cli`** — register agents, request VRF, submit thoughts,
  inspect status, watch logs. Useful for debugging and for the demo flow.

---

## What's intentionally NOT on this list

- **Privacy of model weights (closed-weights inference verification without
  the provider).** This requires either MPC (rejected per
  [alternatives.md §3](alternatives.md#3-multi-party-compute-mpc-inference))
  or zkML over a closed model (impossible without weights). The protocol's
  position is: hosted closed-weights agents accept the host as a trust
  party, made explicit via `model_id` class.
- **Universal cross-chain message bus.** Wormhole/LayerZero/Hyperlane each
  have positions; PoT picks one (Wormhole, above) and ships rather than
  abstracting over all of them.
- **General-purpose agent framework.** PoT is a primitive, not a framework.
  Frameworks (Solana Agent Kit, Frames.ag) integrate PoT; PoT does not
  reproduce them.
