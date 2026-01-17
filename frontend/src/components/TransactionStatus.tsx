
import { CheckCircle2, AlertCircle, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';

type Status = 'idle' | 'loading' | 'success' | 'error';

interface TransactionStatusProps {
  status: Status;
  message: string;
  className?: string;
}

export function TransactionStatus({ status, message, className }: TransactionStatusProps) {
  const getStatusIcon = () => {
    switch (status) {
      case 'loading':
        return <Loader2 className="h-4 w-4 animate-spin" />;
      case 'success':
        return <CheckCircle2 className="h-4 w-4 text-green-500" />;
      case 'error':
        return <AlertCircle className="h-4 w-4 text-red-500" />;
      default:
        return null;
    }
  };

  return (
    <div className={cn("flex items-center gap-2 text-sm", className)}>
      {getStatusIcon()}
      <span>{message}</span>
    </div>
  );
}