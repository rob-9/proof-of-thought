/**
 * PoT-disciplined swap agent.
 *
 * Wraps the mock LLM with the four-step PoT cycle:
 *   1. requestVRF — fresh randomness binds the upcoming inference to a slot.
 *   2. think      — call the (mock) model with canonical I/O assembled.
 *   3. submit     — post the on-chain commitment.
 *   4. (await finalization or attestation; consumers gate downstream actions)
 *
 * The agent never decides outside `runOnce`. Anything not in `runOnce` is
 * setup or audit logging.
 */

import { PublicKey } from "@solana/web3.js";

import {
  type CanonicalInput,
  type CanonicalOutput,
  ProofOfThought,
  type Submitter,
  type SubmitResult,
} from "@canteen/pot";

import { type MarketSnapshot, decideTrade, type TradingDecision } from "./mock_llm.js";

export interface SwapAgentConfig {
  pot: ProofOfThought;
  operator: PublicKey;
  /** Action PDA the agent's downstream program would use to settle the trade. */
  actionPda: PublicKey;
  /** "lazy" mode produces hardcoded decisions — for fraud-detection demos. */
  lazy?: boolean;
}

export interface SwapAgentRunInput {
  market: MarketSnapshot;
  budgetUsd: number;
  vrfSeed: Uint8Array;
  vrfNonceIdx: bigint;
  /** Where the trace bundle would be uploaded. Demo uses a fake URI. */
  traceUri: string;
}

export interface SwapAgentRunOutput {
  decision: TradingDecision;
  thoughtPda: PublicKey;
  inputCommitmentHex: string;
  outputCommitmentHex: string;
  submitSignature: string;
}

export class SwapAgent {
  constructor(private readonly cfg: SwapAgentConfig) {}

  async runOnce(input: SwapAgentRunInput): Promise<SwapAgentRunOutput> {
    // 1. Fresh VRF.
    const handle = await this.cfg.pot.requestVRF(
      this.cfg.operator,
      input.vrfNonceIdx,
      input.vrfSeed,
    );

    // 2. Think under PoT discipline. The decision and reasoning live inside
    //    canonical_output; the inputs that drove it live inside canonical_input.
    const result = await this.cfg.pot.think(handle, async (ctx) => {
      const decision = decideTrade(input.market, input.budgetUsd, { lazy: this.cfg.lazy });

      const canonicalInput: CanonicalInput = {
        system: "you are a careful market-making agent",
        messages: [
          {
            role: "user",
            content: `decide on ${input.market.symbol} at price ${input.market.price} (target ${input.market.target}); budget $${input.budgetUsd}`,
          },
        ],
        tools: [],
        tool_calls: [
          {
            call: "get_price",
            response: {
              symbol: input.market.symbol,
              px: input.market.price,
              ts: input.market.ts,
            },
          },
        ],
        memory_snap: new Uint8Array(32),
        context_t: BigInt(input.market.ts),
        vrf_seed: ctx.seed,
        policy_id: this.cfg.pot.cfg.policyId,
      };

      const canonicalOutput: CanonicalOutput = {
        decision,
        reasoning: decision.reasoning,
        tool_intents: decision.action === "hold"
          ? []
          : [
              {
                tool: "jupiter_swap",
                args: {
                  symbol: input.market.symbol,
                  side: decision.action,
                  size_usd: decision.size_usd,
                },
              },
            ],
        model_id: this.cfg.pot.cfg.modelId,
        sampling: {
          temperature: 0,
          top_p: 1,
          // Sampling seed is a u64 derived from the first 8 bytes of the
          // VRF seed (little-endian). The full 32-byte VRF seed lives in
          // `canonical_input.vrf_seed`; the model API only takes a u64.
          seed: u64FromVrfSeed(ctx.seed),
          max_tokens: 256,
        },
      };

      return { input: canonicalInput, output: canonicalOutput };
    });

    // 3. Submit the on-chain commitment. The agent (or its surrounding
    //    framework) is also responsible for uploading the full trace bundle
    //    to Arweave/Shadow before this call so a watcher can fetch it.
    const submitted: SubmitResult = await this.cfg.pot.submit(
      this.cfg.operator,
      handle,
      result,
      { actionPda: this.cfg.actionPda, traceUri: input.traceUri },
    );

    return {
      decision: result.output.decision as TradingDecision,
      thoughtPda: submitted.thoughtPda,
      inputCommitmentHex: hex(result.inputCommitment),
      outputCommitmentHex: hex(result.outputCommitment),
      submitSignature: submitted.signature,
    };
  }
}

function hex(b: Uint8Array): string {
  return Array.from(b)
    .map((n) => n.toString(16).padStart(2, "0"))
    .join("");
}

function u64FromVrfSeed(seed: Uint8Array): bigint {
  if (seed.length < 8) throw new Error("vrf seed too short");
  const view = new DataView(seed.buffer, seed.byteOffset, 8);
  return view.getBigUint64(0, true);
}

/**
 * Convenience wrapper: builds a `SwapAgent` against a pluggable `Submitter`.
 * The demo uses `MemorySubmitter`, but a real deployment would pass an
 * `RpcSubmitter` that sends `@solana/web3.js` transactions.
 */
export function buildSwapAgent(
  submitter: Submitter,
  programId: PublicKey,
  policyId: Uint8Array,
  modelId: Uint8Array,
  operator: PublicKey,
  actionPda: PublicKey,
  opts: { lazy?: boolean } = {},
): SwapAgent {
  const pot = new ProofOfThought({ programId, submitter, policyId, modelId });
  return new SwapAgent({ pot, operator, actionPda, lazy: opts.lazy });
}
