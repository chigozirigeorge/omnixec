-- Token Approvals Table - User-signed token transfer approvals
-- SECURITY: Enables signature verification and replay protection for token transfers
-- This replaces manual payment transfers with cryptographic verification

CREATE TABLE token_approvals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- References
    quote_id UUID NOT NULL REFERENCES quotes(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Chain and token information
    funding_chain chain_type NOT NULL,
    token TEXT NOT NULL,
    
    -- Amount in base units of token
    amount NUMERIC(78, 0) NOT NULL CHECK (amount > 0),
    
    -- Recipient address (treasury address)
    recipient TEXT NOT NULL,
    
    -- User's wallet address
    user_wallet TEXT NOT NULL,
    
    -- Message user signed (for verification)
    message TEXT NOT NULL,
    
    -- User's signature (base64 encoded)
    signature TEXT,
    
    -- Replay protection
    nonce TEXT NOT NULL UNIQUE,
    
    -- Status tracking
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    -- enum: 'pending', 'signed', 'submitted', 'confirmed', 'executed', 'failed', 'expired', 'cancelled'
    
    -- Blockchain data
    transaction_hash VARCHAR(255),
    block_height BIGINT,
    confirmation_status VARCHAR(50),
    -- enum: 'Processed', 'Confirmed', 'Finalized'
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    submitted_at TIMESTAMP WITH TIME ZONE,
    confirmed_at TIMESTAMP WITH TIME ZONE,
    executed_at TIMESTAMP WITH TIME ZONE,
    failed_at TIMESTAMP WITH TIME ZONE,
    
    -- Error handling
    error_message TEXT,
    error_code VARCHAR(50),
    retry_count INT DEFAULT 0,
    last_retry_at TIMESTAMP WITH TIME ZONE,
    
    -- Constraints
    CONSTRAINT valid_status CHECK (status IN (
        'pending', 'signed', 'submitted', 'confirmed', 'executed', 'failed', 'expired', 'cancelled'
    )),
    CONSTRAINT signature_required_when_signed CHECK (
        (status = 'signed' AND signature IS NOT NULL) OR status = 'pending'
    ),
    CONSTRAINT confirmation_requires_tx CHECK (
        (status IN ('submitted', 'confirmed', 'executed') AND transaction_hash IS NOT NULL) 
        OR status IN ('pending', 'signed', 'failed', 'cancelled', 'expired')
    )
);

-- Indexes for performance
CREATE INDEX idx_token_approvals_user_id ON token_approvals(user_id);
CREATE INDEX idx_token_approvals_quote_id ON token_approvals(quote_id);
CREATE INDEX idx_token_approvals_status ON token_approvals(status);
CREATE INDEX idx_token_approvals_nonce ON token_approvals(nonce);
CREATE INDEX idx_token_approvals_wallet ON token_approvals(user_wallet);
CREATE INDEX idx_token_approvals_expires ON token_approvals(expires_at) 
WHERE status IN ('pending', 'submitted');
CREATE INDEX idx_token_approvals_tx_hash ON token_approvals(transaction_hash) 
WHERE transaction_hash IS NOT NULL;

-- Comment
COMMENT ON TABLE token_approvals IS 'Token transfer approvals with cryptographic signature verification. Replaces manual payment transfers.';
COMMENT ON COLUMN token_approvals.nonce IS 'Unique nonce for each approval to prevent replay attacks';
COMMENT ON COLUMN token_approvals.message IS 'Message that user signed to authorize the transfer';
COMMENT ON COLUMN token_approvals.signature IS 'User signature (base64) - verified against user_wallet public key';
