import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { Header } from '@/components/Header';
import { GlassCard } from '@/components/ui/GlassCard';
import { Button } from '@/components/ui/button';
import { ChainIcon, getChainConfig } from '@/components/ui/ChainIcon';
import { StatusBadge } from '@/components/ui/StatusBadge';
import { useStore, Chain } from '@/stores/useStore';
import { quoteApi, approvalApi } from '@/lib/api';
import { ArrowRight, Clock, AlertTriangle, Check, Loader2, Shield } from 'lucide-react';
import { useToast } from '@/hooks/use-toast';
import { useWalletSign } from '@/hooks/useWalletSign';

export default function QuoteReview() {
  const navigate = useNavigate();
  const { toast } = useToast();
  const { sign } = useWalletSign();
  const { user, wallet, trade, setQuoteStatus, setCurrentApproval, setApprovalStatus } = useStore();
  const quote = trade.currentQuote;

  const [timeRemaining, setTimeRemaining] = useState(0);
  const [stage, setStage] = useState<'review' | 'approval' | 'committing'>('review');
  const [approvalId, setApprovalId] = useState<string | null>(null);
  const [isProcessing, setIsProcessing] = useState(false);

  useEffect(() => {
    if (!quote) {
      navigate('/trade');
      return;
    }

    // Calculate time remaining
    const updateTimer = () => {
      const now = Math.floor(Date.now() / 1000);
      const remaining = quote.expires_at - now;
      setTimeRemaining(Math.max(0, remaining));
    };

    updateTimer();
    const interval = setInterval(updateTimer, 1000);

    return () => clearInterval(interval);
  }, [quote, navigate]);

  if (!quote) return null;

  const fundingConfig = getChainConfig(quote.funding_chain as Chain);
  const executionConfig = getChainConfig(quote.execution_chain as Chain);
  const isExpired = timeRemaining <= 0;

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  /**
   * CRITICAL: Create spending approval
   * This creates an unsigned approval that the user must sign
   */
  const handleCreateApproval = async () => {
    if (!wallet.address) {
      toast({
        title: 'Wallet Not Connected',
        description: 'Please connect your wallet to approve spending',
        variant: 'destructive',
      });
      return;
    }

    setIsProcessing(true);
    try {
      const response = await approvalApi.create({
        quote_id: quote.quote_id,
        approved_amount: quote.max_funding_amount,
        wallet_address: wallet.address,
      });

      setApprovalId(response.data.id);
      setCurrentApproval(response.data);
      setApprovalStatus('pending');
      setStage('approval');

      toast({
        title: 'Approval Created',
        description: 'Ready to sign with your wallet',
      });
    } catch (error: any) {
      toast({
        title: 'Failed to Create Approval',
        description: error.response?.data?.message || 'Please try again.',
        variant: 'destructive',
      });
    } finally {
      setIsProcessing(false);
    }
  };

  /**
   * CRITICAL: Sign and submit approval
   * User signs the nonce from approval with their wallet
   * Backend verifies the signature atomically
   */
  const handleSignAndSubmitApproval = async () => {
    if (!approvalId || !trade.currentApproval) {
      toast({
        title: 'Error',
        description: 'Approval data missing',
        variant: 'destructive',
      });
      return;
    }

    if (!wallet.address || !wallet.chain) {
      toast({
        title: 'Wallet Not Connected',
        description: 'Please connect your wallet',
        variant: 'destructive',
      });
      return;
    }

    setIsProcessing(true);
    try {
      // Step 1: Request wallet signature of the approval nonce
      const signature = await sign(
        wallet.chain,
        wallet.address,
        trade.currentApproval.nonce
      );

      // Step 2: Submit signed approval to backend
      // Backend performs 7-step verification:
      // 1. Approval exists and valid
      // 2. Not already used (atomic check)
      // 3. Not expired
      // 4. User has submitted signature
      // 5. User has sufficient token balance
      // 6. Marks as used atomically
      // 7. Logs audit event
      const submitResponse = await approvalApi.submit(approvalId, signature);

      setApprovalStatus('authorized');
      toast({
        title: 'Approval Authorized',
        description: 'Spending approval verified. Proceeding to commit.',
      });

      // Proceed to commit quote now that approval is authorized
      setStage('committing');
      handleCommit();
    } catch (error: any) {
      toast({
        title: 'Approval Failed',
        description: error.response?.data?.message || 'Signature verification failed. Please try again.',
        variant: 'destructive',
      });
      setStage('approval');
    } finally {
      setIsProcessing(false);
    }
  };

  /**
   * Commit quote (only after approval is authorized)
   */
  const handleCommit = async () => {
    if (trade.approvalStatus !== 'authorized') {
      toast({
        title: 'Approval Required',
        description: 'You must authorize the spending approval first',
        variant: 'destructive',
      });
      return;
    }

    setIsProcessing(true);
    try {
      await quoteApi.commit(quote.quote_id);
      setQuoteStatus('Committed');
      navigate('/execution');
    } catch (error: any) {
      toast({
        title: 'Commitment Failed',
        description: error.response?.data?.message || 'Please try again.',
        variant: 'destructive',
      });
      setStage('review');
    } finally {
      setIsProcessing(false);
    }
  };

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
            <h1 className="text-3xl font-bold mb-2">Review Quote</h1>
            <p className="text-muted-foreground">
              {stage === 'review'
                ? 'Confirm your cross-chain transaction'
                : stage === 'approval'
                ? 'Authorize spending with your wallet'
                : 'Committing transaction...'}
            </p>
          </motion.div>

          <GlassCard>
            {/* Timer */}
            <div className={`flex items-center justify-between p-4 rounded-xl mb-6 ${
              isExpired ? 'bg-destructive/10' : timeRemaining < 120 ? 'bg-warning/10' : 'bg-secondary/50'
            }`}>
              <div className="flex items-center gap-2">
                <Clock className={`h-5 w-5 ${isExpired ? 'text-destructive' : timeRemaining < 120 ? 'text-warning' : 'text-muted-foreground'}`} />
                <span className="text-sm font-medium">Quote expires in</span>
              </div>
              <span className={`text-lg font-bold ${isExpired ? 'text-destructive' : timeRemaining < 120 ? 'text-warning' : 'text-foreground'}`}>
                {isExpired ? 'Expired' : formatTime(timeRemaining)}
              </span>
            </div>

            {/* Chain Flow */}
            <div className="flex items-center justify-center gap-4 mb-8">
              <div className="flex flex-col items-center gap-2">
                <ChainIcon chain={quote.funding_chain as Chain} size="lg" />
                <span className="text-sm font-medium">{fundingConfig.name}</span>
              </div>
              <ArrowRight className="h-6 w-6 text-muted-foreground" />
              <div className="flex flex-col items-center gap-2">
                <ChainIcon chain={quote.execution_chain as Chain} size="lg" />
                <span className="text-sm font-medium">{executionConfig.name}</span>
              </div>
            </div>

            {/* Quote Details */}
            <div className="space-y-4">
              <div className="flex justify-between items-center py-3 border-b border-border/50">
                <span className="text-muted-foreground">You Pay</span>
                <span className="font-semibold text-lg">
                  {quote.funding_amount} {quote.funding_asset}
                </span>
              </div>
              <div className="flex justify-between items-center py-3 border-b border-border/50">
                <span className="text-muted-foreground">You Receive</span>
                <span className="font-semibold text-lg text-accent">
                  {quote.execution_amount} {quote.execution_asset}
                </span>
              </div>
            </div>

            {/* Fee Breakdown */}
            <div className="mt-6 p-4 bg-secondary/30 rounded-xl space-y-3">
              <h3 className="text-sm font-semibold mb-3">Fee Breakdown</h3>
              <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">Execution Cost</span>
                <span>{quote.execution_cost} {quote.execution_asset}</span>
              </div>
              <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">Service Fee</span>
                <span>{quote.service_fee} {quote.execution_asset}</span>
              </div>
              <div className="flex justify-between text-sm pt-2 border-t border-border/50">
                <span className="font-medium">Max Payment</span>
                <span className="font-semibold">{quote.max_funding_amount} {quote.funding_asset}</span>
              </div>
            </div>

            {/* Status */}
            <div className="flex items-center justify-between mt-6">
              <span className="text-sm text-muted-foreground">Status</span>
              <StatusBadge status={quote.status} />
            </div>

            {/* Warning */}
            {isExpired && (
              <div className="flex items-center gap-3 p-4 bg-destructive/10 rounded-xl mt-6">
                <AlertTriangle className="h-5 w-5 text-destructive shrink-0" />
                <p className="text-sm text-destructive">
                  This quote has expired. Please request a new quote.
                </p>
              </div>
            )}

            {/* Security Notice */}
            {stage === 'review' && (
              <div className="flex items-start gap-3 p-4 bg-primary/10 rounded-xl mt-6 border border-primary/20">
                <Shield className="h-5 w-5 text-primary shrink-0 mt-0.5" />
                <div>
                  <p className="text-sm font-medium text-primary">Secure Approval Required</p>
                  <p className="text-xs text-primary/70 mt-1">
                    To proceed, you'll authorize this transaction with your wallet. This ensures only you can approve spending.
                  </p>
                </div>
              </div>
            )}

            {/* Approval Status Indicator */}
            {stage !== 'review' && (
              <div className="flex items-center justify-between p-4 bg-secondary/50 rounded-xl mt-6">
                <div className="flex items-center gap-3">
                  {stage === 'approval' && (
                    <>
                      <div className="w-3 h-3 bg-amber-500 rounded-full animate-pulse" />
                      <span className="text-sm font-medium">
                        {trade.approvalStatus === 'pending'
                          ? 'Approval pending signature'
                          : trade.approvalStatus === 'signed'
                          ? 'Waiting for verification'
                          : 'Approval authorized'}
                      </span>
                    </>
                  )}
                  {stage === 'committing' && (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin text-primary" />
                      <span className="text-sm font-medium">Committing transaction...</span>
                    </>
                  )}
                </div>
              </div>
            )}

            {/* Actions */}
            <div className="flex gap-4 mt-8">
              <Button
                variant="outline"
                className="flex-1"
                onClick={() => navigate('/trade')}
                disabled={isProcessing}
              >
                Cancel
              </Button>

              {/* Review Stage: Create Approval */}
              {stage === 'review' && (
                <Button
                  variant="gradient"
                  className="flex-1"
                  onClick={handleCreateApproval}
                  loading={isProcessing}
                  disabled={isExpired || !wallet.isConnected}
                >
                  <Shield className="h-4 w-4" />
                  Create Approval
                </Button>
              )}

              {/* Approval Stage: Sign & Submit */}
              {stage === 'approval' && (
                <Button
                  variant="gradient"
                  className="flex-1"
                  onClick={handleSignAndSubmitApproval}
                  loading={isProcessing}
                  disabled={isExpired}
                >
                  <Check className="h-4 w-4" />
                  {trade.approvalStatus === 'pending'
                    ? 'Sign & Authorize'
                    : trade.approvalStatus === 'signed'
                    ? 'Verifying...'
                    : 'Authorized'}
                </Button>
              )}

              {/* Committing Stage: Show in progress */}
              {stage === 'committing' && (
                <Button
                  variant="gradient"
                  className="flex-1"
                  disabled
                  loading={true}
                >
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Committing...
                </Button>
              )}
            </div>
          </GlassCard>
        </div>
      </main>
    </div>
  );
}
