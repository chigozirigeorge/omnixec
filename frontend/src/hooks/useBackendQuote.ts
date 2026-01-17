import { useState, useCallback } from 'react';
import { quoteApi, Quote } from '@/lib/api';
import type { Chain } from '@/stores/useStore';

/**
 * React hook for fetching quotes from backend API
 * Uses POST /quote endpoint - backend handles all DEX routing and price calculation
 */
export function useBackendQuote() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [quote, setQuote] = useState<Quote | null>(null);

  const fetchQuote = useCallback(
    async (
      userId: string,
      fundingChain: Chain,
      executionChain: Chain,
      fundingAsset: string,
      executionAsset: string,
      fundingAmount: string,
      executionInstructionsBase64?: string,
      estimatedComputeUnits?: number
    ) => {
      if (!userId || !fundingChain || !executionChain || !fundingAsset || !executionAsset || !fundingAmount) {
        setQuote(null);
        return null;
      }

      setIsLoading(true);
      setError(null);

      try {
        const result = await quoteApi.create({
          user_id: userId,
          funding_chain: fundingChain,
          execution_chain: executionChain,
          funding_asset: fundingAsset,
          execution_asset: executionAsset,
          execution_instructions_base64: executionInstructionsBase64 || '',
          estimated_compute_units: estimatedComputeUnits,
        });
        
        setQuote(result.data);
        return result.data;
      } catch (err: any) {
        const errorMessage = err?.response?.data?.message || err?.message || 'Failed to fetch quote';
        const error = new Error(errorMessage);
        setError(error);
        setQuote(null);
        throw error;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  return { quote, isLoading, error, fetchQuote };
}

