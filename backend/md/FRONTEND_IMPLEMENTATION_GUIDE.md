# CrossChain Payments - Frontend Implementation Guide

Complete code examples and implementation patterns for frontend engineers.

---

## Table of Contents

1. [Frontend Folder Structure](#frontend-folder-structure)
2. [API Client Setup](#api-client-setup)
3. [State Management](#state-management)
4. [Page Components](#page-components)
5. [Polling & Real-time Updates](#polling--real-time-updates)
6. [Error Handling](#error-handling)
7. [Type Definitions](#type-definitions)

---

## Frontend Folder Structure

```
frontend/
├─ public/
├─ src/
│  ├─ api/
│  │  ├─ client.ts           # API HTTP client (axios/fetch)
│  │  ├─ endpoints.ts        # All API endpoints
│  │  └─ types.ts            # Request/Response types
│  │
│  ├─ pages/
│  │  ├─ Onboarding.tsx      # Steps 1-2: Wallet registration & verification
│  │  ├─ TokenDiscovery.tsx  # Step 3: Browse tokens & DEXes
│  │  ├─ TradeSetup.tsx      # Step 4: Quote generation
│  │  ├─ Payment.tsx         # Step 5: Payment instructions
│  │  ├─ Execution.tsx       # Step 6: Real-time execution tracking
│  │  └─ Completion.tsx      # Step 7: Trade complete
│  │
│  ├─ components/
│  │  ├─ WalletConnector.tsx
│  │  ├─ TokenSelector.tsx
│  │  ├─ PriceChart.tsx
│  │  ├─ QuoteDisplay.tsx
│  │  ├─ PaymentQR.tsx
│  │  └─ ProgressTracker.tsx
│  │
│  ├─ hooks/
│  │  ├─ useWallet.ts        # Wallet management
│  │  ├─ useQuote.ts         # Quote generation & tracking
│  │  ├─ usePolling.ts       # Status polling
│  │  └─ usePayment.ts       # Payment flow
│  │
│  ├─ store/
│  │  ├─ userStore.ts        # Zustand/Redux for user data
│  │  ├─ walletStore.ts      # Connected wallets
│  │  ├─ quoteStore.ts       # Current quote & execution
│  │  └─ uiStore.ts          # UI state (modals, toasts)
│  │
│  ├─ utils/
│  │  ├─ format.ts           # Number/date formatting
│  │  ├─ validation.ts       # Input validation
│  │  ├─ explorer.ts         # Blockchain explorer URLs
│  │  └─ crypto.ts           # Wallet signature utilities
│  │
│  └─ App.tsx
│
└─ package.json
```

---

## API Client Setup

### Option 1: Axios Client

```typescript
// src/api/client.ts
import axios, { AxiosInstance, AxiosError } from 'axios';

const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:8080';

export const apiClient: AxiosInstance = axios.create({
  baseURL: API_BASE_URL,
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Add response interceptor for error handling
apiClient.interceptors.response.use(
  (response) => response,
  (error: AxiosError) => {
    const message = error.response?.data?.message || error.message;
    console.error('API Error:', message);
    
    // Handle specific error codes
    if (error.response?.status === 429) {
      // Rate limited
      console.warn('Rate limited - please retry later');
    }
    if (error.response?.status === 503) {
      // Service unavailable - circuit breaker active
      console.warn('Service temporarily unavailable');
    }
    
    return Promise.reject(error);
  }
);

export default apiClient;
```

### Endpoints Definition

```typescript
// src/api/endpoints.ts
export const API_ENDPOINTS = {
  // Wallet endpoints
  wallet: {
    register: '/wallet/register',
    verify: '/wallet/verify',
    getWallets: (userId: string) => `/wallet/user/${userId}`,
    getPortfolio: (userId: string) => `/wallet/portfolio/${userId}`,
  },

  // Discovery endpoints
  discovery: {
    chains: '/discovery/chains',
    dexesForChain: (chain: string) => `/discovery/dexes/${chain}`,
    chainDiscovery: (chain: string) => `/discovery/chain/${chain}`,
    dexAssets: (dexName: string, chain: string) => 
      `/discovery/dex/${dexName}/${chain}`,
  },

  // Quote endpoints
  quote: {
    create: '/quote',
    commit: '/quote/commit',
    status: (quoteId: string) => `/status/${quoteId}`,
    ohlc: '/quote-engine/ohlc',
  },

  // Approval endpoints
  approval: {
    create: '/approval/create',
    submit: '/approval/submit',
  },

  // Admin endpoints
  admin: {
    treasury: '/admin/treasury',
    treasuryChain: (chain: string) => `/admin/treasury/${chain}`,
    health: '/health',
  },
};
```

---

## State Management (Zustand)

### User Store

```typescript
// src/store/userStore.ts
import create from 'zustand';

interface User {
  id: string;
  createdAt: Date;
}

interface UserStore {
  user: User | null;
  setUser: (user: User) => void;
  logout: () => void;
}

export const useUserStore = create<UserStore>((set) => ({
  user: null,
  
  setUser: (user: User) => set({ user }),
  
  logout: () => set({ user: null }),
}));
```

### Wallet Store

```typescript
// src/store/walletStore.ts
import create from 'zustand';

export enum Chain {
  Solana = 'Solana',
  Stellar = 'Stellar',
  Near = 'Near',
}

export enum WalletStatus {
  Unverified = 'unverified',
  Verified = 'verified',
  Frozen = 'frozen',
}

interface Wallet {
  walletId: string;
  chain: Chain;
  address: string;
  status: WalletStatus;
  verifiedAt?: Date;
}

interface WalletStore {
  wallets: Wallet[];
  addWallet: (wallet: Wallet) => void;
  updateWalletStatus: (walletId: string, status: WalletStatus) => void;
  getWalletByChain: (chain: Chain) => Wallet | undefined;
  getFundingWallet: () => Wallet | undefined;
  getExecutionWallet: () => Wallet | undefined;
}

export const useWalletStore = create<WalletStore>((set, get) => ({
  wallets: [],
  
  addWallet: (wallet: Wallet) =>
    set((state) => ({
      wallets: [...state.wallets, wallet],
    })),

  updateWalletStatus: (walletId: string, status: WalletStatus) =>
    set((state) => ({
      wallets: state.wallets.map((w) =>
        w.walletId === walletId ? { ...w, status } : w
      ),
    })),

  getWalletByChain: (chain: Chain) => {
    const state = get();
    return state.wallets.find((w) => w.chain === chain && w.status === WalletStatus.Verified);
  },

  getFundingWallet: () => get().wallets[0],
  getExecutionWallet: () => get().wallets[1],
}));
```

### Quote Store

```typescript
// src/store/quoteStore.ts
import create from 'zustand';

export enum QuoteStatus {
  Pending = 'pending',
  Committed = 'committed',
  Executing = 'executing',
  Completed = 'completed',
  Failed = 'failed',
  Expired = 'expired',
}

interface Quote {
  quoteId: string;
  fundingChain: string;
  executionChain: string;
  fundingAsset: string;
  executionAsset: string;
  maxFundingAmount: string;
  executionCost: string;
  serviceFee: string;
  paymentAddress: string;
  expiresAt: Date;
  nonce: string;
}

interface Execution {
  executionId: string;
  quoteId: string;
  status: QuoteStatus;
  fundingTxHash?: string;
  executionTxHash?: string;
  executedAt?: Date;
  errorMessage?: string;
  currentStep: number;
  stepsCompleted: string[];
}

interface QuoteStore {
  quote: Quote | null;
  execution: Execution | null;
  setQuote: (quote: Quote) => void;
  updateExecution: (execution: Execution) => void;
  clearQuote: () => void;
  getTimeRemaining: () => number; // milliseconds
}

export const useQuoteStore = create<QuoteStore>((set, get) => ({
  quote: null,
  execution: null,

  setQuote: (quote: Quote) => set({ quote }),

  updateExecution: (execution: Execution) => set({ execution }),

  clearQuote: () => set({ quote: null, execution: null }),

  getTimeRemaining: () => {
    const quote = get().quote;
    if (!quote) return 0;
    const remaining = new Date(quote.expiresAt).getTime() - Date.now();
    return Math.max(0, remaining);
  },
}));
```

---

## Custom Hooks

### useQuote Hook

```typescript
// src/hooks/useQuote.ts
import { useCallback, useState } from 'react';
import { apiClient } from '../api/client';
import { API_ENDPOINTS } from '../api/endpoints';
import { useQuoteStore } from '../store/quoteStore';

export const useQuote = () => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { setQuote, updateExecution } = useQuoteStore();

  const generateQuote = useCallback(async (params: {
    userId: string;
    fundingChain: string;
    executionChain: string;
    fundingAsset: string;
    executionAsset: string;
    executionInstructionsBase64: string;
    estimatedComputeUnits?: number;
  }) => {
    setLoading(true);
    setError(null);

    try {
      const response = await apiClient.post(
        API_ENDPOINTS.quote.create,
        {
          user_id: params.userId,
          funding_chain: params.fundingChain,
          execution_chain: params.executionChain,
          funding_asset: params.fundingAsset,
          execution_asset: params.executionAsset,
          execution_instructions_base64: params.executionInstructionsBase64,
          estimated_compute_units: params.estimatedComputeUnits,
        }
      );

      const quote = {
        quoteId: response.data.quote_id,
        fundingChain: response.data.funding_chain,
        executionChain: response.data.execution_chain,
        fundingAsset: response.data.funding_asset,
        executionAsset: response.data.execution_asset,
        maxFundingAmount: response.data.max_funding_amount,
        executionCost: response.data.execution_cost,
        serviceFee: response.data.service_fee,
        paymentAddress: response.data.payment_address,
        expiresAt: new Date(response.data.expires_at),
        nonce: response.data.nonce,
      };

      setQuote(quote);
      return quote;
    } catch (err: any) {
      const errorMsg = err.response?.data?.message || err.message;
      setError(errorMsg);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [setQuote]);

  const checkStatus = useCallback(async (quoteId: string) => {
    try {
      const response = await apiClient.get(
        API_ENDPOINTS.quote.status(quoteId)
      );

      const execution = {
        executionId: response.data.execution_id,
        quoteId,
        status: response.data.status,
        fundingTxHash: response.data.transaction_hash,
        executionTxHash: response.data.execution_tx_hash,
        executedAt: response.data.executed_at 
          ? new Date(response.data.executed_at) 
          : undefined,
        errorMessage: response.data.error_message,
        currentStep: 0,
        stepsCompleted: [],
      };

      updateExecution(execution);
      return execution;
    } catch (err: any) {
      const errorMsg = err.response?.data?.message || err.message;
      setError(errorMsg);
      throw err;
    }
  }, [updateExecution]);

  return {
    loading,
    error,
    generateQuote,
    checkStatus,
  };
};
```

### usePolling Hook

```typescript
// src/hooks/usePolling.ts
import { useEffect, useRef } from 'react';

interface UsePollingOptions {
  interval: number; // milliseconds
  enabled: boolean;
  onPoll: () => Promise<void>;
  maxAttempts?: number;
  onMaxAttemptsReached?: () => void;
}

export const usePolling = ({
  interval,
  enabled,
  onPoll,
  maxAttempts = 120, // 120 * 5s = 10 minutes
  onMaxAttemptsReached,
}: UsePollingOptions) => {
  const intervalRef = useRef<NodeJS.Timeout>();
  const attemptsRef = useRef(0);

  useEffect(() => {
    if (!enabled) {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
      return;
    }

    // Initial poll
    onPoll().catch(console.error);
    attemptsRef.current = 1;

    // Set up polling
    intervalRef.current = setInterval(async () => {
      attemptsRef.current += 1;

      if (maxAttempts && attemptsRef.current > maxAttempts) {
        clearInterval(intervalRef.current);
        onMaxAttemptsReached?.();
        return;
      }

      try {
        await onPoll();
      } catch (error) {
        console.error('Polling error:', error);
      }
    }, interval);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [enabled, interval, onPoll, maxAttempts, onMaxAttemptsReached]);

  const stop = () => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
    }
  };

  const reset = () => {
    attemptsRef.current = 0;
  };

  return { stop, reset, attempts: attemptsRef.current };
};
```

---

## Page Components

### Onboarding Page

```typescript
// src/pages/Onboarding.tsx
import React, { useState } from 'react';
import { useUserStore } from '../store/userStore';
import { useWalletStore, Chain, WalletStatus } from '../store/walletStore';
import { apiClient } from '../api/client';
import { API_ENDPOINTS } from '../api/endpoints';

export const Onboarding: React.FC = () => {
  const [step, setStep] = useState<1 | 2 | 3 | 4 | 5>(1);
  const [wallet1, setWallet1] = useState<{ chain: Chain; address: string } | null>(null);
  const [wallet2, setWallet2] = useState<{ chain: Chain; address: string } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const user = useUserStore((state) => state.user);
  const { addWallet } = useWalletStore();

  const handleRegisterWallet = async (chain: Chain, address: string) => {
    setLoading(true);
    setError(null);

    try {
      const response = await apiClient.post(
        API_ENDPOINTS.wallet.register,
        {
          user_id: user?.id,
          chain,
          address,
        }
      );

      const wallet = {
        walletId: response.data.wallet_id,
        chain: response.data.chain,
        address: response.data.address,
        status: response.data.status,
      };

      if (step === 1) {
        setWallet1(wallet);
        setStep(2); // Move to verify wallet 1
      } else if (step === 3) {
        setWallet2(wallet);
        setStep(4); // Move to verify wallet 2
      }
    } catch (err: any) {
      setError(err.response?.data?.message || 'Failed to register wallet');
    } finally {
      setLoading(false);
    }
  };

  const handleVerifyWallet = async (
    walletData: typeof wallet1
  ) => {
    if (!walletData) return;

    // In real app, trigger wallet signature request
    const signature = await requestWalletSignature(walletData.address);

    try {
      const response = await apiClient.post(
        API_ENDPOINTS.wallet.verify,
        {
          user_id: user?.id,
          chain: walletData.chain,
          address: walletData.address,
          signature,
        }
      );

      const verifiedWallet = {
        walletId: response.data.wallet_id,
        chain: response.data.chain,
        address: response.data.address,
        status: response.data.verified ? WalletStatus.Verified : WalletStatus.Unverified,
        verifiedAt: new Date(),
      };

      addWallet(verifiedWallet);

      if (step === 2) {
        setStep(3); // Move to register wallet 2
      } else if (step === 4) {
        setStep(5); // Onboarding complete
      }
    } catch (err: any) {
      setError(err.response?.data?.message || 'Failed to verify wallet');
    }
  };

  return (
    <div className="onboarding-container">
      {step === 1 && (
        <WalletRegistrationStep
          title="Connect Your Funding Wallet"
          description="Select the chain where you'll send tokens from"
          onRegister={(chain, address) => handleRegisterWallet(chain, address)}
          loading={loading}
          error={error}
        />
      )}

      {step === 2 && wallet1 && (
        <WalletVerificationStep
          wallet={wallet1}
          onVerify={() => handleVerifyWallet(wallet1)}
          loading={loading}
          error={error}
        />
      )}

      {step === 3 && (
        <WalletRegistrationStep
          title="Connect Your Execution Wallet"
          description="Select the chain where you'll receive tokens"
          onRegister={(chain, address) => handleRegisterWallet(chain, address)}
          loading={loading}
          error={error}
        />
      )}

      {step === 4 && wallet2 && (
        <WalletVerificationStep
          wallet={wallet2}
          onVerify={() => handleVerifyWallet(wallet2)}
          loading={loading}
          error={error}
        />
      )}

      {step === 5 && (
        <div className="completion-screen">
          <h2>✓ All Set!</h2>
          <p>Your wallets are connected and verified.</p>
          <button onClick={() => navigateTo('/discover')}>
            Start Trading
          </button>
        </div>
      )}
    </div>
  );
};

async function requestWalletSignature(address: string): Promise<string> {
  // Implementation depends on wallet type (Phantom, Ledger, etc.)
  // This is a placeholder
  return 'signature_from_wallet';
}
```

### Trade Setup Page

```typescript
// src/pages/TradeSetup.tsx
import React, { useState, useEffect } from 'react';
import { useQuote } from '../hooks/useQuote';
import { useQuoteStore } from '../store/quoteStore';
import { useWalletStore, Chain } from '../store/walletStore';
import { useUserStore } from '../store/userStore';

export const TradeSetup: React.FC = () => {
  const [fundingAmount, setFundingAmount] = useState('');
  const [executionChain, setExecutionChain] = useState<Chain>(Chain.Stellar);
  const [executionAsset, setExecutionAsset] = useState('XLM');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { generateQuote } = useQuote();
  const { quote, setQuote } = useQuoteStore();
  const { getFundingWallet } = useWalletStore();
  const user = useUserStore((state) => state.user);

  const fundingWallet = getFundingWallet();

  const handleGetQuote = async () => {
    if (!fundingAmount || !user) return;

    setLoading(true);
    setError(null);

    try {
      await generateQuote({
        userId: user.id,
        fundingChain: fundingWallet?.chain || Chain.Solana,
        executionChain,
        fundingAsset: 'USDC', // User selected
        executionAsset,
        executionInstructionsBase64: 'base64_encoded_instructions',
      });

      // Quote generated successfully
      setFundingAmount(''); // Clear form
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  if (quote) {
    return <QuoteReview quote={quote} />;
  }

  return (
    <div className="trade-setup">
      <h2>Set Up Your Trade</h2>

      <div className="form-group">
        <label>Amount to Send (USDC)</label>
        <input
          type="number"
          value={fundingAmount}
          onChange={(e) => setFundingAmount(e.target.value)}
          placeholder="100.00"
          disabled={loading}
        />
      </div>

      <div className="form-group">
        <label>Receive on Chain</label>
        <select
          value={executionChain}
          onChange={(e) => setExecutionChain(e.target.value as Chain)}
          disabled={loading}
        >
          <option value={Chain.Stellar}>Stellar (XLM)</option>
          <option value={Chain.Near}>NEAR</option>
        </select>
      </div>

      {error && <div className="error">{error}</div>}

      <button
        onClick={handleGetQuote}
        disabled={!fundingAmount || loading}
        className="btn-primary"
      >
        {loading ? 'Generating Quote...' : 'Get Quote'}
      </button>
    </div>
  );
};

const QuoteReview: React.FC<{ quote: Quote }> = ({ quote }) => {
  const [secondsRemaining, setSecondsRemaining] = useState(
    Math.round((new Date(quote.expiresAt).getTime() - Date.now()) / 1000)
  );

  useEffect(() => {
    const timer = setInterval(() => {
      setSecondsRemaining((prev) => (prev > 0 ? prev - 1 : 0));
    }, 1000);

    return () => clearInterval(timer);
  }, []);

  return (
    <div className="quote-review">
      <h3>Quote Summary</h3>

      <div className="quote-details">
        <div>
          <strong>You Send:</strong> {quote.maxFundingAmount} {quote.fundingAsset}
        </div>
        <div>
          <strong>You Receive:</strong> {quote.executionCost} {quote.executionAsset}
        </div>
        <div>
          <strong>Fee:</strong> {quote.serviceFee} {quote.fundingAsset}
        </div>
      </div>

      <div className="quote-timer">
        <strong>Expires in:</strong> {secondsRemaining} seconds
        {secondsRemaining < 60 && <span className="warning">⚠️ Quote expiring soon</span>}
      </div>

      <div className="action-buttons">
        <button className="btn-primary" onClick={() => navigateToPayment(quote)}>
          Approve & Continue
        </button>
        <button className="btn-secondary" onClick={() => navigateToTradeSetup()}>
          Get New Quote
        </button>
      </div>
    </div>
  );
};
```

### Execution Status Page

```typescript
// src/pages/ExecutionStatus.tsx
import React, { useState } from 'react';
import { useQuote } from '../hooks/useQuote';
import { usePolling } from '../hooks/usePolling';
import { useQuoteStore } from '../store/quoteStore';
import { API_ENDPOINTS } from '../api/endpoints';

export const ExecutionStatus: React.FC<{ quoteId: string }> = ({ quoteId }) => {
  const { checkStatus } = useQuote();
  const { execution, quote } = useQuoteStore();
  const [isComplete, setIsComplete] = useState(false);

  const { stop } = usePolling({
    interval: 5000, // Poll every 5 seconds
    enabled: !isComplete && !!quoteId,
    onPoll: async () => {
      const exec = await checkStatus(quoteId);

      // Determine if we should stop polling
      if (
        exec.status === 'completed' ||
        exec.status === 'failed' ||
        exec.status === 'expired'
      ) {
        setIsComplete(true);
        stop();
      }
    },
    maxAttempts: 120, // 10 minutes total
    onMaxAttemptsReached: () => {
      setIsComplete(true);
    },
  });

  if (!execution) {
    return <div>Loading...</div>;
  }

  const steps = [
    {
      title: 'Payment Received',
      status: execution.status === 'pending' ? 'pending' : 'complete',
      description: `Received ${quote?.maxFundingAmount} ${quote?.fundingAsset}`,
    },
    {
      title: 'Quote Committed',
      status: execution.status === 'committed' ? 'in-progress' : 
              (execution.status === 'pending' ? 'pending' : 'complete'),
      description: 'Quote locked and verified',
    },
    {
      title: 'Executing Swap',
      status: execution.status === 'executing' ? 'in-progress' :
              (execution.status === 'pending' || execution.status === 'committed' ? 'pending' : 'complete'),
      description: `Swapping on ${quote?.executionChain}`,
    },
    {
      title: 'Finalizing',
      status: execution.status === 'completed' ? 'complete' :
              (execution.status !== 'executing' ? 'pending' : 'in-progress'),
      description: 'Sending tokens to your wallet',
    },
  ];

  const progress = {
    pending: 0,
    committed: 25,
    executing: 50,
    completed: 100,
    failed: 0,
  }[execution.status];

  return (
    <div className="execution-status">
      <h2>Trade Execution</h2>

      {/* Progress Bar */}
      <div className="progress-container">
        <div className="progress-bar" style={{ width: `${progress}%` }} />
        <span className="progress-text">{progress}%</span>
      </div>

      {/* Steps */}
      <div className="steps">
        {steps.map((step, index) => (
          <div key={index} className={`step step-${step.status}`}>
            <div className="step-icon">
              {step.status === 'complete' && '✓'}
              {step.status === 'in-progress' && '⏳'}
              {step.status === 'pending' && '○'}
            </div>
            <div className="step-content">
              <h4>{step.title}</h4>
              <p>{step.description}</p>
            </div>
          </div>
        ))}
      </div>

      {/* Transaction Hashes */}
      {execution.fundingTxHash && (
        <div className="tx-info">
          <p>
            <strong>Funding TX:</strong>
            <a href={getExplorerUrl(quote?.fundingChain, execution.fundingTxHash)} target="_blank">
              {execution.fundingTxHash.slice(0, 20)}...
            </a>
          </p>
        </div>
      )}

      {execution.executionTxHash && (
        <div className="tx-info">
          <p>
            <strong>Execution TX:</strong>
            <a href={getExplorerUrl(quote?.executionChain, execution.executionTxHash)} target="_blank">
              {execution.executionTxHash.slice(0, 20)}...
            </a>
          </p>
        </div>
      )}

      {/* Error Display */}
      {execution.status === 'failed' && execution.errorMessage && (
        <div className="error-box">
          <h4>Trade Failed</h4>
          <p>{execution.errorMessage}</p>
          <button onClick={() => navigateToTradeSetup()}>Try Again</button>
        </div>
      )}

      {/* Completion Display */}
      {execution.status === 'completed' && (
        <div className="success-box">
          <h4>✓ Trade Complete!</h4>
          <p>You received {quote?.executionCost} {quote?.executionAsset}</p>
          <button onClick={() => navigateToPortfolio()}>View Portfolio</button>
        </div>
      )}
    </div>
  );
};

function getExplorerUrl(chain: string | undefined, txHash: string): string {
  const explorers: Record<string, string> = {
    Solana: 'https://explorer.solana.com/tx/',
    Stellar: 'https://stellar.expert/explorer/public/tx/',
    Near: 'https://explorer.near.org/transactions/',
  };
  return `${explorers[chain || 'Solana']}${txHash}`;
}
```

---

## Type Definitions

```typescript
// src/api/types.ts

// === REQUESTS ===

export interface RegisterWalletRequest {
  user_id: string;
  chain: 'Solana' | 'Stellar' | 'Near';
  address: string;
}

export interface CreateQuoteRequest {
  user_id: string;
  funding_chain: 'Solana' | 'Stellar' | 'Near';
  execution_chain: 'Solana' | 'Stellar' | 'Near';
  funding_asset: string;
  execution_asset: string;
  execution_instructions_base64: string;
  estimated_compute_units?: number;
}

export interface CreateApprovalRequest {
  quote_id: string;
  user_id: string;
}

// === RESPONSES ===

export interface RegisterWalletResponse {
  wallet_id: string;
  user_id: string;
  chain: string;
  address: string;
  status: 'unverified' | 'verified' | 'frozen';
}

export interface QuoteResponse {
  quote_id: string;
  user_id: string;
  funding_chain: string;
  execution_chain: string;
  funding_asset: string;
  execution_asset: string;
  max_funding_amount: string;
  execution_cost: string;
  service_fee: string;
  payment_address: string;
  expires_at: string; // ISO datetime
  nonce: string;
}

export interface StatusResponse {
  quote_id: string;
  funding_chain: string;
  execution_chain: string;
  status: 'pending' | 'committed' | 'executing' | 'completed' | 'failed' | 'expired';
  transaction_hash?: string;
  executed_at?: string;
  error_message?: string;
}

export interface HealthResponse {
  status: 'healthy' | 'degraded';
  timestamp: string;
  circuit_breakers: Array<{
    chain: string;
    active: boolean;
    reason?: string;
  }>;
}
```

---

## Environment Configuration

```bash
# .env.local
REACT_APP_API_URL=http://localhost:8080
REACT_APP_ENVIRONMENT=development
REACT_APP_LOG_LEVEL=debug

# Chain RPC endpoints (optional, for direct calls)
REACT_APP_SOLANA_RPC=https://api.mainnet-beta.solana.com
REACT_APP_STELLAR_HORIZON=https://horizon.stellar.org
REACT_APP_NEAR_RPC=https://rpc.mainnet.near.org
```

---

## Error Handling Best Practices

```typescript
// src/utils/errorHandler.ts
export type ApiErrorCode = 
  | 'RATE_LIMIT'
  | 'SERVICE_UNAVAILABLE'
  | 'INVALID_PARAMS'
  | 'DAILY_LIMIT_EXCEEDED'
  | 'QUOTE_EXPIRED'
  | 'EXECUTION_FAILED'
  | 'NETWORK_ERROR';

export interface ErrorMessage {
  code: ApiErrorCode;
  message: string;
  userMessage: string;
  recoveryAction?: string;
}

export const errorMap: Record<ApiErrorCode, ErrorMessage> = {
  RATE_LIMIT: {
    code: 'RATE_LIMIT',
    message: 'Rate limited',
    userMessage: 'Too many requests. Please try again in a few moments.',
    recoveryAction: 'Wait and retry',
  },
  SERVICE_UNAVAILABLE: {
    code: 'SERVICE_UNAVAILABLE',
    message: 'Service unavailable',
    userMessage: 'Service is temporarily unavailable. Please try again later.',
    recoveryAction: 'Check system status',
  },
  DAILY_LIMIT_EXCEEDED: {
    code: 'DAILY_LIMIT_EXCEEDED',
    message: 'Daily limit exceeded',
    userMessage: 'You have reached your daily trading limit. Please try again tomorrow.',
    recoveryAction: 'Wait until next day',
  },
  QUOTE_EXPIRED: {
    code: 'QUOTE_EXPIRED',
    message: 'Quote expired',
    userMessage: 'Your quote has expired. Please generate a new quote.',
    recoveryAction: 'Generate new quote',
  },
  EXECUTION_FAILED: {
    code: 'EXECUTION_FAILED',
    message: 'Execution failed',
    userMessage: 'The trade failed to execute. Please check the error details.',
    recoveryAction: 'Review error and retry',
  },
  INVALID_PARAMS: {
    code: 'INVALID_PARAMS',
    message: 'Invalid parameters',
    userMessage: 'Please check your input and try again.',
    recoveryAction: 'Fix inputs',
  },
  NETWORK_ERROR: {
    code: 'NETWORK_ERROR',
    message: 'Network error',
    userMessage: 'Unable to connect. Please check your internet connection.',
    recoveryAction: 'Check internet and retry',
  },
};

export function parseApiError(error: any): ErrorMessage {
  const status = error.response?.status;
  const data = error.response?.data;

  if (status === 429) {
    return errorMap.RATE_LIMIT;
  }
  if (status === 503) {
    return errorMap.SERVICE_UNAVAILABLE;
  }
  if (data?.error === 'RiskControl') {
    return errorMap.DAILY_LIMIT_EXCEEDED;
  }
  if (data?.error === 'InvalidParameters' && data?.message?.includes('expired')) {
    return errorMap.QUOTE_EXPIRED;
  }

  return {
    code: 'NETWORK_ERROR',
    message: error.message,
    userMessage: 'An unexpected error occurred.',
  };
}
```

---

This guide provides complete foundation for frontend implementation!
