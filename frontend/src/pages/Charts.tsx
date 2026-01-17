import { useEffect, useRef, useState } from 'react';
import { motion } from 'framer-motion';
import { Header } from '@/components/Header';
import { GlassCard } from '@/components/ui/GlassCard';
import { Button } from '@/components/ui/button';
import { ChainSelector } from '@/components/ui/ChainSelector';
import { Chain } from '@/stores/useStore';
import { chartApi, OHLCData, discoveryApi } from '@/lib/api';
import { createChart, IChartApi, ISeriesApi, CandlestickSeries } from 'lightweight-charts';
import { RefreshCw } from 'lucide-react';

const timeframes = ['1m', '5m', '15m', '1h', '4h', '1d'];

interface TokenInfo {
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  logo_uri?: string;
}


export default function Charts() {
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);

  const [chain, setChain] = useState<Chain>('solana');
  const [asset, setAsset] = useState('SOL');
  const [timeframe, setTimeframe] = useState('15m');
  const [isLoading, setIsLoading] = useState(false);
  const [availableTokens, setAvailableTokens] = useState<TokenInfo[]>([]);
  const [isLoadingTokens, setIsLoadingTokens] = useState(false);

  useEffect(() => {
    if (!chartContainerRef.current) return;

    // Create chart
    const chart = createChart(chartContainerRef.current, {
      layout: {
        background: { color: 'transparent' },
        textColor: '#7A7F8C',
      },
      grid: {
        vertLines: { color: 'rgba(255, 255, 255, 0.05)' },
        horzLines: { color: 'rgba(255, 255, 255, 0.05)' },
      },
      crosshair: {
        vertLine: { color: '#6C5CE7', width: 1, style: 2 },
        horzLine: { color: '#6C5CE7', width: 1, style: 2 },
      },
      rightPriceScale: {
        borderColor: 'rgba(255, 255, 255, 0.1)',
      },
      timeScale: {
        borderColor: 'rgba(255, 255, 255, 0.1)',
        timeVisible: true,
      },
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
      if (chartContainerRef.current) {
        chart.applyOptions({
          width: chartContainerRef.current.clientWidth,
          height: chartContainerRef.current.clientHeight,
        });
      }
    };

    window.addEventListener('resize', handleResize);
    handleResize();

    return () => {
      window.removeEventListener('resize', handleResize);
      chart.remove();
    };
  }, []);

  const fetchData = async () => {
    if (!asset) return;
    setIsLoading(true);
    try {
      // Fetch chart data strictly from backend API
      const response = await chartApi.getOHLC(asset, chain, timeframe);
      if (seriesRef.current && response.data) {
        seriesRef.current.setData(response.data as any);
        chartRef.current?.timeScale().fitContent();
      }
    } catch (error) {
      console.error('Failed to fetch chart data:', error);
      // No fallback - show error state or empty chart
    } finally {
      setIsLoading(false);
    }
  };

  // Fetch available tokens for selected chain from backend
  useEffect(() => {
    const fetchTokens = async () => {
      setIsLoadingTokens(true);
      try {
        const chainName = chain.charAt(0).toUpperCase() + chain.slice(1);
        const response = await discoveryApi.getChainInfo(chainName);
        if (response.data?.supported_tokens) {
          setAvailableTokens(response.data.supported_tokens);
          // Auto-select first token if current selection is not in the list
          if (!response.data.supported_tokens.find(t => t.symbol === asset)) {
            setAsset(response.data.supported_tokens[0]?.symbol || '');
          }
        }
      } catch (error) {
        console.error('Failed to fetch tokens for chain:', error);
        setAvailableTokens([]);
      } finally {
        setIsLoadingTokens(false);
      }
    };
    fetchTokens();
  }, [chain]);

  useEffect(() => {
    fetchData();
  }, [chain, asset, timeframe]);

  return (
    <div className="min-h-screen">
      <Header />

      <main className="pt-24 pb-12 px-4">
        <div className="container mx-auto max-w-6xl">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="flex flex-col md:flex-row md:items-center justify-between gap-4 mb-6"
          >
            <div>
              <h1 className="text-3xl font-bold">Price Charts</h1>
              <p className="text-muted-foreground">Real-time cross-chain asset prices</p>
            </div>

            <div className="flex flex-wrap items-center gap-3">
              <ChainSelector
                value={chain}
                onChange={setChain}
                className="w-40"
              />
              <select
                value={asset}
                onChange={(e) => setAsset(e.target.value)}
                disabled={isLoadingTokens}
                className="bg-secondary/50 border border-border/50 rounded-xl px-4 py-2 text-foreground focus:outline-none focus:border-primary/50 disabled:opacity-50"
              >
                {isLoadingTokens ? (
                  <option>Loading tokens...</option>
                ) : availableTokens.length === 0 ? (
                  <option>No tokens available</option>
                ) : (
                  availableTokens.map((token) => (
                    <option key={token.address} value={token.symbol}>
                      {token.symbol} - {token.name}
                    </option>
                  ))
                )}
              </select>
              <Button
                variant="ghost"
                size="icon"
                onClick={fetchData}
                disabled={isLoading}
              >
                <RefreshCw className={`h-4 w-4 ${isLoading ? 'animate-spin' : ''}`} />
              </Button>
            </div>
          </motion.div>

          <GlassCard className="p-0 overflow-hidden">
            {/* Timeframe Selector */}
            <div className="flex items-center gap-1 p-4 border-b border-border/50">
              {timeframes.map((tf) => (
                <Button
                  key={tf}
                  variant={timeframe === tf ? 'secondary' : 'ghost'}
                  size="sm"
                  onClick={() => setTimeframe(tf)}
                  className="text-xs"
                >
                  {tf}
                </Button>
              ))}
            </div>

            {/* Chart */}
            <div
              ref={chartContainerRef}
              className="w-full h-[500px]"
            />
          </GlassCard>
        </div>
      </main>
    </div>
  );
}
