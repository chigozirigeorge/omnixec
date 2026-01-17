/**
 * Settlement Types
 * Tracks cross-chain transaction settlement status and records
 */

export interface SettlementRecord {
  settlement_id: string;
  chain: 'solana' | 'stellar' | 'near';
  transaction_hash: string;
  amount: string;
  settled_at: string;
  verified_at: string | null;
}

export interface SettlementStatus {
  quote_id: string;
  status: 'pending' | 'committed' | 'executed' | 'failed' | 'expired' | 'settled';
  execution_chain: 'solana' | 'stellar' | 'near';
  funding_chain: 'solana' | 'stellar' | 'near';
  execution_cost: string;
  max_funding_amount: string;
  service_fee: string;
  settlement_records: SettlementRecord[];
  created_at: string;
  expires_at: string;
}

export interface ChainTreasuryStatus {
  chain: 'solana' | 'stellar' | 'near';
  asset: string;
  balance: string;
  daily_limit: string;
  daily_spending: string;
  daily_remaining: string;
  daily_transaction_count: number;
  circuit_breaker: {
    active: boolean;
    reason: string | null;
    triggered_at: string | null;
  };
  last_updated: string;
}

export interface AllTreasuriesStatus {
  treasuries: {
    chain: 'solana' | 'stellar' | 'near';
    asset: string;
    balance: string;
    circuit_breaker_active: boolean;
    last_updated: string;
  }[];
  total_chains: number;
  timestamp: string;
}
