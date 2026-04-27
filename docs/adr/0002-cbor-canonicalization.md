# ADR 0002 — Deterministic CBOR for canonical input/output

- Status: Accepted (2026-04-26)
- Deciders: PoT core
- Supersedes: —
- Superseded by: —

## Context

The on-chain commitments in a ThoughtRecord — `input_commitment`,
`output_commitment`, `policy_id`, `trace_uri_hash` — are blake3 hashes
of structured documents (spec §4.2, §4.3). For these hashes to be
verifiable by an independent watcher, every party must serialize the
same logical document to the same bytes, every time. Any serializer
divergence is a fatal mismatch: the agent commits to one byte string,
the watcher computes another, the dispute resolver cannot tell who is
correct.

The protocol's documents are nested, schema-light (some fields are
free-form strings or arrays), and contain raw bytes (model digests,
VRF seeds, attestation blobs). They are produced by code in TypeScript,
Rust, and Python (any inference adapter), and must round-trip through
storage that may transcode (Arweave gateways, Shadow Drive). A canonical
serialization format is therefore mandatory.

Candidate formats:

1. **JCS** (RFC 8785, JSON Canonicalization Scheme).
2. **Borsh** (Solana ecosystem default for binary serialization).
3. **Protocol Buffers** with a canonicalizing encoder.
4. **Deterministic CBOR** (RFC 8949 §4.2.1, "deterministically encoded
   CBOR").
5. **ASN.1 DER**.

## Decision

PoT uses **deterministic CBOR per RFC 8949 §4.2.1** for all canonical
input, canonical output, manifest, and policy documents. The hash
function over those bytes is blake3 ([ADR 0003](0003-blake3-hash-choice.md)).

Rules in force:

- Integers in shortest form, definite-length encoding.
- Map keys sorted lexicographically as encoded byte strings (the
  bytewise rule, not the length-first rule from RFC 7049).
- No floats unless the field's schema requires one (sampling
  parameters); when present, half/single/double width is the shortest
  that preserves the value, and NaN/Inf are rejected.
- No tags except for byte strings (well-known tag 24 is not used; all
  byte fields are major type 2 directly).
- Strings are UTF-8 NFC-normalized at the producer.

A reference canonicalizer ships in the TS SDK and in Rust; both have
golden vectors checked into the repo.

## Consequences

### Positive

- **Native bytes.** CBOR's major type 2 carries raw bytes directly. Model
  digests, VRF seeds, and TEE quote blobs do not need base64 padding,
  saving roughly 33% on trace-bundle size and eliminating an
  encode/decode round-trip that JCS would force.
- **Schema flexibility.** Adding a new optional field to canonical
  output (e.g. `self_score`) does not require a schema migration in
  the canonicalizer, only in the validator. Schema enforcement lives in
  policy validation, not in serialization.
- **Single-pass determinism.** The deterministic encoding rules are
  applied during encoding, not in a separate canonicalizer pass over
  produced bytes. JCS, by contrast, is a re-canonicalize step over an
  arbitrary JSON encoding, which adds an extra failure mode where the
  initial encoder produced something the canonicalizer then has to fix.
- **Strong tooling.** Mature deterministic CBOR libraries exist in
  Rust (`ciborium` with a det-encoder wrapper, plus `serde_cbor`-derived
  forks), TypeScript (`cbor2`), Go (`fxamacker/cbor`), and Python
  (`cbor2`). Tooling beats ASN.1 DER by a wide margin.
- **Auditability.** CBOR diagnostic notation is human-readable, so
  dispute evidence ("byte 47 is 0x42 not 0x40") can be presented in
  prose without dumping raw hex.

### Negative

- **Less universally familiar than JSON.** Reviewers may need a few
  minutes with the spec. We mitigate with a section in the SDK README
  and golden vectors.
- **Two encodings in flight.** Off-chain logs (events, Arweave manifests)
  often surface JSON for ergonomics. We solve this by emitting JSON for
  *display* and CBOR for *commitment*, with the rule that anything
  hashed is CBOR.
- **Float edge cases.** Rejecting NaN/Inf is correct but requires
  validators in producers; bug surface area.

### Neutral

- CBOR's optional indefinite-length encoding is forbidden by §4.2.1; we
  enforce it in the encoder.

## Alternatives considered

- **JCS (RFC 8785).** Strong choice for an all-JSON ecosystem. Rejected
  because (a) it forces base64 for binary fields, bloating traces and
  giving two ways to encode the same byte string; (b) the spec
  explicitly excludes binary types, which we have many of; (c) it is a
  re-canonicalization layer rather than a single-pass encoder.
- **Borsh.** The Solana default. Rejected because Borsh is rigid
  positional encoding — adding a field is a breaking change, optional
  fields are awkward, and the format has no standardized
  canonicalization story across language implementations. Acceptable
  inside the Anchor program (where layouts are fixed and small) but
  wrong for variable-shape documents like canonical input.
- **Protobuf with a canonicalizing encoder.** Rejected: canonical proto
  is folklore, not a standard. Implementations differ. Field-number
  reordering and unknown-field handling are footguns.
- **ASN.1 DER.** Has a real canonicalization standard. Rejected on
  tooling — outside of TLS/PKIX, library quality is poor, and
  producers in TS / Python would be writing fragile encoders for an
  encoding most engineers cannot read.

## References

- Spec §4.2 (canonical input), §4.3 (canonical output), §4.5 (trace
  bundle).
- RFC 8949 §4.2.1 — deterministically encoded CBOR.
- RFC 8785 — JSON Canonicalization Scheme (JCS).
- [ADR 0003](0003-blake3-hash-choice.md) — hash function over the canonical bytes.
