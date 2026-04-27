/**
 * A deterministic stand-in for an LLM call.
 *
 * Real production agents wrap a frontier-model API. The demo's job is to
 * exercise the PoT discipline (canonical I/O, freshness binding, on-chain
 * commit, optional fraud), not to model real reasoning. The "model" here
 * follows a simple rule: buy if price < target, sell if price > target * 1.05,
 * else hold. The reasoning string is templated so a SemanticCommittee
 * verifier could in principle entail it (this opens the door for the Z3
 * composition described in `docs/related-work.md`).
 */

export interface MarketSnapshot {
  symbol: string;
  price: number;
  target: number;
  ts: number;
}

export interface TradingDecision {
  action: "buy" | "sell" | "hold";
  size_usd: number;
  reasoning: string;
}

/**
 * Deterministic decision rule. Same inputs → same output, byte-for-byte.
 *
 * If `lazy = true`, returns a hardcoded "buy" regardless of inputs — used
 * by the fraud-mode demo to simulate a lazy-reasoning attacker.
 */
export function decideTrade(
  market: MarketSnapshot,
  budget_usd: number,
  opts: { lazy?: boolean } = {},
): TradingDecision {
  if (opts.lazy === true) {
    return {
      action: "buy",
      size_usd: budget_usd,
      reasoning:
        "Approving trade based on positive momentum signals and favourable risk/reward profile.",
    };
  }

  if (market.price < market.target) {
    return {
      action: "buy",
      size_usd: Math.min(budget_usd, 100),
      reasoning: `${market.symbol} at ${market.price} is below target ${market.target}; buying $${Math.min(budget_usd, 100)} of size.`,
    };
  }
  if (market.price > market.target * 1.05) {
    return {
      action: "sell",
      size_usd: Math.min(budget_usd, 100),
      reasoning: `${market.symbol} at ${market.price} is more than 5% above target ${market.target}; selling.`,
    };
  }
  return {
    action: "hold",
    size_usd: 0,
    reasoning: `${market.symbol} at ${market.price} is within 5% of target ${market.target}; holding.`,
  };
}
