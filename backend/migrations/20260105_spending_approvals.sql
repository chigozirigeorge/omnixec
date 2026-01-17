-- Spending Approvals Table - User authorization for transactions
-- Stores user-signed approvals for spending transactions
-- SECURITY: Enables signature verification and replay protection

CREATE TABLE spending_approvals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- User and quote references
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    quote_id UUID NOT NULL REFERENCES quotes(id) ON DELETE CASCADE,
    
    -- Chain and asset information
    funding_chain chain_type NOT NULL,
    asset TEXT NOT NULL,
    
    -- Amounts (all in base units)
    approved_amount NUMERIC(78, 0) NOT NULL CHECK (approved_amount > 0),
    fee_amount NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (fee_amount >= 0),
    gas_amount NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (gas_amount >= 0),
    execution_amount NUMERIC(78, 0) NOT NULL CHECK (execution_amount > 0),
    
    -- Wallet information
    wallet_address TEXT NOT NULL,
    treasury_address TEXT NOT NULL,
    
    -- User's signature (will be verified on submission)
    user_signature TEXT,
    
    -- Replay protection
    nonce TEXT NOT NULL UNIQUE,
    
    -- Status
    is_used BOOLEAN NOT NULL DEFAULT false,
    used_at TIMESTAMP WITH TIME ZONE,
    
    -- Validity
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    
    -- Metadata
    ip_address INET,
    user_agent TEXT,
    
    CONSTRAINT valid_amounts CHECK (
        execution_amount = approved_amount - fee_amount - gas_amount
    ),
    CONSTRAINT signature_required_when_used CHECK (
        (is_used AND user_signature IS NOT NULL) OR NOT is_used
    )
);

-- Indexes for fast lookups
CREATE INDEX idx_spending_approvals_user ON spending_approvals(user_id);
CREATE INDEX idx_spending_approvals_quote ON spending_approvals(quote_id);
CREATE INDEX idx_spending_approvals_active ON spending_approvals(user_id, is_used, expires_at DESC)
    WHERE is_used = false;
CREATE INDEX idx_spending_approvals_nonce ON spending_approvals(nonce);
CREATE INDEX idx_spending_approvals_expires ON spending_approvals(expires_at DESC);
CREATE INDEX idx_spending_approvals_created ON spending_approvals(created_at DESC);

-- View for active (unused and non-expired) approvals
CREATE VIEW active_spending_approvals AS
SELECT 
    id,
    user_id,
    quote_id,
    funding_chain,
    asset,
    approved_amount,
    fee_amount,
    gas_amount,
    execution_amount,
    wallet_address,
    treasury_address,
    nonce,
    created_at,
    expires_at,
    EXTRACT(EPOCH FROM (expires_at - NOW()))::INTEGER AS seconds_until_expiry
FROM spending_approvals
WHERE is_used = false
  AND expires_at > NOW()
ORDER BY created_at DESC;

-- Trigger to update quote spending approval count (optional audit)
CREATE TRIGGER mark_approval_used_at BEFORE UPDATE ON spending_approvals
    FOR EACH ROW
    WHEN (OLD.is_used = false AND NEW.is_used = true)
    EXECUTE FUNCTION update_updated_at_column();
