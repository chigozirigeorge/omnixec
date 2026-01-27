# Omnixec - Cross-Chain Symmetric Payment Execution Platform

A comprehensive, multi-chain execution layer that enables seamless cryptocurrency payments and trades across Solana, Stellar, and NEAR blockchains with unified wallet management, intelligent quote routing, and advanced risk controls.

#RustAfricaHackathon

## Hackathon Scope & Project Status

OmniXec is being developed as a Rust-based prototype during the Rust Africa Hackathon.  
The focus of this submission is on backend architecture, cross-chain execution modeling, and safe financial infrastructure design.

Due to the limited hackathon timeframe:

- Some modules contain partial implementations
- Several functions are marked as TODO where interfaces and system flow are defined but full logic is still in progress
- Core architectural components such as intent routing, risk controls, settlement modeling, and execution adapters are implemented at a structural level

This project demonstrates how Rust can be used to build high-performance, secure, and modular infrastructure for cross-chain Web3 execution.


## 📋 Table of Contents

- [Project Overview](#project-overview)
- [Key Features](#key-features)
- [What Problems It Solves](#what-problems-it-solves)
- [Architecture](#architecture)
- [Technology Stack](#technology-stack)
- [Prerequisites](#prerequisites)
- [Installation & Setup](#installation--setup)
- [Configuration](#configuration)
- [Running the Project](#running-the-project)
- [API Documentation](#api-documentation)
- [Project Structure](#project-structure)
- [Smart Contracts](#smart-contracts)
- [Key Modules](#key-modules)
- [Database Schema](#database-schema)

---

## 🎯 Project Overview

**Omnixec** is a sophisticated backend execution platform designed to provide a unified interface for cross-chain cryptocurrency transactions. It abstracts the complexity of interacting with multiple blockchains (Solana, Stellar, and NEAR) behind a single, cohesive REST API.

The platform enables:
- **Multi-chain wallet management** with unified user experience
- **Intelligent quote routing** across multiple DEX protocols and chains
- **Automated trade execution** with signature management and submission
- **Risk-aware transaction handling** with daily limits and circuit breakers
- **Real-time settlement tracking** with blockchain webhook integration
- **Token approval workflows** for secure cross-chain interactions

### Mission
To democratize cross-chain trading by providing developers and users with a simple, secure, and robust interface to swap assets across different blockchains without deep blockchain expertise.

---

## 🌟 Key Features

### 1. **Multi-Chain Wallet Management**
   - Register and verify wallets across Solana, Stellar, and NEAR
   - Cryptographic signature verification for wallet ownership
   - User portfolio tracking with real-time balance updates
   - Support for multiple wallets per user per chain

### 2. **Intelligent Quote Engine**
   - Real-time price aggregation from multiple sources
   - Multi-path route discovery and optimization
   - Price impact calculation to ensure best execution
   - Quote expiration management (TTL-based)
   - OHLC (Open, High, Low, Close) chart data for technical analysis

### 3. **Advanced Trade Execution**
   - Automated transaction building and signing
   - Support for complex multi-step settlement processes
   - Real-time trade status tracking
   - Transaction history with detailed execution logs
   - Spending approval flows for secure token interactions

### 4. **Risk Management & Controls**
   - Per-chain daily outflow limits
   - Circuit breaker mechanisms to pause trading
   - Hourly outflow thresholds
   - Comprehensive risk assessment before execution
   - Audit trails for all transactions

### 5. **Settlement & Monitoring**
   - Real-time blockchain event monitoring via webhooks
   - Settlement status tracking
   - Cross-chain treasury management
   - Automatic payment verification
   - Treasury balance tracking across all chains

### 6. **Token Approval System**
   - Streamlined token approval workflows
   - Spending approval management
   - Secure approval tracking and verification
   - NFT approval support where applicable

---

## 🔧 What Problems It Solves

### Problem 1: **Blockchain Fragmentation**
**Challenge**: Users must interact with multiple blockchain networks, each with different protocols, wallets, and RPC providers.

**Solution**: Omnixec provides a unified API abstraction layer that handles all blockchain-specific logic internally, allowing developers to work with a single REST interface regardless of the underlying blockchain.

### Problem 2: **Complex Trade Routing**
**Challenge**: Finding the best price for a token swap across multiple chains and DEXs requires complex calculations and real-time price feeds.

**Solution**: The quote engine aggregates prices, calculates routes, and determines price impact automatically, returning the optimal execution path to the user.

### Problem 3: **Transaction Security & Signing**
**Challenge**: Managing private keys and signing transactions securely across multiple chains is complex and error-prone.

**Solution**: The platform implements secure signature management with treasury account architecture, ensuring private keys are never exposed to the frontend while maintaining cryptographic security.

### Problem 4: **Risk & Compliance**
**Challenge**: Organizations need to control exposure with daily limits, rate limiting, and transaction caps.

**Solution**: Omnixec provides comprehensive risk controls including per-chain limits, circuit breakers, and hourly thresholds that can be enforced before any transaction executes.

### Problem 5: **Settlement Verification**
**Challenge**: After initiating a blockchain transaction, tracking its confirmation and completion across chains is non-trivial.

**Solution**: The platform implements webhook-based monitoring for all three chains, automatically detecting payments and updating settlement status in real-time.

### Problem 6: **User Experience**
**Challenge**: Most users don't understand blockchain-specific concepts like wallet formats, network passphrases, and RPC URLs.

**Solution**: Omnixec provides high-level APIs for common operations (register wallet, get price, execute trade) that abstract away blockchain complexity.

---

## 🏗️ Architecture

### High-Level System Design

```
┌─────────────────────────────────────────────────────────────────┐
│                     FRONTEND APPLICATION                        │
│  (React - Pages for onboarding, discovery, trade, status)      │
└────────────────────────────┬────────────────────────────────────┘
                             │ HTTP REST APIs
                             ↓
┌─────────────────────────────────────────────────────────────────┐
│                    BACKEND API SERVER (Rust/Axum)              │
│ ┌──────────────────────────────────────────────────────────┐   │
│ │ CORE MODULES                                             │   │
│ │ ├─ Wallet Management                                     │   │
│ │ ├─ Quote Engine                                          │   │
│ │ ├─ Execution Router                                      │   │
│ │ ├─ Risk Controller                                       │   │
│ │ ├─ Settlement Tracker                                    │   │
│ │ └─ Token Approval Manager                                │   │
│ └──────────────────────────────────────────────────────────┘   │
│                             │                                    │
│  ┌──────────────────────────┼──────────────────────────────┐   │
│  ↓                          ↓                              ↓   │
│ ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐ │
│ │ PostgreSQL DB    │ │ Execution Layer  │ │ Event Monitors   │ │
│ │                  │ │ ├─ Solana        │ │ ├─ Tx Listeners  │ │
│ │ ├─ Users/Wallets │ │ ├─ Stellar       │ │ └─ Webhooks      │ │
│ │ ├─ Quotes        │ │ └─ NEAR          │ │                  │ │
│ │ └─ Trades        │ │                  │ │ (Detects payments)│
│ └──────────────────┘ └──────────────────┘ └──────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        ↓                    ↓                    ↓
   ┌─────────┐         ┌──────────┐         ┌──────────┐
   │ Solana  │         │ Stellar  │         │ NEAR     │
   │ Network │         │ Network  │         │ Network  │
   └─────────┘         └──────────┘         └──────────┘
```

### Key Design Principles

1. **Separation of Concerns**: Each module (wallet, quote_engine, execution, etc.) has a single responsibility
2. **Chain Abstraction**: Blockchain-specific logic is isolated in adapters
3. **Async-First**: Built on Tokio for high-performance async operations
4. **Type Safety**: Rust's type system ensures compile-time safety
5. **Error Handling**: Comprehensive error types with context
6. **Observability**: Structured logging and tracing throughout

---

## 🛠️ Technology Stack

### Backend
- **Language**: Rust (2021 edition)
- **Web Framework**: Axum 0.7 (async, tower-based)
- **Async Runtime**: Tokio (full features)
- **Database**: PostgreSQL with SQLx (compile-time checked queries)
- **HTTP Client**: Reqwest with JSON support
- **Blockchain SDKs**:
  - `solana-sdk` 3.0.0 & `solana-client` for Solana
  - `stellar-rs` 1.0.0 & `stellar-sdk` for Stellar
  - `near-jsonrpc-client` 0.20.0 for NEAR
- **Cryptography**: Ed25519 signing (Dalek), SHA-256
- **Serialization**: Serde + serde_json
- **Error Handling**: Thiserror, Anyhow
- **Observability**: Tracing & tracing-subscriber
- **Validation**: Validator crate
- **Rate Limiting**: Governor

### Frontend
- **Framework**: React 18 with TypeScript
- **Build Tool**: Vite
- **Styling**: Tailwind CSS
- **UI Components**: Radix UI
- **Wallet Integration**:
  - NEAR: `@near-wallet-selector/*`
  - Solana: `@solana/wallet-adapter-react`
- **State Management**: React Context
- **HTTP Client**: Native Fetch API
- **Form Handling**: React Hook Form

### Infrastructure
- **Database**: PostgreSQL (locally: localhost:5432)
- **API Communication**: HTTP REST with JSON
- **Blockchain Nodes**: RPC endpoints (Devnet for testing)
- **Monitoring**: Webhook-based event listeners

---

## 📋 Prerequisites

### Required
- **Rust** >= 1.70 ([Install](https://rustup.rs/))
- **Node.js** >= 18 ([Install](https://nodejs.org/))
- **PostgreSQL** >= 14 ([Install](https://www.postgresql.org/download/))
- **Cargo** (comes with Rust)

### Optional but Recommended
- **Docker** - for PostgreSQL containerization
- **Solana CLI** - for Solana development
- **NEAR CLI** - for NEAR contract interactions
- **git** - for version control

### System Requirements
- **OS**: Linux, macOS, or WSL2 on Windows
- **RAM**: Minimum 4GB, recommended 8GB+
- **Disk**: ~5GB for build artifacts and dependencies

---

## 🚀 Installation & Setup

### Step 1: Clone the Repository

```bash
git clone https://github.com/yourusername/omnixec.git
cd omnixec
```

### Step 2: Setup PostgreSQL Database

**Option A: Using Docker (Recommended)**

```bash
docker run --name omnixec-postgres \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=crosschain \
  -p 5432:5432 \
  -d postgres:15-alpine
```

**Option B: Local PostgreSQL Installation**

```bash
# macOS with Homebrew
brew install postgresql
brew services start postgresql

# Linux (Ubuntu/Debian)
sudo apt update
sudo apt install postgresql postgresql-contrib
sudo systemctl start postgresql

# Then create database
psql -U postgres -c "CREATE DATABASE crosschain;"
```

### Step 3: Setup Backend

```bash
cd backend

# Install Rust dependencies
cargo check

# Setup environment variables (see Configuration section)
cp .env.example .env
# Edit .env with your values

# Run database migrations
sqlx migrate run --database-url postgresql://postgres:postgres@localhost:5432/crosschain

# Verify setup
cargo build
```

### Step 4: Setup Frontend

```bash
cd ../frontend

# Install Node dependencies
npm install

# Create environment configuration
cp .env.example .env.local
# Edit .env.local if needed

# Build frontend
npm run build
```

### Step 5: Verify Installation

```bash
# Terminal 1: Start Backend
cd backend
cargo run

# Terminal 2: Start Frontend
cd frontend
npm run dev

# Terminal 3: Verify health
curl http://localhost:8080/health
```

Expected output:
```json
{
  "status": "healthy",
  "database": "connected",
  "timestamp": "2026-01-15T10:30:00Z"
}
```

---

## ⚙️ Configuration

### Backend Environment Variables

Create a `.env` file in the `backend/` directory:

```env
# ====== DATABASE ======
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/crosschain
BIND_ADDRESS=0.0.0.0:8080
RUST_LOG=info,sqlx=warn

# ====== SOLANA ======
SOLANA_TREASURY_KEY=<your-solana-secret-key-base58>
SOLANA_RPC_URL=https://api.devnet.solana.com

# ====== STELLAR ======
STELLAR_TREASURY_KEY=<your-stellar-secret-key>
STELLAR_HORIZON_URL=https://horizon-testnet.stellar.org
STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

# ====== NEAR ======
NEAR_ACCOUNT_ID=<your-near-account.testnet>
NEAR_TREASURY_KEY=<your-near-secret-key>
NEAR_RPC_URL=https://rpc.testnet.near.org

# ====== RISK CONTROLS (Testnet - Conservative) ======
STELLAR_DAILY_LIMIT=1000000
NEAR_DAILY_LIMIT=10000
SOLANA_DAILY_LIMIT=100
CIRCUIT_BREAKER_ENABLED=true
HOURLY_OUTFLOW_THRESHOLD=0.1

# ====== QUOTE ENGINE ======
SERVICE_FEE_RATE=0.001
QUOTE_TTL_SECONDS=300
```

### Key Configuration Sections

#### Database Configuration
- `DATABASE_URL`: PostgreSQL connection string
- Format: `postgresql://[user]:[password]@[host]:[port]/[database]`

#### Chain-Specific Configuration
Each blockchain requires:
- **RPC URL**: Endpoint to communicate with the network
- **Treasury Key**: Signing key for transaction submission
- **Network Identifier**: Specific network parameters (e.g., Stellar network passphrase)

#### Risk Controls
- `*_DAILY_LIMIT`: Maximum amount transferable per chain per day (in native units)
- `CIRCUIT_BREAKER_ENABLED`: Global kill switch for risk management
- `HOURLY_OUTFLOW_THRESHOLD`: Maximum percentage outflow per hour
- `SERVICE_FEE_RATE`: Commission rate (0.001 = 0.1%)

#### Quote Engine
- `QUOTE_TTL_SECONDS`: How long a quote remains valid
- `SERVICE_FEE_RATE`: Platform fee applied to all trades

### Frontend Environment Variables

Create a `.env.local` file in the `frontend/` directory:

```env
VITE_API_URL=http://localhost:8080
VITE_SOLANA_NETWORK=devnet
VITE_NEAR_NETWORK=testnet
VITE_STELLAR_NETWORK=testnet
```

---

## 🏃 Running the Project

### Development Mode

**Terminal 1 - Backend Server:**
```bash
cd backend
RUST_LOG=debug cargo run
```

**Terminal 2 - Frontend Dev Server:**
```bash
cd frontend
npm run dev
```

**Terminal 3 - Database Monitoring (Optional):**
```bash
psql postgresql://postgres:postgres@localhost:5432/crosschain
```

### Production Build

**Backend:**
```bash
cd backend
cargo build --release
./target/release/backend
```

**Frontend:**
```bash
cd frontend
npm run build
npm run preview
```

### Running Tests

```bash
# Backend unit tests
cd backend
cargo test

# Backend with logging
cargo test -- --nocapture

# Frontend tests (if configured)
cd frontend
npm test
```

### Database Migrations

```bash
# Check migration status
sqlx migrate info --database-url postgresql://postgres:postgres@localhost:5432/crosschain

# Run pending migrations
sqlx migrate run --database-url postgresql://postgres:postgres@localhost:5432/crosschain

# Revert last migration
sqlx migrate revert --database-url postgresql://postgres:postgres@localhost:5432/crosschain
```

---

## 📡 API Documentation

### Core Endpoints

#### Wallet Management

**Register Wallet**
```http
POST /wallet/register
Content-Type: application/json

{
  "user_id": "user_123",
  "chain": "solana",
  "address": "7qLLjHXcwENKXVVLNrKYNGcVq3qKCfzYxXfFjMt8J7h"
}
```

**Verify Wallet**
```http
POST /wallet/verify
Content-Type: application/json

{
  "wallet_id": "wallet_456",
  "signature": "base64_encoded_signature",
  "signed_message": "I own this wallet"
}
```

**Get Portfolio**
```http
GET /wallet/portfolio/{user_id}
```

#### Quote Engine

**Get Quote**
```http
POST /quote
Content-Type: application/json

{
  "user_id": "user_123",
  "from_chain": "solana",
  "to_chain": "stellar",
  "from_asset": "SOL",
  "to_asset": "USDC",
  "amount": "100.5"
}
```

**Get Best Route**
```http
POST /routes/find
Content-Type: application/json

{
  "from_chain": "solana",
  "to_chain": "near",
  "from_asset": "USDC",
  "to_asset": "USDT",
  "amount": "1000"
}
```

#### Trade Execution

**Initiate Trade**
```http
POST /trade/initiate
Content-Type: application/json

{
  "user_id": "user_123",
  "quote_id": "quote_789",
  "recipient_address": "stellar_account_address"
}
```

**Get Trade Status**
```http
GET /trade/status/{trade_id}
```

#### Risk & Limits

**Get Treasury Balances**
```http
GET /treasury/balances
```

**Get Chain Limits**
```http
GET /risk/limits/{chain}
```

#### Settlement

**Get Settlement Status**
```http
GET /settlement/status/{transaction_id}
```

**Webhook - Payment Received**
```http
POST /webhook/payment
Content-Type: application/json

{
  "chain": "solana",
  "transaction_id": "tx_hash",
  "from_address": "sender_address",
  "amount": "100.5",
  "timestamp": "2026-01-15T10:30:00Z"
}
```

---

## 📁 Project Structure

```
omnixec/
├── backend/                          # Rust backend
│   ├── src/
│   │   ├── main.rs                  # Entry point
│   │   ├── server.rs                # HTTP server setup
│   │   ├── bootstrap.rs             # App initialization
│   │   ├── config.rs                # Configuration loading
│   │   ├── error.rs                 # Error types
│   │   │
│   │   ├── api/                     # API handlers & models
│   │   │   ├── handler.rs           # Main request handlers
│   │   │   ├── models.rs            # Data models
│   │   │   ├── discovery.rs         # Asset & chain discovery
│   │   │   ├── token_approval.rs    # Token approval flows
│   │   │   ├── spending_approval.rs # Spending controls
│   │   │   └── notifications.rs     # Notification system
│   │   │
│   │   ├── adapters/                # Blockchain SDKs wrapping
│   │   │   ├── solana.rs            # Solana adapter
│   │   │   ├── stellar.rs           # Stellar adapter
│   │   │   └── near.rs              # NEAR adapter
│   │   │
│   │   ├── execution/               # Trade execution logic
│   │   │   ├── router.rs            # Route execution
│   │   │   ├── signature.rs         # Signature management
│   │   │   └── *.rs                 # Chain-specific executors
│   │   │
│   │   ├── quote_engine/            # Price calculation
│   │   │   └── *.rs                 # Quote generation logic
│   │   │
│   │   ├── risk/                    # Risk management
│   │   │   └── *.rs                 # Limits, circuit breakers
│   │   │
│   │   ├── settlement/              # Trade settlement
│   │   │   └── *.rs                 # Settlement tracking
│   │   │
│   │   ├── trading/                 # Trade history
│   │   │   └── *.rs                 # Trade records
│   │   │
│   │   ├── wallet/                  # Wallet management
│   │   │   └── *.rs                 # Wallet operations
│   │   │
│   │   ├── funding/                 # Treasury management
│   │   │   └── *.rs                 # Fund transfers
│   │   │
│   │   ├── ledger/                  # Event ledger
│   │   │   └── *.rs                 # Event tracking
│   │   │
│   │   ├── middleware/              # HTTP middleware
│   │   │   └── *.rs                 # Auth, logging, etc.
│   │   │
│   │   └── routes/                  # Route handlers
│   │       ├── quotes.rs            # Quote routes
│   │       ├── charts.rs            # Chart data routes
│   │       ├── trade.rs             # Trade routes
│   │       └── wallet.rs            # Wallet routes
│   │
│   ├── migrations/                  # SQL migrations
│   │   ├── 20251220171924_models.sql
│   │   ├── 20260105_spending_approvals.sql
│   │   └── 20260107_token_approvals.sql
│   │
│   ├── contracts/                   # Smart contracts
│   │   ├── solana-swap/             # Solana program
│   │   ├── stellar-swap/            # Stellar contract
│   │   └── near/                    # NEAR contract
│   │
│   ├── md/                          # Documentation
│   │   ├── SYSTEM_ARCHITECTURE.md
│   │   ├── API_DOCUMENTATION.md
│   │   ├── DEPLOYMENT_GUIDE.md
│   │   └── ... (20+ guides)
│   │
│   ├── Cargo.toml                   # Rust dependencies
│   ├── .env                         # Environment variables
│   └── .gitignore
│
├── frontend/                        # React frontend
│   ├── src/
│   │   ├── main.tsx                 # Entry point
│   │   ├── App.tsx                  # Root component
│   │   │
│   │   ├── pages/                   # Page components
│   │   ├── components/              # Reusable components
│   │   ├── contexts/                # React contexts
│   │   ├── hooks/                   # Custom hooks
│   │   ├── stores/                  # State management
│   │   ├── types/                   # TypeScript types
│   │   ├── lib/                     # Utilities
│   │   ├── assets/                  # Images, etc.
│   │   └── polyfills/               # Browser polyfills
│   │
│   ├── public/                      # Static files
│   ├── package.json                 # Node dependencies
│   ├── tsconfig.json                # TypeScript config
│   ├── vite.config.ts               # Vite build config
│   ├── tailwind.config.ts           # Tailwind config
│   └── index.html
│
├── README.md                        # This file
└── LICENSE
```

---

## 🎯 Smart Contracts

### Solana Program (`contracts/solana-swap/`)

**Purpose**: Atomic token swaps on Solana

**Key Instructions**:
- `initialize_pool`: Setup trading pool
- `swap`: Execute token swap
- `deposit_liquidity`: Add to pool
- `withdraw_liquidity`: Remove from pool

**Technologies**:
- Anchor framework for instruction definition
- SPL Token standard
- Associated Token Accounts

### Stellar Contract (`contracts/stellar-swap/`)

**Purpose**: Cross-asset swaps on Stellar

**Key Functions**:
- `initialize`: Setup contract
- `execute_swap`: Perform exchange
- `manage_trustlines`: Handle token relationships

**Standards**:
- Stellar CAP-0046 (smart contracts)
- Stellar asset standards

### NEAR Contract (`contracts/near/`)

**Purpose**: Multi-token swap execution on NEAR

**Key Methods**:
- `new()`: Initialize contract
- `swap()`: Execute token swap
- `add_liquidity()`: Provide liquidity
- `get_price()`: Query prices

**Standards**:
- NEP-141 (fungible token)
- NEP-245 (multi-fungible token)

---

## 🔧 Key Modules Explained

### 1. **Wallet Module** (`src/wallet/`)
Manages user wallet lifecycle:
- Registration and ownership verification
- Signature validation using blockchain-specific crypto
- Portfolio aggregation across all chains
- Balance tracking and updates

### 2. **Quote Engine** (`src/quote_engine/`)
Generates optimal trade quotes:
- Aggregates prices from multiple sources
- Calculates multi-hop routes
- Applies service fees
- Manages quote expiration (TTL)

### 3. **Execution Layer** (`src/execution/`)
Handles transaction submission:
- Signs transactions with treasury keys
- Submits to blockchain RPC
- Handles chain-specific transaction formats
- Manages nonces and gas fees

### 4. **Risk Module** (`src/risk/`)
Enforces trading limits:
- Daily outflow tracking per chain
- Circuit breaker activation
- Hourly rate limiting
- Pre-execution risk assessment

### 5. **Settlement** (`src/settlement/`)
Tracks trade completion:
- Listens for blockchain confirmations
- Processes webhook notifications
- Updates trade status
- Handles settlement failures

### 6. **Ledger** (`src/ledger/`)
Records all activities:
- Event logging
- Audit trail maintenance
- Analytics data collection

---

## 💾 Database Schema

### Key Tables

**users**
```sql
CREATE TABLE users (
  id UUID PRIMARY KEY,
  email VARCHAR(255) UNIQUE,
  created_at TIMESTAMP DEFAULT NOW(),
  updated_at TIMESTAMP DEFAULT NOW()
);
```

**wallets**
```sql
CREATE TABLE wallets (
  id UUID PRIMARY KEY,
  user_id UUID REFERENCES users(id),
  chain VARCHAR(50),
  address VARCHAR(255),
  verified BOOLEAN DEFAULT FALSE,
  created_at TIMESTAMP DEFAULT NOW()
);
```

**quotes**
```sql
CREATE TABLE quotes (
  id UUID PRIMARY KEY,
  user_id UUID REFERENCES users(id),
  from_asset VARCHAR(100),
  to_asset VARCHAR(100),
  from_amount DECIMAL(30, 8),
  to_amount DECIMAL(30, 8),
  expires_at TIMESTAMP,
  created_at TIMESTAMP DEFAULT NOW()
);
```

**trades**
```sql
CREATE TABLE trades (
  id UUID PRIMARY KEY,
  user_id UUID REFERENCES users(id),
  quote_id UUID REFERENCES quotes(id),
  status VARCHAR(50),
  tx_hash VARCHAR(255),
  created_at TIMESTAMP DEFAULT NOW(),
  settled_at TIMESTAMP
);
```

**spending_approvals**
```sql
CREATE TABLE spending_approvals (
  id UUID PRIMARY KEY,
  user_id UUID REFERENCES users(id),
  chain VARCHAR(50),
  token VARCHAR(255),
  amount DECIMAL(30, 8),
  created_at TIMESTAMP DEFAULT NOW()
);
```

---

## 📊 Monitoring & Observability

### Logging

Configure with `RUST_LOG`:
```bash
# Development (verbose)
RUST_LOG=debug,tower_http=debug cargo run

# Production (less noise)
RUST_LOG=info,sqlx=warn cargo run
```

### Health Checks

```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "database": "connected",
  "timestamp": "2026-01-15T10:30:00Z"
}
```

### Performance Metrics

The system includes:
- **Request tracing** via tower_http
- **Database query logging** via SQLx
- **Structured logging** via tracing
- **Rate limiting** via Governor

---

## 🔐 Security Considerations

### Key Management
- Treasury keys stored in environment variables (not in code)
- Signatures validated cryptographically before submission
- Private keys never exposed to frontend

### Transaction Safety
- All transactions validated before submission
- Double-spend prevention via nonce tracking
- Circuit breakers to prevent excessive outflows
- Comprehensive audit trails

### API Security
- CORS configured appropriately
- Rate limiting via Governor
- Input validation on all endpoints
- Error messages don't leak sensitive info

---

## 🐛 Troubleshooting

### Common Issues

**1. "DATABASE_URL not set"**
```bash
# Solution: Set environment variable
export DATABASE_URL=postgresql://postgres:postgres@localhost:5432/crosschain
```

**2. "Connection refused" to PostgreSQL**
```bash
# Check if PostgreSQL is running
docker ps | grep postgres

# Or restart:
docker start omnixec-postgres
```

**3. Port 8080 already in use**
```bash
# Use different port
BIND_ADDRESS=0.0.0.0:3000 cargo run
```

**4. Migration errors**
```bash
# Check migration status
sqlx migrate info

# Rollback and re-run
sqlx migrate revert
sqlx migrate run
```

---

## 📚 Additional Resources

- [API Documentation](./backend/md/API_DOCUMENTATION.md)
- [System Architecture](./backend/md/SYSTEM_ARCHITECTURE.md)
- [Deployment Guide](./backend/md/DEPLOYMENT_GUIDE.md)
- [Smart Contract Guide](./backend/md/COMPLETE_PYTH_SMART_CONTRACT_GUIDE.md)
- [Token Approval Flow](./backend/md/TOKEN_APPROVAL_FLOW.md)
- [Audit Report](./backend/md/SECURITY_AUDIT_REPORT.md)

---

## 🤝 Contributing

1. Create a feature branch: `git checkout -b feature/amazing-feature`
2. Make your changes and commit: `git commit -am 'Add amazing feature'`
3. Push to the branch: `git push origin feature/amazing-feature`
4. Open a Pull Request

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🚀 Next Steps

1. **Complete Setup**: Follow the [Installation & Setup](#installation--setup) section
2. **Review Architecture**: Read [System Architecture](./backend/md/SYSTEM_ARCHITECTURE.md)
3. **Test API**: Use the [API Documentation](#api-documentation) endpoints
4. **Deploy**: Follow the [Deployment Guide](./backend/md/DEPLOYMENT_GUIDE.md)

---

## 📞 Support

For issues and questions:
- Open an issue on GitHub
- Review existing documentation in `backend/md/`
- Check API documentation for endpoint details

---

**Last Updated**: January 2026  
**Version**: 0.1.0  
**Status**: Active Development
