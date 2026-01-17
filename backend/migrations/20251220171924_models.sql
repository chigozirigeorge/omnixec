-- Add migration script here
-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Universal chain type - any chain can be funding or execution
CREATE TYPE chain_type AS ENUM ('solana', 'stellar', 'near');

-- Users table (symmetric - support all chains)
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    solana_address TEXT,
    stellar_address TEXT,
    near_address TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_solana UNIQUE (solana_address),
    CONSTRAINT unique_stellar UNIQUE (stellar_address),
    CONSTRAINT unique_near UNIQUE (near_address),
    CONSTRAINT at_least_one_address CHECK (
        solana_address IS NOT NULL OR 
        stellar_address IS NOT NULL OR 
        near_address IS NOT NULL
    )
);

CREATE INDEX idx_users_solana ON users(solana_address) WHERE solana_address IS NOT NULL;
CREATE INDEX idx_users_stellar ON users(stellar_address) WHERE stellar_address IS NOT NULL;
CREATE INDEX idx_users_near ON users(near_address) WHERE near_address IS NOT NULL;

-- Balances table (per chain, per asset)
CREATE TABLE balances (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    chain chain_type NOT NULL,
    asset TEXT NOT NULL,
    amount NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (amount >= 0),
    locked_amount NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (locked_amount >= 0),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, chain, asset),
    CONSTRAINT valid_locked_amount CHECK (locked_amount <= amount)
);

CREATE INDEX idx_balances_user ON balances(user_id);
CREATE INDEX idx_balances_chain ON balances(chain);

-- Quotes table (symmetric cross-chain)
CREATE TYPE quote_status AS ENUM ('pending', 'committed', 'executed', 'expired', 'failed');

CREATE TABLE quotes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- CRITICAL: Symmetric chain pair (must be different)
    funding_chain chain_type NOT NULL,
    execution_chain chain_type NOT NULL,
    
    -- Assets (chain-specific)
    funding_asset TEXT NOT NULL,
    execution_asset TEXT NOT NULL,
    
    -- Amounts
    max_funding_amount NUMERIC(78, 0) NOT NULL CHECK (max_funding_amount > 0),
    execution_cost NUMERIC(78, 0) NOT NULL CHECK (execution_cost > 0),
    service_fee NUMERIC(78, 0) NOT NULL CHECK (service_fee >= 0),
    
    -- Execution payload (chain-agnostic bytes)
    execution_instructions BYTEA NOT NULL,
    estimated_compute_units INTEGER,
    
    -- Metadata
    nonce TEXT NOT NULL UNIQUE,
    status quote_status NOT NULL DEFAULT 'pending',
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    payment_address TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    -- SECURITY: Enforce different chains for cross-chain execution
    CONSTRAINT different_chains CHECK (funding_chain != execution_chain)
);

CREATE INDEX idx_quotes_user ON quotes(user_id);
CREATE INDEX idx_quotes_status ON quotes(status);
CREATE INDEX idx_quotes_funding_chain ON quotes(funding_chain);
CREATE INDEX idx_quotes_execution_chain ON quotes(execution_chain);
CREATE INDEX idx_quotes_chain_pair ON quotes(funding_chain, execution_chain);
CREATE INDEX idx_quotes_expires ON quotes(expires_at);
CREATE INDEX idx_quotes_nonce ON quotes(nonce);
CREATE INDEX idx_quotes_created ON quotes(created_at DESC);

-- Executions table (chain-agnostic)
CREATE TYPE execution_status AS ENUM ('pending', 'success', 'failed');

CREATE TABLE executions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    quote_id UUID NOT NULL REFERENCES quotes(id) ON DELETE CASCADE UNIQUE,
    
    -- Which chain was this executed on?
    execution_chain chain_type NOT NULL,
    
    -- Chain-specific transaction identifier
    transaction_hash TEXT,
    
    status execution_status NOT NULL DEFAULT 'pending',
    gas_used NUMERIC(78, 0),
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    executed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE,
    
    CONSTRAINT tx_hash_required_on_success CHECK (
        (status = 'success' AND transaction_hash IS NOT NULL) OR status != 'success'
    )
);

CREATE UNIQUE INDEX idx_executions_quote ON executions(quote_id);
CREATE INDEX idx_executions_status ON executions(status);
CREATE INDEX idx_executions_chain ON executions(execution_chain);
CREATE INDEX idx_executions_tx_hash ON executions(transaction_hash) WHERE transaction_hash IS NOT NULL;
CREATE INDEX idx_executions_executed ON executions(executed_at DESC);

-- Settlements table (records funding chain payment)
CREATE TABLE settlements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    execution_id UUID NOT NULL REFERENCES executions(id) ON DELETE CASCADE,
    
    -- Which chain was funding from?
    funding_chain chain_type NOT NULL,
    funding_txn_hash TEXT NOT NULL,
    funding_amount NUMERIC(78, 0) NOT NULL,
    
    settled_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    verified_at TIMESTAMP WITH TIME ZONE,
    
    CONSTRAINT unique_settlement_per_execution UNIQUE (execution_id)
);

CREATE INDEX idx_settlements_execution ON settlements(execution_id);
CREATE INDEX idx_settlements_funding_chain ON settlements(funding_chain);
CREATE INDEX idx_settlements_funding_txn ON settlements(funding_txn_hash);
CREATE INDEX idx_settlements_settled ON settlements(settled_at DESC);

-- Treasury balances table (per chain, per asset)
CREATE TABLE treasury_balances (
    chain chain_type NOT NULL,
    asset TEXT NOT NULL,
    balance NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (balance >= 0),
    last_updated TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_reconciled TIMESTAMP WITH TIME ZONE,
    PRIMARY KEY (chain, asset)
);

-- Initialize native asset for each chain
INSERT INTO treasury_balances (chain, asset) VALUES 
    ('solana', 'SOL'),
    ('stellar', 'XLM'),
    ('near', 'NEAR');

-- Daily spending limits tracking (per chain)
CREATE TABLE daily_spending (
    chain chain_type NOT NULL,
    date DATE NOT NULL,
    amount_spent NUMERIC(78, 0) NOT NULL DEFAULT 0 CHECK (amount_spent >= 0),
    transaction_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (chain, date)
);

CREATE INDEX idx_daily_spending_date ON daily_spending(date DESC);

-- Circuit breaker state (per chain)
CREATE TABLE circuit_breaker_state (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    chain chain_type NOT NULL,
    triggered_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    reason TEXT NOT NULL,
    resolved_at TIMESTAMP WITH TIME ZONE,
    resolved_by TEXT
);

CREATE INDEX idx_circuit_breaker_chain ON circuit_breaker_state(chain);
CREATE INDEX idx_circuit_breaker_active ON circuit_breaker_state(chain, triggered_at DESC)
    WHERE resolved_at IS NULL;

-- Audit log for critical operations
CREATE TYPE audit_event_type AS ENUM (
    'quote_created',
    'quote_committed',
    'execution_started',
    'execution_completed',
    'execution_failed',
    'settlement_recorded',
    'circuit_breaker_triggered',
    'circuit_breaker_reset',
    'limit_exceeded'
);

CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type audit_event_type NOT NULL,
    chain chain_type,
    entity_id UUID,
    user_id UUID,
    details JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_log_event_type ON audit_log(event_type);
CREATE INDEX idx_audit_log_chain ON audit_log(chain) WHERE chain IS NOT NULL;
CREATE INDEX idx_audit_log_entity ON audit_log(entity_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at DESC);

-- Chain pair configuration (which pairs are supported)
CREATE TABLE supported_chain_pairs (
    funding_chain chain_type NOT NULL,
    execution_chain chain_type NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    min_amount NUMERIC(78, 0) NOT NULL DEFAULT 0,
    max_amount NUMERIC(78, 0),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (funding_chain, execution_chain),
    CONSTRAINT different_chains_pair CHECK (funding_chain != execution_chain)
);

-- Initialize all supported pairs
INSERT INTO supported_chain_pairs (funding_chain, execution_chain) VALUES
    ('stellar', 'solana'),
    ('stellar', 'near'),
    ('solana', 'stellar'),
    ('solana', 'near'),
    ('near', 'stellar'),
    ('near', 'solana');

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for updated_at
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_balances_updated_at BEFORE UPDATE ON balances
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_quotes_updated_at BEFORE UPDATE ON quotes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Function to validate quote chain pair on insert/update
CREATE OR REPLACE FUNCTION validate_quote_chain_pair()
RETURNS TRIGGER AS $$
BEGIN
    -- Check if chain pair is supported
    IF NOT EXISTS (
        SELECT 1 FROM supported_chain_pairs
        WHERE funding_chain = NEW.funding_chain
          AND execution_chain = NEW.execution_chain
          AND enabled = true
    ) THEN
        RAISE EXCEPTION 'Chain pair % -> % is not supported',
            NEW.funding_chain, NEW.execution_chain;
    END IF;
    
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER validate_quote_chains BEFORE INSERT OR UPDATE ON quotes
    FOR EACH ROW EXECUTE FUNCTION validate_quote_chain_pair();

-- View for active circuit breakers
CREATE VIEW active_circuit_breakers AS
SELECT 
    chain,
    triggered_at,
    reason,
    NOW() - triggered_at AS duration
FROM circuit_breaker_state
WHERE resolved_at IS NULL
ORDER BY triggered_at DESC;

-- View for daily spending summary
CREATE VIEW daily_spending_summary AS
SELECT 
    date,
    chain,
    amount_spent,
    transaction_count,
    amount_spent::float / transaction_count AS avg_per_transaction
FROM daily_spending
WHERE date >= CURRENT_DATE - INTERVAL '30 days'
ORDER BY date DESC, chain;
