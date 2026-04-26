# ADR 0005 — Three equivalence regimes for non-deterministic models

- Status: Accepted (2026-04-26)
- Deciders: PoT core
- Supersedes: —
- Superseded by: —

## Context

Re-execution as a fraud-proof primitive (the foundation of the
optimistic regime — see [ADR 0001](0001-optimistic-vs-zkml-vs-tee.md))
demands that running the same model on the same input produces a
verifiable output. This is straightforwardly true for open-weights
local models with `temperature=0` and pinned deterministic CUDA
kernels. It is straightforwardly false for production frontier APIs:
OpenAI's `seed` parameter is best-effort (the docs say so), Anthropic
exposes none, and silent backend changes are routine (model snapshots
update, kernel choices vary, MoE routing introduces stochasticity).

Forcing strict determinism would lock the protocol out of frontier
hosted models — the very models the agent economy actually runs on.
Forcing no determinism would make fraud-proofs impossible. PoT must
handle a spectrum of models honestly without pretending the messy
middle does not exist.

The spec (§6) defines three regimes that map onto three classes of
model. The decision below ratifies that mapping and pins the
mechanism details.

## Decision

PoT defines three **equivalence regimes** that policies declare and
consumers gate on. A policy declares one regime per model class.

### 6.1 Strict regime — byte-exact replay

Used by open-weights local-execution agents. Watcher re-runs
`M(input) → output'`; verdict is `output' == output` byte-for-byte
after canonicalization. Determinism prerequisites pinned in the
policy: safetensors digest (matches `model_id`), inference engine and
version (e.g. `vllm==0.7.3`), `CUBLAS_WORKSPACE_CONFIG=:4096:8`,
`torch.use_deterministic_algorithms(True)`, `temperature=0`,
`top_p=1.0`, no speculative decoding, batch size 1, GPU class
declared. If any prerequisite fails, the regime falls through to
6.3.

### 6.2 Attested regime — TEE quote short-circuits replay

Used by TEE-fronted models. The trace contains a TEE quote whose
`report_data` field equals
`H(input_commitment ∥ output_commitment ∥ vrf_seed)`. The resolver
verifies the quote against the registered TEE root CA and marks
`attestation_verified`. The challenge window collapses to zero. This
is the only path that works for hosted frontier APIs without
re-execution, *and* only when the host ships a verifiable TEE-CC API.

### 6.3 Soft-equivalence regime — entailment, not equality

Used when re-execution is probabilistic. The policy picks one of
three sub-modes:

- **`StructuralJSON`** — only the `decision` field of canonical
  output must match exactly. The `reasoning` field may differ. Suits
  narrow decisions like "buy/hold/sell" or "approve/deny."
- **`SemanticCommittee`** — a registered committee of `k` independent
  verifier models is asked: "do these two outputs entail the same
  downstream action under policy P?" Quorum `q-of-k` decides. **At
  least one committee member must be open-weights** (the constraint
  from spec §13.3). Each committee model run is itself a PoT under
  StrictRegime; bottom of the recursion is verifiable
  ([ADR 0008](0008-recursive-pot-committee.md)).
- **`AnyOfN`** — agent commits to a Merkle root of N samples taken
  with declared seeds; reveals one. Watcher re-samples; accepts if
  the revealed output is among the N. Dispute is a Merkle
  non-membership proof.

The framing is **entailment, not equality**. Two reasoning traces
that drive the same policy-relevant decision under the same inputs
are equivalent for fraud-detection purposes, even when their natural
language differs.

## Consequences

### Positive

- **Honest about the LLM landscape.** The protocol meets models where
  they are. Hosted frontier APIs are not banned; they are accommodated
  through SemanticCommittee with explicit trust shape.
- **Recursive verification has a base case.** Committee members are
  small enough to run under StrictRegime, so the
  "who watches the watchers" recursion terminates at byte-exact
  open-weights inference. See [ADR 0008](0008-recursive-pot-committee.md).
- **Composability.** Each regime exposes the same on-chain status
  semantics (`Pending → Finalized | Slashed`); consumers do not have
  to special-case regimes when gating actions.
- **Clear consumer choice.** A policy is a public document. A
  consumer that does not trust StructuralJSON for high-stakes
  decisions can simply require Strict or Attested.

### Negative

- **Three regimes is more code.** Each has its own watcher path,
  dispute claim type, and policy schema fields.
- **SemanticCommittee is statistical, not cryptographic.** Quorum
  `q-of-k` is a tunable parameter; setting it too low admits adversarial
  outputs, too high makes any disagreement a slashing event.
- **Soft regime puts cost on watchers.** Running a committee on
  every challenged thought is expensive. The watcher's bond + slash
  share must price this in, and operators must keep committee
  members hot.
- **AnyOfN is a sampling proof, not an entailment proof.** It catches
  T2 model substitution and T4 lazy reasoning only when the revealed
  sample is the one the agent claims; an adversary that controls
  sampling can game it. Marked as "use sparingly" in the SDK
  policy presets.

### Neutral

- The regime is selected by *policy*, not by model. The same model
  could be used under Strict in one policy and SemanticCommittee in
  another. Consumer trust attaches to the policy, not the model in
  isolation.

## Alternatives considered

- **Strict regime only.** Rejected: locks out hosted frontier models,
  which are the agent economy's center of gravity.
- **Soft regime only.** Rejected: removes the hard guarantee for
  open-weights paths and inflates dispute costs unnecessarily.
- **Per-thought entailment proof via zkML.** Beautiful in theory; not
  feasible at frontier-LLM size in 2026.
- **Allow closed-API-only committees.** Rejected because a committee
  of three closed APIs creates a 3-API trust dependency
  (spec §13.3); a single coordinated provider could undermine all
  members. The "at least one open-weights" constraint is non-negotiable.

## References

- Spec §6 (handling LLM nondeterminism), §13.3, §13.5.
- [ADR 0001](0001-optimistic-vs-zkml-vs-tee.md), [ADR 0008](0008-recursive-pot-committee.md).
- OpenAI seed parameter documentation
  (`platform.openai.com` — note "best effort").
- vLLM determinism notes, `torch.use_deterministic_algorithms`
  documentation.
