# proof of thought, documentation

proof of thought (pot) is a solana primitive that lets autonomous agents cryptographically attest to having performed genuine reasoning with a specific model, on specific inputs, bound to a specific moment in time, before executing an on-chain action. pot does not prove the reasoning was correct; it proves it was generated, attributed, fresh, and coherent with the action it gates.

the protocol composes content-addressed storage, a verifiable random function (pyth entropy), and an optimistic challenge window with cryptoeconomic slashing. an agent posts a fixed-size thoughtrecord on chain, uploads a re-executable trace bundle off chain, and waits a challenge window. watchers re-execute the trace; if the output diverges, they file a fraud proof and split the slashed stake. three regimes (strict, attested, soft) handle llm non-determinism: byte-exact replay, tee quotes, or a recursive committee of verifier models.

## reading order

1. [`specs/2026-04-26-proof-of-thought-design.md`](specs/2026-04-26-proof-of-thought-design.md), the full protocol design.
2. [`adr/`](adr/), the eight architecture decision records, read in numeric order.
3. [`tradeoffs.md`](tradeoffs.md), cross-cutting tradeoffs.
4. [`alternatives.md`](alternatives.md), rejected designs.
5. [`future-work.md`](future-work.md), the roadmap.
6. [`glossary.md`](glossary.md), defined terms.

## architecture decision records

- [adr 0001, optimistic vs zkml vs tee](adr/0001-optimistic-vs-zkml-vs-tee.md)
- [adr 0002, cbor canonicalization](adr/0002-cbor-canonicalization.md)
- [adr 0003, blake3 hash choice](adr/0003-blake3-hash-choice.md)
- [adr 0004, pyth entropy as vrf source](adr/0004-pyth-entropy-vrf.md)
- [adr 0005, equivalence classes for non-determinism](adr/0005-equivalence-classes.md)
- [adr 0006, arweave vs shadow drive for trace storage](adr/0006-arweave-vs-shadow-storage.md)
- [adr 0007, stake and bond economics](adr/0007-stake-bond-economics.md)
- [adr 0008, recursive pot for the semantic committee](adr/0008-recursive-pot-committee.md)

## cross-cutting documents

- [`tradeoffs.md`](tradeoffs.md), latency vs assurance, cost vs decentralization, determinism vs model choice, storage cost vs auditability, policy flexibility vs consumer trust.
- [`alternatives.md`](alternatives.md), pure zkml, pure tee, mpc inference, witness-encrypted decisions, reputation-only, on-chain inference.
- [`future-work.md`](future-work.md), fifteen sized roadmap items: pyth entropy, arweave, shadow, tdx, sev-snp, h100 cc, zkml, cross-chain pot, multi-step thoughts, privacy, governance, economic simulation.
- [`glossary.md`](glossary.md), protocol term definitions.

## conventions

- section references like §4.1 refer to the design spec.
- "approximately" and "as of late 2025 / q1 2026" are deliberate hedges.
- us english. active voice. no marketing.
