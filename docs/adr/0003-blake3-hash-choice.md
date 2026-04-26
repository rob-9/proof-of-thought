# ADR 0003 — blake3 as the protocol hash function

- Status: Accepted (2026-04-26)
- Deciders: PoT core
- Supersedes: —
- Superseded by: —

## Context

PoT computes hashes over potentially large documents — canonical
input (a full prompt context with tool outputs and memory snapshots
can run into hundreds of kilobytes), canonical output (reasoning
traces), trace manifests, and trace tarballs. Hashes appear in five
places (spec §4.1): `model_id` (over safetensors bytes for open
weights), `input_commitment`, `output_commitment`, `trace_uri_hash`,
and `policy_id`.

The chosen hash is the protocol's universal cryptographic primitive.
It must be:

1. Collision-resistant at the 128-bit security level (256-bit output).
2. Fast on commodity CPUs at producer time, because agents will rehash
   on every inference and watchers will rehash on every challenge.
3. Available as a maintained, audited library in Rust (Anchor program +
   watcher), TypeScript (SDK), and Python (inference adapters).
4. Cheap to verify on chain when a hash appears as program input.
5. Compatible with the long-term roadmap for zkML
   ([`future-work.md`](../future-work.md)).

Candidates: **sha256**, **sha3-256 / keccak-256**, **blake2b**,
**blake3**, **poseidon**.

## Decision

PoT uses **blake3 with a 256-bit output** as its hash function for
all canonical input, canonical output, trace, manifest, and model
hashes. The Solana program verifies blake3 hashes via the syscall
exposed in recent Solana SDK versions; the same `[u8; 32]` digest
size is used everywhere on chain.

Domain separation, where required (e.g., committing to multiple
fields in a single hash), uses blake3's keyed mode with a per-domain
ASCII key string (e.g., `b"pot.input.v1"`), not ad-hoc prefix bytes.

## Consequences

### Positive

- **Speed.** blake3 reaches roughly 3 GB/s single-threaded on modern
  x86 with SIMD, against roughly 500 MB/s for sha256 on the same
  hardware (orders-of-magnitude figures from the official blake3
  benchmarks; exact numbers vary by CPU). Hashing a 200 KB canonical
  input is a sub-millisecond operation. This matters for watchers
  re-executing at scale.
- **Built-in tree mode.** blake3 is internally Merkle-structured.
  Future incremental verification — verifying a trace bundle without
  reading every byte — drops in without changing the hash function.
- **Built-in keyed-hash and KDF modes.** Domain separation is a
  first-class API, not a convention. Reduces protocol-bug surface.
- **256-bit security.** Same security level as sha256 against
  collisions and preimages.
- **Library quality.** The reference C and Rust implementations are
  well-audited; `@noble/hashes` provides a maintained TypeScript
  port. Python bindings exist (`blake3-py`).
- **Hardware-friendly.** SIMD-friendly on x86 / ARM; explicitly
  designed for parallelism. No vendor instruction dependency.

### Negative

- **Less universally deployed than sha256.** Some auditors will ask why.
  We answer with this ADR.
- **Not zk-friendly.** Proving a blake3 hash inside a SNARK is
  expensive (bitwise operations, no algebraic structure). For the
  zkML-of-verifier-models path ([`future-work.md`](../future-work.md))
  we accept this and use poseidon *internally* in any zkML
  sub-circuit, with a blake3 wrapper for the on-chain commitment.
- **No FIPS certification.** Irrelevant for an open agent protocol;
  noted for completeness.

### Neutral

- The Solana program's syscall surface is the limiting factor for
  on-chain hashing. Where the program merely *checks* a hash equality
  (the common case), no syscall is invoked; the digest is supplied by
  the caller and compared by `==`. The cost difference between
  sha256 and blake3 on chain is therefore zero in the hot path.

## Alternatives considered

- **sha256.** The conservative default. Rejected on producer-side
  speed: hashing the full canonical input on every inference and on
  every watcher re-exec adds up at scale, and we get no protocol
  benefit from the slower function. Solana itself uses sha256 widely;
  there is no protocol reason a higher layer must.
- **sha3-256 / keccak-256.** keccak is the EVM default. Rejected on
  the same speed grounds as sha256 plus no clear ecosystem benefit on
  Solana.
- **blake2b.** The closest competitor. blake3 is its successor:
  faster, simpler API, native tree/keyed modes. We chose the
  successor.
- **poseidon.** Designed for SNARK efficiency. Excellent for zkML
  internals but extremely slow on a CPU and not native to most
  ecosystems. Rejected as the universal hash; reserved as the
  *internal* hash inside future zkML sub-circuits, with blake3
  wrapping the public commitment.

## References

- Spec §4.1 (account layout), §4.2, §4.3 (canonical input/output),
  §4.4 (model identity).
- BLAKE3 specification and benchmarks
  ([blake3-team/BLAKE3](https://github.com/BLAKE3-team/BLAKE3)).
- [ADR 0002](0002-cbor-canonicalization.md) — bytes that get hashed.
- [ADR 0008](0008-recursive-pot-committee.md) — recursive PoT and the
  zkML-of-verifier-model future path.
