// src/contexts/WalletProvider.tsx
import React, { createContext, useContext, useEffect, useState, useCallback, ReactNode } from 'react';
import { SolanaWalletAdapter } from '@/lib/wallet-adapters/solana';
import { StellarWalletAdapter } from '@/lib/wallet-adapters/stellar';
import { NearWalletAdapter } from '@/lib/wallet-adapters/near';
import { WalletAdapter, WalletAccount, ChainType } from '@/lib/wallet-adapters/types';
import { walletApi } from '@/lib/api';
import { useAuth } from './AuthContext';

interface ConnectedWallet {
  chain: ChainType;
  account: WalletAccount;
  adapter: WalletAdapter;
}

interface WalletContextType {
  connectedWallets: ConnectedWallet[];
  connect: (chain: ChainType) => Promise<void>;
  disconnect: (address: string) => Promise<void>;
  signMessage: (message: string, chain: ChainType) => Promise<string>;
  isConnecting: boolean;
  error: string | null;
  getConnectedAddress: (chain: ChainType) => string | null;
}

const WalletContext = createContext<WalletContextType | undefined>(undefined);

export const WalletProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [connectedWallets, setConnectedWallets] = useState<ConnectedWallet[]>([]);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { userId, setUserId } = useAuth();

  const getAdapter = useCallback((chain: ChainType): WalletAdapter => {
    switch (chain) {
      case 'solana':
        return new SolanaWalletAdapter({ network: 'mainnet' });
      case 'stellar':
        return new StellarWalletAdapter({ network: 'testnet' });
      case 'near':
        return new NearWalletAdapter({ network: 'testnet' });
      default:
        throw new Error(`Unsupported chain: ${chain}`);
    }
  }, []);

  const connect = useCallback(
  async (chain: ChainType): Promise<void> => {
    if (connectedWallets.length >= 2) {
      throw new Error('Maximum of 2 wallets can be connected at the same time');
    }

    if (connectedWallets.some(w => w.chain === chain)) {
      console.log(`Already connected to ${chain} wallet`);
      return;
    }

    try {
      setIsConnecting(true);
      setError(null);
      
      const adapter = getAdapter(chain);
      const account = await adapter.connect();
      
      // Check if we already have this wallet connected
      if (connectedWallets.some(w => w.account.address === account.address)) {
        throw new Error('This wallet is already connected');
      }

      // Prepare registration data
      const currentUserId = localStorage.getItem('omniexec_user_id');
      const registerData: any = {
        chain: chain.charAt(0).toUpperCase() + chain.slice(1),
        address: account.address,
      };

      if (currentUserId) {
        registerData.user_id = currentUserId;
      }

      try {
        // Try to register the wallet
        const regRes = await walletApi.register(registerData);
        
        // Handle new user ID if this is the first wallet
        if (regRes.data?.user_id && !currentUserId) {
          localStorage.setItem('omniexec_user_id', regRes.data.user_id);
          setUserId(regRes.data.user_id);
        }

        // Handle verification if needed
        if (regRes.data?.status === 'unverified' && regRes.data?.challenge) {
          try {
            const sig = await adapter.signMessage(regRes.data.challenge);
            await walletApi.verify({
              user_id: regRes.data.user_id || currentUserId,
              chain: registerData.chain,
              address: account.address,
              nonce: regRes.data.challenge,
              signature: sig,
              signed_message: regRes.data.challenge,
            });
          } catch (err) {
            console.error('Verification failed:', err);
            // Continue even if verification fails
          }
        }
      } catch (error: any) {
        // If it's a 500 error, it might be because the wallet is already registered
        if (error.response?.status !== 500) {
          throw error;
        }
        console.log('Wallet might already be registered, continuing with connection...');
      }

      // Add the new wallet to connected wallets
      const newWallet: ConnectedWallet = {
        chain,
        account,
        adapter
      };

      setConnectedWallets(prev => [...prev, newWallet]);
    } catch (err) {
      console.error('Connection error:', err);
      setError(err instanceof Error ? err.message : 'Failed to connect wallet');
      throw err;
    } finally {
      setIsConnecting(false);
    }
  },
  [connectedWallets, getAdapter, setUserId]
);

  const disconnect = useCallback(async (address: string) => {
    try {
      const wallet = connectedWallets.find(w => w.account.address === address);
      if (!wallet) return;

      await wallet.adapter.disconnect();
      setConnectedWallets(prev => prev.filter(w => w.account.address !== address));
    } catch (err) {
      console.error('Disconnect error:', err);
      setError('Failed to disconnect wallet');
    }
  }, [connectedWallets]);

  const signMessage = useCallback(async (message: string, chain: ChainType) => {
    const wallet = connectedWallets.find(w => w.chain === chain);
    if (!wallet) {
      throw new Error(`No ${chain} wallet connected`);
    }
    return wallet.adapter.signMessage(message);
  }, [connectedWallets]);

  const getConnectedAddress = useCallback((chain: ChainType): string | null => {
    const wallet = connectedWallets.find(w => w.chain === chain);
    return wallet ? wallet.account.address : null;
  }, [connectedWallets]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      connectedWallets.forEach(wallet => {
        wallet.adapter.disconnect().catch(console.error);
      });
    };
  }, [connectedWallets]);

  return (
    <WalletContext.Provider
      value={{
        connectedWallets,
        connect,
        disconnect,
        signMessage,
        isConnecting,
        error,
        getConnectedAddress,
      }}
    >
      {children}
    </WalletContext.Provider>
  );
};

export const useWallet = (): WalletContextType => {
  const context = useContext(WalletContext);
  if (context === undefined) {
    throw new Error('useWallet must be used within a WalletProvider');
  }
  return context;
};