// components/TransactionHistory.tsx
import { formatDistanceToNow } from 'date-fns';
import { ArrowUpRight, CheckCircle, XCircle, Clock, ExternalLink } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface Transaction {
  id: string;
  type: 'swap' | 'deposit' | 'withdraw';
  status: 'pending' | 'completed' | 'failed';
  from: { amount: string; currency: string };
  to: { amount: string; currency: string };
  timestamp: number;
  txHash?: string;
  explorerUrl?: string;
}

interface TransactionHistoryProps {
  transactions: Transaction[];
  className?: string;
}

export function TransactionHistory({ transactions, className }: TransactionHistoryProps) {
  if (transactions.length === 0) {
    return (
      <div className={cn("text-center py-8 text-muted-foreground", className)}>
        No transactions yet
      </div>
    );
  }

  return (
    <div className={cn("space-y-2", className)}>
      <h3 className="text-lg font-medium mb-4">Recent Transactions</h3>
      <div className="space-y-3">
        {transactions.map((tx) => (
          <div key={tx.id} className="p-4 border rounded-lg bg-card">
            <div className="flex justify-between items-center">
              <div className="flex items-center gap-2">
                {tx.status === 'completed' ? (
                  <CheckCircle className="h-5 w-5 text-green-500" />
                ) : tx.status === 'failed' ? (
                  <XCircle className="h-5 w-5 text-red-500" />
                ) : (
                  <Clock className="h-5 w-5 text-yellow-500" />
                )}
                <span className="font-medium capitalize">{tx.type}</span>
              </div>
              <span className="text-sm text-muted-foreground">
                {formatDistanceToNow(new Date(tx.timestamp), { addSuffix: true })}
              </span>
            </div>
            <div className="mt-2 flex justify-between items-center">
              <div className="text-sm">
                {tx.from.amount} {tx.from.currency} → {tx.to.amount} {tx.to.currency}
              </div>
              {tx.explorerUrl && tx.txHash && (
                <a
                  href={tx.explorerUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm text-blue-500 hover:underline flex items-center"
                >
                  View <ExternalLink className="h-3 w-3 ml-1" />
                </a>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}