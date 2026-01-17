import { useStore, Chain } from '@/stores/useStore';
import { Button } from '@/components/ui/button';
import { ChainIcon, getChainConfig, supportedChains } from '@/components/ui/ChainIcon';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Wallet, LogOut, ChevronDown } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

export function WalletConnect() {
  const { wallet, connectWallet, disconnectWallet } = useStore();

  if (wallet.isConnected && wallet.address && wallet.chain) {
    const config = getChainConfig(wallet.chain);
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="glass" size="sm" className="gap-2">
            <ChainIcon chain={wallet.chain} size="sm" />
            <span className="hidden sm:inline">{wallet.address}</span>
            <ChevronDown className="h-4 w-4 opacity-50" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-48 bg-card border-border/50">
          <div className="px-3 py-2">
            <p className="text-xs text-muted-foreground">Connected to</p>
            <p className="font-medium">{config.name}</p>
          </div>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            onClick={disconnectWallet}
            className="text-destructive focus:text-destructive cursor-pointer"
          >
            <LogOut className="mr-2 h-4 w-4" />
            Disconnect
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="gradient" size="sm" disabled={wallet.isConnecting}>
          {wallet.isConnecting ? (
            <>
              <span className="animate-spin">◌</span>
              Connecting...
            </>
          ) : (
            <>
              <Wallet className="h-4 w-4" />
              <span className="hidden sm:inline">Connect Wallet</span>
            </>
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56 bg-card border-border/50">
        <div className="px-3 py-2">
          <p className="text-sm font-medium">Select a wallet</p>
          <p className="text-xs text-muted-foreground">Connect to start trading</p>
        </div>
        <DropdownMenuSeparator />
        {supportedChains.map((chain) => {
          const config = getChainConfig(chain);
          const walletNames: Record<Chain, string> = {
            solana: 'Phantom',
            stellar: 'Freighter',
            near: 'NEAR Wallet',
          };
          return (
            <DropdownMenuItem
              key={chain}
              onClick={() => connectWallet(chain)}
              className="cursor-pointer"
            >
              <ChainIcon chain={chain} size="sm" className="mr-3" />
              <div>
                <p className="font-medium">{walletNames[chain]}</p>
                <p className="text-xs text-muted-foreground">{config.name}</p>
              </div>
            </DropdownMenuItem>
          );
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
