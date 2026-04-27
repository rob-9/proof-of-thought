# PoT — Initial Code Review

- Reviewer: senior code reviewer (agentic)
- Date: 2026-04-26
- Scope: spec `docs/specs/2026-04-26-proof-of-thought-design.md`,
  plan `docs/superpowers/plans/2026-04-26-pot-implementation.md`,
  Anchor program `programs/pot_program/`,
  TS SDK `sdk/ts/`,
  Rust watcher `watcher/`,
  ADR/docs suite `docs/`.
- Verdict at a glance: the architecture is sound and the partition into
  three workspaces matches the spec; the canonical-encoding layer is the
  strongest piece in the repo. **However, three of the four cross-component
  contracts (program ↔ SDK, program ↔ watcher event wire format, watcher ↔
  program ix encoding) are out of sync today, and one of those will fault
  every transaction that touches the stake vault.** None of these
  surface as test failures because the workspaces are tested in isolation.

---

## Findings

### Critical

#### C1. Stake vault is SystemProgram-owned but the program mutates its lamports directly
- File: `programs/pot_program/src/instructions/stake.rs:88-94` (`withdraw_handler`),
  `programs/pot_program/src/instructions/resolve.rs:239-256` (`debit_pda_to`).
- What is wrong: `register_agent` funds the vault PDA via
  `system_program::transfer` (`register_agent.rs:50-57`), which leaves the
  vault owned by the System Program. `withdraw_handler` and the slash path
  then try to debit it via `**vault.try_borrow_mut_lamports()? = ...`. The
  Solana runtime only permits a program to decrease the lamports of an
  account it owns; debiting a System-owned account from inside `pot_program`
  will fail at runtime with `ProgramFailedToComplete` /
  `ExternalAccountLamportSpend`. The author already flagged the issue in a
  doc comment (`register_agent.rs:23-26`) but the code does not match the
  comment.
- Why it matters: every flow that pays out of the vault — withdrawal, slash
  to challenger, slash to treasury, slash burn — is unreachable. The TS
  test suite *appears* to exercise these paths but never asserts that
  lamports actually moved (only that on-chain state fields update), so the
  bug is masked.
- Suggested fix: make the vault a program-owned PDA. Two options:
  1. Initialize a small program-owned account (`#[account(init, ..)]` with
     8 bytes of state) at register time, rent-fund it, and keep stake as
     extra lamports above rent. The program then owns the lamports and
     `try_borrow_mut_lamports` is legal.
  2. Keep it System-owned but move funds out via signed CPI:
     `system_program::transfer` with `with_signer(&[seeds])`. This is the
     cleaner pattern and avoids the rent-exempt fee surface. The same fix
     must be applied to the `bond_vault` PDA in `resolve_challenged`.

#### C2. `ThoughtSubmitted` event layout in watcher does not match what the program emits
- Files: `programs/pot_program/src/state/thought.rs:76-84` (program emit),
  `watcher/src/types.rs:199-211` and `watcher/src/subscribe.rs:202-231`
  (watcher decode).
- What is wrong: the program emits 6 fields
  (`thought_pda, agent, model_id, policy_id, slot, trace_uri`). The watcher
  decoder reads 10 fields in a different order
  (`agent, thought_pda, model_id, input_commitment, output_commitment, trace_uri_hash, vrf_seed, policy_id, slot, trace_uri`).
  Borsh deserialization will run off the end of the real payload before it
  reaches the trace_uri length prefix, returning `BadPayload("unexpected EOF")`
  for every real event — or, worse, decoding garbage into commitment fields
  if the buffer happens to be long enough.
- Why it matters: in production the watcher would silently drop every
  event. The whole pipeline downstream (`fetcher → verifier → filer`) is
  exercised in tests with a synthetic event the watcher itself produces,
  so the test harness rounds-trips against its own (wrong) layout.
- Suggested fix: align the two. The spec is silent on the event payload
  contents, so the simpler change is to extend the program event to
  include the commitments (or, better, have the watcher pull them from the
  ThoughtRecord account using `thought_pda`). Either way, lock the choice
  with a single source of truth (Anchor IDL) and bind the watcher to it
  rather than hand-rolling the layout in `subscribe.rs`.

#### C3. `ChallengeClaim` numeric values diverge across program / SDK / watcher
- Files: `programs/pot_program/src/state/challenge.rs:5-12`,
  `sdk/ts/src/types.ts:64-72`, `watcher/src/types.rs:237-248`.
- What is wrong:
  - Program defines variant 5 as `AttestationInvalid`.
  - SDK has only variants 0–4; no value 5 at all.
  - Watcher defines variant 5 as `InconsistentCommitments`.
  - Default verdict path in `verify/strict.rs:69` produces
    `ChallengeClaim::InconsistentCommitments` (= 5 over the wire), which
    `ix_data()` (`challenge.rs:62-66`) sends to the program; the program
    will Borsh-deserialize `5` as `AttestationInvalid` and accept the
    challenge under the wrong claim category. That is silent type
    confusion at the protocol boundary.
- Why it matters: dispute classification is load-bearing for §5.4
  ("60% to challenger on guilty verdict"); a wrongly-labelled claim still
  resolves but the policy resolver and on-chain explorers will report the
  wrong attack class. Fix before any external watcher writes UIs against
  these enums.
- Suggested fix: pick one canonical set, ideally
  `{ModelMismatch, OutputMismatch, InputOmission, Replay, StaleVRF, AttestationInvalid, InconsistentCommitments}`
  (7 variants), update all three sites, and lock it with a cross-language
  test that round-trips each variant.

#### C4. SDK account types are missing fields that exist on-chain
- File: `sdk/ts/src/types.ts`.
- What is wrong (each one trips byte-offset comments downstream):
  - `AgentProfile` (lines 75-96) is missing `vrf_nonce: u64`. Program defines
    it (`state/agent.rs:27-28`). Every offset after `cooldown_until` is wrong.
  - `Challenge` (lines 173-195) is missing the leading `thought: Pubkey`
    *and* the `verdict: bool`. The byte-offset comment opens at byte 8 with
    `challenger`, but the program's first field is `thought`. Total length
    in the comment is 91 bytes; actual is 124.
  - `Policy` (lines 197-222) is missing `resolver: Pubkey`,
    `treasury: Pubkey`, and `max_action_age_slots: u64`; `max_inference_ms`
    in the SDK is `max_inference_slots` in the program.
  - `VrfRequest` (lines 224-247) declares a `fulfilled_slot: u64` field
    that does not exist in the program's `VrfRequest`
    (`state/vrf.rs:17-24`).
- Why it matters: Phase B of the plan is "TS SDK client wrapping program
  using Anchor IDL + canonical types." The IDL-generated types will not
  match these hand-written interfaces, every consumer of a manual decoder
  will read garbage, and the byte-offset comments are documentation
  liabilities — anyone re-implementing the wire format from this file
  will deserialize wrong.
- Suggested fix: drop the hand-written interfaces in favour of the Anchor
  IDL types once Phase B lands; until then, regenerate the comments and
  field lists from the program structs directly (a one-page Rust→TS
  sync script is the right primitive). At minimum, fix the missing
  fields and the offset arithmetic this week.

### Important

#### I1. `request_vrf` does not pin `nonce_idx == agent.vrf_nonce`
- File: `programs/pot_program/src/instructions/request_vrf.rs:42-58`.
- What is wrong: the agent supplies `nonce_idx` and the program creates
  a PDA at that nonce, but never checks that the nonce equals the
  monotonic counter on `AgentProfile`. The agent can call `request_vrf`
  with arbitrary indexes — including nonces with a chosen relationship
  to a future block hash. That doesn't break the VRF *freshness* check
  in `submit_thought` (still time-bounded), but it weakens the
  "monotonic nonce" invariant the SDK and the spec implicitly rely on.
- Suggested fix: `require!(nonce_idx == agent.vrf_nonce, …)` before bumping.

#### I2. Ordering invariant in `submit_thought` is undocumented
- File: `programs/pot_program/src/instructions/submit_thought.rs:115-119`.
- Today the ix is correct (full revert on failure), but the ordering
  (`vrf.consumed = true` → field writes → `agent.active_thoughts += 1`)
  has no comment. Future refactors that introduce a fallible step
  after the increment will drift `active_thoughts`. Add a one-line
  invariant.

#### I3. Single-resolver authority is a known but under-documented centralization point
- File: `programs/pot_program/src/instructions/resolve.rs:88-93`.
- The resolver can keep both halves of every dispute by walking off the
  verdict. ADR 0007 and `future-work.md` mention "decentralized
  dispute" but don't call out the trust shape today. Surface in the
  README threat model.

#### I4. EV math in the watcher uses `saturating_mul` and silently mis-shapes large stakes
- File: `watcher/src/challenge.rs:244`.
- `agent_stake_lamports.saturating_mul(6) / 10`. If
  `agent_stake_lamports >= u64::MAX/6` (~3 trillion SOL — currently
  unreachable but real-world stake balances cross 10^17 lamports = ~1B
  SOL when watchers serve many agents), the multiplication saturates at
  `u64::MAX` and the division produces a wildly wrong "expected payout"
  that drives false-positive filings.
- Suggested fix: `(agent_stake_lamports as u128 * 6 / 10) as u64` with
  a saturating cast. Cheaper still: divide first
  `(agent_stake_lamports / 10) * 6`, accepting the truncation, which is
  what the program will actually compute on chain.

#### I5. `WebsocketLogStream` reconnect loop has no max-backoff and no jitter
- File: `watcher/src/subscribe.rs:75-87`.
- A flapping endpoint produces a 5s constant-interval reconnect storm; a
  fleet of watchers behind a shared NAT will hammer Solana RPC in
  lockstep. Cheap fix: `tokio::time::sleep(jittered_backoff(attempt))`.

#### I6. Tokio cancel-safety in pipeline driver
- File: `watcher/src/main.rs:172-178`.
- `tokio::select!` over `pipeline`, `stream_handle`, `ctrl_c`. If
  `pipeline` (which is `run_pipeline`) wins, in-flight `handle_event`
  tasks are dropped silently — they were spawned with `tokio::spawn` and
  will continue running, but the parent has already returned. This is
  not a correctness bug (the runtime is dropped right after `main`
  returns) but it breaks graceful shutdown: any half-filed challenge
  network call gets cancelled mid-fly. For an audit-grade watcher,
  collect spawn handles in a `JoinSet` and drain on shutdown.

#### I7. `ChallengeFiler` does not reserve bond against `max_stake_at_risk`
- File: `watcher/src/challenge.rs:255-261`.
- The check is `bond > max_stake_at_risk_lamports`, comparing one
  filing's bond against the cap. The cap is documented as "sum of active
  bond + this challenge's bond." There is no active-bond bookkeeping —
  the watcher could file 50 challenges of 0.5 SOL each and exceed a 5 SOL
  cap. Fix with an `AtomicU64` of in-flight bond, decremented when the
  RPC returns or the challenge resolves.

#### I8. `ResolveChallenged.challenger` aliasing risk
- File: `programs/pot_program/src/instructions/resolve.rs:122-123`,
  142-145. The handler enforces `challenge.challenger == challenger.key()`
  but nothing forbids the resolver from passing a write-locked alias
  of `bond_vault` or `stake_vault` as `challenger` — `debit_pda_to`
  would then be a self-credit no-op. Add explicit
  `key != bond_vault.key && key != stake_vault.key` constraints.

### Nit

- **N1.** `state/thought.rs:67` `LEN` comment partial sums don't add
  up cleanly; final 302 is correct.
- **N2.** `lib.rs:43` `GOVERNANCE` is a vanity literal with no
  derivable private key, so `register_model` is uncallable in any
  environment. Fine as MVP placeholder; document explicitly.
- **N3.** `sdk/ts/src/canonical/cbor.ts:74` accepts i64-range
  bigints; comment says "u64-only." Tighten one or the other.
- **N4.** `programs/pot_program/src/instructions/slash.rs` is a no-op
  stub. Delete or `#[allow(dead_code)]`.
- **N5.** `state/challenge.rs:31` LEN comment doesn't name fields; math right.
- **N6.** `MAX_ALLOWED_MODELS = 16` is undocumented in spec §5.1; add
  to `glossary.md`.

---

## What's solid

- **Canonicalisation layer (SDK).** `sdk/ts/src/canonical/cbor.ts` and
  `schema.ts` are the highest-quality code in the repo. The locked
  regression vectors plus determinism tests (key reorder, length-first
  sort, integer/float coalescing) are the right shape; the hand-rolled
  validators throw with JSON-pointer paths, which is exactly what
  watchers need when they bisect a fraud claim. Don't undo this.
- **Trace-fetch hash binding** (`watcher/src/trace_fetch.rs:97-106`) is
  a lean implementation of the spec's "URI swap defence." Correct,
  cheap, and the test (`uri_hash_match_accepted`) has the right
  property under test.
- **Verifier composition** (`watcher/src/verify/strict.rs`). Splitting
  byte-compare from re-execution and returning `Inconclusive` for
  cases the engine can't decide is exactly the right shape; once a
  real engine lands, the rest doesn't change.
- **Doc suite.** Eight ADRs that argue *against* their own decisions in
  parts (esp. ADR 0007 and 0005) is unusually honest. The "Why this
  isn't snake oil" framing in the spec carries through.

## What's risky

1. **No cross-language conformance test.** Three components share three
   contracts (account layouts, event layouts, ix discriminators), and
   none of them are tested across the boundary. Findings C2, C3, C4
   would be impossible if there were a single test that built an
   on-chain account, sent it through the SDK decoder, and let the
   watcher confirm it.
2. **Two Cargo workspaces.** Documented as a deviation, but the
   practical cost is that the watcher cannot import the program's IDL
   types, so it hand-rolls layouts that drift. Adding a single
   `pot-shared-types` crate (no anchor dep, just plain `repr(C)` /
   `borsh::Serialize`) used by both would close that drift surface.
3. **`Mock*Submitter` is what runs by default.** `main.rs:160-169`
   instantiates a real `RpcChallengeSubmitter` *only to drop it* and
   uses the mock. Operators reading the help text who set
   `--bond-strategy aggressive` might reasonably believe their watcher
   is filing real challenges; it isn't. The branch needs an explicit
   `--dry-run` flag and the default for non-dry-run should be the real
   submitter (gated behind a `todo!()` if necessary so misuse is loud).
4. **Slash distribution math is split between the program (60/30/10)
   and the policy account (which carries treasury but not the
   percentages).** Per spec §5.4 the percentages are global. If a
   future ADR wants per-policy distribution, the policy account has
   no place for it; refactoring later will break the wire format.
   Decide now.

---

## On the documented deviations from spec

| Deviation | Justified? | Comment |
|---|---|---|
| ThoughtRecord 264 → 302 bytes | **Yes.** Added fields are referenced in spec §5.3 lifecycle prose. | Update spec §4.1 to match. |
| `resolve` split into two ix | **Mostly.** Anchor can't dispatch on optional accounts. | The split also changed authority (unchallenged = permissionless, challenged = single resolver). That change deserves an ADR — see I3. |
| 30% burn → locked in vault | **Partially.** | Locked-in-still-owned-vault is not a burn; a future authority change could re-spend. Send to the incinerator pubkey instead. |
| declare_id char count | **Non-issue.** Base58 pubkeys are 32–44 chars. | See N2. |
| Two Cargo workspaces | **Tactical.** | Add a shared types crate before next milestone — see risk #2. |
| `RpcChallengeSubmitter` stubbed | **Honest.** | But the default binary uses the mock — risk #3. |

Net: deviations are individually defensible. The cluster
(lifecycle fields + resolve split + mock-default submitter) means the
implementation is one or two careful refactors away from the spec, not
adjacent to it. Update the spec before declaring v0.1 done.

---

## Gap to a production-grade protocol (beyond `future-work.md`)

Items missing from `future-work.md` that would block a real launch:

1. **Cross-language conformance harness.** Single CI target that round-trips
   a fixture event through program, SDK, and watcher. Without it, C2–C4
   will recur at every refactor.
2. **Account-rent reclamation.** No `close_thought` / `close_challenge`
   ix. At hackathon volume, fine; at "every Jupiter swap" volume, ~0.0021
   SOL of rent leaks into a dead PDA per thought, plus `Challenge` +
   `bond_vault` + `VrfRequest`.
3. **Account versioning + migration.** No `version: u8` on any account,
   no upgrade authority, no migrate-on-read pattern. The 264→302
   ThoughtRecord change has already happened once; the next one will
   silently break consumers.
4. **Self-slashing collusion.** Sybil attacker runs both agent and
   watcher; "guilty" verdict pays 60% to the colluding watcher and 10%
   to a controlled treasury — only the 30% burn (which today is
   *locked, not burned*, see deviations) is friction. Raise burn to a
   real burn or require watcher stake at risk.
5. **Griefing-watcher EV simulation.** Spec §5.4 picks percentages
   from intuition; no simulation bounds the griefing watcher's
   expected loss against protocol slash revenue at varying fraud
   rates. Numbers may be EV-positive for adversaries.
6. **Trace privacy.** Spec §12 punts; production won't tolerate
   plaintext system prompts on Arweave forever. Design selective
   disclosure before v1.0 or accept that PoT is "public-prompt only."
7. **Observability.** Watcher has no metrics endpoint, no
   "filed-but-underwater" structured event. Operators can't track EV
   regression except by greping `tracing::info!`.

---

## Recommended next actions (ordered)

1. Fix the vault-lamports bug (C1). Without it, slashing doesn't work.
2. Reconcile the event layout (C2) and the ChallengeClaim enum (C3) by
   introducing a single shared types crate / IDL export.
3. Bring the SDK type mirrors back in line with the program (C4) and
   add the cross-language conformance harness so they stay aligned.
4. Decide the resolve_challenged authority story (single-resolver vs
   committee) — write the ADR before any consumer integrates against
   the current shape.
5. Tighten the watcher's EV math (I4), submitter default (risk #3),
   and bond-cap accounting (I7).
6. Update the spec §4.1 layout table to match the program. Tag the
   spec `v0.2` so downstream readers don't trust the 264-byte number.
