// src/components/WalletConnectButton.tsx
import { Button } from '@/components/ui/button';
import { useWallet } from '@/contexts/WalletProvider';
import { Wallet, LogOut } from 'lucide-react';
import { useState, useCallback, useEffect } from 'react';
import { ChainType } from '@/lib/wallet-adapters/types';
import near from "@/assets/near-logo.png";
import stellar from "@/assets/stellar-logo.png";
import solana from "@/assets/solana-logo.png";

const CHAIN_DISPLAY_NAMES: Record<ChainType, string> = {
  solana: 'Solana',
  stellar: 'Stellar',
  near: 'NEAR',
};

const CHAIN_ICONS: Record<ChainType, string> = {
  solana,
  stellar,
  near,
};

export const WalletConnectButton = () => {
  const { connectedWallets, connect, disconnect, isConnecting } = useWallet();
  const [showChainSelector, setShowChainSelector] = useState(false);

  const handleConnect = useCallback(async (chain: ChainType) => {
    try {

      if (chain === 'near') {
      // Store the current URL to return to after NEAR wallet connection
      const redirectUrl = new URL(window.location.href);
      redirectUrl.searchParams.set('near_connect', 'true');
      localStorage.setItem('near_redirect_url', redirectUrl.toString());
    }

      await connect(chain);
      setShowChainSelector(false);
    } catch (error) {
      console.error('Connection error:', error);
    }
  }, [connect]);

    useEffect(() => {
      const params = new URLSearchParams(window.location.search);
      if (params.get('near_connect') === 'true' && connectedWallets.some(w => w.chain === 'near')) {
        // Clean up the URL
        params.delete('near_connect');
        const newUrl = `${window.location.pathname}${params.toString() ? '?' + params.toString() : ''}`;
        window.history.replaceState({}, '', newUrl);
      }
    }, [connectedWallets]);

  const handleDisconnect = useCallback(async (address: string) => {
    try {
      await disconnect(address);
    } catch (error) {
      console.error('Disconnect error:', error);
    }
  }, [disconnect]);

  const availableChains = (['solana', 'stellar', 'near'] as ChainType[])
    .filter(chain => !connectedWallets.some(w => w.chain === chain));

  if (connectedWallets.length > 0) {
    return (
      <div className="flex items-center gap-2">
        {connectedWallets.map((wallet) => (
          <div key={wallet.account.address} className="flex items-center gap-2 bg-gray-100 dark:bg-gray-800 px-3 py-1 rounded-md">
            <img 
              src={CHAIN_ICONS[wallet.chain]} 
              alt={wallet.chain} 
              className="w-4 h-4" 
            />
            <span className="text-sm">
              {`${wallet.account.address.slice(0, 4)}...${wallet.account.address.slice(-4)}`}
            </span>
            <button
              onClick={() => handleDisconnect(wallet.account.address)}
              disabled={isConnecting}
              className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              title={`Disconnect ${CHAIN_DISPLAY_NAMES[wallet.chain]} wallet`}
            >
              <LogOut className="w-3.5 h-3.5" />
            </button>
          </div>
        ))}
        
        {connectedWallets.length < 2 && (
          <div className="relative">
            <Button
              onClick={() => setShowChainSelector(!showChainSelector)}
              disabled={isConnecting || availableChains.length === 0}
              className="gap-2"
              variant="outline"
              size="sm"
            >
              <Wallet className="w-4 h-4" />
              Add Wallet
            </Button>

            {showChainSelector && (
              <div className="absolute right-0 z-10 mt-2 w-48 origin-top-right rounded-md bg-white shadow-lg ring-1 ring-black ring-opacity-5 focus:outline-none">
                <div className="py-1">
                  {availableChains.map((chain) => (
                    <button
                      key={chain}
                      onClick={() => handleConnect(chain)}
                      className="flex w-full items-center px-4 py-2 text-sm text-gray-700 hover:bg-gray-100"
                    >
                      <img 
                        src={CHAIN_ICONS[chain]} 
                        alt={chain} 
                        className="w-4 h-4 mr-2" 
                      />
                      {`Connect ${CHAIN_DISPLAY_NAMES[chain]}`}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="relative">
      <Button
        onClick={() => setShowChainSelector(!showChainSelector)}
        disabled={isConnecting}
        className="gap-2"
      >
        <Wallet className="w-4 h-4" />
        {isConnecting ? 'Connecting...' : 'Connect Wallet'}
      </Button>

      {showChainSelector && (
        <div className="absolute right-0 z-10 mt-2 w-48 origin-top-right rounded-md bg-white shadow-lg ring-1 ring-black ring-opacity-5 focus:outline-none">
          <div className="py-1">
            {availableChains.map((chain) => (
              <button
                key={chain}
                onClick={() => handleConnect(chain)}
                className="flex w-full items-center px-4 py-2 text-sm text-gray-700 hover:bg-gray-100"
              >
                <img 
                  src={CHAIN_ICONS[chain]} 
                  alt={chain} 
                  className="w-4 h-4 mr-2" 
                />
                {`Connect ${CHAIN_DISPLAY_NAMES[chain]}`}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};