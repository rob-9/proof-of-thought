# Proof of Thought — Implementation Plan

> **For agentic workers:** Implement task-by-task. The spec at `docs/specs/2026-04-26-proof-of-thought-design.md` is the contract.

**Goal:** Ship a credible MVP of the PoT protocol — Anchor program, TS SDK, watcher daemon, demo agent, and complete docs — all locally compilable and testable, with honest stubs where external services (live Solana, Pyth Entropy, Arweave, real LLMs) are unavailable in this environment.

**Architecture:** Solana program in Anchor, TS SDK on `@solana/web3.js`, watcher daemon in Rust on `solana-client`. CBOR canonicalization + blake3 hashing. Mocked external services behind clean trait/interface boundaries so live wiring is a follow-up.

**Tech Stack:** Anchor 0.30, Rust 1.83, TypeScript 5.5, `@noble/hashes` blake3, `cbor2` library, Tokio for the watcher.

---

## File Structure

```
proof-of-thought/
├── docs/
│   ├── specs/2026-04-26-proof-of-thought-design.md     (already exists)
│   ├── superpowers/plans/2026-04-26-pot-implementation.md  (this file)
│   ├── adr/
│   │   ├── 0001-optimistic-vs-zkml-vs-tee.md
│   │   ├── 0002-cbor-canonicalization.md
│   │   ├── 0003-blake3-hash-choice.md
│   │   ├── 0004-pyth-entropy-vrf.md
│   │   ├── 0005-equivalence-classes.md
│   │   ├── 0006-arweave-vs-shadow-storage.md
│   │   ├── 0007-stake-bond-economics.md
│   │   └── 0008-recursive-pot-committee.md
│   ├── tradeoffs.md
│   ├── alternatives.md
│   ├── future-work.md
│   └── README.md
├── programs/pot_program/
│   ├── Cargo.toml
│   ├── Xargo.toml
│   └── src/
│       ├── lib.rs
│       ├── state/{agent.rs, model.rs, policy.rs, thought.rs, challenge.rs, mod.rs}
│       ├── instructions/{register_agent.rs, register_model.rs, register_policy.rs,
│       │                 request_vrf.rs, submit_thought.rs, consume_thought.rs,
│       │                 challenge.rs, resolve.rs, slash.rs, stake.rs, mod.rs}
│       └── errors.rs
├── sdk/ts/
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── canonical/{cbor.ts, hash.ts, schema.ts, index.ts}
│       ├── types.ts
│       ├── client.ts
│       ├── policies.ts
│       ├── storage.ts
│       └── index.ts
│       └── __tests__/
├── watcher/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── subscribe.rs
│       ├── trace_fetch.rs
│       ├── verify/{strict.rs, attested.rs, soft.rs, mod.rs}
│       └── challenge.rs
├── demo/swap-agent/
│   ├── package.json
│   └── src/{agent.ts, mock_llm.ts, run.ts}
├── Cargo.toml         (workspace)
├── Anchor.toml
└── README.md
```

## Decomposition (parallel where independent)

**Phase A (parallel — no cross-deps):**
- A1: Anchor program (state + instructions + tests)
- A2: TS SDK canonicalizer (CBOR + blake3 + types + vectors)
- A3: Watcher daemon (Rust scaffolding + verifier traits)
- A4: Documentation suite (ADRs, tradeoffs, alternatives, future)

**Phase B (depends on A1 + A2):**
- B1: TS SDK client wrapping program (uses Anchor IDL + canonical types)
- B2: Demo agent (uses B1)

**Phase C:**
- C1: Code review pass against spec

## Acceptance per phase

A1 done when: `anchor build` compiles, all `anchor test` unit tests pass.
A2 done when: `pnpm test` passes, vectors match a hand-computed reference.
A3 done when: `cargo build -p pot-watcher` succeeds, unit tests pass.
A4 done when: 8 ADRs + 3 cross-cutting docs landed, all commits pushed.
B1 done when: SDK builds, integration test against the Anchor program (using bankrun or local validator if available; otherwise unit-tested against IDL types).
B2 done when: `pnpm demo` runs end-to-end with mocked services, prints a trace.
C1 done when: review document committed listing issues + decisions.

## Out of scope for this session (documented as future work)

- Live devnet deploy + Pyth Entropy live integration
- Real Arweave/Shadow Drive uploads
- Real TEE quote verification against Intel/AMD/NVIDIA root CAs (parser only)
- Real LLM committee runs
- zkML prover stubs

These are tracked in `docs/future-work.md`.

## Commit cadence

One commit per logical chunk per agent (target ~10–20 commits). Conventional commits.
