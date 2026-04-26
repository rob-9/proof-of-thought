# Tradeoffs

Every interesting decision in PoT is a tradeoff between two properties
that both matter. This document names five cross-cutting axes,
states the position we took, says what we gave up, and lists what
would flip the decision. The ADRs in [`adr/`](adr/) hold the
bilateral context for each individual decision; this is the
synthesized cross-axis view.

| Axis | Position chosen | Given up | Would flip if … |
|---|---|---|---|
| Latency vs assurance | Default 150-slot challenge window (~60s); 1500 slots for high-value | Sub-second action gating | TEE attestation becomes universally available, or zkML throughput closes the gap |
| Cost vs decentralization | Treasury-funded CI watcher backstop atop permissionless watcher network | Pure permissionlessness on day one | Watcher economics scale to fund a thick set without protocol subsidy |
| Determinism vs model choice | Three regimes (Strict / Attested / Soft), at least one open-weights committee member | Byte-exact replay across the whole protocol | Hosted frontier APIs ship verifiable TEE-CC endpoints by default |
| Storage cost vs auditability | Hybrid: Shadow primary, Arweave async mirror | Single-system simplicity | A storage layer offers Arweave-grade permanence at Shadow-grade latency |
| Policy flexibility vs consumer trust burden | Policies are public, declarative, single-purpose | A "one-size" out-of-the-box policy | Sufficient field experience produces a small, audited canonical policy set |

The prose below expands each axis. Each section is structured
identically: the choice, the cost, the flip condition.

## 1. Latency vs assurance

**Choice.** PoT's base assurance comes from re-execution during a
challenge window. The default window is 150 Solana slots (roughly
60 seconds); high-value policies may require 1500 slots (~10 minutes)
or a TEE attestation that collapses the window to zero.

**What we gave up.** Sub-second action gating is impossible in the
default optimistic mode. PoT is therefore inappropriate as a
front-line gate for MEV back-runs, cross-exchange arbitrage at the
millisecond scale, and any control loop where the action is stale by
the time it can be finalized. We accept this exclusion explicitly
([ADR 0001](adr/0001-optimistic-vs-zkml-vs-tee.md)). For
real-time-adjacent flows, the Attested regime's zero-window TEE path
is the only option PoT offers.

The window length itself is a sub-tradeoff: shorter windows reduce
agent latency but tighten the watcher's reaction time. With the
defaults, a watcher must subscribe, fetch, verify, and submit a
challenge transaction inside roughly 60 seconds. That is achievable
for Strict-regime watchers but tight for Soft-regime watchers
running an LLM committee. Policies that mandate Soft regime should
use the longer 1500-slot window for that reason.

**What would flip this.** If TEE-CC endpoints become universally
available across hosted frontier providers — i.e., the providers
ship verifiable quotes a third party can check — the Attested
regime would dominate, and the optimistic window's role would
shrink to a fallback. Alternatively, if zkML throughput at frontier
sizes closes by an order of magnitude or two (a multi-year horizon),
zkML proofs would short-circuit the window entirely. Neither flip is
in sight in 2026; we ship for the world we have.

## 2. Cost vs decentralization

**Choice.** PoT's watcher network is permissionless and paid by
slashed bonds ([ADR 0007](adr/0007-stake-bond-economics.md)). To
prevent a thin early watcher set from giving fraud a free ride, the
protocol treasury funds a **CI watcher** as a backstop: it always
runs, always checks, always files when EV-positive.

**What we gave up.** Pure permissionlessness on day one. A
treasury-funded backstop is, definitionally, a privileged actor.
We mitigate by making it inspectable (the CI watcher runs the
reference daemon, no special instruction access) and by sizing the
slash distribution to make permissionless watchers economically
viable as the network thickens (60% slash share to challengers, see
ADR 0007).

The specific number that should set this tradeoff is **expected
fraud frequency × average loss per fraud**. If fraud is rare, the
watcher economy starves; if fraud is common, watchers are well-paid
but the protocol is broken. The CI watcher absorbs the volatility in
the rare-fraud regime that early-stage protocols always inhabit.

**What would flip this.** If post-launch fraud frequency or watcher
economics become favorable enough to support a thick permissionless
set without subsidy, the CI watcher becomes a vestigial backstop
that runs on autopilot. A future governance vote could discontinue
it. Conversely, if fraud rates fall *below* expected (good outcome,
financially bad for watchers), the slash distribution may need
adjustment to prevent watcher exit; this is a parameter, not a
protocol change.

## 3. Determinism vs model choice

**Choice.** Three regimes ([ADR 0005](adr/0005-equivalence-classes.md)):
Strict (byte-exact replay), Attested (TEE quote), Soft (entailment
via a recursive committee). The protocol does not pick a single
determinism story for all models; it lets a policy declare which
regime is acceptable for which model class, and consumers gate on
the policy they trust.

**What we gave up.** A single, simple verification rule. A protocol
that demanded `output' == output` byte-for-byte everywhere would be
trivial to reason about — and useless for any frontier hosted model,
because seed determinism on hosted APIs is best-effort at best (spec
§13.5). PoT instead adopts a regime *per* model class and accepts
the resulting cognitive load on consumers.

A second piece given up: hosted closed APIs in Strict mode. Strict
locks out OpenAI, Anthropic, xAI, Mistral hosted, and any provider
that does not honor a deterministic seed end to end. Those models
are accommodated through the SemanticCommittee path
([ADR 0008](adr/0008-recursive-pot-committee.md)), at the cost of
explicit additional trust assumptions baked into the policy.

**What would flip this.** A few things, in order of likelihood:
(a) hosted providers ship deterministic-seed APIs with a verifiable
guarantee (rare today; possible at the scale of e.g. dedicated
inference partners); (b) hosted providers ship verifiable TEE-CC
endpoints (then they move to the Attested regime); (c) zkML at
frontier size becomes real (then closed-API agents move to a
ZkProven class). Until at least one happens, the three-regime
design is not optional.

## 4. Storage cost vs auditability

**Choice.** Trace bundles upload synchronously to Shadow Drive
(low-latency, cheap) and asynchronously mirror to Arweave
(permanent). `trace_uri_hash` covers the Shadow URI; high-value
policies may also require a confirmed Arweave archive
([ADR 0006](adr/0006-arweave-vs-shadow-storage.md)).

**What we gave up.** Single-system simplicity. Two storage
integrations means twice the SDK surface, twice the failure modes,
and a small ambiguity around which URI counts as authoritative for
a given dispute. We pay that complexity in exchange for being
operationally honest: Arweave's confirmation latency cannot block
`submit_thought`, and Shadow's mutability story cannot be the sole
basis for a multi-year audit.

A subtler thing given up: a clean "all on-chain" story. Some
adjacent designs (with a different cost model) keep traces on
chain. PoT's traces can run into low megabytes; on-chain storage
is not viable at any plausible volume. The off-chain trace + on-chain
hash model is a deliberate choice; the cost is that consumers
inspecting actions post hoc need a working off-chain client.

**What would flip this.** A storage layer that combined Arweave's
permanence with Shadow's latency and cost would obviate the
hybrid. The closest current contenders (Walrus, Filecoin's
retrievable storage providers) do not yet make that case
convincingly *on Solana*. If that changes, the protocol simplifies.

## 5. Policy flexibility vs consumer trust burden

**Choice.** Policies are public, declarative, schema-driven
documents stored at `["policy", policy_id]`. Anyone can register
one; consumers are responsible for picking which policies to
trust. The protocol does not bless a "default" policy.

**What we gave up.** Drop-in trust. A consumer integrating PoT for
the first time has to read at least one policy document end to end
to know what guarantees they are buying. We considered shipping a
small set of canonical policies with the protocol; we did not, on
the principle that a single canonical policy creates one-shot
captures of the protocol's trust model and entrenches whatever was
chosen for the demo.

The trust burden is meaningful. A consumer that gates on a
SemanticCommittee policy whose committee is three closed APIs has
weak guarantees, and the protocol does not protect them from this
mistake — it only makes it inspectable. The "at least one
open-weights committee member" rule is enforced by the program;
beyond that, consumer judgment is required.

**What would flip this.** Months of field experience with concrete
policies, audited by a security firm, would let the protocol ship
a small canonical set ("PoT-Strict-OpenWeights-v1",
"PoT-SoftCommittee-Llama-Mistral-Qwen-v1") that consumers can use
without bespoke review. Until then, naming a default is premature.

## Cross-axis observations

Two patterns recur across all five axes:

1. **TEE availability is the master variable.** If high-quality
   TEE-CC endpoints become universal, axes 1 and 3 both get
   easier — the Attested regime becomes the default and the
   challenge-window tradeoff weakens. PoT is intentionally
   forward-compatible with that world.

2. **Policy is where the trust shape lives.** Axes 3, 4, and 5 are
   all parameterized by policy. The protocol's job is to make the
   trust shape *legible*, not to make a single trust shape mandatory.
   That is the deeper reason policies are first-class accounts and
   not protocol constants.

For the dimensional alternatives we did **not** build at all, see
[`alternatives.md`](alternatives.md). For the roadmap items that
loosen these tradeoffs over time, see [`future-work.md`](future-work.md).
