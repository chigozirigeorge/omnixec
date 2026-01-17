import { useState, useCallback } from 'react';
import axios from 'axios';

type ChainType = 'solana' | 'stellar' | 'near';

export type Quote = {
  from: string;   // mint / asset ID
  to: string;     // mint / asset ID
  inAmount: string;
  outAmount: string;
  minOutAmount?: string; // Minimum amount out (for slippage)
  priceImpactPct?: number;
  routeData: any; // Opaque, used later for swap
  dexName: string;
  feePct?: number;
  estimatedGas?: string;
};

// Chain-specific DEX configurations
const DEX_CONFIG = {
  solana: {
    baseUrl: 'https://quote-api.jup.ag/v6',
    getQuote: async (from: string, to: string, amount: string): Promise<Quote> => {
      try {
        const response = await axios.get(`${DEX_CONFIG.solana.baseUrl}/quote`, {
          params: {
            inputMint: from,
            outputMint: to,
            amount: amount,
            slippageBps: 50, // 0.5% slippage
            onlyDirectRoutes: false,
            asLegacyTransaction: false,
          },
        });

        return {
          from,
          to,
          inAmount: amount,
          outAmount: response.data.outAmount,
          minOutAmount: response.data.otherAmountThreshold,
          priceImpactPct: response.data.priceImpactPct,
          routeData: response.data,
          dexName: 'Jupiter',
          feePct: 0.1, // 0.1% fee
        };
      } catch (error) {
        console.error('Jupiter quote error:', error);
        throw new Error('Failed to fetch quote from Jupiter');
      }
    },
  },
  stellar: {
    baseUrl: 'https://horizon-testnet.stellar.org',
    getQuote: async (from: string, to: string, amount: string): Promise<Quote> => {
      try {
        // For Stellar, we'll use the path payments endpoint
        const response = await axios.get(
          `${DEX_CONFIG.stellar.baseUrl}/paths/strict-send`,
          {
            params: {
              source_assets: `${from}:${getStellarAssetIssuer(from)}`,
              source_amount: amount,
              destination_assets: `${to}:${getStellarAssetIssuer(to)}`,
            },
          }
        );

        const bestPath = response.data._embedded.records[0];
        if (!bestPath) throw new Error('No path found');

        return {
          from,
          to,
          inAmount: amount,
          outAmount: bestPath.destination_amount,
          routeData: bestPath,
          dexName: 'Stellar DEX',
        };
      } catch (error) {
        console.error('Stellar quote error:', error);
        throw new Error('Failed to fetch quote from Stellar DEX');
      }
    },
  },
  near: {
    baseUrl: 'https://api.ref.finance',
    getQuote: async (from: string, to: string, amount: string): Promise<Quote> => {
      try {
        // Get pool info first
        const poolsResponse = await axios.get(`${DEX_CONFIG.near.baseUrl}/list-pools`);
        const pool = poolsResponse.data.find((p: any) => 
          p.token_account_ids.includes(from) && p.token_account_ids.includes(to)
        );

        if (!pool) {
          throw new Error('No pool found for the token pair');
        }

        // Get quote from Ref Finance
        const response = await axios.get(`${DEX_CONFIG.near.baseUrl}/get_quote`, {
          params: {
            token_in: from,
            token_out: to,
            amount_in: amount,
            fee_24h: 0.3, // 0.3% fee
          },
        });

        return {
          from,
          to,
          inAmount: amount,
          outAmount: response.data.amount_out,
          routeData: response.data,
          dexName: 'Ref Finance',
          feePct: 0.3, // 0.3% fee
        };
      } catch (error) {
        console.error('NEAR quote error:', error);
        throw new Error('Failed to fetch quote from Ref Finance');
      }
    },
  },
};

// Helper function to get Stellar asset issuer
function getStellarAssetIssuer(asset: string): string {
  const issuers: Record<string, string> = {
    XLM: 'native',
    USDC: 'GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN', // Testnet USDC
    yUSDC: 'GDGTVWSM4MGS4T7Z6TB4X5ZJSEBJS4XUUCILKHMQRFY7YTDPKX5ALGU7', // Testnet yUSDC
  };
  return issuers[asset] || '';
}

/**
 * Fetches a quote for a cross-chain swap
 * @param chain - The blockchain network ('solana' | 'stellar' | 'near')
 * @param fromMint - Source token mint/asset ID
 * @param toMint - Destination token mint/asset ID
 * @param amount - Amount to swap in base units (e.g., lamports, stroops, yoctoNEAR)
 * @returns Promise<Quote> - The quote details
 */
export async function getQuote(
  chain: ChainType,
  fromMint: string,
  toMint: string,
  amount: string
): Promise<Quote> {
  if (!DEX_CONFIG[chain]) {
    throw new Error(`Unsupported chain: ${chain}`);
  }

  if (!fromMint || !toMint || !amount) {
    throw new Error('Missing required parameters');
  }

  try {
    const quote = await DEX_CONFIG[chain].getQuote(fromMint, toMint, amount);
    return quote;
  } catch (error) {
    console.error(`Error getting quote for ${chain}:`, error);
    throw new Error(`Failed to get quote: ${error instanceof Error ? error.message : 'Unknown error'}`);
  }
}

/**
 * React hook for fetching quotes
 * @returns An object containing the quote, loading state, and error state
 */
export function useQuote() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [quote, setQuote] = useState<Quote | null>(null);

  const fetchQuote = useCallback(
    async (chain: ChainType, fromMint: string, toMint: string, amount: string) => {
      if (!chain || !fromMint || !toMint || !amount) {
        setQuote(null);
        return;
      }

      setIsLoading(true);
      setError(null);

      try {
        const result = await getQuote(chain, fromMint, toMint, amount);
        setQuote(result);
        return result;
      } catch (err) {
        setError(err instanceof Error ? err : new Error('Failed to fetch quote'));
        setQuote(null);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  return { quote, isLoading, error, fetchQuote };
}
