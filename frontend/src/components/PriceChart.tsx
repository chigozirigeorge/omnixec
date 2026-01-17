// components/PriceChart.tsx
import { useEffect, useRef, useState } from 'react';
import { createChart, IChartApi, ISeriesApi, LineData, UTCTimestamp } from 'lightweight-charts';
import { chartApi } from '@/lib/api';
import { Skeleton } from './ui/skeleton';
import { cn } from '@/lib/utils';

interface PriceChartProps {
  asset: string;
  chain: string;
  timeframe?: '1h' | '4h' | '1d' | '1w' | '1m';
  height?: number;
  className?: string;
}

export function PriceChart({ 
  asset, 
  chain, 
  timeframe = '1d', 
  height = 300, 
  className 
}: PriceChartProps) {
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chart = useRef<IChartApi | null>(null);
  const lineSeries = useRef<ISeriesApi<'Line'> | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!chartContainerRef.current) return;

    // Initialize chart
    chart.current = createChart(chartContainerRef.current, {
      height,
      layout: {
        backgroundColor: 'transparent',
        textColor: 'hsl(var(--muted-foreground))',
      },
      grid: {
        vertLines: { color: 'hsl(var(--border))' },
        horzLines: { color: 'hsl(var(--border))' },
      },
      timeScale: {
        timeVisible: true,
        secondsVisible: false,
        borderColor: 'hsl(var(--border))',
      },
      rightPriceScale: {
        borderColor: 'hsl(var(--border))',
      },
      crosshair: {
        vertLine: {
          color: 'hsl(var(--primary))',
          width: 1,
          style: 0, // 0 = solid
        },
        horzLine: {
          color: 'hsl(var(--primary))',
          width: 1,
          style: 0, // 0 = solid
        },
      },
    });

    // Add line series
    lineSeries.current = chart.current.addLineSeries({
      color: 'hsl(var(--primary))',
      lineWidth: 2,
      priceLineVisible: false,
      lastValueVisible: false,
    });

    // Handle resize
    const handleResize = () => {
      if (chart.current) {
        chart.current.applyOptions({ 
          width: chartContainerRef.current?.clientWidth 
        });
      }
    };

    window.addEventListener('resize', handleResize);

    // Initial fetch
    fetchChartData();

    return () => {
      window.removeEventListener('resize', handleResize);
      if (chart.current) {
        chart.current.remove();
        chart.current = null;
      }
    };
  }, [asset, chain, timeframe]);

  const fetchChartData = async () => {
    try {
      setIsLoading(true);
      setError(null);
      
      // Fetch OHLC data from the backend
      const response = await chartApi.getOHLC(asset, chain, timeframe);
      const ohlcData = response.data;

      // Convert to LineData format
      const lineData: LineData[] = ohlcData.map((d: any) => ({
        time: (new Date(d.time).getTime() / 1000) as UTCTimestamp,
        value: d.close,
      }));

      if (lineSeries.current) {
        lineSeries.current.setData(lineData);
        
        // Fit the chart to the data
        if (chart.current && lineData.length > 0) {
          chart.current.timeScale().fitContent();
        }
      }
    } catch (err) {
      console.error('Error fetching chart data:', err);
      setError('Failed to load price data');
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading) {
    return (
      <div className={cn("relative", className)}>
        <Skeleton className="w-full" style={{ height: `${height}px` }} />
      </div>
    );
  }

  if (error) {
    return (
      <div className={cn("flex items-center justify-center text-destructive", className)} style={{ height: `${height}px` }}>
        {error}
      </div>
    );
  }

  return <div ref={chartContainerRef} className={className} />;
}