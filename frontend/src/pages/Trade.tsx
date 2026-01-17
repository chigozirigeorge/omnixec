import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { Header } from '@/components/Header';
import { GlassCard } from '@/components/ui/GlassCard';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ChainSelector } from '@/components/ui/ChainSelector';
import { ChainIcon, getChainConfig } from '@/components/ui/ChainIcon';
import { useStore } from '@/stores/useStore';
import { quoteApi, treasuryApi } from '@/lib/api';
import { useQuote } from '@/lib/aggregators/quotes';
import { ArrowDownUp, Zap, Info, AlertTriangle, Loader2 } from 'lucide-react';
import { useToast } from '@/hooks/use-toast';
import { TransactionStatus } from '@/components/TransactionStatus';
import { TransactionHistory } from '@/components/TransactionHistory';
import { PriceChart } from '@/components/PriceChart';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

const assetsByChain = {
  solana: ['SOL', 'USDC', 'USDT'],
  stellar: ['XLM', 'USDC', 'yUSDC'],
  near: ['NEAR', 'USDC', 'USDT'],
};

export default function Trade() {
  const navigate = useNavigate();
  const { toast } = useToast();
  const [isLoading, setIsLoading] = useState(false);
  const [treasuryError, setTreasuryError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState('trade');
  
  const {
    user,
    trade,
    setFundingChain,
    setExecutionChain,
    setFundingAsset,
    setExecutionAsset,
    setAmount,
    setCurrentQuote,
  } = useStore();

  const handleSwapChains = () => {
    const tempChain = trade.fundingChain;
    const tempAsset = trade.fundingAsset;
    setFundingChain(trade.executionChain);
    setExecutionChain(tempChain);
    setFundingAsset(trade.executionAsset);
    setExecutionAsset(tempAsset);
  };

  const { quote, isLoading: isQuoteLoading, error: quoteError, fetchQuote } = useQuote();
  const [isTreasuryCheckLoading, setIsTreasuryCheckLoading] = useState(false);

  // Handle quote changes
  useEffect(() => {
    if (quote) {
      // When we get a new quote, update the store and navigate to quote page
      setCurrentQuote({
        quote_id: crypto.randomUUID(),
        user_id: user.userId,
        funding_chain: trade.fundingChain,
        execution_chain: trade.executionChain,
        funding_asset: trade.fundingAsset,
        execution_asset: trade.executionAsset,
        funding_amount: trade.amount,
        execution_amount: quote.outAmount, // This is the estimated out amount
        execution_cost: '0', // Not provided by the quote
        service_fee: String(Number(quote.inAmount) * (quote.feePct ? quote.feePct / 100 : 0.001)),
        max_funding_amount: trade.amount, // Same as funding_amount for now
        status: 'Pending',
        expires_at: Date.now() + 5 * 60 * 1000, // 5 minutes from now
        created_at: Date.now(),
      });
      navigate('/quote');
    }
  }, [quote]);

  // Handle quote errors
  useEffect(() => {
    if (quoteError) {
      toast({
        title: 'Failed to get quote',
        description: quoteError.message,
        variant: 'destructive',
      });
    }
  }, [quoteError]);

  const handleGetQuote = async () => {
    if (!trade.amount || parseFloat(trade.amount) <= 0) {
      toast({
        title: 'Invalid Amount',
        description: 'Please enter a valid amount to trade.',
        variant: 'destructive',
      });
      return;
    }

    setIsTreasuryCheckLoading(true);
    setTreasuryError(null);
    
    try {
      // CRITICAL: Check treasury status and circuit breaker before creating quote
      try {
        const treasuryResponse = await treasuryApi.getChainStatus(trade.fundingChain);
        const treasuryStatus = treasuryResponse.data;
        
        if (treasuryStatus.circuit_breaker?.active) {
          const errorMsg = treasuryStatus.circuit_breaker.reason || 'Circuit breaker is active';
          setTreasuryError(errorMsg);
          toast({
            title: 'System Unavailable',
            description: `Circuit breaker: ${errorMsg}`,
            variant: 'destructive',
          });
          return;
        }

        // Check if daily limit would be exceeded
        const quoteAmount = parseFloat(trade.amount);
        const dailyRemaining = parseFloat(treasuryStatus.daily_remaining || '0');
        if (quoteAmount > dailyRemaining) {
          const errorMsg = `Daily limit exceeded. Remaining: ${treasuryStatus.daily_remaining} ${treasuryStatus.asset}`;
          setTreasuryError(errorMsg);
          toast({
            title: 'Amount Exceeds Daily Limit',
            description: errorMsg,
            variant: 'destructive',
          });
          return;
        }
      } catch (treasuryCheckError: any) {
        // Log but don't fail - treasury check is advisory
        console.warn('Treasury check failed:', treasuryCheckError);
      } finally {
        setIsTreasuryCheckLoading(false);
      }

      // Fetch the quote using our new hook
      await fetchQuote(
        trade.fundingChain,
        trade.fundingAsset,
        trade.executionAsset,
        trade.amount
      );
      
      // The quote will be handled by the useEffect above
    } catch (error) {
      console.error('Error in handleGetQuote:', error);
      // Error is already handled by the quoteError useEffect
    }
  };

  const fundingConfig = getChainConfig(trade.fundingChain);
  const executionConfig = getChainConfig(trade.executionChain);

  return (
    <div className="min-h-screen">
      <Header />

      <main className="pt-24 pb-12 px-4">
        <div className="container mx-auto max-w-xl">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="text-center mb-8"
          >
            <h1 className="text-3xl font-bold mb-2">Cross-Chain Trade</h1>
            <p className="text-muted-foreground">
              Pay on one chain, execute on another
            </p>
          </motion.div>

          <GlassCard glow="primary">
            {/* Funding Section */}
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-muted-foreground">From</span>
                <span className="text-xs text-muted-foreground">Balance: 0.00</span>
              </div>

              <div className="flex gap-4">
                <ChainSelector
                  value={trade.fundingChain}
                  onChange={setFundingChain}
                  disabledChain={trade.executionChain}
                  className="flex-1"
                />
                <select
                  value={trade.fundingAsset}
                  onChange={(e) => setFundingAsset(e.target.value)}
                  className="flex-1 bg-secondary/50 border border-border/50 rounded-xl px-4 py-2 text-foreground focus:outline-none focus:border-primary/50"
                >
                  {assetsByChain[trade.fundingChain].map((asset) => (
                    <option key={asset} value={asset}>{asset}</option>
                  ))}
                </select>
              </div>

              <div className="relative">
                <Input
                  type="number"
                  placeholder="0.00"
                  value={trade.amount}
                  onChange={(e) => setAmount(e.target.value)}
                  className="text-2xl h-14 bg-secondary/30 border-border/50 focus:border-primary/50"
                />
                <Button
                  variant="ghost"
                  size="sm"
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-primary"
                >
                  MAX
                </Button>
              </div>
            </div>

            {/* Swap Button */}
            <div className="flex justify-center -my-2 relative z-10">
              <Button
                variant="secondary"
                size="icon"
                className="rounded-full border-4 border-background"
                onClick={handleSwapChains}
              >
                <ArrowDownUp className="h-4 w-4" />
              </Button>
            </div>

            {/* Execution Section */}
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-muted-foreground">To</span>
              </div>

              <div className="flex gap-4">
                <ChainSelector
                  value={trade.executionChain}
                  onChange={setExecutionChain}
                  disabledChain={trade.fundingChain}
                  className="flex-1"
                />
                <select
                  value={trade.executionAsset}
                  onChange={(e) => setExecutionAsset(e.target.value)}
                  className="flex-1 bg-secondary/50 border border-border/50 rounded-xl px-4 py-2 text-foreground focus:outline-none focus:border-primary/50"
                >
                  {assetsByChain[trade.executionChain].map((asset) => (
                    <option key={asset} value={asset}>{asset}</option>
                  ))}
                </select>
              </div>

              <div className="bg-secondary/30 border border-border/50 rounded-xl p-4">
                <p className="text-2xl font-semibold text-muted-foreground">~0.00</p>
                <p className="text-xs text-muted-foreground mt-1">Estimated output</p>
              </div>
            </div>

            {/* Info Row */}
            <div className="flex items-center gap-2 text-sm text-muted-foreground bg-secondary/20 rounded-lg p-3 mt-4">
              <Info className="h-4 w-4 shrink-0" />
              <p>
                Trading {trade.fundingAsset} on {fundingConfig.name} → {trade.executionAsset} on {executionConfig.name}
              </p>
            </div>

            {/* Treasury Error Alert */}
            {treasuryError && (
              <div className="flex items-start gap-3 bg-destructive/10 border border-destructive/30 rounded-lg p-4 mt-4">
                <AlertTriangle className="h-5 w-5 text-destructive shrink-0 mt-0.5" />
                <div>
                  <p className="font-medium text-destructive text-sm">{treasuryError}</p>
                  <p className="text-xs text-destructive/70 mt-1">Please try again later or use a smaller amount</p>
                </div>
              </div>
            )}

            {/* CTA */}
            <Button
              variant="gradient"
              size="lg"
              className="w-full mt-6"
              onClick={handleGetQuote}
              disabled={
                !trade.amount || 
                parseFloat(trade.amount) <= 0 || 
                !!treasuryError || 
                isQuoteLoading || 
                isTreasuryCheckLoading
              }
            >
              {isQuoteLoading || isTreasuryCheckLoading ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin mr-2" />
                  {isTreasuryCheckLoading ? 'Checking Limits...' : 'Fetching Quote...'}
                </>
              ) : (
                <>
                  <Zap className="h-4 w-4 mr-2" />
                  Get Quote
                </>
              )}
            </Button>
          </GlassCard>
        </div>
      </main>
    </div>
  );
}
