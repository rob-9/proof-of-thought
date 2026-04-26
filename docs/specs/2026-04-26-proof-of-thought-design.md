---
title: Proof of Thought (PoT) — Verifiable Reasoning for Agent Economies
target: SWARM / Colosseum Frontier Hackathon (Canteen track)
chain: Solana (mainnet-beta + devnet)
date: 2026-04-26
status: design v0.1
---

# Proof of Thought (PoT)

A protocol for cryptographically attesting that an autonomous agent performed
genuine reasoning — with a specific model, on specific inputs, bound to a
specific moment in time — before executing an on-chain action.

PoT is the missing primitive between "an agent took an action" and "an agent
*should have* taken that action." Without it, the agent economy is rubber-stamp
fraud waiting to happen.

---

## 1. Threat model

The protocol must defeat the following adversaries:

| # | Attack | What the adversary does |
|---|---|---|
| T1 | **Replay** | Cache a past "thought" and reuse it for new actions. |
| T2 | **Model substitution** | Claim GPT-5 / Claude Opus 4.7 / Llama-405B; actually run a 7B local model. |
| T3 | **Input omission** | Claim to have considered tool outputs / memory / risk signals; actually ignore them. |
| T4 | **Lazy / null reasoning** | Hardcode a decision; pad with plausible-looking CoT that doesn't entail it. |
| T5 | **Sybil reasoning** | Spin up 1,000 agents emitting the same canned thought to manipulate a market. |
| T6 | **Decision/CoT divergence** | Emit reasoning that supports decision A, then settle action B. |
| T7 | **Front-running the prompt** | Read pending mempool action; craft retroactive "thought" that justifies copying it. |
| T8 | **Time-warp** | Generate the thought after seeing oracle/price reveal but claim it predates. |

PoT does not need to defeat T9 ("the agent is dumb"). Quality of reasoning is
out of scope; **provenance** of reasoning is in scope.

---

## 2. Approach selection

Three candidate primitives, ranked by feasibility within a 4-week hackathon
window (Apr 6 – May 11):

### A. zkML — zero-knowledge proof of inference

Prover generates SNARK that `output = M(input)` for model `M`. Tools: EZKL,
RiscZero, Modulus Labs.

- ✅ Trustless, no hardware assumption.
- ❌ Prover cost is catastrophic for >1B-param models. State of the art (Q1 2026)
  proves ~100M-param transformers in tens of minutes per inference. **Not
  viable as the primary mechanism for frontier-LLM agents.**
- 🎯 Use as a *narrow* component: prove a small "verifier model" run, not the
  big LLM run.

### B. TEE-attested inference — Intel TDX / AMD SEV-SNP / NVIDIA H100 CC

Inference happens inside a confidential VM. Hardware root-of-trust signs a
quote over `(measurement, report_data)` where `report_data = H(input ∥ output ∥ nonce)`.

- ✅ Cheap on-chain verification (one signature check + cert-chain).
- ✅ Works for any model size — including hosted frontier models that ship via a
  TEE-fronted API (e.g., OpenAI Confidential / Anthropic Confidential / Marlin
  Oyster, Phala, Super Protocol).
- ❌ Hardware trust assumption + side-channel history.
- ❌ Hosted frontier APIs that don't ship CC have no path to attestation.

### C. Optimistic + cryptoeconomic — commit, reveal, challenge, slash

Agent posts a commitment to (model, input, output, time). Trace stored
off-chain in content-addressed storage. Anyone can re-execute and submit a
fraud proof during a challenge window. Misbehavior slashes the agent's stake.

- ✅ No exotic crypto, no exotic hardware. Ships in 4 weeks.
- ✅ Generalizes to any model — including non-deterministic and closed-weights.
- ❌ Depends on at least one honest watcher (1-of-N).
- ❌ Requires a finalization delay before downstream actions.

### Recommendation

Build **C as the base layer** (the only thing actually shippable in 4 weeks),
with **B as an opt-in upgrade tier** for high-value actions (a TEE attestation
short-circuits the challenge window), and **A as a future-work component** for
the verifier-model path.

The submitted hackathon deliverable is C + a stub for B.

---

## 3. Architecture overview

```
┌─────────────────────────────────────────────────────────────────────┐
│  Agent Runtime (off-chain)                                           │
│  ┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐    │
│  │ Input        │──▶│ Inference        │──▶│ Canonicalizer +  │    │
│  │ Assembler    │   │ Adapter          │   │ Commitment Builder│   │
│  └──────────────┘   └──────────────────┘   └────────┬─────────┘    │
│                                                      │              │
│                            ┌─────────────────────────┴────────┐     │
│                            ▼                                  ▼     │
│                    ┌────────────────┐                 ┌────────────┐│
│                    │ Trace Uploader │                 │ Tx Builder ││
│                    │ (Arweave/Shadow│                 │            ││
│                    │  Drive bundle) │                 │            ││
│                    └────────┬───────┘                 └─────┬──────┘│
└─────────────────────────────┼─────────────────────────────────┼─────┘
                              │ blob_uri = ar://...             │
                              │                                 │
                              ▼                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Solana Program: pot_program (Anchor)                                │
│  ┌────────────────┐  ┌────────────────┐  ┌──────────────────────┐  │
│  │ submit_thought │  │ challenge      │  │ resolve              │  │
│  │ (commit)       │  │ (open dispute) │  │ (slash or finalize)  │  │
│  └────────────────┘  └────────────────┘  └──────────────────────┘  │
│  ┌────────────────┐  ┌────────────────┐  ┌──────────────────────┐  │
│  │ register_agent │  │ register_model │  │ stake / unstake      │  │
│  └────────────────┘  └────────────────┘  └──────────────────────┘  │
└─────────────────────────────┬─────────────────────────────────┬─────┘
                              │                                 │
              Pyth Entropy ───┘                  ┌──────────────┘
              (VRF for freshness)                │
                                                 ▼
                                  ┌────────────────────────────┐
                                  │ Watcher Network (off-chain)│
                                  │ - subscribes to commits     │
                                  │ - re-executes deterministic │
                                  │ - files challenges          │
                                  └────────────────────────────┘
```

Three parties:

1. **Agent.** Produces ThoughtRecords. Posts commitments. Has stake at risk.
2. **Watchers.** Permissionless. Re-execute, challenge, earn slashed stake.
3. **Consumers.** Programs/users that gate downstream actions on a finalized PoT.

---

## 4. Data model

### 4.1 ThoughtRecord (on-chain commitment)

256 bytes, fixed layout. Posted via `submit_thought` instruction.

```rust
#[account]
#[repr(C)]
pub struct ThoughtRecord {
    pub agent: Pubkey,            // 32 — registered agent identity
    pub model_id: [u8; 32],       // 32 — canonical model digest (see §4.4)
    pub input_commitment: [u8; 32],   // 32 — H(canonical_input)
    pub output_commitment: [u8; 32],  // 32 — H(canonical_output)
    pub trace_uri_hash: [u8; 32], // 32 — H(arweave/shadow URI), URI itself in event log
    pub vrf_seed: [u8; 32],       // 32 — Pyth Entropy seed bound to commit slot
    pub policy_id: [u8; 32],      // 32 — H(policy doc — schema, equiv-class rules, etc.)
    pub slot: u64,                // 8  — Solana slot at submit time
    pub action_pda: Pubkey,       // 32 — PDA of the downstream action this thought gates
    pub status: u8,               // 1  — Pending | Challenged | Finalized | Slashed
    pub _pad: [u8; 7],            // 7  — alignment
}
// Total: 264 bytes incl. discriminator 8.
```

### 4.2 Canonical input

The input that gets hashed is **everything the model saw**, in a fixed
serialization. This kills T3 and T7.

```
canonical_input = CBOR_canonical({
  "system":      <string>,
  "messages":    [<role, content, attachments[]>],
  "tools":       [<tool schema>],
  "tool_calls":  [<call, response>],     // tool outputs the model consumed
  "memory_snap": <merkle_root_of_kv>,    // long-term memory included by reference
  "context_t":   <slot>,                 // Solana slot at start of inference
  "vrf_seed":    <32 bytes>,             // pulled BEFORE inference; binds freshness
  "policy_id":   <32 bytes>,
})
input_commitment = blake3(canonical_input)
```

**Memory inclusion rule:** if the agent's policy declares it uses long-term
memory, `memory_snap` MUST be the Merkle root of the KV store *as of slot
`context_t`*. A challenger can prove omission by demonstrating a memory entry
that the policy says is in-scope but is absent from the snapshot.

### 4.3 Canonical output

The "thought" itself: structured output the agent produced.

```
canonical_output = CBOR_canonical({
  "decision":     <typed JSON, schema-validated against policy>,
  "reasoning":    <string | array of steps>,
  "tool_intents": [<tool, args>],
  "self_score":   <float>,                // optional: agent's confidence
  "model_id":     <32 bytes>,             // claimed model
  "sampling": {
    "temperature": <float>,
    "top_p":       <float>,
    "seed":        <u64>,                 // REQUIRED; from vrf_seed
    "max_tokens":  <int>,
  },
})
output_commitment = blake3(canonical_output)
```

**Determinism rule.** For the base optimistic tier, sampling MUST be
`temperature = 0, seed = derive(vrf_seed)`. Closed-weights APIs that don't
honor seeds (most do not, deterministically) trigger the "soft equivalence
class" path — see §6.3.

### 4.4 Model identity

`model_id` is the digest the agent commits to. Three classes:

- **Open weights:** `model_id = blake3(safetensors_bytes)`. Deterministic,
  cheap to verify by re-download.
- **Hosted (closed) frontier:** `model_id = H(provider ∥ model_name ∥ snapshot_date ∥ hosted_pubkey)` —
  the provider's published key signs an attestation that this snapshot exists.
  Trust in the provider is explicit and isolated.
- **TEE-fronted:** `model_id = H(measurement ∥ image_id)` — the TEE's
  measurement register binds the running binary to a specific model.

A `register_model` instruction stores `(model_id → class, verifier_pubkey)`
the dispute resolver uses.

### 4.5 Trace bundle (off-chain)

The trace is a tarball stored on Arweave (permanent) or Shadow Drive (cheap,
mutable-but-signed). It contains everything a watcher needs to re-execute:

```
trace_<commit_id>.tar.zst
├── canonical_input.cbor
├── canonical_output.cbor
├── raw_provider_response.json   // for hosted models — signed if available
├── tool_io/
│   ├── 00_get_price.req.json
│   ├── 00_get_price.res.json    // includes provider signature if oracle
│   └── ...
├── memory_proof.cbor            // Merkle inclusion proofs for memory cells the model used
├── attestation.bin              // optional: TEE quote — short-circuits dispute
└── manifest.cbor                // ordered hashes; hashed to give trace_uri_hash
```

The `trace_uri` (Arweave tx id) is logged in a Solana event, not stored in the
account, to keep the account small. Its `H(uri)` lives in the account so the
URI cannot be silently swapped.

---

## 5. On-chain program (Anchor)

### 5.1 Accounts

```
AgentProfile     PDA: ["agent", agent_pubkey]
  - operator: Pubkey
  - stake_vault: Pubkey
  - stake_amount: u64
  - reputation: i64
  - active_thoughts: u32
  - cooldown_until: u64

ModelRegistry    PDA: ["model", model_id]
  - class: u8
  - verifier_pubkey: Pubkey  // for hosted-attested
  - tee_root_ca: Pubkey      // for TEE class
  - registered_by: Pubkey

ThoughtRecord    PDA: ["thought", agent, nonce]   (see §4.1)

Challenge        PDA: ["challenge", thought_pda, challenger]
  - challenger: Pubkey
  - bond: u64
  - claim: ChallengeClaim    // enum: ModelMismatch | OutputMismatch | InputOmission | Replay | StaleVRF
  - evidence_uri_hash: [u8; 32]
  - opened_at_slot: u64
  - resolved: bool

Policy           PDA: ["policy", policy_id]
  - schema_uri_hash: [u8; 32]
  - equiv_class: EquivClass  // Strict | StructuralJSON | SemanticCommittee
  - max_inference_ms: u32
  - allowed_models: Vec<[u8;32]>   // bounded
  - challenge_window_slots: u64
  - bond_min: u64
```

### 5.2 Instructions

| ix | who | does what |
|---|---|---|
| `register_agent(stake)` | operator | creates AgentProfile, locks stake |
| `register_model(model_id, class, verifier)` | governance | adds to ModelRegistry |
| `register_policy(...)` | anyone | self-describes a policy a consumer trusts |
| `request_vrf()` | agent | requests Pyth Entropy → nonce stored against (agent, nonce_idx) |
| `submit_thought(record, trace_uri)` | agent | atomic: validates VRF freshness, agent stake ≥ bond_min, model registered, action_pda owned by agent. Emits `ThoughtSubmitted` event. |
| `consume_thought(thought_pda)` | downstream program (CPI) | gates an action: requires `status == Finalized` OR `attestation_verified == true`. |
| `challenge(thought_pda, claim, bond, evidence_uri)` | watcher | locks bond, opens dispute, freezes thought |
| `resolve(thought_pda)` | crank | after challenge_window with no challenge: Finalize. Or runs verdict logic if challenged. |
| `slash(agent, amount, beneficiary)` | program-internal | called by `resolve` on guilty verdict |
| `withdraw_stake(amount)` | operator | only after cooldown and 0 active_thoughts |

### 5.3 Lifecycle

```
   t0           t0+1 slot                t0 + W slots
   │              │                          │
   │  request_vrf │  submit_thought          │  resolve (no challenge → Finalize)
   ▼              ▼                          ▼
 ──●──────────────●──────────────────────────●────────────────▶ slot
                  │                          │
                  ├─ ThoughtSubmitted event  │
                  │   trace_uri logged       │
                  │                          │
                  │       challenge(...)     │
                  │           │              │
                  │           ▼              │
                  │     ──────●───────       │
                  │     evidence + bond      │
                  │           │              │
                  │           ▼  resolve(...)│
                  │  Slash agent  OR  Slash challenger
```

`W` (challenge window) is policy-defined. Default 150 slots (~60s). For
high-value actions, a policy may require `W ≥ 1500` slots (~10 min) and/or a
TEE attestation to skip.

### 5.4 Stake & bond economics

- Agent stake `S_a`: locked, slashed on guilty verdict. Recommended floor:
  `10 × max_loss_per_thought` for the policies it participates in.
- Challenger bond `B_c`: per-challenge lockup. Returned + 50% of slashed agent
  stake on success; forfeited on bad-faith failure.
- Distribution on guilty verdict: `60%` to challenger, `30%` burned, `10%` to
  protocol treasury (pays for CI watchers as backstop).
- Failed challenge: `90%` to agent (penalty: griefing tax), `10%` treasury.

---

## 6. The hard part: handling LLM nondeterminism

Re-execution as a fraud-proof primitive only works if running the same model on
the same input produces a verifiable output. This is straightforwardly true for
open-weights local models with `temperature=0, deterministic CUDA kernels` and
straightforwardly false for production LLM APIs. PoT handles three regimes:

### 6.1 Strict regime (open weights, local exec)

Watcher re-runs `M(input) → output'`. Verdict = `output' == output`
byte-for-byte after canonicalization.

Determinism prerequisites pinned by policy:

- safetensors hash matches `model_id`
- inference engine + version pinned (e.g. `vllm==0.7.3`, `flash-attn==2.x`)
- `CUBLAS_WORKSPACE_CONFIG=:4096:8`, `torch.use_deterministic_algorithms(True)`
- temperature = 0, top_p = 1.0, no speculative decoding
- batch size = 1 (kernel selection varies with batch)
- GPU class declared in policy (kernel selection varies across SM versions)

If any prerequisite fails, fall back to 6.3.

### 6.2 Hardware-attested regime

Trace contains a TEE quote whose `report_data` field equals
`H(input_commitment ∥ output_commitment ∥ vrf_seed)`. Resolver verifies the
quote against the registered TEE root CA, marks `attestation_verified`, and
finalization is immediate (challenge window collapses to 0).

This is the ONLY path that works for hosted frontier APIs without a re-exec —
provided the host ships a TEE-CC API. If they don't, this regime is unavailable
and the policy must allow 6.3.

### 6.3 Soft-equivalence regime

For non-deterministic / closed-weights / no-attestation models, re-execution is
probabilistic. The policy declares an `EquivClass`:

- **`StructuralJSON`** — only the `decision` field of canonical_output must
  match exactly. The `reasoning` field may differ. Suitable for narrow
  decisions like "buy/hold/sell" or "approve/deny."
- **`SemanticCommittee`** — a registered committee of `k` independent verifier
  models (e.g. `[gpt-4o-mini, claude-haiku-4.5, llama-3.3-70b-instruct]`) is
  asked: "do these two outputs entail the same downstream action under
  policy P?" Quorum `q-of-k` decides. Each committee model run is itself a
  PoT under StrictRegime (small open model with seed=0) — bottom of the
  recursion is verifiable.
- **`AnyOfN`** — agent commits to a Merkle root of N samples taken with
  declared seeds; reveals one. Watcher re-samples and accepts if the revealed
  output is among the N samples (dispute is a Merkle non-membership proof).

The policy decides which class is acceptable; consumers decide which policies
to trust.

### 6.4 Why this isn't snake oil

The non-determinism objection — "you can't verify a black-box LLM" — is true
only if the goal is *full reproducibility*. PoT's goal is *fraud detection*.
A lazy agent that hardcodes "buy" is detected because the verifier model
committee will not entail "buy" from inputs that include a red price signal.
A model-substitution agent (claims GPT-5, ran a 7B) is detected because the
quality of reasoning under SemanticCommittee diverges sharply from the
declared model's prior runs.

What PoT does NOT prove: that the reasoning was *correct*. It proves the
reasoning was *generated*, *attributed*, *fresh*, and *coherent with the
declared decision*. That's enough for the agent economy.

---

## 7. Freshness: VRF binding

Every thought references `vrf_seed` from Pyth Entropy. The seed is requested
*before* inference and embedded in the canonical input. Two checks at submit
time:

1. `vrf_seed` is the latest unconsumed seed for this agent (mempool monitor),
   AND
2. `slot - vrf_seed.slot ≤ max_inference_ms / 400ms` (capped freshness).

This kills:

- **T1 replay** — the seed is fresh and single-use, the input commitment binds
  to it, so the same output cannot be reused.
- **T8 time-warp** — the VRF binds the thought to a specific slot interval; the
  agent cannot have observed any data revealed after `vrf_seed.slot` (Pyth
  price updates, trade fills) and rolled it into reasoning that pretends to
  predate it.
- **T7 prompt-front-running** — the input commitment is sealed before any
  pending action it could be derived from is observable.

---

## 8. Watcher network

Permissionless, off-chain, paid by slashed bonds.

**Reference watcher daemon** ships as part of the protocol:

```
pot-watcher \
  --rpc https://api.mainnet-beta.solana.com \
  --policies pol_<id>,pol_<id>... \
  --models /mnt/models/ \                  # local weights cache
  --max-stake-at-risk 50_SOL \
  --bond-strategy aggressive \
  --redrive-storage shadow                  # for fetching traces
```

Watcher loop:

1. Subscribe to `ThoughtSubmitted` events for declared policies.
2. Pull trace from Arweave/Shadow (verify `H(uri) == trace_uri_hash`).
3. Decide regime from policy. Run verification:
   - Strict: re-exec, byte-compare.
   - Attested: verify TEE quote.
   - Soft: run committee, check equivalence.
4. On mismatch and EV-positive (`expected_payout - bond - gas > 0`), file
   `challenge(...)`.

A first-class **CI-watcher** runs from the protocol treasury as a backstop in
case the open watcher network is thin in early days.

---

## 9. Consumer integration

A consumer program (e.g. an agent's DEX adapter) doesn't trust an action
unless gated:

```rust
// consumer's instruction
pub fn execute_with_pot(ctx: Context<ExecuteWithPot>, ...) -> Result<()> {
    let thought = &ctx.accounts.thought_record;

    require!(thought.action_pda == ctx.accounts.action.key(), ErrorCode::WrongAction);
    require!(thought.policy_id == ctx.accounts.policy.expected_policy, ErrorCode::WrongPolicy);
    require!(
        thought.status == ThoughtStatus::Finalized
            || thought.attestation_verified,
        ErrorCode::ThoughtNotReady
    );
    require!(
        Clock::get()?.slot - thought.slot <= ctx.accounts.policy.max_action_age_slots,
        ErrorCode::ThoughtStale
    );

    // ... do the thing
}
```

CPI to `pot_program::consume_thought` increments a usage counter (each thought
single-use unless policy says otherwise) and emits an audit event.

---

## 10. SDK surface (TypeScript)

```ts
import { ProofOfThought, models, policies } from "@canteen/pot";

const pot = new ProofOfThought({
  connection,
  agentKeypair,
  storage: "arweave",        // or "shadow"
  policy: policies.JUP_SWAP_V1,
});

// 1. fetch fresh randomness
const seed = await pot.requestVRF();

// 2. wrap inference — pot intercepts the call, captures canonical I/O,
//    records seed inside the prompt
const result = await pot.think(seed, async (ctx) => {
  return openai.chat.completions.create({
    model: "gpt-5",
    seed: ctx.seed,
    temperature: 0,
    messages: ctx.messages,
    tools: ctx.tools,
    response_format: { type: "json_schema", schema: ctx.schema },
  });
});

// 3. submit commitment — returns thought PDA
const thought = await pot.submit(result, { actionPda });

// 4. wait for finalization (or attestation accepted)
await pot.waitFinalized(thought, { timeoutMs: 90_000 });

// 5. CPI from your program
await yourProgram.methods.executeWithPot(...).accounts({ thought }).rpc();
```

Rust SDK mirrors this for native agents.

---

## 11. Build plan (4 weeks)

Implementation will be planned in detail in the writing-plans phase; this is
the rough decomposition.

**Week 1 — protocol skeleton**
- Anchor program: accounts, `register_agent`, `register_model`, `submit_thought`, `consume_thought`, `resolve` (no-challenge fast path)
- Pyth Entropy integration for VRF
- Devnet deploy
- TS SDK skeleton: canonicalizer (CBOR), commitment builder, submit flow

**Week 2 — challenge & dispute**
- `challenge`, `resolve` with verdict logic
- Stake/bond accounting, slashing distribution
- Reference watcher daemon (Strict regime only): pull trace from Arweave, re-exec with vLLM + Llama-3.1-8B, byte-compare, file challenge

**Week 3 — soft-equivalence + storage**
- StructuralJSON equivalence
- SemanticCommittee — three small open models, recursive PoT inside committee runs
- Trace bundling + Arweave/Shadow uploader
- Memory Merkleization

**Week 4 — polish, demo, attestation stub**
- TEE attestation verifier (Intel TDX quote — signature check + cert chain). Real H100 CC integration is post-hackathon; demo uses a mocked attestation against a held-out test key.
- Demo agent: a Jupiter swap agent that uses PoT to gate trades > $100. Multi-watcher demo: spin up an honest watcher and a malicious agent that hardcodes "buy" — show slashing in action.
- Pitch deck: real on-chain transactions on devnet, $-volume gated by PoT.

---

## 12. Out of scope (explicit non-goals for hackathon)

- zkML prover for the main inference (deferred — research stub only)
- Cross-chain PoT (Solana-only)
- Privacy-preserving traces (assumes traces are public; private-trace mode is
  future work using TEE + selective disclosure)
- Streaming / agentic-loop reasoning (each thought is one inference call;
  multi-step plans = multiple chained PoTs, plan structure is policy-defined)
- Proving *quality* of reasoning (only provenance + coherence)
- Defending against the host of a closed model colluding with the agent (the
  hosted-model class makes this trust assumption explicit)

---

## 13. Open questions

1. **Pyth Entropy latency.** Current p99 is ~2 slots. If higher under load,
   freshness window has to be padded, which expands T8 surface. Fallback:
   Switchboard VRF, or a 2-of-2 commit-reveal where the agent supplies half.
2. **Arweave finality.** Bundles confirm in minutes; Solana actions can outpace
   storage. Mitigation: short-window agents post traces to Shadow Drive
   first, mirror to Arweave async.
3. **Committee gameability.** A SemanticCommittee of three closed-API models
   creates a 3-API trust dependency. Must be at least one open-weights model
   in any committee.
4. **Memory Merkle cost.** Maintaining a Merkleized KV with O(log n) updates
   for every memory write is meaningful overhead. Acceptable for hot-path
   agents? Likely yes; benchmark in week 3.
5. **Hosted seed determinism.** OpenAI's `seed` is best-effort. Anthropic
   doesn't expose one. In practice, closed-API agents will live in
   SemanticCommittee mode, not Strict. The protocol should make this trust
   shape explicit to consumers in the policy schema.

---

## 14. Why this wins the track

- **Innovation (40%).** No production protocol today provides verifiable
  agent reasoning provenance on-chain. zkML demos exist for tiny models,
  TEE-LLM exists in research; PoT is the first protocol-level composition
  shipping a usable primitive for agent economies.
- **Agentic sophistication (30%).** PoT is *agent infrastructure* — it makes
  every agent that uses it more sophisticated by giving its actions a verifiable
  cognitive provenance. Demo includes recursive PoT (committee verifiers are
  themselves agents).
- **Real traction (30%).** Demo executes real swaps on Solana devnet/mainnet
  gated by PoT, with a public watcher that has slashed an attacker. Numbers
  to show: thoughts/sec, finalization latency, end-to-end gas cost,
  $-volume gated, slashings.
