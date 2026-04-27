# Alternatives Considered

This document walks through six protocol designs we evaluated as alternatives to
the optimistic + cryptoeconomic mechanism described in the
[design spec](specs/2026-04-26-proof-of-thought-design.md). For each, we cover
what the design would look like, what it would buy us, and the concrete reasons
we rejected or deferred it.

The relevant decision is recorded in [ADR-0001](adr/0001-optimistic-vs-zkml-vs-tee.md).
This document expands on the alternatives surveyed there with deeper treatment.

---

## 1. Pure zero-knowledge ML (zkML)

**The pitch.** A prover runs the LLM inference and emits a SNARK or STARK that
attests `output = M(input)`. A consumer (or the chain itself) verifies the proof
in milliseconds. No watchers, no challenge windows, no stake — the math is the
trust.

**What it would replace.** The entire optimistic challenge layer
(spec §5.3, §6, §8). `submit_thought` would carry a proof; `consume_thought`
would verify it inline.

**State of the art (as of late 2025).** EZKL, RiscZero, and Modulus Labs have
all demoed zkML, but the regime that's practical today is models on the order
of 100 M parameters or less, with prove times in the tens of minutes to hours
per inference and proof sizes in the tens to hundreds of kilobytes. Progress
is steep — recursive proofs and lookup-argument advances cut costs by an order
of magnitude per year — but the headline number for a frontier-class LLM
(70B–500B params) remains hours of GPU time per proof. That isn't compatible
with an agent that needs to make a decision in seconds.

**Why we deferred it.** The agent economy demands sub-second to single-digit
second decision latency. zkML gives sub-100ms verification but at the cost of
an unbounded prover lag. Even if you front-loaded the prove, a single Jupiter
swap would be paying minutes of GPU time per trade — economically backwards.

**Where it fits later.** The protocol's recursive structure
([ADR-0008](adr/0008-recursive-pot-committee.md)) makes zkML usable as a
*component* without making it the foundation: a `SemanticCommittee` member is
itself a small open-weights model running under `StrictRegime`, and proving
that small model is in scope for current zkML. We've routed for this in
[future-work.md](future-work.md).

**Verdict.** Defer. Use as a component, not a foundation.

---

## 2. TEE-only (no optimistic layer)

**The pitch.** Run all LLM inference inside a confidential VM (Intel TDX,
AMD SEV-SNP, or NVIDIA H100 Confidential Computing). The hardware signs a
quote; the chain verifies the quote against the manufacturer's root CA. No
watchers, no challenge windows, no stake.

**What it would replace.** Same as above — the whole optimistic layer
collapses. `submit_thought` is simultaneously `consume_thought`, modulo the
quote verification.

**Why we did not adopt it as the base.**

1. **Hardware monoculture.** A bug or backdoor in TDX/SEV/H100-CC compromises
   every participant atomically. The track record on this front is not
   reassuring: Foreshadow, ZenBleed, Aepic Leak, ÆPIC, the `MachineLearning`
   side-channels — every couple of years a side-channel publishes that
   invalidates a generation of attestations.
2. **Closed-API frontier models still need a TEE-fronted path.** OpenAI and
   Anthropic ship Confidential APIs, but the trust model collapses to the
   provider's deployment hygiene plus the CPU vendor's CA. That isn't worse
   than today's status quo, but it isn't *better* either, and it's
   incompatible with permissionless agent participation.
3. **Cost.** H100 Confidential Computing carries a 5–15% throughput hit on
   inference plus a non-trivial premium on hosted GPU. Per-thought economics
   matter for high-frequency agents.
4. **Long-tail open-source models.** Researchers want to try a 7B fine-tune on
   their laptop. TEE-only forces every agent to rent a confidential GPU,
   instantly killing the long tail.

**Where it fits in our design.** TEE attestation is an opt-in *upgrade tier*
(spec §6.2). High-value actions (large swaps, treasury moves) that can pay
for the hardware get immediate finality; everything else uses optimistic.
The same `consume_thought` ix accepts either path. See ADR-0001.

**Verdict.** Reject as base; accept as opt-in tier.

---

## 3. Multi-party compute (MPC) inference

**The pitch.** Split the LLM weights across N parties, run inference under
secret-sharing such that no single party sees the prompt or the model
weights. Cited examples: Orion (semi-honest), Iron (malicious-secure for
small models), recent work on FHE-MPC hybrids.

**What it would buy us.** Privacy: the prompt is never decrypted at any single
party. Provenance: the protocol output is provably the result of the agreed-on
model and inputs because no one party could have computed it alone.

**Why we rejected it.**

1. **Latency.** A 2-of-3 secret-shared 70B-class inference is, today, on the
   order of 10× to 100× slower than plaintext inference, often more for
   malicious-secure protocols. Tail latencies are worse — straggling parties
   stall the whole inference. Agent trade decisions on second-scale windows
   can't tolerate this.
2. **Bandwidth.** Each forward pass burns hundreds of megabytes to gigabytes
   of inter-party communication for a frontier-sized model. Running a watcher
   network where everyone can re-MPC the inference is a non-starter on
   commodity infra.
3. **Coordination overhead.** The protocol now has to bootstrap a 2-of-3 (or
   k-of-n) party set per agent, which makes "permissionless" complicated and
   reintroduces a quasi-trusted-setup story.
4. **Privacy is solving the wrong problem.** PoT's threat model
   (spec §2) cares about provenance, not privacy. If a future use case
   demands private traces, the protocol can compose with TEE + selective
   disclosure, which we're already routing for in
   [future-work.md](future-work.md).

**Verdict.** Reject. Re-evaluate if MPC overhead drops 10×.

---

## 4. Witness-encrypted decisions (commit-then-countersign)

**The pitch.** The agent commits to an encrypted decision. A watcher fetches
the trace, verifies it, countersigns, and the encryption is only unlocked
once a quorum of countersignatures lands. Inspired by witness encryption and
threshold-witness primitives.

**What it would buy us.** Decisions cannot be observed by adversaries before
the quorum forms — this neutralizes front-running.

**Why we rejected it.**

1. **It addresses the wrong threat.** Front-running of pending actions
   (spec §2 T7) is already neutralized in PoT by the input-commitment binding
   to the VRF seed: the agent's reasoning is sealed before any front-runnable
   data is observable. We don't need encryption to solve provenance.
2. **UX friction.** Every consumer ix becomes two-phase (commit, then reveal).
   Composability with existing Solana programs collapses.
3. **Quorum complexity.** The watcher network shifts from "any 1-of-N can
   challenge" (cheap, robust) to "k-of-N must countersign before the
   decision unlocks" (Byzantine quorum, key management, liveness risk).

**Verdict.** Reject. Front-running is solved by VRF binding.

---

## 5. Reputation-only (no slashing)

**The pitch.** Drop the bonded-stake mechanism. Each agent maintains a public
reputation score updated by watchers. Consumers choose whom to trust; bad
agents lose business. Like a prediction market scoring rule on top of a
public log.

**What it would buy us.** No bonding capital lockup, simpler protocol,
permissionless onboarding without a SOL balance.

**Why we rejected it.**

1. **Cheap fraud in adversarial markets.** A throwaway identity that
   defrauds once and disappears has no penalty. Reputation only deters
   *recurring* participants. PoT's adversary model includes one-shot
   exploits (spec §2 T1, T7), so reputation alone is insufficient.
2. **Whitewashing.** Without a tie between identity and capital, an agent
   can spin up arbitrarily many identities, build reputation on each, and
   defect on the one with the most TVL. Slashing makes this expensive
   per-attack.
3. **Scoring rule design is hard.** Reputation aggregation is itself a
   protocol that has to be Sybil-resistant — and the simplest Sybil
   defenses (proof of stake, proof of unique humanity) collapse back to
   slashing or attestations.

**Where it could fit.** A reputation overlay on top of slashing is on the
roadmap ([future-work.md](future-work.md)) — it's useful as a UX hint and as
a watcher-griefing deterrent. As a *replacement* for slashing, no.

**Verdict.** Reject as replacement; accept as future overlay.

---

## 6. On-chain inference (tiny model on Solana)

**The pitch.** Embed a tiny model (think TinyLlama or a 1B distilled
checkpoint) directly in a Solana program. Decisions are derived on-chain
from inputs; the trace is the transaction itself. No off-chain trace, no
watchers.

**What it would buy us.** Trustless. Verifiable. Fully on-chain agent.

**Why we rejected it.**

1. **Compute units.** Solana programs have a per-tx CU budget on the order
   of 1.4 M (post-CU-budget upgrades, raisable but bounded). A single
   transformer forward pass on a 1B model is millions to billions of FLOPs
   even with int8 quantization — that's orders of magnitude over the CU
   budget. You'd need many tx to assemble one inference, plus state-spillover
   costs that dwarf any reasonable economics.
2. **Capability ceiling.** Tiny models are not what the agent economy is
   building on. The whole pitch of "agentic sophistication" (the Colosseum
   judging rubric) is frontier-model behavior. Capping at 1B reduces
   the project to a toy.
3. **Storage.** Even quantized weights are tens to hundreds of megabytes.
   Solana account size limits and rent economics make this infeasible
   without exotic compression schemes.
4. **No solution for hosted-frontier-model use cases.** The whole point of
   PoT is to make agents *that use frontier models* trustworthy. On-chain
   inference can't run those models, period.

**Where it could fit.** Useful as a *verifier* primitive: a tiny decision-rule
classifier running on-chain to gate whether a thought is even worth
challenging. But that's a niche optimization, not the protocol.

**Verdict.** Reject. Capability and CU walls are categorical.

---

## Summary table

| Alternative | Trustless? | Latency | Cost | Frontier models? | Verdict |
|---|---|---|---|---|---|
| zkML | yes | hours | high | no (today) | defer; use as component |
| TEE-only | no (HW trust) | low | medium | yes (CC API) | reject as base; opt-in tier |
| MPC inference | yes (k-of-n) | very high | very high | barely | reject; revisit at 10× speedup |
| Witness-encrypted | yes | medium | medium | yes | reject; addresses wrong threat |
| Reputation-only | no | low | low | yes | reject as replacement; future overlay |
| On-chain inference | yes | very high | infeasible | no | reject; CU & capability walls |
| **Optimistic + crypto** (chosen) | 1-of-N honest watcher | low | low | yes | adopted; see ADR-0001 |

---

## What we'd revisit

- **zkML at the verifier-model layer.** Once EZKL or RiscZero can prove a
  10–30B model in single-digit minutes, the SemanticCommittee becomes
  trustlessly verifiable bottom-up. That's a meaningful upgrade to the
  protocol's overall trust posture even though it doesn't change the base
  layer.
- **MPC at sub-second latency.** Today's MPC is 10–100× slower than
  plaintext. If that gap closes (FHE-MPC hybrids are improving fast), MPC
  becomes a credible alternative to TEE for the high-assurance path —
  same trustlessness as zkML without the prove-time tax.
- **Reputation overlay.** Layered on top of slashing, reputation lowers the
  griefing surface in `Soft` regime mismatches and improves consumer-side
  policy selection. This is a near-term roadmap item.

The chosen design is a position taken under 2026 constraints. The
constraints will move; so should the design.
