# ADR 0007 — Stake floor, bond sizing, and slashing distribution

- Status: Accepted (2026-04-26)
- Deciders: PoT core
- Supersedes: —
- Superseded by: —

## Context

The optimistic regime (see [ADR 0001](0001-optimistic-vs-zkml-vs-tee.md))
relies on cryptoeconomic security: a misbehaving agent must stand to
lose more than it can gain by defrauding a single thought. The
mechanism design must answer four concrete questions:

1. How much **stake** must an agent post?
2. How much **bond** must a challenger post?
3. How is **slashed stake distributed**?
4. What is the penalty for **bad-faith challenges** (griefing)?

Three failure modes drive the design:

- **High-EV defection.** An adversary attacks a single high-value
  thought, accepting the loss of stake because the gain exceeds it.
  Defense: stake floor must dominate any single-thought gain.
- **Watcher under-funding.** The honest watcher set is paid only
  through slashed stake, so the slash-share must materially
  compensate watchers for re-execution costs (which include LLM
  compute, storage egress, and gas).
- **Griefing.** A malicious challenger files frivolous challenges
  to delay an honest agent. Defense: bond is forfeited on a bad
  challenge.

The relevant prior art is optimistic rollup dispute games (Optimism,
Arbitrum) and EigenLayer's slashing model. Both teach that
slash-share to the challenger must be high enough to pay actual
costs, not just a token reward, and that protocol treasury (or burn)
should absorb the rest to avoid creating second-order incentives
where challengers profit from manufactured disputes.

## Decision

PoT pins the following parameters at the protocol layer; policies
may *raise* them but not lower below the protocol floor.

### Stake

- **Agent stake floor (`S_a`):** `10 × max_loss_per_thought` summed
  across the policies the agent participates in, where
  `max_loss_per_thought` is the policy's declared upper bound on
  damage a single fraudulent thought can cause downstream.
- Stake is locked in `AgentProfile.stake_vault`. Withdrawal requires
  a cooldown and zero `active_thoughts`.

### Challenger bond

- **Bond minimum (`B_min`):** policy-defined `bond_min`. Must
  reasonably cover the watcher's marginal cost to re-execute and
  file (LLM compute, storage egress, transaction fees) plus a small
  margin.
- A successful challenge returns the bond plus a 60% share of the
  slashed agent stake (below).

### Slashing distribution — guilty agent

- **60% to the challenger.** Pays for both incurred and ongoing
  watcher costs. This is the primary watcher revenue line and must
  be material.
- **30% burned.** Removes the largest fraction from circulation; this
  blocks an attack where a single party plays both agent and
  challenger to wash stake into themselves at low cost.
- **10% to the protocol treasury.** Funds the CI watcher backstop
  and protocol operations.

### Failed-challenge penalty (griefing)

- **90% of bond to the agent**, **10% to the treasury**. The agent
  receives the dominant share to compensate for the operational
  griefing cost (delayed action, locked thought account); the
  treasury share funds anti-griefing infrastructure (challenger
  reputation, future Soft-regime watcher reputation — see
  [`future-work.md`](../future-work.md)).

## Consequences

### Positive

- **Single-thought defection is uneconomic.** With a 10× floor, an
  agent cannot break even on a one-shot fraud unless its
  `max_loss_per_thought` was massively under-declared, in which case
  consumers should not have trusted that policy in the first place.
- **Watcher economics are honest.** A 60% slash share is large
  enough to fund the LLM committee re-execution costs that
  Soft-regime watchers incur. We avoid the failure mode of
  "watching is technically permissionless but financially impossible."
- **Wash-trade attacks are bounded.** The 30% burn means an attacker
  controlling both sides of an artificial dispute loses 30% of stake
  per cycle, which dominates any plausible griefing reward.
- **Griefing is taxed.** A bad challenge costs the challenger
  ~100% of their bond. Combined with future watcher reputation,
  this provides asymmetric protection for honest agents.
- **Composability with EigenLayer-style restaking.** Stake-vault
  semantics are simple enough to wrap inside a restaking primitive
  later without breaking the slashing math.

### Negative

- **Stake floor is high for low-margin agents.** A 10× floor relative
  to declared max-loss is conservative. We accept this; a thinner
  margin invites attacks. Policies serving low-stake decisions can
  set lower `max_loss_per_thought`, which directly lowers the floor.
- **Burn is socially controversial.** Some governance frameworks
  prefer redistribution to validators or stakers. We chose burn for
  attack-resistance reasons; revisitable in future governance.
- **Failed-challenge penalty is harsh on honest mistakes.** A
  watcher whose verifier model glitches loses their bond. We
  mitigate by allowing watchers to test against a "shadow" mode
  (compute the verdict, do not file) and by funding a CI watcher
  backstop that absorbs early-stage operational variance.
- **Treasury accumulates value.** A treasury is a governance
  attractor and must be controlled by a multisig with a documented
  transition path to a DAO ([`future-work.md`](../future-work.md)).

### Neutral

- All percentages are protocol-layer floors. Policies may
  redistribute differently within the policy's portion of treasury
  inflows; the 60/30/10 split is the floor, not a ceiling.

## Alternatives considered

- **Equal-share slashing (50/50 challenger/treasury, no burn).**
  Cleaner accounting, but vulnerable to wash-trade attacks where one
  party plays both sides at low net cost.
- **All-burn slashing.** Maximally hostile to attackers, but starves
  the watcher economy. Watchers would file challenges only out of
  altruism; we cannot rely on that.
- **No griefing penalty (full bond return).** Tested against
  optimistic rollup griefing literature: bad without it. Filing
  becomes free, agents face death-by-thousand-disputes.
- **Bond as a percentage of agent stake.** Considered; rejected
  because it couples watcher cost to agent stake, which has no
  causal relation to the watcher's actual cost of work.
- **Reputation-only.** See [`alternatives.md`](../alternatives.md).
  Insufficient under adversarial assumptions.

## References

- Spec §5.4 (Stake & bond economics), §13.4.
- Optimism / Arbitrum dispute-game economic literature.
- EigenLayer slashing whitepaper — restaking and slashing primitives.
- [ADR 0001](0001-optimistic-vs-zkml-vs-tee.md) — base mechanism this
  ADR parameterizes.
- [ADR 0005](0005-equivalence-classes.md) — Soft regime that drives
  watcher cost structure.
