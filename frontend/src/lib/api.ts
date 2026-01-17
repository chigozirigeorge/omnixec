import axios from 'axios';
import type {
  SpendingApprovalResponse,
  SubmitSpendingApprovalRequest,
  SubmitSpendingApprovalResponse,
  SpendingApprovalStatus,
  UserApprovalsResponse,
  CreateSpendingApprovalRequest,
} from '@/types/approval';
import type {
  SettlementStatus,
  ChainTreasuryStatus,
  AllTreasuriesStatus,
} from '@/types/settlement';

const API_BASE_URL = 'http://localhost:8080';

export const api = axios.create({
  baseURL: API_BASE_URL,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Types based on API documentation
export interface CreateQuoteRequest {
  user_id: string;
  funding_chain: 'solana' | 'stellar' | 'near';
  execution_chain: 'solana' | 'stellar' | 'near';
  funding_asset: string;
  execution_asset: string;
  execution_instructions_base64: string;
  estimated_compute_units?: number;
}

export interface Quote {
  quote_id: string;
  user_id: string;
  funding_chain: string;
  execution_chain: string;
  funding_asset: string;
  execution_asset: string;
  funding_amount: string;
  execution_amount: string;
  execution_cost: string;
  service_fee: string;
  max_funding_amount: string;
  status: QuoteStatus;
  expires_at: number;
  created_at: number;
}

export type QuoteStatus = 'Pending' | 'Committed' | 'Executed' | 'Failed' | 'Expired';

export interface CommitResponse {
  quote_id: string;
  status: string;
  message: string;
  execution_chain: string;
}

export interface QuoteStatusResponse {
  quote_id: string;
  status: QuoteStatus;
  funding_amount: string;
  execution_amount: string;
  funding_chain: string;
  execution_chain: string;
  created_at: number;
  executed_at?: number;
  transaction_hash?: string;
  execution_details?: {
    status: string;
    fee_paid: string;
    confirmation_time_secs: number;
  };
}

export interface OHLCData {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume?: number;
}

export interface HealthResponse {
  status: string;
  version: string;
  uptime: number;
}

export interface TreasuryResponse {
  total_value: string;
  chains: {
    chain: string;
    balance: string;
    assets: { symbol: string; amount: string }[];
  }[];
}

// ===== WALLET API =====
// src/lib/api.ts (update the walletApi section)
export const walletApi = {
  register: async (data: { user_id?: string; chain: string; address: string }) => {
    try {
      const response = await api.post('/api/v1/wallet/register', data);
      return response;
    } catch (error: any) {
      // If it's a 500 error, check if it's because the wallet is already registered
      if (error.response?.status === 500 && 
          error.response?.data?.message?.includes('already registered')) {
        // Return a success response since the wallet is already registered
        return { 
          data: { 
            status: 'registered', 
            user_id: data.user_id || null 
          } 
        };
      }
      throw error;
    }
  },
  
  verify: (params: {
    user_id: string;
    chain: string;
    address: string;
    nonce: string;
    signature: string;
    signed_message: string;
  }) => api.post('/api/v1/wallet/verify', params),
  
  getUserWallets: (user_id: string) =>
    api.get(`/api/v1/wallet/user/${user_id}`),
};

// ===== DISCOVERY API =====
export const discoveryApi = {
  getChains: () => api.get('/discovery/chains'),
  getChainInfo: (chain: string) => api.get(`/discovery/chain/${chain}`), // tokens + dexes
  getDexTokens: (dex: string, chain: string) => api.get(`/discovery/dex/${dex}/${chain}`),
};

// ===== QUOTE API =====
export const quoteApi = {
  /**
   * Create a cross-chain quote
   * Matches API_FLOW_GUIDE.md: POST /quote
   * Backend handles DEX routing, price calculation, and fee estimation
   */
  create: (data: CreateQuoteRequest) => 
    api.post<Quote>('/api/v1/quote', {
      user_id: data.user_id,
      funding_chain: data.funding_chain.charAt(0).toUpperCase() + data.funding_chain.slice(1),
      execution_chain: data.execution_chain.charAt(0).toUpperCase() + data.execution_chain.slice(1),
      funding_asset: data.funding_asset,
      execution_asset: data.execution_asset,
      execution_instructions_base64: data.execution_instructions_base64 || '',
      estimated_compute_units: data.estimated_compute_units,
    }),
  
  commit: (quoteId: string) => 
    api.post<CommitResponse>('/api/v1/commit', { quote_id: quoteId }),
  
  getStatus: (quoteId: string) => 
    api.get<QuoteStatusResponse>(`/api/v1/status/${quoteId}`),
};

// ===== SPENDING APPROVAL API =====
export const approvalApi = {
  /**
   * Create unsigned spending approval for user to sign on their device
   * User must then sign the nonce and submit it via submit()
   */
  create: (data: CreateSpendingApprovalRequest) =>
    api.post<SpendingApprovalResponse>('/api/v1/spending-approval/create', data),
  
  /**
   * Submit user's signed approval
   * This is the critical authorization step - verifies signature and marks approval as authorized
   */
  submit: (approvalId: string, signature: string) =>
    api.post<SubmitSpendingApprovalResponse>(
      `/api/v1/spending-approval/${approvalId}/submit`,
      { approval_id: approvalId, signature }
    ),
  
  /**
   * Get status of a spending approval
   */
  getStatus: (approvalId: string) =>
    api.get<SpendingApprovalStatus>(`/api/v1/spending-approval/${approvalId}`),
  
  /**
   * List all user's spending approvals (active and inactive)
   * Useful for audit trail and history
   */
  listUserApprovals: (userId: string) =>
    api.get<UserApprovalsResponse>(`/api/v1/spending-approval/user/${userId}`),
};

// ===== SETTLEMENT API =====
export const settlementApi = {
  /**
   * Get complete settlement information for a quote
   * Includes execution status and all recorded settlement transactions
   */
  getStatus: (quoteId: string) =>
    api.get<SettlementStatus>(`/api/v1/settlement/${quoteId}`),
};

// ===== TREASURY API =====
export const treasuryApi = {
  /**
   * Get all treasury balances with circuit breaker status
   */
  getAllBalances: () =>
    api.get<AllTreasuriesStatus>('/api/v1/admin/treasury'),
  
  /**
   * Get detailed treasury information for a specific chain
   * Includes daily limits, spending, and circuit breaker details
   */
  getChainStatus: (chain: 'solana' | 'stellar' | 'near') =>
    api.get<ChainTreasuryStatus>(`/api/v1/admin/treasury/${chain}`),
};

// ===== CHART API =====
export const chartApi = {
  /**
   * Get OHLC data for charts
   * Matches API_FLOW_GUIDE.md: GET /quote-engine/ohlc?asset=SOL&chain=Solana&timeframe=1h&limit=24
   */
  getOHLC: (asset: string, chain: string, timeframe: string, limit?: number) => {
    const chainName = chain.charAt(0).toUpperCase() + chain.slice(1);
    return api.get<OHLCData[]>(`/quote-engine/ohlc`, {
      params: {
        asset,
        chain: chainName,
        timeframe,
        limit: limit || 100,
      },
    });
  },
};

// ===== ADMIN API =====
export const adminApi = {
  getTreasury: () => api.get<TreasuryResponse>('/api/v1/admin/treasury'),
  getHealth: () => api.get<HealthResponse>('/api/v1/health'),
};
