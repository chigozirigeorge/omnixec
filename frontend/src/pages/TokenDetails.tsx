import { useEffect, useState } from 'react';
import { useNavigate, useParams, useLocation } from 'react-router-dom';
import { motion } from 'framer-motion';
import { Header } from '@/components/Header';
import { GlassCard } from '@/components/ui/GlassCard';
import { Button } from '@/components/ui/button';
import { ChainIcon, getChainConfig } from '@/components/ui/ChainIcon';
import { Chain } from '@/stores/useStore';
import { chartApi, discoveryApi } from '@/lib/api';
import { ArrowLeft, TrendingUp, TrendingDown, Loader2 } from 'lucide-react';
import { useToast } from '@/hooks/use-toast';
import { createChart, IChartApi, ISeriesApi, CandlestickSeries } from 'lightweight-charts';
import { useRef } from 'react';

interface TokenInfo {
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  logo_uri?: string;
}

export default function TokenDetails() {
  const navigate = useNavigate();
  const { chain, symbol } = useParams<{ chain: Chain; symbol: string }>();
  const location = useLocation();
  const { toast } = useToast();
  
  const [token, setToken] = useState<TokenInfo | null>(
    location.state?.token || null
  );
  const [isLoading, setIsLoading] = useState(false);
  const [chartData, setChartData] = useState<any[]>([]);
  const [timeframe, setTimeframe] = useState('1h');
  const [currentPrice, setCurrentPrice] = useState<number | null>(null);
  const [priceChange24h, setPriceChange24h] = useState<number | null>(null);
  const [volume24h, setVolume24h] = useState<number | null>(null);
  const [availableDexes, setAvailableDexes] = useState<string[]>([]);
  
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);

  useEffect(() => {
    if (!chain || !symbol) {
      navigate('/token-discovery');
      return;
    }

    // Fetch token details if not in state
    const fetchTokenDetails = async () => {
      if (token) return;
      
      setIsLoading(true);
      try {
        const chainName = chain.charAt(0).toUpperCase() + chain.slice(1);
        const response = await discoveryApi.getChainInfo(chainName);
        const foundToken = response.data?.supported_tokens?.find(
          (t: TokenInfo) => t.symbol === symbol
        );
        if (foundToken) {
          setToken(foundToken);
        } else {
          toast({
            title: 'Token not found',
            description: `Token ${symbol} not found on ${chainName}`,
            variant: 'destructive',
          });
          navigate('/token-discovery');
        }
      } catch (error) {
        console.error('Failed to fetch token details:', error);
        toast({
          title: 'Error',
          description: 'Failed to load token details',
          variant: 'destructive',
        });
      } finally {
        setIsLoading(false);
      }
    };

    fetchTokenDetails();
  }, [chain, symbol, token]);

  // Fetch DEXes where this token is available
  useEffect(() => {
    const fetchDexes = async () => {
      if (!chain || !token) return;
      
      try {
        const chainName = chain.charAt(0).toUpperCase() + chain.slice(1);
        const response = await discoveryApi.getChainInfo(chainName);
        const dexes: string[] = [];
        
        if (response.data?.dexes) {
          // Check each DEX to see if it supports this token
          for (const dex of response.data.dexes.filter((d: any) => d.available)) {
            try {
              const dexResponse = await discoveryApi.getDexTokens(dex.name, chainName);
              if (dexResponse.data?.assets) {
                const hasToken = dexResponse.data.assets.some(
                  (t: TokenInfo) => t.symbol === token.symbol || t.address === token.address
                );
                if (hasToken) {
                  dexes.push(dex.name);
                }
              }
            } catch (error) {
              console.warn(`Failed to check DEX ${dex.name}:`, error);
            }
          }
        }
        
        setAvailableDexes(dexes);
      } catch (error) {
        console.error('Failed to fetch DEX information:', error);
      }
    };

    if (token) {
      fetchDexes();
    }
  }, [chain, token]);

  // Fetch and render chart
  useEffect(() => {
    if (!token || !chain) return;

    const fetchChartData = async () => {
      try {
        const response = await chartApi.getOHLC(token.symbol, chain, timeframe, 100);
        if (response.data) {
          setChartData(response.data);
          
          // Initialize chart if container exists
          if (chartContainerRef.current && !chartRef.current) {
            const chart = createChart(chartContainerRef.current, {
              layout: {
                background: { color: 'transparent' },
                textColor: '#7A7F8C',
              },
              grid: {
                vertLines: { color: 'rgba(255, 255, 255, 0.05)' },
                horzLines: { color: 'rgba(255, 255, 255, 0.05)' },
              },
              width: chartContainerRef.current.clientWidth,
              height: 400,
            });

            const candlestickSeries = chart.addSeries(CandlestickSeries, {
              upColor: '#2EE59D',
              downColor: '#FF6B6B',
              borderUpColor: '#2EE59D',
              borderDownColor: '#FF6B6B',
              wickUpColor: '#2EE59D',
              wickDownColor: '#FF6B6B',
            });

            chartRef.current = chart;
            seriesRef.current = candlestickSeries;

            // Handle resize
            const handleResize = () => {
              if (chartContainerRef.current && chartRef.current) {
                chartRef.current.applyOptions({
                  width: chartContainerRef.current.clientWidth,
                });
              }
            };

            window.addEventListener('resize', handleResize);
            
            return () => {
              window.removeEventListener('resize', handleResize);
              chart.remove();
            };
          }

          if (seriesRef.current && response.data.length > 0) {
            // Map timestamp to time format expected by lightweight-charts
            const formattedData = response.data.map((d: any) => ({
              time: d.timestamp || d.time,
              open: d.open,
              high: d.high,
              low: d.low,
              close: d.close,
            }));
            seriesRef.current.setData(formattedData as any);
            chartRef.current?.timeScale().fitContent();

            // Calculate price stats from chart data
            if (formattedData.length > 0) {
              const latest = formattedData[formattedData.length - 1];
              const oldest = formattedData[0];
              setCurrentPrice(latest.close);
              
              if (formattedData.length > 1) {
                const priceChange = ((latest.close - oldest.open) / oldest.open) * 100;
                setPriceChange24h(priceChange);
              }

              // Calculate 24h volume if available
              const volumes = response.data.map((d: any) => d.volume || 0);
              const totalVolume = volumes.reduce((sum: number, vol: number) => sum + vol, 0);
              setVolume24h(totalVolume);
            }
          }
        }
      } catch (error) {
        console.error('Failed to fetch chart data:', error);
      }
    };

    fetchChartData();

    // Cleanup chart on unmount
    return () => {
      if (chartRef.current) {
        chartRef.current.remove();
        chartRef.current = null;
        seriesRef.current = null;
      }
    };
  }, [token, chain, timeframe]);

  const handleTrade = () => {
    if (!token || !chain) return;
    // Navigate to trade page with token pre-selected
    navigate('/trade', {
      state: {
        executionChain: chain,
        executionAsset: token.symbol,
        token,
      },
    });
  };

  if (isLoading || !token) {
    return (
      <div className="min-h-screen">
        <Header />
        <main className="pt-24 pb-12 px-4">
          <div className="container mx-auto max-w-4xl">
            <GlassCard className="p-12 text-center">
              <Loader2 className="h-8 w-8 animate-spin mx-auto mb-4" />
              <p className="text-muted-foreground">Loading token details...</p>
            </GlassCard>
          </div>
        </main>
      </div>
    );
  }

  const chainConfig = getChainConfig(chain as Chain);

  return (
    <div className="min-h-screen">
      <Header />

      <main className="pt-24 pb-12 px-4">
        <div className="container mx-auto max-w-4xl">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
          >
            <Button
              variant="ghost"
              onClick={() => navigate('/token-discovery')}
              className="mb-6"
            >
              <ArrowLeft className="h-4 w-4 mr-2" />
              Back to Tokens
            </Button>

            {/* Token Header */}
            <GlassCard className="p-6 mb-6">
              <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
                <div className="flex items-center gap-4">
                  {token.logo_uri ? (
                    <img
                      src={token.logo_uri}
                      alt={token.symbol}
                      className="w-16 h-16 rounded-full"
                    />
                  ) : (
                    <div className="w-16 h-16 rounded-full bg-primary/20 flex items-center justify-center">
                      <span className="text-2xl font-bold">{token.symbol[0]}</span>
                    </div>
                  )}
                  <div>
                    <h1 className="text-3xl font-bold">{token.symbol}</h1>
                    <p className="text-muted-foreground">{token.name}</p>
                    <div className="flex items-center gap-2 mt-2">
                      <ChainIcon chain={chain as Chain} size="sm" />
                      <span className="text-sm text-muted-foreground">
                        {chainConfig.name}
                      </span>
                    </div>
                  </div>
                </div>
                
                {/* Price Stats */}
                <div className="flex flex-col items-end gap-2">
                  {currentPrice !== null && (
                    <div className="text-right">
                      <p className="text-3xl font-bold">${currentPrice.toFixed(4)}</p>
                      {priceChange24h !== null && (
                        <div className={`flex items-center gap-1 text-sm ${
                          priceChange24h >= 0 ? 'text-green-400' : 'text-red-400'
                        }`}>
                          {priceChange24h >= 0 ? (
                            <TrendingUp className="h-4 w-4" />
                          ) : (
                            <TrendingDown className="h-4 w-4" />
                          )}
                          <span>{Math.abs(priceChange24h).toFixed(2)}%</span>
                        </div>
                      )}
                    </div>
                  )}
                  <Button variant="gradient" size="lg" onClick={handleTrade}>
                    Trade {token.symbol}
                  </Button>
                </div>
              </div>
            </GlassCard>

            {/* Chart */}
            <GlassCard className="p-6 mb-6">
              <div className="flex items-center justify-between mb-4">
                <h2 className="text-xl font-semibold">Price Chart</h2>
                <div className="flex gap-2">
                  {['1h', '4h', '1d', '1w'].map((tf) => (
                    <Button
                      key={tf}
                      variant={timeframe === tf ? 'secondary' : 'ghost'}
                      size="sm"
                      onClick={() => setTimeframe(tf)}
                    >
                      {tf}
                    </Button>
                  ))}
                </div>
              </div>
              <div ref={chartContainerRef} className="w-full h-[400px]" />
            </GlassCard>

            {/* Token Info */}
            <GlassCard className="p-6">
              <h2 className="text-xl font-semibold mb-4">Token Information</h2>
              <div className="grid md:grid-cols-2 gap-6">
                <div className="space-y-4">
                  <div>
                    <p className="text-sm text-muted-foreground mb-1">Symbol</p>
                    <p className="font-semibold">{token.symbol}</p>
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground mb-1">Name</p>
                    <p className="font-semibold">{token.name}</p>
                  </div>
                  <div>
                    <p className="text-sm text-muted-foreground mb-1">Decimals</p>
                    <p className="font-semibold">{token.decimals}</p>
                  </div>
                  {volume24h !== null && (
                    <div>
                      <p className="text-sm text-muted-foreground mb-1">24h Volume</p>
                      <p className="font-semibold">
                        ${volume24h.toLocaleString(undefined, { maximumFractionDigits: 2 })}
                      </p>
                    </div>
                  )}
                </div>
                <div className="space-y-4">
                  <div>
                    <p className="text-sm text-muted-foreground mb-1">Contract Address</p>
                    <p className="font-mono text-sm break-all bg-secondary/30 p-2 rounded">
                      {token.address}
                    </p>
                  </div>
                  {availableDexes.length > 0 && (
                    <div>
                      <p className="text-sm text-muted-foreground mb-2">Available on DEXes</p>
                      <div className="flex flex-wrap gap-2">
                        {availableDexes.map((dex) => (
                          <span
                            key={dex}
                            className="px-3 py-1 bg-primary/20 text-primary rounded-full text-sm font-medium"
                          >
                            {dex}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </GlassCard>
          </motion.div>
        </div>
      </main>
    </div>
  );
}

