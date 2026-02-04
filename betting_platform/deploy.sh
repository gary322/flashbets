#!/bin/bash
# deploy.sh - Main deployment script for Native Solana Implementation

set -e  # Exit on error

echo "=== BETTING PLATFORM NATIVE DEPLOYMENT ==="
echo "Network: $SOLANA_NETWORK"
echo "Deployer: $(solana address)"

# Build native program
echo "Building native program..."
cd programs/betting_platform_native
cargo build-sbf --manifest-path Cargo.toml

# Deploy program  
echo "Deploying native program..."
PROGRAM_ID=$(solana program deploy target/deploy/betting_platform_native.so --output json | jq -r '.programId')
echo "Program deployed at: $PROGRAM_ID"

# Initialize global config
echo "Initializing global config..."
anchor run initialize -- --program-id $PROGRAM_ID

# Burn upgrade authority
echo "Burning upgrade authority (making immutable)..."
solana program set-upgrade-authority $PROGRAM_ID \
  --new-upgrade-authority 11111111111111111111111111111111 \
  --keypair ~/.config/solana/id.json

# Verify immutability
echo "Verifying immutability..."
AUTHORITY=$(solana program show $PROGRAM_ID --output json | jq -r '.authority')
if [ "$AUTHORITY" = "11111111111111111111111111111111" ]; then
  echo "✓ Program is now immutable"
else
  echo "✗ ERROR: Program is not immutable! Authority: $AUTHORITY"
  exit 1
fi

# Save deployment info
echo "{
  \"programId\": \"$PROGRAM_ID\",
  \"deployedAt\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",
  \"network\": \"$SOLANA_NETWORK\",
  \"deployer\": \"$(solana address)\"
}" > deployment.json

echo "=== DEPLOYMENT COMPLETE ==="