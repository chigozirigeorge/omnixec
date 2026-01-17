import { cn } from '@/lib/utils';
import { QuoteStatus } from '@/lib/api';
import { motion } from 'framer-motion';

interface StatusBadgeProps {
  status: QuoteStatus;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
  animated?: boolean;
}

const statusConfig: Record<QuoteStatus, { label: string; color: string; bgColor: string }> = {
  Pending: {
    label: 'Pending',
    color: 'hsl(var(--warning))',
    bgColor: 'hsl(var(--warning) / 0.15)',
  },
  Committed: {
    label: 'Committed',
    color: 'hsl(var(--primary))',
    bgColor: 'hsl(var(--primary) / 0.15)',
  },
  Executed: {
    label: 'Executed',
    color: 'hsl(var(--success))',
    bgColor: 'hsl(var(--success) / 0.15)',
  },
  Failed: {
    label: 'Failed',
    color: 'hsl(var(--destructive))',
    bgColor: 'hsl(var(--destructive) / 0.15)',
  },
  Expired: {
    label: 'Expired',
    color: 'hsl(var(--muted-foreground))',
    bgColor: 'hsl(var(--muted) / 0.5)',
  },
};

const sizeClasses = {
  sm: 'px-2 py-0.5 text-xs',
  md: 'px-3 py-1 text-sm',
  lg: 'px-4 py-1.5 text-base',
};

export function StatusBadge({ status, size = 'md', className, animated = true }: StatusBadgeProps) {
  const config = statusConfig[status];
  const isPending = status === 'Pending' || status === 'Committed';

  const badge = (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 font-medium rounded-full',
        sizeClasses[size],
        className
      )}
      style={{
        backgroundColor: config.bgColor,
        color: config.color,
        border: `1px solid ${config.color}40`,
      }}
    >
      {isPending && (
        <span className="relative flex h-2 w-2">
          <span
            className="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75"
            style={{ backgroundColor: config.color }}
          />
          <span
            className="relative inline-flex rounded-full h-2 w-2"
            style={{ backgroundColor: config.color }}
          />
        </span>
      )}
      {config.label}
    </span>
  );

  if (animated) {
    return (
      <motion.div
        initial={{ scale: 0.9, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ duration: 0.2 }}
      >
        {badge}
      </motion.div>
    );
  }

  return badge;
}
