# Proof of Thought — Documentation

Proof of Thought (PoT) is a Solana primitive that lets autonomous agents
cryptographically attest to having performed genuine reasoning — with a
specific model, on specific inputs, bound to a specific moment in time —
before executing an on-chain action. PoT does not prove that the reasoning
was *correct*; it proves it was *generated*, *attributed*, *fresh*, and
*coherent* with the action it gates. That is the missing primitive between
"an agent took an action" and "an agent should have taken that action."

The protocol composes three off-the-shelf cryptographic ingredients —
content-addressed storage, a verifiable random function (Pyth Entropy),
and an optimistic challenge window with cryptoeconomic slashing — into a
shippable agent-economy primitive. A registered agent posts a fixed-size
ThoughtRecord on chain, uploads a re-executable trace bundle off chain,
and waits a policy-defined challenge window. Watchers re-execute the
trace; if the output diverges, they file a fraud proof and split the
slashed stake. Three regimes (Strict, Attested, Soft) handle the
non-determinism of real frontier LLMs honestly: byte-exact replay where
possible, TEE quotes where available, and a recursive committee of
smaller open-weights verifier models elsewhere.

This documentation suite explains the design, the decisions, and the
honest tradeoffs behind every piece. The design spec is the contract; the
ADRs are the receipts.

## Reading order

If you have an hour, read in this order:

1. [`specs/2026-04-26-proof-of-thought-design.md`](specs/2026-04-26-proof-of-thought-design.md) — the full protocol design. Threat model, data model, on-chain program, equivalence regimes, watcher network, build plan. Everything else references it.
2. [`adr/`](adr/) — the eight Architecture Decision Records. Read in numeric order; later ADRs assume earlier ones.
3. [`tradeoffs.md`](tradeoffs.md) — cross-cutting tradeoffs we made and what would flip each decision.
4. [`alternatives.md`](alternatives.md) — alternative protocol designs we considered and rejected, with honest assessments.
5. [`future-work.md`](future-work.md) — the roadmap. Concrete items, sized.
6. [`glossary.md`](glossary.md) — every term used in the docs, defined.

## Architecture Decision Records

Each ADR follows the [MADR](https://adr.github.io/madr/) template — Status,
Context, Decision, Consequences, Alternatives, References. Decisions are
load-bearing: changing one usually requires re-reading the others.

- [ADR 0001 — Optimistic vs zkML vs TEE](adr/0001-optimistic-vs-zkml-vs-tee.md)
- [ADR 0002 — CBOR canonicalization](adr/0002-cbor-canonicalization.md)
- [ADR 0003 — blake3 hash choice](adr/0003-blake3-hash-choice.md)
- [ADR 0004 — Pyth Entropy as VRF source](adr/0004-pyth-entropy-vrf.md)
- [ADR 0005 — Equivalence classes for non-determinism](adr/0005-equivalence-classes.md)
- [ADR 0006 — Arweave vs Shadow Drive for trace storage](adr/0006-arweave-vs-shadow-storage.md)
- [ADR 0007 — Stake and bond economics](adr/0007-stake-bond-economics.md)
- [ADR 0008 — Recursive PoT for the semantic committee](adr/0008-recursive-pot-committee.md)

## Cross-cutting documents

- [`tradeoffs.md`](tradeoffs.md) — five cross-cutting tradeoff axes: latency vs assurance, cost vs decentralization, determinism vs model choice, storage cost vs auditability, policy flexibility vs consumer trust burden.
- [`alternatives.md`](alternatives.md) — pure zkML, pure TEE, MPC inference, witness-encrypted decisions, reputation-only, and on-chain inference, with honest reasons for rejecting each as the base layer.
- [`future-work.md`](future-work.md) — fifteen sized roadmap items spanning live wiring (Pyth Entropy, Arweave, Shadow), real attestation paths (TDX, SEV-SNP, H100 CC), zkML for the verifier-model path, cross-chain PoT, multi-step thoughts, privacy, governance, and economic simulation.
- [`glossary.md`](glossary.md) — definitions for every protocol term.

## Conventions

- Section references like §4.1 always refer to the design spec.
- "Approximately" and "as of late 2025 / Q1 2026" are deliberate hedges
  on benchmark numbers — the field moves fast, and we would rather be
  slightly under-specific than wrong.
- US English. Active voice. No marketing.
