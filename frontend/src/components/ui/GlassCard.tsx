import { cn } from '@/lib/utils';
import { motion, HTMLMotionProps } from 'framer-motion';
import { forwardRef } from 'react';

interface GlassCardProps extends HTMLMotionProps<'div'> {
  hover?: boolean;
  glow?: 'primary' | 'accent' | 'none';
}

export const GlassCard = forwardRef<HTMLDivElement, GlassCardProps>(
  ({ className, hover = true, glow = 'none', children, ...props }, ref) => {
    return (
      <motion.div
        ref={ref}
        className={cn(
          'glass-card',
          hover && 'glass-hover',
          glow === 'primary' && 'glow-primary',
          glow === 'accent' && 'glow-accent',
          className
        )}
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4 }}
        {...props}
      >
        {children}
      </motion.div>
    );
  }
);

GlassCard.displayName = 'GlassCard';
