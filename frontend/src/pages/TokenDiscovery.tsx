import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { Header } from '@/components/Header';
import { GlassCard } from '@/components/ui/GlassCard';
import { Button } from '@/components/ui/button';
import { ChainSelector } from '@/components/ui/ChainSelector';
import { ChainIcon, getChainConfig } from '@/components/ui/ChainIcon';
import { Chain } from '@/stores/useStore';
import { discoveryApi } from '@/lib/api';
import { ArrowLeft, Info, Loader2 } from 'lucide-react';
import { useToast } from '@/hooks/use-toast';

interface TokenInfo {
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  logo_uri?: string;
}

interface DexInfo {
  name: string;
  chain: string;
  fee_tier: string;
  available: boolean;
}

interface ChainDiscoveryData {
  chain: string;
  dexes: DexInfo[];
  supported_tokens: TokenInfo[];
}

export default function TokenDiscovery() {
  const navigate = useNavigate();
  const { toast } = useToast();
  const [executionChain, setExecutionChain] = useState<Chain>('solana');
  const [chainData, setChainData] = useState<ChainDiscoveryData | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [tokenDexMap, setTokenDexMap] = useState<Record<string, string[]>>({});

  // Fetch chain discovery data (tokens + DEXes)
  useEffect(() => {
    const fetchChainData = async () => {
      setIsLoading(true);
      try {
        const chainName = executionChain.charAt(0).toUpperCase() + executionChain.slice(1);
        const response = await discoveryApi.getChainInfo(chainName);
        const data = response.data as ChainDiscoveryData;
        setChainData(data);

        // Build token-to-DEX mapping by checking each DEX
        const mapping: Record<string, string[]> = {};
        if (data.supported_tokens && data.dexes) {
          // For each token, check which DEXes support it
          // We'll fetch tokens per DEX and map them
          for (const dex of data.dexes.filter(d => d.available)) {
            try {
              const dexResponse = await discoveryApi.getDexTokens(dex.name, chainName);
              if (dexResponse.data?.assets) {
                dexResponse.data.assets.forEach((token: TokenInfo) => {
                  if (!mapping[token.symbol]) {
                    mapping[token.symbol] = [];
                  }
                  if (!mapping[token.symbol].includes(dex.name)) {
                    mapping[token.symbol].push(dex.name);
                  }
                });
              }
            } catch (error) {
              console.warn(`Failed to fetch tokens for DEX ${dex.name}:`, error);
            }
          }
        }
        setTokenDexMap(mapping);
      } catch (error) {
        console.error('Failed to fetch chain data:', error);
        toast({
          title: 'Failed to load tokens',
          description: 'Could not fetch token and DEX information',
          variant: 'destructive',
        });
      } finally {
        setIsLoading(false);
      }
    };

    fetchChainData();
  }, [executionChain]);

  const handleTokenClick = (token: TokenInfo) => {
    navigate(`/token-details/${executionChain}/${token.symbol}`, {
      state: { token, chainData },
    });
  };

  const chainConfig = getChainConfig(executionChain);

  return (
    <div className="min-h-screen">
      <Header />

      <main className="pt-24 pb-12 px-4">
        <div className="container mx-auto max-w-6xl">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="mb-8"
          >
            <Button
              variant="ghost"
              onClick={() => navigate('/action-selection')}
              className="mb-4"
            >
              <ArrowLeft className="h-4 w-4 mr-2" />
              Back to Actions
            </Button>

            <div className="flex items-center justify-between mb-6">
              <div>
                <h1 className="text-3xl font-bold mb-2">Trade Tokens</h1>
                <p className="text-muted-foreground">
                  Select a token to see details and trade
                </p>
              </div>
              <div className="flex items-center gap-4">
                <div className="flex items-center gap-2">
                  <span className="text-sm text-muted-foreground">Execution Chain:</span>
                  <ChainSelector
                    value={executionChain}
                    onChange={setExecutionChain}
                    className="w-40"
                  />
                </div>
              </div>
            </div>
          </motion.div>

          {isLoading ? (
            <GlassCard className="p-12 text-center">
              <Loader2 className="h-8 w-8 animate-spin mx-auto mb-4" />
              <p className="text-muted-foreground">Loading tokens...</p>
            </GlassCard>
          ) : chainData && chainData.supported_tokens.length > 0 ? (
            <div className="grid md:grid-cols-2 lg:grid-cols-3 gap-4">
              {chainData.supported_tokens.map((token, index) => {
                const dexes = tokenDexMap[token.symbol] || [];
                return (
                  <motion.div
                    key={token.address}
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ delay: index * 0.05 }}
                  >
                    <GlassCard
                      className="cursor-pointer hover:scale-105 transition-all p-6"
                      onClick={() => handleTokenClick(token)}
                    >
                      <div className="flex items-start justify-between mb-4">
                        <div className="flex items-center gap-3">
                          {token.logo_uri ? (
                            <img
                              src={token.logo_uri}
                              alt={token.symbol}
                              className="w-10 h-10 rounded-full"
                            />
                          ) : (
                            <div className="w-10 h-10 rounded-full bg-primary/20 flex items-center justify-center">
                              <span className="text-lg font-bold">{token.symbol[0]}</span>
                            </div>
                          )}
                          <div>
                            <h3 className="font-semibold text-lg">{token.symbol}</h3>
                            <p className="text-sm text-muted-foreground">{token.name}</p>
                          </div>
                        </div>
                        <ChainIcon chain={executionChain} size="sm" />
                      </div>

                      {dexes.length > 0 ? (
                        <div className="mt-4 pt-4 border-t border-border/50">
                          <p className="text-xs text-muted-foreground mb-2 flex items-center gap-1">
                            <Info className="h-3 w-3" />
                            Available on:
                          </p>
                          <div className="flex flex-wrap gap-2">
                            {dexes.map((dex) => (
                              <span
                                key={dex}
                                className="text-xs px-2 py-1 bg-primary/20 text-primary rounded-full"
                              >
                                {dex}
                              </span>
                            ))}
                          </div>
                        </div>
                      ) : (
                        <div className="mt-4 pt-4 border-t border-border/50">
                          <p className="text-xs text-muted-foreground">
                            DEX information loading...
                          </p>
                        </div>
                      )}

                      <Button
                        variant="outline"
                        className="w-full mt-4"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleTokenClick(token);
                        }}
                      >
                        View Details
                      </Button>
                    </GlassCard>
                  </motion.div>
                );
              })}
            </div>
          ) : (
            <GlassCard className="p-12 text-center">
              <p className="text-muted-foreground">No tokens available on {chainConfig.name}</p>
            </GlassCard>
          )}
        </div>
      </main>
    </div>
  );
}

