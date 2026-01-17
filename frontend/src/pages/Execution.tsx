import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { Header } from '@/components/Header';
import { GlassCard } from '@/components/ui/GlassCard';
import { Button } from '@/components/ui/button';
import { ChainIcon, getChainConfig } from '@/components/ui/ChainIcon';
import { useStore, Chain } from '@/stores/useStore';
import { settlementApi } from '@/lib/api';
import { ArrowRight, ExternalLink, CheckCircle2, XCircle, Loader2 } from 'lucide-react';
import type { SettlementStatus, SettlementRecord } from '@/types/settlement';

const steps = [
  { id: 'quote', label: 'Quote Created', status: 'pending' as const },
  { id: 'commit', label: 'Funds Locked', status: 'committed' as const },
  { id: 'executing', label: 'Executing', status: 'committed' as const },
  { id: 'complete', label: 'Complete', status: 'executed' as const },
];

export default function Execution() {
  const navigate = useNavigate();
  const { trade, resetTrade } = useStore();
  const quote = trade.currentQuote;

  const [status, setStatus] = useState<SettlementStatus | null>(null);
  const [isPolling, setIsPolling] = useState(true);

  useEffect(() => {
    if (!quote) {
      navigate('/trade');
      return;
    }

    const pollStatus = async () => {
      try {
        // IMPROVED: Use settlement endpoint instead of basic status
        // This gets full settlement records with transaction hashes and verification status
        const response = await settlementApi.getStatus(quote.quote_id);
        setStatus(response.data);

        // Stop polling when transaction is complete or failed
        if (
          response.data.status === 'executed' ||
          response.data.status === 'failed' ||
          response.data.status === 'expired' ||
          response.data.status === 'settled'
        ) {
          setIsPolling(false);
        }
      } catch (error) {
        console.error('Failed to fetch settlement status:', error);
        // Continue polling even on error
      }
    };

    pollStatus();
    const interval = isPolling ? setInterval(pollStatus, 3000) : undefined;

    return () => {
      if (interval) clearInterval(interval);
    };
  }, [quote, navigate, isPolling]);

  if (!quote) return null;

  const currentStatus = status?.status || 'pending';
  const fundingConfig = getChainConfig(quote.funding_chain as Chain);
  const executionConfig = getChainConfig(quote.execution_chain as Chain);

  const getStepState = (stepStatus: string) => {
    const statusOrder = ['pending', 'committed', 'executed'];
    const currentIndex = statusOrder.indexOf(currentStatus);
    const stepIndex = statusOrder.indexOf(stepStatus);

    if (currentStatus === 'failed' || currentStatus === 'expired') {
      return stepIndex <= currentIndex ? 'error' : 'pending';
    }
    if (stepIndex < currentIndex) return 'complete';
    if (stepIndex === currentIndex) return 'active';
    return 'pending';
  };

  const isSuccess = currentStatus === 'executed' || currentStatus === 'settled';
  const isFailed = currentStatus === 'failed' || currentStatus === 'expired';

  const handleNewTrade = () => {
    resetTrade();
    navigate('/trade');
  };

  return (
    <div className="min-h-screen">
      <Header />

      <main className="pt-24 pb-12 px-4">
        <div className="container mx-auto max-w-2xl">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="text-center mb-8"
          >
            <h1 className="text-3xl font-bold mb-2">
              {isSuccess ? 'Trade Complete!' : isFailed ? 'Trade Failed' : 'Executing Trade'}
            </h1>
            <p className="text-muted-foreground">
              {isSuccess
                ? 'Your cross-chain transaction was successful'
                : isFailed
                ? 'Something went wrong with your transaction'
                : 'Your transaction is being processed'}
            </p>
          </motion.div>

          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            {/* Progress Steps */}
            <div className="lg:col-span-2">
              <GlassCard>
                <h2 className="text-lg font-semibold mb-6">Transaction Progress</h2>
                <div className="space-y-6">
                  {steps.map((step, index) => {
                    const stepState = getStepState(step.status);
                    const isActive = stepState === 'active';
                    const isComplete = stepState === 'complete';
                    const isError = stepState === 'error';

                    return (
                      <div key={step.id}>
                        <div className="flex items-start gap-4">
                          <div className="flex flex-col items-center">
                            <div
                              className={`w-10 h-10 rounded-full flex items-center justify-center transition-all ${
                                isComplete
                                  ? 'bg-green-500/20 border-2 border-green-500'
                                  : isActive
                                  ? 'bg-primary/20 border-2 border-primary'
                                  : isError
                                  ? 'bg-destructive/20 border-2 border-destructive'
                                  : 'bg-secondary/50 border-2 border-border'
                              }`}
                            >
                              {isComplete ? (
                                <CheckCircle2 className="w-5 h-5 text-green-500" />
                              ) : isError ? (
                                <XCircle className="w-5 h-5 text-destructive" />
                              ) : isActive ? (
                                <Loader2 className="w-5 h-5 text-primary animate-spin" />
                              ) : (
                                <span className="text-sm font-medium">{index + 1}</span>
                              )}
                            </div>
                            {index < steps.length - 1 && (
                              <div
                                className={`w-0.5 h-12 mt-2 ${
                                  isComplete ? 'bg-green-500' : isActive ? 'bg-primary' : 'bg-border'
                                }`}
                              />
                            )}
                          </div>
                          <div className="flex-1 pt-1">
                            <p className={`font-semibold ${isActive ? 'text-primary' : isComplete ? 'text-green-500' : 'text-foreground'}`}>
                              {step.label}
                            </p>
                            <p className="text-sm text-muted-foreground mt-1">
                              {isActive
                                ? 'Processing...'
                                : isComplete
                                ? 'Completed'
                                : isError
                                ? 'Error'
                                : 'Pending'}
                            </p>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </GlassCard>

              {/* Settlement Records - NEW */}
              {status && status.settlement_records && status.settlement_records.length > 0 && (
                <GlassCard className="mt-6">
                  <h2 className="text-lg font-semibold mb-4">Settlement Records</h2>
                  <div className="space-y-4">
                    {status.settlement_records.map((record: SettlementRecord) => (
                      <motion.div
                        key={record.settlement_id}
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        className="p-4 bg-secondary/30 rounded-lg border border-border/50"
                      >
                        <div className="flex items-start justify-between">
                          <div className="flex-1">
                            <div className="flex items-center gap-2 mb-2">
                              <ChainIcon chain={record.chain as Chain} size="sm" />
                              <span className="font-medium capitalize">{record.chain} Settlement</span>
                              {record.verified_at && (
                                <CheckCircle2 className="w-4 h-4 text-green-500 ml-auto" />
                              )}
                            </div>
                            <p className="text-xs text-muted-foreground font-mono break-all mb-2">
                              {record.transaction_hash}
                            </p>
                            <div className="flex gap-4 text-sm">
                              <div>
                                <span className="text-muted-foreground">Amount: </span>
                                <span className="font-medium">{record.amount}</span>
                              </div>
                              {record.verified_at && (
                                <div>
                                  <span className="text-muted-foreground">Verified: </span>
                                  <span className="font-medium text-green-500">On-chain</span>
                                </div>
                              )}
                            </div>
                          </div>
                          <a
                            href={`https://explorer.${record.chain}.com/tx/${record.transaction_hash}`}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="ml-2"
                          >
                            <ExternalLink className="w-4 h-4 text-primary hover:text-primary/80" />
                          </a>
                        </div>
                      </motion.div>
                    ))}
                  </div>
                </GlassCard>
              )}
            </div>

            {/* Summary Card */}
            <div>
              <GlassCard>
                <h2 className="text-lg font-semibold mb-6">Summary</h2>
                <div className="space-y-6">
                  {/* Chain Flow */}
                  <div>
                    <p className="text-xs text-muted-foreground mb-3 uppercase tracking-wide">Routes</p>
                    <div className="flex items-center justify-center gap-3">
                      <div className="flex flex-col items-center gap-1">
                        <ChainIcon chain={quote.funding_chain as Chain} size="md" />
                        <span className="text-xs font-medium">{fundingConfig.name}</span>
                      </div>
                      <ArrowRight className="h-5 w-5 text-muted-foreground" />
                      <div className="flex flex-col items-center gap-1">
                        <ChainIcon chain={quote.execution_chain as Chain} size="md" />
                        <span className="text-xs font-medium">{executionConfig.name}</span>
                      </div>
                    </div>
                  </div>

                  {/* Amounts */}
                  <div className="space-y-3">
                    <p className="text-xs text-muted-foreground mb-3 uppercase tracking-wide">Amounts</p>
                    <div className="flex justify-between text-sm">
                      <span className="text-muted-foreground">Funding</span>
                      <span className="font-medium">
                        {quote.funding_amount} {quote.funding_asset}
                      </span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span className="text-muted-foreground">Execution</span>
                      <span className="font-medium">
                        {quote.execution_amount} {quote.execution_asset}
                      </span>
                    </div>
                  </div>

                  {/* Status */}
                  <div className="pt-3 border-t border-border/50">
                    <p className="text-xs text-muted-foreground mb-2 uppercase tracking-wide">Status</p>
                    <div className="inline-block px-3 py-1 rounded-full bg-primary/10 text-primary text-xs font-medium capitalize">
                      {currentStatus}
                    </div>
                  </div>

                  {/* Action */}
                  {(isSuccess || isFailed) && (
                    <Button
                      variant="gradient"
                      className="w-full mt-6"
                      onClick={handleNewTrade}
                    >
                      New Trade
                    </Button>
                  )}
                </div>
              </GlassCard>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
