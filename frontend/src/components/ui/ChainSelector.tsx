import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { ChainIcon, getChainConfig, supportedChains } from '@/components/ui/ChainIcon';
import { Chain } from '@/stores/useStore';
import { cn } from '@/lib/utils';

interface ChainSelectorProps {
  value: Chain;
  onChange: (chain: Chain) => void;
  disabledChain?: Chain;
  label?: string;
  className?: string;
}

export function ChainSelector({
  value,
  onChange,
  disabledChain,
  label,
  className,
}: ChainSelectorProps) {
  const config = getChainConfig(value);

  return (
    <div className={cn('space-y-2', className)}>
      {label && (
        <label className="text-sm font-medium text-muted-foreground">{label}</label>
      )}
      <Select value={value} onValueChange={(v) => onChange(v as Chain)}>
        <SelectTrigger className="w-full bg-secondary/50 border-border/50 hover:border-primary/50 transition-colors">
          <SelectValue>
            <div className="flex items-center gap-3">
              <ChainIcon chain={value} size="sm" />
              <span className="font-medium">{config.name}</span>
            </div>
          </SelectValue>
        </SelectTrigger>
        <SelectContent className="bg-card border-border/50">
          {supportedChains.map((chain) => {
            const chainConfig = getChainConfig(chain);
            const isDisabled = chain === disabledChain;
            return (
              <SelectItem
                key={chain}
                value={chain}
                disabled={isDisabled}
                className={cn(
                  'cursor-pointer',
                  isDisabled && 'opacity-50 cursor-not-allowed'
                )}
              >
                <div className="flex items-center gap-3">
                  <ChainIcon chain={chain} size="sm" />
                  <span>{chainConfig.name}</span>
                </div>
              </SelectItem>
            );
          })}
        </SelectContent>
      </Select>
    </div>
  );
}
