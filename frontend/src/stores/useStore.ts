import { create } from 'zustand';
import { Quote, QuoteStatus } from '@/lib/api';
import type { SpendingApprovalResponse } from '@/types/approval';

export type Chain = 'solana' | 'stellar' | 'near';

export interface WalletState {
  address: string | null;
  chain: Chain | null;
  isConnected: boolean;
  isConnecting: boolean;
  walletId?: string;
  status?: 'verified' | 'unverified' | 'pending_verification';
  challenge?: string | null;
}

interface TradeState {
  fundingChain: Chain;
  executionChain: Chain;
  fundingAsset: string;
  executionAsset: string;
  amount: string;
  currentQuote: Quote | null;
  quoteStatus: QuoteStatus | null;
  currentApproval: SpendingApprovalResponse | null;
  approvalStatus: 'pending' | 'signed' | 'authorized' | null;
}

interface UserState {
  /**
   * Persistent user ID - persisted to localStorage for session continuity
   * Used for all API calls to track user history, approvals, etc.
   */
  userId: string;
}

interface AppState {
  // User (persistent)
  user: UserState;
  initializeUser: () => void;

  // Wallet
  wallet: WalletState;
  setWallet: (wallet: Partial<WalletState>) => void;
  connectWallet: (chain: Chain) => Promise<void>;
  disconnectWallet: () => void;

  // Trade
  trade: TradeState;
  setFundingChain: (chain: Chain) => void;
  setExecutionChain: (chain: Chain) => void;
  setFundingAsset: (asset: string) => void;
  setExecutionAsset: (asset: string) => void;
  setAmount: (amount: string) => void;
  setCurrentQuote: (quote: Quote | null) => void;
  setQuoteStatus: (status: QuoteStatus | null) => void;
  setCurrentApproval: (approval: SpendingApprovalResponse | null) => void;
  setApprovalStatus: (status: 'pending' | 'signed' | 'authorized' | null) => void;
  resetTrade: () => void;
}

const initialTradeState: TradeState = {
  fundingChain: 'solana',
  executionChain: 'stellar',
  fundingAsset: 'SOL',
  executionAsset: 'USDC',
  amount: '',
  currentQuote: null,
  quoteStatus: null,
  currentApproval: null,
  approvalStatus: null,
};

const initialWalletState: WalletState = {
  address: null,
  chain: null,
  isConnected: false,
  isConnecting: false,
};

/**
 * Get or create persistent user ID stored in localStorage
 */
const getPersistentUserId = (): string => {
  const storageKey = 'crosschain_user_id';
  let userId = localStorage.getItem(storageKey);
  
  if (!userId) {
    // Generate new user ID if not exists
    userId = crypto.randomUUID();
    localStorage.setItem(storageKey, userId);
  }
  
  return userId;
};

export const useStore = create<AppState>((set, get) => ({
  // User state - persistent across sessions
  user: {
    userId: getPersistentUserId(),
  },
  
  initializeUser: () => {
    set({
      user: {
        userId: getPersistentUserId(),
      },
    });
  },

  // Wallet state
  wallet: initialWalletState,
  
  setWallet: (wallet) => 
    set((state) => ({ wallet: { ...state.wallet, ...wallet } })),
  
  connectWallet: async (chain: Chain, address: string, userId: string) => {
    set((state) => ({ wallet: { ...state.wallet, isConnecting: true } }));
    try {
      // Step 1: Register the wallet address for the chain with backend
      const response = await fetch('/wallet/register', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ user_id: userId, chain, address }),
      });
      const data = await response.json();

      // Step 2: If backend says wallet is unverified, prompt user to sign a challenge message
      if (data.status === 'unverified' && data.challenge) {
        // Here you must prompt the wallet adapter to sign (handle in component/WalletProvider)
        // The component should call setWallet({ status: 'pending_verification', ... })
        // Actual signature and /wallet/verify handled in WalletProvider or component.
        set({ wallet: {
          address: data.address,
          chain: data.chain.toLowerCase(),
          isConnected: false,
          isConnecting: false,
          walletId: data.wallet_id,
          status: 'pending_verification',
          challenge: data.challenge,
        }});
        return;
      }

      // Step 3: If verified in response, mark as connected
      set({ wallet: {
        address: data.address,
        chain: data.chain.toLowerCase(),
        isConnected: data.verified || false,
        isConnecting: false,
        walletId: data.wallet_id,
        status: data.verified ? 'verified' : 'unverified',
        challenge: null,
      }});
    } catch (error) {
      set((state) => ({ wallet: { ...state.wallet, isConnecting: false } }));
      throw error;
    }
  },
  
  disconnectWallet: () => set({ wallet: initialWalletState }),

  // Trade state
  trade: initialTradeState,
  
  setFundingChain: (chain) =>
    set((state) => ({ 
      trade: { 
        ...state.trade, 
        fundingChain: chain,
        executionChain: chain === state.trade.executionChain 
          ? (chain === 'solana' ? 'stellar' : 'solana') 
          : state.trade.executionChain
      } 
    })),
  
  setExecutionChain: (chain) =>
    set((state) => ({ 
      trade: { 
        ...state.trade, 
        executionChain: chain,
        fundingChain: chain === state.trade.fundingChain 
          ? (chain === 'solana' ? 'stellar' : 'solana') 
          : state.trade.fundingChain
      } 
    })),
  
  setFundingAsset: (asset) =>
    set((state) => ({ trade: { ...state.trade, fundingAsset: asset } })),
  
  setExecutionAsset: (asset) =>
    set((state) => ({ trade: { ...state.trade, executionAsset: asset } })),
  
  setAmount: (amount) =>
    set((state) => ({ trade: { ...state.trade, amount } })),
  
  setCurrentQuote: (quote) =>
    set((state) => ({ trade: { ...state.trade, currentQuote: quote } })),
  
  setQuoteStatus: (status) =>
    set((state) => ({ trade: { ...state.trade, quoteStatus: status } })),
  
  setCurrentApproval: (approval) =>
    set((state) => ({ trade: { ...state.trade, currentApproval: approval } })),
  
  setApprovalStatus: (status) =>
    set((state) => ({ trade: { ...state.trade, approvalStatus: status } })),
  
  resetTrade: () => set({ trade: initialTradeState }),
}));
