#!/bin/bash

# Initialize Test Data - Creates wallets and test markets

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
API_BASE_URL="http://localhost:8081"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Initializing Test Data${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Step 1: Create test wallets
echo -e "${BLUE}Step 1: Creating Test Wallets${NC}"
mkdir -p "$SCRIPT_DIR/wallets"

for i in {1..5}; do
    WALLET_FILE="$SCRIPT_DIR/wallets/user$i.json"
    if [ ! -f "$WALLET_FILE" ]; then
        # Create proper Solana keypair
        if command -v solana-keygen > /dev/null 2>&1; then
            solana-keygen new --outfile "$WALLET_FILE" --no-bip39-passphrase --force > /dev/null 2>&1
            echo -e "${GREEN}Created Solana wallet for user$i${NC}"
        else
            # Fallback: create test wallet with mock address
            echo "test-wallet-user$i-$(date +%s)" > "$SCRIPT_DIR/wallets/user$i.address"
            echo -e "${YELLOW}Created mock wallet for user$i (Solana CLI not available)${NC}"
        fi
    else
        echo "Wallet for user$i already exists"
    fi
done

# Step 2: Create test markets using the API
echo -e "\n${BLUE}Step 2: Creating Test Markets${NC}"

# Market 1: Bitcoin price prediction
echo "Creating Bitcoin market..."
curl -s -X POST "$API_BASE_URL/api/markets/create" \
    -H "Content-Type: application/json" \
    -d '{
        "question": "Will Bitcoin reach $100,000 by December 31, 2024?",
        "outcomes": ["Yes", "No"],
        "end_time": 1735689600,
        "market_type": "binary"
    }' > /dev/null 2>&1 && echo -e "${GREEN}✓ Bitcoin market created${NC}" || echo -e "${YELLOW}⚠ Failed to create Bitcoin market${NC}"

# Market 2: Election prediction
echo "Creating election market..."
curl -s -X POST "$API_BASE_URL/api/markets/create" \
    -H "Content-Type: application/json" \
    -d '{
        "question": "Who will win the 2024 US Presidential Election?",
        "outcomes": ["Biden", "Trump", "Kennedy", "Other"],
        "end_time": 1730851200,
        "market_type": "multiple"
    }' > /dev/null 2>&1 && echo -e "${GREEN}✓ Election market created${NC}" || echo -e "${YELLOW}⚠ Failed to create election market${NC}"

# Market 3: Sports prediction
echo "Creating sports market..."
curl -s -X POST "$API_BASE_URL/api/markets/create" \
    -H "Content-Type: application/json" \
    -d '{
        "question": "Will the Lakers win the 2024 NBA Championship?",
        "outcomes": ["Yes", "No"],
        "end_time": 1719792000,
        "market_type": "binary"
    }' > /dev/null 2>&1 && echo -e "${GREEN}✓ Sports market created${NC}" || echo -e "${YELLOW}⚠ Failed to create sports market${NC}"

# Market 4: Tech prediction
echo "Creating tech market..."
curl -s -X POST "$API_BASE_URL/api/markets/create" \
    -H "Content-Type: application/json" \
    -d '{
        "question": "Will Apple release a foldable iPhone by 2025?",
        "outcomes": ["Yes", "No"],
        "end_time": 1735689600,
        "market_type": "binary"
    }' > /dev/null 2>&1 && echo -e "${GREEN}✓ Tech market created${NC}" || echo -e "${YELLOW}⚠ Failed to create tech market${NC}"

# Market 5: Climate prediction
echo "Creating climate market..."
curl -s -X POST "$API_BASE_URL/api/markets/create" \
    -H "Content-Type: application/json" \
    -d '{
        "question": "Will 2024 be the hottest year on record?",
        "outcomes": ["Yes", "No"],
        "end_time": 1735689600,
        "market_type": "binary"
    }' > /dev/null 2>&1 && echo -e "${GREEN}✓ Climate market created${NC}" || echo -e "${YELLOW}⚠ Failed to create climate market${NC}"

# Step 3: Create demo accounts with initial balances
echo -e "\n${BLUE}Step 3: Creating Demo Accounts${NC}"

for i in {1..3}; do
    echo "Creating demo account $i..."
    RESPONSE=$(curl -s -X POST "$API_BASE_URL/api/wallet/demo/create" \
        -H "Content-Type: application/json" \
        -d '{
            "initial_balance": 10000000
        }')
    
    if echo "$RESPONSE" | grep -q "wallet"; then
        WALLET=$(echo "$RESPONSE" | sed -n 's/.*"wallet":"\([^"]*\)".*/\1/p')
        echo -e "${GREEN}✓ Created demo account: $WALLET${NC}"
        echo "$WALLET" > "$SCRIPT_DIR/wallets/demo$i.txt"
    else
        echo -e "${YELLOW}⚠ Failed to create demo account $i${NC}"
    fi
done

# Step 4: Verify markets were created
echo -e "\n${BLUE}Step 4: Verifying Markets${NC}"
MARKETS=$(curl -s "$API_BASE_URL/api/markets")
MARKET_COUNT=$(echo "$MARKETS" | grep -o '"id"' | wc -l)

if [ "$MARKET_COUNT" -gt 0 ]; then
    echo -e "${GREEN}✓ Found $MARKET_COUNT markets${NC}"
else
    echo -e "${RED}✗ No markets found${NC}"
fi

# Step 5: Save test configuration
echo -e "\n${BLUE}Step 5: Saving Test Configuration${NC}"

cat > "$SCRIPT_DIR/test_config.json" << EOF
{
    "api_base_url": "$API_BASE_URL",
    "test_wallets": [
        "test-wallet-user1-$(date +%s)",
        "test-wallet-user2-$(date +%s)",
        "test-wallet-user3-$(date +%s)",
        "test-wallet-user4-$(date +%s)",
        "test-wallet-user5-$(date +%s)"
    ],
    "demo_wallets": $(ls "$SCRIPT_DIR/wallets/demo"*.txt 2>/dev/null | xargs -I {} cat {} | jq -R . | jq -s . || echo '[]'),
    "initialized_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
}
EOF

echo -e "${GREEN}Test configuration saved to test_config.json${NC}"

# Summary
echo -e "\n${BLUE}========================================${NC}"
echo -e "${GREEN}Test Data Initialization Complete!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Created:"
echo "- 5 test wallet files"
echo "- 3 demo accounts with balance"
echo "- 5 test markets"
echo ""
echo -e "Next step: Run ${YELLOW}./run_e2e_tests.sh${NC} to execute tests"