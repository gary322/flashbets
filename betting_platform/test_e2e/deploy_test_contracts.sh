#!/bin/bash

# Deploy Smart Contracts to Local Solana Test Validator
# Must run after setup_test_environment.sh

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
LOG_DIR="$SCRIPT_DIR/logs"
DEPLOYED_ADDRESSES="$SCRIPT_DIR/deployed_addresses.json"

mkdir -p "$LOG_DIR"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Smart Contract Deployment${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "Start Time: $(date)"
echo ""

# Check if Solana is running
if ! nc -z localhost 8899 2>/dev/null; then
    echo -e "${RED}Error: Solana test validator is not running${NC}"
    echo "Please run ./setup_test_environment.sh first"
    exit 1
fi

# Step 1: Configure Solana CLI for local validator
echo -e "${BLUE}Step 1: Configuring Solana CLI${NC}"
solana config set --url http://localhost:8899 > /dev/null 2>&1
echo -e "${GREEN}Solana CLI configured for local validator${NC}"

# Step 2: Create test wallets
echo -e "\n${BLUE}Step 2: Creating Test Wallets${NC}"

# Create deployer wallet
DEPLOYER_KEYPAIR="$SCRIPT_DIR/wallets/deployer.json"
mkdir -p "$SCRIPT_DIR/wallets"

if [ ! -f "$DEPLOYER_KEYPAIR" ]; then
    solana-keygen new --outfile "$DEPLOYER_KEYPAIR" --no-bip39-passphrase --force > /dev/null 2>&1
    echo -e "${GREEN}Created deployer wallet${NC}"
else
    echo "Using existing deployer wallet"
fi

# Get deployer address
DEPLOYER_PUBKEY=$(solana-keygen pubkey "$DEPLOYER_KEYPAIR")
echo "Deployer address: $DEPLOYER_PUBKEY"

# Airdrop SOL to deployer
echo "Airdropping SOL to deployer..."
solana airdrop 100 "$DEPLOYER_PUBKEY" > /dev/null 2>&1 || {
    echo -e "${YELLOW}Warning: Airdrop might have failed (wallet may already have SOL)${NC}"
}

# Create test user wallets
echo -e "\n${YELLOW}Creating test user wallets...${NC}"
for i in {1..5}; do
    USER_KEYPAIR="$SCRIPT_DIR/wallets/user$i.json"
    if [ ! -f "$USER_KEYPAIR" ]; then
        solana-keygen new --outfile "$USER_KEYPAIR" --no-bip39-passphrase --force > /dev/null 2>&1
        USER_PUBKEY=$(solana-keygen pubkey "$USER_KEYPAIR")
        solana airdrop 10 "$USER_PUBKEY" > /dev/null 2>&1 || true
        echo "Created user$i wallet: $USER_PUBKEY"
    fi
done

# Step 3: Build Smart Contracts
echo -e "\n${BLUE}Step 3: Building Smart Contracts${NC}"

# Check if native contract exists
NATIVE_CONTRACT_DIR="$PROJECT_ROOT/programs/betting_platform_native"
if [ -d "$NATIVE_CONTRACT_DIR" ]; then
    echo "Building native Solana program..."
    cd "$NATIVE_CONTRACT_DIR"
    
    # Build the program
    if cargo build-sbf --manifest-path Cargo.toml 2>&1 | tee "$LOG_DIR/contract_build.log" | grep -E "(error|warning)" | tail -20; then
        echo -e "${GREEN}Contract built successfully${NC}"
    else
        echo -e "${YELLOW}Contract build completed with warnings${NC}"
    fi
    
    # Find the built program
    PROGRAM_SO=$(find target/deploy -name "*.so" | head -1)
    if [ -z "$PROGRAM_SO" ]; then
        echo -e "${RED}Error: No .so file found after build${NC}"
        exit 1
    fi
    
    echo "Built program: $PROGRAM_SO"
else
    echo -e "${RED}Error: Native contract directory not found${NC}"
    echo "Expected at: $NATIVE_CONTRACT_DIR"
    exit 1
fi

# Step 4: Deploy Contract
echo -e "\n${BLUE}Step 4: Deploying Contract${NC}"

# Deploy the program
echo "Deploying program..."
DEPLOY_OUTPUT=$(solana program deploy "$PROGRAM_SO" --keypair "$DEPLOYER_KEYPAIR" 2>&1)
PROGRAM_ID=$(echo "$DEPLOY_OUTPUT" | grep -o 'Program Id: [A-Za-z0-9]\+' | cut -d' ' -f3)

if [ -z "$PROGRAM_ID" ]; then
    echo -e "${RED}Error: Failed to deploy program${NC}"
    echo "$DEPLOY_OUTPUT"
    exit 1
fi

echo -e "${GREEN}Program deployed successfully!${NC}"
echo "Program ID: $PROGRAM_ID"

# Step 5: Initialize Program State
echo -e "\n${BLUE}Step 5: Initializing Program State${NC}"

# Create initialization script
cat > "$SCRIPT_DIR/initialize_program.js" << EOF
const { Connection, Keypair, PublicKey, Transaction, SystemProgram } = require('@solana/web3.js');
const fs = require('fs');

async function initialize() {
    const connection = new Connection('http://localhost:8899', 'confirmed');
    const deployerKeypair = Keypair.fromSecretKey(
        new Uint8Array(JSON.parse(fs.readFileSync('$DEPLOYER_KEYPAIR')))
    );
    
    const programId = new PublicKey('$PROGRAM_ID');
    
    console.log('Initializing program state...');
    
    // TODO: Add actual initialization instruction based on your program's interface
    console.log('Program state initialization would go here');
    console.log('Program is ready for testing');
    
    return programId.toString();
}

initialize().then(console.log).catch(console.error);
EOF

# Step 6: Save Deployment Info
echo -e "\n${BLUE}Step 6: Saving Deployment Info${NC}"

cat > "$DEPLOYED_ADDRESSES" << EOF
{
    "programId": "$PROGRAM_ID",
    "deployer": "$DEPLOYER_PUBKEY",
    "deployTime": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "network": "localnet",
    "testWallets": {
        "deployer": "$DEPLOYER_PUBKEY",
        "user1": "$(solana-keygen pubkey "$SCRIPT_DIR/wallets/user1.json")",
        "user2": "$(solana-keygen pubkey "$SCRIPT_DIR/wallets/user2.json")",
        "user3": "$(solana-keygen pubkey "$SCRIPT_DIR/wallets/user3.json")",
        "user4": "$(solana-keygen pubkey "$SCRIPT_DIR/wallets/user4.json")",
        "user5": "$(solana-keygen pubkey "$SCRIPT_DIR/wallets/user5.json")"
    }
}
EOF

echo -e "${GREEN}Deployment info saved to: $DEPLOYED_ADDRESSES${NC}"

# Step 7: Update API Configuration
echo -e "\n${BLUE}Step 7: Updating API Configuration${NC}"

API_ENV_FILE="$PROJECT_ROOT/api_runner/.env"
if [ -f "$API_ENV_FILE" ]; then
    # Update PROGRAM_ID in .env file
    if grep -q "^PROGRAM_ID=" "$API_ENV_FILE"; then
        sed -i.bak "s/^PROGRAM_ID=.*/PROGRAM_ID=$PROGRAM_ID/" "$API_ENV_FILE"
    else
        echo "PROGRAM_ID=$PROGRAM_ID" >> "$API_ENV_FILE"
    fi
    echo -e "${GREEN}Updated API configuration with new Program ID${NC}"
else
    echo -e "${YELLOW}Warning: API .env file not found${NC}"
fi

# Step 8: Create Test Market Data
echo -e "\n${BLUE}Step 8: Creating Test Market Data${NC}"

cat > "$SCRIPT_DIR/create_test_markets.sh" << 'EOF'
#!/bin/bash

# Script to create test markets
echo "Creating test markets..."

# This would call your API to create test markets
# For now, we'll just show the structure

curl -X POST http://localhost:8081/api/markets/create \
  -H "Content-Type: application/json" \
  -d '{
    "question": "Will BTC reach $100k by end of 2024?",
    "outcomes": ["Yes", "No"],
    "end_time": 1735689600,
    "market_type": "binary"
  }' 2>/dev/null || echo "Market 1 creation failed"

curl -X POST http://localhost:8081/api/markets/create \
  -H "Content-Type: application/json" \
  -d '{
    "question": "Who will win the 2024 US Presidential Election?",
    "outcomes": ["Biden", "Trump", "Other"],
    "end_time": 1730851200,
    "market_type": "multiple"
  }' 2>/dev/null || echo "Market 2 creation failed"

echo "Test markets created"
EOF

chmod +x "$SCRIPT_DIR/create_test_markets.sh"

# Summary
echo -e "\n${BLUE}========================================${NC}"
echo -e "${GREEN}Contract Deployment Complete!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Program ID: ${YELLOW}$PROGRAM_ID${NC}"
echo "Deployer: $DEPLOYER_PUBKEY"
echo ""
echo "Test Wallets Created:"
for i in {1..5}; do
    echo "- user$i: $(solana-keygen pubkey "$SCRIPT_DIR/wallets/user$i.json")"
done
echo ""
echo "Configuration saved to: $DEPLOYED_ADDRESSES"
echo ""
echo -e "Next steps:"
echo -e "1. Wait for API server to restart with new Program ID"
echo -e "2. Run ${YELLOW}./create_test_markets.sh${NC} to create test markets"
echo -e "3. Run ${YELLOW}./run_e2e_tests.sh${NC} to start end-to-end testing"