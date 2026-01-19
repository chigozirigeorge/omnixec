import { cn } from '@/lib/utils';
import { Chain } from '@/stores/useStore';
import near from "@/assets/near-logo.png";
import solana from "@/assets/solana-logo.png";
import stellar from "@/assets/stellar-logo.png"

interface ChainIconProps {
  chain: Chain;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const chainConfigs: Record<Chain, { name: string; color: string; icon: string }> = {
  solana: {
    name: 'Solana',
    color: 'hsl(280 70% 60%)',
    icon: solana,
  },
  stellar: {
    name: 'Stellar',
    color: 'hsl(200 100% 50%)',
    icon: stellar,
  },
  near: {
    name: 'NEAR',
    color: 'hsl(168 100% 48%)',
    icon: near,
  },
};

const sizeClasses = {
  sm: 'w-6 h-6 text-xs',
  md: 'w-8 h-8 text-sm',
  lg: 'w-10 h-10 text-base',
};

export function ChainIcon({ chain, size = 'md', className }: ChainIconProps) {
  const config = chainConfigs[chain];

  return (
    <div
      className={cn(
        'rounded-full flex items-center justify-center font-bold',
        sizeClasses[size],
        className
      )}
      style={{ 
        backgroundColor: `${config.color}20`,
        color: config.color,
        border: `1px solid ${config.color}40`,
      }}
    >
      <img src={config.icon} alt={config.name} className='rounded-full'/>
    </div>
  );
}

export function getChainConfig(chain: Chain) {
  return chainConfigs[chain];
}

export const supportedChains: Chain[] = ['solana', 'stellar', 'near'];
