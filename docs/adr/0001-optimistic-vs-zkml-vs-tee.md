# ADR 0001 — Optimistic + cryptoeconomic base, TEE opt-in, zkML deferred

- Status: Accepted (2026-04-26)
- Deciders: PoT core
- Supersedes: —
- Superseded by: —

## Context

PoT must answer one question on chain: "did the agent actually run the
model it claims, on the inputs it claims, at the time it claims?" Three
families of primitive can plausibly answer that question (spec §2):

1. **zkML** — a SNARK proving `output = M(input)`. Tools include EZKL,
   RiscZero, Modulus Labs, and Giza.
2. **TEE-attested inference** — Intel TDX, AMD SEV-SNP, NVIDIA H100 CC.
   Hardware signs a quote over `H(input ∥ output ∥ nonce)`.
3. **Optimistic + cryptoeconomic** — commit, reveal, challenge, slash.
   Watchers re-execute and file fraud proofs.

The hackathon window is four weeks (Apr 6 – May 11). The protocol must
work for frontier LLMs (>100B parameters, often hosted/closed) on
day one, not only for tiny demo models. It must also resist a strong
threat model (T1–T8 in spec §1) including model substitution and
time-warp attacks.

zkML's prover cost is the dominant constraint. As of Q1 2026, the
public state of the art (EZKL, Modulus benchmarks, Giza demos) proves
transformers in roughly the 10M–100M parameter range in tens of minutes
per inference on a serious machine. Order-of-magnitude figures are
stated cautiously here because vendor numbers shift monthly; the point
is that frontier-LLM zkML is still several orders of magnitude away
from real-time agent action gating. Even optimistic projections do not
close that gap inside a month.

TEEs solve the cost problem (one signature check on chain) but
introduce a hardware trust assumption with a non-trivial side-channel
history (Foreshadow, ÆPIC, multiple SGX breaks). More importantly, most
hosted frontier APIs (OpenAI, Anthropic, xAI) do not currently expose
TEE-attested endpoints with verifiable quotes a third party can check.
A TEE-only protocol would lock out the dominant model providers.

Optimistic systems shipped at scale on Ethereum (Optimism, Arbitrum,
Cartesi) and inspired EigenLayer's slashing model. The pattern is well
understood: a finalization delay plus a 1-of-N honest-watcher
assumption gives strong guarantees with no exotic crypto. The cost is
latency.

## Decision

PoT uses **optimistic + cryptoeconomic as the base layer**, with
**TEE attestation as an opt-in upgrade tier** that short-circuits the
challenge window when present, and **zkML deferred to future work**
(see [ADR 0008](0008-recursive-pot-committee.md) and
[`future-work.md`](../future-work.md)) for the verifier-model path.

Concretely:

- The submitted hackathon deliverable is the optimistic mechanism plus
  a TEE-quote *parser* (signature + cert-chain check) wired to a
  mocked attestation. Real Intel TDX / AMD SEV-SNP / NVIDIA H100 CC
  integration is post-hackathon.
- A policy may declare a TEE-CA root and require attested mode; in that
  mode `challenge_window_slots` collapses to zero.
- zkML is reserved for proving small SemanticCommittee verifier-model
  runs (sub-100M params) once EZKL throughput improves. The base
  optimistic layer never depends on it.

## Consequences

### Positive

- Ships in four weeks. Optimistic mechanisms are the only family that
  meaningfully covers all three regimes — open weights, hosted closed,
  TEE-fronted — without ground-breaking new crypto.
- Generalizes to any model. The base does not care whether the model is
  Llama-3.3-70B local, GPT-5 hosted, or Claude in a TEE.
- Composes with the upgrade path: when a hosted provider eventually
  ships TEE-CC, the same protocol absorbs it via policy with no
  protocol change.
- Cryptoeconomic incentives align watchers (slashed stake pays them) and
  are well-understood at scale.

### Negative

- **Finalization delay.** Default 150 slots (~60s) on Solana, longer for
  high-value actions. This rules out PoT for sub-second flows like MEV
  back-runs unless TEE attestation is available.
- **1-of-N honest-watcher assumption.** If the watcher network is thin
  early on, a colluding watcher set could let fraud through. We mitigate
  with a treasury-funded CI watcher backstop ([ADR 0007](0007-stake-bond-economics.md))
  and economic incentives.
- **No real-time guarantee.** During the challenge window the action is
  pending. Consumers must understand "Pending" ≠ "Finalized" and gate
  irreversible actions on the latter.

### Neutral

- TEE quotes act as a fast lane, not a default. Operators must opt in
  per policy, accepting the hardware trust assumption explicitly.
- The protocol is forward-compatible with zkML: `model_id` already has
  classes, and a future `ZkProven` class fits the existing dispute
  resolver shape.

## Alternatives considered

- **zkML-only.** Rejected: prover throughput at frontier sizes is
  multiple orders of magnitude away from real-time as of Q1 2026.
  Forces the protocol into toy-model demos.
- **TEE-only.** Rejected: hardware monoculture, side-channel history,
  and exclusion of any provider that does not ship CC endpoints.
  Detailed analysis in [`alternatives.md`](../alternatives.md).
- **Reputation-only (no slashing).** Rejected: reputation alone cannot
  deter an adversary who plans a single high-EV defection.
- **Centralized verifier.** Rejected on principle; defeats the
  trust-minimization goal.

## References

- Spec §2 (Approach selection), §6 (Equivalence regimes), §13 (Open
  questions).
- EZKL public benchmarks (zkonduit, late 2025) — order-of-magnitude
  prover costs for transformer inference.
- Modulus Labs research notes on zkML throughput.
- EigenLayer slashing whitepaper — design of cryptoeconomic security
  with restaking primitives.
- Optimism / Arbitrum fault-proof papers — finalization-delay design.
- Intel SGX/TDX side-channel literature (Foreshadow, ÆPIC Leak).
