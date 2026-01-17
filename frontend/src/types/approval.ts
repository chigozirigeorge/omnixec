/**
 * Spending Approval Types
 * Handles user's authorization to spend tokens across chains
 */

export interface CreateSpendingApprovalRequest {
  quote_id: string;
  approved_amount: string;
  wallet_address: string;
}

export interface SpendingApprovalResponse {
  id: string;
  user_id: string;
  funding_chain: 'solana' | 'stellar' | 'near';
  approved_amount: string;
  fee_amount: string;
  gas_amount: string;
  execution_amount: string;
  asset: string;
  quote_id: string;
  wallet_address: string;
  treasury_address: string;
  is_used: boolean;
  created_at: string;
  expires_at: string;
  nonce: string;
}

export interface SubmitSpendingApprovalRequest {
  approval_id: string;
  signature: string;
}

export interface SubmitSpendingApprovalResponse {
  approval_id: string;
  quote_id: string;
  status: 'authorized';
  message: string;
  authorized_amount: string;
  authorized_at: string;
  asset: string;
  chain: 'solana' | 'stellar' | 'near';
}

export interface SpendingApprovalStatus {
  id: string;
  user_id: string;
  funding_chain: 'solana' | 'stellar' | 'near';
  approved_amount: string;
  fee_amount: string;
  gas_amount: string;
  execution_amount: string;
  asset: string;
  quote_id: string;
  wallet_address: string;
  treasury_address: string;
  is_used: boolean;
  created_at: string;
  expires_at: string;
  nonce: string;
}

export interface UserApprovalsResponse {
  user_id: string;
  count: number;
  approvals: SpendingApprovalStatus[];
  fetched_at: string;
}
