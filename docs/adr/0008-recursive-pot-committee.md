# ADR 0008 — Recursive PoT for SemanticCommittee members

- Status: Accepted (2026-04-26)
- Deciders: PoT core
- Supersedes: —
- Superseded by: —

## Context

The SemanticCommittee sub-mode of the Soft-equivalence regime
([ADR 0005](0005-equivalence-classes.md)) asks `k` registered
verifier models whether two outputs entail the same downstream action
under a given policy. The committee's verdict drives the dispute
resolver. This raises an obvious recursion: **who watches the
watchers?**

If committee members are themselves frontier LLMs running under Soft
regime, the recursion never terminates — the verifier models'
verdicts are themselves only checkable via committees, ad infinitum.
If committee members are arbitrary closed APIs, the trust shape
collapses to "trust the committee operators," which removes the
protocol guarantee.

The protocol must close the recursion at a verifiable base case.

A useful analogy is interactive proof systems: a complex decision
can be reduced to many smaller decisions, terminating in a base step
the verifier can check directly. We do **not** claim PoT achieves
the formal soundness of an IP system; we are borrowing the
*structural* idea, not the math. Hand-waving as proof would be
dishonest.

## Decision

**Each SemanticCommittee member produces its own ThoughtRecord under
the Strict regime.** Concretely:

- Committee members are small open-weights models (e.g.,
  `Llama-3.3-70B-Instruct` quantized for cost, or a smaller open
  judge model) that meet the StrictRegime determinism prerequisites
  (spec §6.1 — pinned engine, deterministic kernels, `temperature=0`).
- When the dispute resolver invokes a committee on a contested
  thought, each member runs against canonical input
  `(original_input, original_output, candidate_output)` and emits a
  PoT in turn.
- Those PoTs are themselves on-chain ThoughtRecords. They reference
  their own model digest (open-weights `model_id`), their own VRF
  seed, and a trace bundle. Watchers can re-execute them
  byte-exactly.
- The base of the recursion is therefore byte-exact open-weights
  inference: a watcher with the safetensors and a deterministic
  engine can reproduce the committee member's output and check
  agreement with the on-chain commitment.

The committee composition rule (spec §13.3, [ADR 0005](0005-equivalence-classes.md))
requires at least one open-weights member. With the recursive
construction, in practice **all** SemanticCommittee members are
open-weights so that the entire committee runs under Strict.

## Consequences

### Positive

- **Recursion terminates.** Every Soft-regime dispute reduces to a
  finite set of Strict-regime claims, each of which is byte-exactly
  reproducible. The "infinite committee" objection is closed.
- **Verifier trust is replaced by verifier *replay*.** A consumer no
  longer has to trust a committee operator's word; the committee
  member's reasoning is itself fraud-provable.
- **Aligns with the recursive-proof aesthetic of zk-rollups.**
  Without claiming the soundness, we get the same compositional
  property: the base layer's primitives are reused at a higher
  level.
- **Demo-friendly.** The hackathon demo can show a thought triggering
  a dispute, the committee running, and each member's PoT
  appearing on chain — concrete recursion, not abstract.

### Negative

- **More on-chain accounts.** A challenged thought materializes
  `k` extra ThoughtRecord accounts (one per committee member). At a
  3-of-5 committee that is 5 extra accounts per dispute. Storage and
  gas costs scale.
- **Latency stacks.** Each committee member's PoT has its own
  inference time and (in principle) its own challenge window —
  though committee members can run on a faster, policy-specific
  shorter window because their inputs are bounded. The aggregate
  dispute latency is therefore bounded but not negligible.
- **Open-weights ceiling.** Committee verifiers are limited to the
  capability of open-weights models that meet Strict determinism
  prerequisites. A committee of small open models is less capable
  than the frontier; this is acceptable for *entailment* judgments
  (the committee is asked a narrow comparison question, not an
  open-ended reasoning task) but caps the policies for which
  SemanticCommittee is viable.
- **Honest framing burden.** The interactive-proof analogy is
  attractive but easy to overclaim. Documentation must be careful:
  PoT does *not* achieve information-theoretic soundness; it
  achieves cryptoeconomic-plus-replay soundness with a clear base
  case.

### Neutral

- A future zkML upgrade can prove committee-member runs (which are
  small, open, deterministic, sub-100M-class for some judge models)
  inside SNARKs, removing the challenge window for the committee
  layer entirely. Out of scope for the hackathon; in scope for
  [`future-work.md`](../future-work.md).

## Alternatives considered

- **Trusted committee operators.** Reject committee runs as
  off-chain trusted computation. Fails the trust-minimization goal.
- **Single-shot committee with no recursion.** Treat committee
  output as authoritative. Simpler, but the "who watches the
  watchers" hole is real and exploitable: a malicious committee
  member could rubber-stamp fraud.
- **zkML for committee members today.** The right long-term answer.
  Today's prover throughput on even sub-100M models is not where
  it needs to be for production, and we cannot ship it in the
  hackathon window. Deferred.
- **Mixed open + closed committees with weighted voting.** Considered;
  rejected because the trust shape gets opaque fast and the closed
  members defeat the recursion-termination property.

## References

- Spec §6.3 (Soft-equivalence regime), §13.3 (committee gameability).
- [ADR 0001](0001-optimistic-vs-zkml-vs-tee.md), [ADR 0005](0005-equivalence-classes.md).
- Goldwasser, Micali, Rackoff — "The Knowledge Complexity of
  Interactive Proof Systems." Cited only for analogy; no formal
  claim of IPS-style soundness is made.
- EZKL, Modulus Labs benchmarks for the future-work zkML path.
