/**
 * End-to-end demo. Runs an honest swap agent and (optionally) a fraudulent
 * "lazy" swap agent, and shows what a watcher's `ByteCompareVerifier` would
 * see for each.
 *
 * Run:
 *   pnpm demo            # honest agent
 *   pnpm demo:fraud      # honest + fraudulent, with mocked watcher detection
 *
 * No real RPC calls. Every submission goes to a `MemorySubmitter` so the
 * audit trail is printed inline.
 */

import { Keypair, PublicKey } from "@solana/web3.js";

import { MemorySubmitter, hashCommitment } from "@canteen/pot";

import { buildSwapAgent } from "./agent.js";

// Fresh keypairs for each demo run. In production these are stable identities;
// for the demo we just need any valid 32-byte pubkeys.
const PROGRAM_ID = Keypair.generate().publicKey;
const OPERATOR = Keypair.generate().publicKey;
const ACTION_PDA = Keypair.generate().publicKey;
const POLICY_ID = new Uint8Array(32).fill(0xa0);
const MODEL_ID = new Uint8Array(32).fill(0x4d);

void PublicKey; // re-export hint, suppress "unused" lint

const SOL_MARKET = {
  symbol: "SOL/USDC",
  price: 142.5,
  target: 150.0,
  ts: 1_740_000_000,
};

function freshSeed(byte: number): Uint8Array {
  // Demo seed: deterministic so repeated runs produce the same commitments,
  // making the regression behaviour easy to inspect. Real agents would use
  // Pyth Entropy.
  return new Uint8Array(32).fill(byte);
}

function divider(label: string): void {
  console.log(`\n=== ${label} ${"=".repeat(Math.max(0, 60 - label.length))}`);
}

async function runHonest(): Promise<void> {
  divider("HONEST AGENT");
  const submitter = new MemorySubmitter();
  const agent = buildSwapAgent(
    submitter,
    PROGRAM_ID,
    POLICY_ID,
    MODEL_ID,
    OPERATOR,
    ACTION_PDA,
  );

  const out = await agent.runOnce({
    market: SOL_MARKET,
    budgetUsd: 250,
    vrfSeed: freshSeed(0x11),
    vrfNonceIdx: 0n,
    traceUri: "ar://honest-trace-001",
  });

  console.log("decision        :", out.decision);
  console.log("thought_pda     :", out.thoughtPda.toString());
  console.log("input commit    :", out.inputCommitmentHex);
  console.log("output commit   :", out.outputCommitmentHex);
  console.log("submit signature:", out.submitSignature);
  console.log(`submitter saw ${submitter.submitted.length} ix(s)`);
}

async function runFraud(): Promise<void> {
  divider("FRAUDULENT AGENT (lazy mode — hardcoded BUY)");
  const submitter = new MemorySubmitter();
  const agent = buildSwapAgent(
    submitter,
    PROGRAM_ID,
    POLICY_ID,
    MODEL_ID,
    OPERATOR,
    ACTION_PDA,
    { lazy: true },
  );

  // Same market as honest agent, but the lazy mode hardcodes "buy" with full
  // budget regardless of the price/target signal. The on-chain commit is
  // still well-formed; the fraud is detectable because (a) the reasoning
  // doesn't match the inputs and (b) a watcher running a re-execution or
  // a SemanticCommittee will produce a different decision under StructuralJSON
  // equivalence. For this demo we simulate the simplest detection: the
  // inconsistent-commitments check, by intentionally mis-committing the
  // output below.
  const out = await agent.runOnce({
    market: SOL_MARKET,
    budgetUsd: 1_000_000,
    vrfSeed: freshSeed(0x22),
    vrfNonceIdx: 1n,
    traceUri: "ar://fraud-trace-001",
  });

  console.log("decision        :", out.decision);
  console.log("thought_pda     :", out.thoughtPda.toString());
  console.log("input commit    :", out.inputCommitmentHex);
  console.log("output commit   :", out.outputCommitmentHex);
  console.log("submit signature:", out.submitSignature);

  divider("WATCHER DETECTION (simulated ByteCompareVerifier)");
  // The watcher fetches the trace bundle and recomputes blake3(canonical_output).
  // If the agent committed honestly the result matches; if it tried to swap
  // outputs, the hash diverges and InconsistentCommitments is filed.
  // For this demo we emulate the agent attempting to swap a "buy huge" output
  // for a "hold" output post-commit.
  const honestSeedU64 = (() => {
    const view = new DataView(freshSeed(0x22).buffer, 0, 8);
    return view.getBigUint64(0, true);
  })();
  const recomputed = hashCommitment({
    decision: { action: "hold", size_usd: 0, reasoning: "..." },
    reasoning: "...",
    tool_intents: [],
    model_id: MODEL_ID,
    sampling: { temperature: 0, top_p: 1, seed: honestSeedU64, max_tokens: 256 },
  });
  console.log("watcher recomputed:", hex(recomputed));
  console.log("agent claimed:    ", out.outputCommitmentHex);
  if (hex(recomputed) !== out.outputCommitmentHex) {
    console.log("→ MISMATCH detected. Watcher would file ChallengeClaim::InconsistentCommitments.");
    console.log("  Bond: 0.5 SOL. Expected payout (60% of 100 SOL stake): 60 SOL. EV-positive → file.");
  } else {
    console.log("→ commitments match — no fraud detected.");
  }
}

function hex(b: Uint8Array): string {
  return Array.from(b)
    .map((n) => n.toString(16).padStart(2, "0"))
    .join("");
}

const FRAUD = process.argv.includes("--fraud");

(async () => {
  await runHonest();
  if (FRAUD) await runFraud();
  console.log("\ndemo complete.");
})().catch((e) => {
  console.error("demo failed:", e);
  process.exit(1);
});
