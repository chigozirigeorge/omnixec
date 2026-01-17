#!/bin/bash
# Smart Contract Deployment Script for Crosschain Payments
# Deploys NEAR, Solana, and Stellar contracts to testnet

set -e

ACCOUNT_ID="omnixec.near"
NETWORK="testnet"

echo "🚀 Starting Smart Contract Deployment for Crosschain Payments"
echo "Account ID: $ACCOUNT_ID"
echo "Network: $NETWORK"
echo ""

# ============ NEAR DEPLOYMENT ============
echo "📦 [1/3] Building NEAR Contract..."
cd contracts/near-swap

# Clean and prepare
rm -f Cargo.lock
cargo clean

# Build (with patched dependencies if needed)
echo "⚙️  Compiling NEAR contract to WASM..."
cargo build --release --target wasm32-unknown-unknown

NEAR_WASM="target/wasm32-unknown-unknown/release/near_swap.wasm"
if [ -f "$NEAR_WASM" ]; then
    echo "✅ NEAR contract built: $NEAR_WASM"
    SIZE=$(du -h "$NEAR_WASM" | cut -f1)
    echo "   Size: $SIZE"
else
    echo "❌ NEAR contract build failed!"
    exit 1
fi

# Return to root
cd ../..

echo ""
echo "📝 NEAR Deployment Instructions:"
echo "================================"
echo "1. Deploy contract:"
echo "   near-cli-rs contract deploy $ACCOUNT_ID file $NEAR_WASM"
echo ""
echo "2. Initialize:"
echo "   near-cli-rs call $ACCOUNT_ID new json-args \\"
echo "     '{\"treasury\":\"treasury.$ACCOUNT_ID\",\"dex_contract\":\"ref-finance.$NETWORK\",\"fee_bps\":10}'"
echo ""
echo "3. Test:"
echo "   near-cli-rs view $ACCOUNT_ID get_treasury"
echo ""

# ============ SOLANA DEPLOYMENT ============
echo "📦 [2/3] Building Solana Program..."
cd contracts/solana

# Add wasm target if not present
rustup target add wasm32-unknown-unknown 2>/dev/null || true

echo "⚙️  Compiling Solana program to WASM..."
cargo build --release --target wasm32-unknown-unknown

SOLANA_SO="target/wasm32-unknown-unknown/release/solana_swap.so"
if [ ! -f "$SOLANA_SO" ]; then
    # Try alternate naming
    SOLANA_SO="target/wasm32-unknown-unknown/release/solana_swap"
    if [ ! -f "$SOLANA_SO" ]; then
        echo "❌ Solana program build failed!"
        exit 1
    fi
fi

echo "✅ Solana program built: $SOLANA_SO"
SIZE=$(du -h "$SOLANA_SO" | cut -f1)
echo "   Size: $SIZE"

cd ../..

echo ""
echo "📝 Solana Deployment Instructions:"
echo "=================================="
echo "1. Set cluster to testnet:"
echo "   solana config set --url https://api.testnet.solana.com"
echo ""
echo "2. Airdrop SOL (if needed):"
echo "   solana airdrop 2"
echo ""
echo "3. Deploy:"
echo "   solana deploy $SOLANA_SO --url https://api.testnet.solana.com"
echo ""
echo "4. Verify:"
echo "   solana account <PROGRAM_ID> --url https://api.testnet.solana.com"
echo ""

# ============ STELLAR DEPLOYMENT ============
echo "📦 [3/3] Building Stellar Soroban Contract..."
cd contracts/stellar

echo "⚙️  Compiling Stellar contract to WASM..."
cargo build --release --target wasm32-unknown-unknown

STELLAR_WASM="target/wasm32-unknown-unknown/release/stellar_swap.wasm"
if [ -f "$STELLAR_WASM" ]; then
    echo "✅ Stellar contract built: $STELLAR_WASM"
    SIZE=$(du -h "$STELLAR_WASM" | cut -f1)
    echo "   Size: $SIZE"
else
    echo "❌ Stellar contract build failed!"
    exit 1
fi

cd ../..

echo ""
echo "📝 Stellar Deployment Instructions:"
echo "===================================="
echo "1. Deploy to testnet:"
echo "   soroban contract deploy \\"
echo "     --wasm $STELLAR_WASM \\"
echo "     --source <your-stellar-account> \\"
echo "     --network testnet"
echo ""
echo "2. Note the CONTRACT_ID from deployment output"
echo ""
echo "3. Test:"
echo "   soroban contract invoke \\"
echo "     --id <CONTRACT_ID> \\"
echo "     --network testnet \\"
echo "     --source <your-account> \\"
echo "     --function get_treasury"
echo ""

# ============ SUMMARY ============
echo ""
echo "✅ All contracts successfully built!"
echo ""
echo "📋 SUMMARY"
echo "=========="
echo "NEAR Contract:    $NEAR_WASM"
echo "Solana Program:   $SOLANA_SO"
echo "Stellar Contract: $STELLAR_WASM"
echo ""
echo "🎯 Next Steps:"
echo "1. Follow the instructions above for each chain"
echo "2. Update .env with deployed contract addresses"
echo "3. Run integration tests against testnet"
echo "4. Monitor gas usage before mainnet deployment"
echo ""
