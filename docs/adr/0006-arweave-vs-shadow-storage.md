# ADR 0006 — Hybrid Shadow Drive + Arweave for trace storage

- Status: Accepted (2026-04-26)
- Deciders: PoT core
- Supersedes: —
- Superseded by: —

## Context

Each ThoughtRecord references an off-chain trace bundle (spec §4.5)
containing the canonical input, canonical output, raw provider
response, tool I/O, memory proofs, optional TEE attestation, and a
manifest. Watchers fetch this bundle to re-execute. Disputes reference
it as evidence. Consumers may inspect it post hoc to audit an action.

The on-chain account stores only `trace_uri_hash = blake3(uri)`; the
URI itself is logged in an event. This keeps account size small while
making URI substitution provably detectable.

The storage layer must satisfy:

1. **Availability during the challenge window.** A trace that watchers
   cannot fetch for the duration of `challenge_window_slots` is a
   livelock — the agent finalizes by default. This is unsafe.
2. **Permanence for audit.** Disputes can reach back days or weeks.
   Consumer post-hoc audits reach back arbitrarily far.
3. **Cost-bounded.** Trace bundles run from low kilobytes (a tiny
   policy) to low megabytes (a long reasoning trace with extensive
   tool I/O). At hundreds to thousands of thoughts per agent per day,
   per-agent storage spend must stay reasonable.
4. **Censorship-resistant.** A storage host that can take down
   inconvenient traces is a centralization vector.

Candidate storage layers on or adjacent to Solana:

- **Arweave** — pay-once, store-forever via the Arweave endowment
  model.
- **Shadow Drive** (GenesysGo) — Solana-native, low-latency, cheap,
  signed-mutable storage.
- **Filecoin / IPFS** — content-addressed, multi-provider, but the
  retrievability story is operationally weaker than Arweave for
  long-tail data.
- **Walrus** — Sui-native; cross-chain dependency for a Solana
  protocol.

## Decision

PoT uses a **hybrid storage scheme**: Shadow Drive is the primary
upload target during the challenge window; the trace is mirrored to
Arweave asynchronously for long-term auditability.

Concretely:

1. The agent uploads the trace tarball to Shadow Drive synchronously
   before submitting the on-chain commitment. The Shadow URI is
   logged in the `ThoughtSubmitted` event.
2. A background uploader mirrors the same tarball to Arweave; once
   the Arweave tx confirms, an optional `set_archive_uri`
   instruction (or a second event) records the Arweave tx id.
3. `trace_uri_hash` covers the *primary* URI (Shadow) so that during
   the challenge window the watcher's fetch path is unambiguous.
4. Policies for high-value actions may *require* Arweave-confirmed
   archival before consume_thought succeeds; default policies do not.

## Consequences

### Positive

- **Low-latency dispute path.** Shadow Drive uploads are typically
  fast and cheap on a per-trace basis, which keeps the challenge
  window watcher path responsive. Arweave's confirmation latency
  (minutes) does not block submit_thought.
- **Permanence by default.** Asynchronous Arweave mirroring covers
  the audit case without making the synchronous path expensive or
  slow.
- **Cost-economical.** The bulk of storage volume sits on Shadow
  during the brief challenge window where retrievability is most
  important; Arweave receives only the long-tail audit copy. For
  policies with shorter audit half-lives, Arweave mirroring can be
  configured off.
- **Spec-aligned.** The spec §13.2 explicitly anticipated this hybrid
  shape (Arweave bundles confirm in minutes; mitigation is Shadow
  primary).

### Negative

- **Two storage systems to integrate.** Two SDKs, two failure modes,
  two cost-tracking dashboards. We mitigate with a `storage`
  abstraction in the SDK ([spec §10](../specs/2026-04-26-proof-of-thought-design.md))
  that takes a string ("shadow" or "arweave") today and grows.
- **URI-hash ambiguity.** `trace_uri_hash` covers exactly one URI.
  If the policy *also* requires Arweave archival, a separate event
  must carry the second hash. That is a protocol-shaped surface
  area, not a bug.
- **Shadow Drive trust.** Shadow's storage is signed-mutable; the
  protocol assumes the host honors immutability for the challenge
  window. We mitigate by hashing the URI plus the bundle bytes —
  any mutation breaks the bundle hash in the manifest.
- **Operator responsibility.** Agents must keep Shadow accounts
  funded and monitor Arweave mirror status. The SDK ships sane
  defaults; operators can still misconfigure.

### Neutral

- Filecoin/IPFS remain viable alternates for any operator that
  prefers them. The protocol is agnostic to the URI scheme as long
  as the watcher can resolve it.

## Alternatives considered

- **Arweave only.** Rejected for sync upload latency. Pushing trace
  bundles synchronously through Arweave delays submit_thought past
  the freshness window in the worst case, opening a T8 surface.
- **Shadow Drive only.** Rejected on permanence. A Shadow-only trace
  that depends on continued account funding is fragile for
  long-window audits and dispute reach-back.
- **IPFS / Filecoin.** Filecoin's deal model is operationally heavy
  for sub-megabyte traces; bare IPFS retrievability for long-tail
  content is poor without a paid pinning service that becomes a
  trust point.
- **Walrus.** Promising, but cross-chain dependency from Solana is a
  surface we do not need.
- **On-chain trace storage.** Rejected outright on cost; a few
  kilobytes per ThoughtRecord on chain is already at the upper edge
  of what is reasonable.

## References

- Spec §4.5 (trace bundle), §5.2 (submit_thought / consume_thought),
  §13.2 (open question on Arweave finality).
- Arweave yellow paper — endowment-based permanence.
- Shadow Drive (GenesysGo) documentation.
- [ADR 0001](0001-optimistic-vs-zkml-vs-tee.md) — challenge window
  design that this storage layer supports.
