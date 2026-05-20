/**
 * Token pricing for cost estimation (Feature #21).
 *
 * Updated manually when Anthropic changes prices. Values are USD per million tokens
 * (i.e. `$/MT`). When the session's model is unknown (we don't currently capture it
 * from the `system:init` event into state), we default to Sonnet 4.6 rates.
 */

export interface ModelPricing {
  inputPerMTok: number;
  outputPerMTok: number;
  cachedInputPerMTok?: number;
}

export const MODEL_PRICING: Record<string, ModelPricing> = {
  "claude-opus-4-7": { inputPerMTok: 15.0, outputPerMTok: 75.0, cachedInputPerMTok: 1.5 },
  "claude-opus-4-6": { inputPerMTok: 15.0, outputPerMTok: 75.0, cachedInputPerMTok: 1.5 },
  "claude-opus-4-5": { inputPerMTok: 15.0, outputPerMTok: 75.0, cachedInputPerMTok: 1.5 },
  "claude-sonnet-4-6": { inputPerMTok: 3.0, outputPerMTok: 15.0, cachedInputPerMTok: 0.3 },
  "claude-sonnet-4-5": { inputPerMTok: 3.0, outputPerMTok: 15.0, cachedInputPerMTok: 0.3 },
  "claude-haiku-4-5-20251001": {
    inputPerMTok: 1.0,
    outputPerMTok: 5.0,
    cachedInputPerMTok: 0.1,
  },
};

/** Fallback when the session's model is unknown. */
export const DEFAULT_PRICING: ModelPricing = MODEL_PRICING["claude-sonnet-4-6"];

export function pricingFor(model: string | null | undefined): ModelPricing {
  if (!model) return DEFAULT_PRICING;
  return MODEL_PRICING[model] ?? DEFAULT_PRICING;
}

export function estimateCost(
  inputTokens: number,
  outputTokens: number,
  pricing: ModelPricing = DEFAULT_PRICING,
): number {
  return (
    (inputTokens / 1_000_000) * pricing.inputPerMTok +
    (outputTokens / 1_000_000) * pricing.outputPerMTok
  );
}

export function formatUSD(amount: number): string {
  if (amount < 0.01) return `$${amount.toFixed(4)}`;
  if (amount < 1) return `$${amount.toFixed(3)}`;
  return `$${amount.toFixed(2)}`;
}
