# ADR 0004 — Pyth Entropy as the VRF source

- Status: Accepted (2026-04-26)
- Deciders: PoT core
- Supersedes: —
- Superseded by: —

## Context

Freshness is the deepest defense in PoT (spec §7). The protocol must
prevent three temporal attacks:

- **T1 replay** — reusing a past thought for a new action.
- **T7 prompt-front-running** — observing a pending mempool action and
  retroactively crafting a "thought" that justifies copying it.
- **T8 time-warp** — generating a thought after observing oracle
  reveals (price feeds, trade fills) and claiming it predates them.

The mechanism is a verifiable random function (VRF) consumed *before*
inference and embedded in the canonical input. The VRF seed binds the
thought to a specific slot interval. Two checks at submit time
(spec §7) enforce single-use and freshness windows.

The VRF source must:

1. Be available on Solana with a CPI-callable on-chain interface.
2. Have low latency — ideally a small number of slots, so that the
   freshness window is tight without blocking agents.
3. Be cryptographically verifiable on chain. A randomness source the
   program merely *trusts* is no better than a centralized timestamper.
4. Be cost-bounded at hackathon-realistic agent QPS (single digits to
   low hundreds per second across the network).
5. Be production-supported, not a research artifact.

Candidates on Solana:

- **Pyth Entropy** — Pyth Network's commit-reveal randomness service.
- **Switchboard VRF** — VRF-on-demand from Switchboard's oracle network.
- **A bespoke on-chain VRF** — operator runs a node, signs RFC 9381
  VRF outputs.
- **Chainlink VRF** — well-known on EVM; nascent on Solana.

## Decision

PoT uses **Pyth Entropy** as the canonical VRF source for the base
protocol. Agents call `request_vrf` (spec §5.2), which CPIs Pyth
Entropy; the resulting seed is bound to a slot in `AgentProfile`. The
seed is single-use, embedded in `canonical_input.vrf_seed`, and
checked against the latest unconsumed seed at `submit_thought` time.

The protocol records a documented fallback path: if Pyth Entropy is
degraded or unavailable for a policy, the policy may declare a
**2-of-2 commit-reveal** mode in which the agent contributes a random
half and a registered freshness oracle (or the program itself, via a
slot hash) contributes the other half. The fallback is enabled by
policy and is not the default.

## Consequences

### Positive

- **Solana-native.** Pyth is already heavily integrated in the Solana
  ecosystem and has direct CPI ergonomics. No cross-chain bridge.
- **Low latency.** Current observed p99 is around two slots
  (~800ms) — tight enough that the freshness window does not need
  pathological padding. The spec's open question §13.1 acknowledges
  this can shift under load.
- **Familiar.** Pyth is the de facto oracle on Solana; consumers and
  operators already trust its operational posture.
- **Composable.** Pyth Entropy seeds compose cleanly with the
  `vrf_seed` field in canonical input; no protocol gymnastics
  required.

### Negative

- **Cost at high QPS.** Pyth Entropy charges per request. At thousands
  of agent thoughts per second, the per-request fee dominates the
  protocol's economic surface. Mitigations: agents may batch when
  policy allows (a single seed gates a small ordered batch of
  thoughts produced inside `max_inference_ms`), and the documented
  2-of-2 fallback removes the dependency.
- **Centralization vector.** Pyth's randomness contributor set is
  smaller than the Solana validator set. A compromise of Pyth's
  randomness operators degrades T8 resistance for the duration of the
  compromise. The 2-of-2 fallback exists precisely to bound this
  risk; high-value policies should declare it.
- **External dependency.** A Pyth Entropy outage halts new thought
  submissions in the strict path. Watchers and existing thoughts are
  unaffected.

### Neutral

- The freshness check `slot - vrf_seed.slot ≤ max_inference_ms / 400ms`
  is independent of the VRF source; swapping providers requires no
  on-chain logic change beyond the program ID consumed by
  `request_vrf`.

## Alternatives considered

- **Switchboard VRF.** Strong second choice; Solana-native and
  permissionless. Rejected for two reasons: (1) higher observed
  latency tail than Pyth Entropy in our spot checks, and (2) we
  already lean on Pyth's price feeds in demos, so coupling to a
  single oracle ecosystem reduces operational surface for the
  hackathon. Will revisit; documented as a future swap-in.
- **Chainlink VRF.** Mature on EVM but Solana support is younger and
  less battle-tested. Rejected on operational risk for a 4-week ship.
- **Bespoke on-chain VRF.** A single operator running an RFC 9381 VRF
  node would be the simplest implementation but reintroduces a
  centralization point we are explicitly trying to avoid. Rejected.
- **2-of-2 commit-reveal as the primary.** This is the documented
  fallback. Rejected as primary because it adds a round trip
  (commit, then reveal) and an additional account, which complicates
  the SDK call surface for a benefit only realized when Pyth is
  unavailable.
- **Slot hash only.** Solana exposes recent slot hashes on chain. As a
  *sole* freshness source they are predictable to a sufficiently
  resourced adversary (validators) and provide weaker T8 guarantees
  than a true VRF. Rejected.

## References

- Spec §7 (Freshness: VRF binding), §13.1 (Open question on Pyth
  latency).
- Pyth Entropy documentation (`pyth.network/entropy`).
- RFC 9381 — Verifiable Random Functions.
- Switchboard VRF documentation.
- [ADR 0001](0001-optimistic-vs-zkml-vs-tee.md) — base protocol that
  the VRF freshness check defends.
