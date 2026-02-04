#!/bin/bash

# Test deployed smart contracts

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
API_URL="${API_URL:-http://localhost:8081}"

# Load deployment info
if [ -f "deployment_info.json" ]; then
    PROGRAM_ID=$(cat deployment_info.json | grep -o '"program_id":"[^"]*' | cut -d'"' -f4)
else
    echo -e "${RED}ERROR: deployment_info.json not found. Run deploy_contracts.sh first.${NC}"
    exit 1
fi

echo -e "${GREEN}=== Testing Deployed Smart Contracts ===${NC}"
echo ""
echo "Program ID: $PROGRAM_ID"
echo ""

# Check API health
echo "1. Checking API health..."
HEALTH=$(curl -s $API_URL/health)
echo "$HEALTH" | python3 -m json.tool
echo ""

# Check Solana RPC health
echo "2. Checking Solana RPC health..."
RPC_HEALTH=$(curl -s $API_URL/api/solana/rpc/health)
echo "$RPC_HEALTH" | python3 -m json.tool 2>/dev/null || echo "RPC health check"
echo ""

# Verify program deployment
echo "3. Verifying program deployment..."
VERIFY=$(curl -s $API_URL/api/deployment/verify/$PROGRAM_ID)
echo "$VERIFY" | python3 -m json.tool
echo ""

# Get deployment status
echo "4. Getting deployment status..."
DEPLOY_STATUS=$(curl -s $API_URL/api/deployment/status/betting_platform)
echo "$DEPLOY_STATUS" | python3 -m json.tool 2>/dev/null || echo "Deployment status"
echo ""

# Test wallet for transactions
TEST_WALLET="TestWallet111111111111111111111111111111111"
TEST_TOKEN=$(curl -s -X POST $API_URL/api/auth/login \
    -H "Content-Type: application/json" \
    -d "{
        \"wallet\": \"$TEST_WALLET\",
        \"signature\": \"test_signature\",
        \"message\": \"Sign this message to authenticate with betting platform\"
    }" | grep -o '"access_token":"[^"]*' | cut -d'"' -f4)

# Create a test market
echo "5. Creating test market..."
MARKET_ID=$((RANDOM % 1000000 + 1000000))
MARKET_RESPONSE=$(curl -s -X POST $API_URL/api/markets/create \
    -H "Authorization: Bearer $TEST_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{
        \"market_id\": $MARKET_ID,
        \"title\": \"Test Market: Will deployment succeed?\",
        \"description\": \"Testing smart contract deployment\",
        \"outcomes\": [\"Yes\", \"No\"],
        \"end_time\": $(($(date +%s) + 86400)),
        \"creator_fee_bps\": 250
    }")

echo "$MARKET_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$MARKET_RESPONSE"
echo ""

# Check if market was created
if echo "$MARKET_RESPONSE" | grep -q "error"; then
    echo -e "${YELLOW}Warning: Market creation may have failed${NC}"
else
    echo -e "${GREEN}✓ Market created successfully${NC}"
    
    # Get market details
    echo "6. Getting market details..."
    curl -s $API_URL/api/markets/$MARKET_ID | python3 -m json.tool 2>/dev/null || echo "Market details"
    echo ""
fi

# Test transaction simulation
echo "7. Testing transaction simulation..."
SIM_RESPONSE=$(curl -s -X POST $API_URL/api/solana/tx/simulate \
    -H "Authorization: Bearer $TEST_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{
        \"transaction\": \"AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\",
        \"sig_verify\": false
    }")

echo "$SIM_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$SIM_RESPONSE"
echo ""

# Get program accounts
echo "8. Getting program accounts..."
ACCOUNTS=$(curl -s "$API_URL/api/solana/program/$PROGRAM_ID/accounts?limit=5")
echo "$ACCOUNTS" | python3 -m json.tool 2>/dev/null || echo "[]"
echo ""

# Test transaction manager status
echo "9. Checking transaction manager status..."
TX_STATUS=$(curl -s $API_URL/api/solana/tx/manager-status)
echo "$TX_STATUS" | python3 -m json.tool 2>/dev/null || echo "Transaction manager status"
echo ""

# Summary
echo -e "${GREEN}=== Test Summary ===${NC}"
echo ""

# Parse verification response
if echo "$VERIFY" | grep -q '"is_deployed":true'; then
    echo -e "${GREEN}✓ Program is deployed${NC}"
else
    echo -e "${RED}✗ Program deployment not verified${NC}"
fi

if echo "$VERIFY" | grep -q '"is_executable":true'; then
    echo -e "${GREEN}✓ Program is executable${NC}"
else
    echo -e "${RED}✗ Program is not executable${NC}"
fi

# Check RPC health
if echo "$RPC_HEALTH" | grep -q "healthy"; then
    echo -e "${GREEN}✓ Solana RPC is healthy${NC}"
else
    echo -e "${YELLOW}⚠ Solana RPC may have issues${NC}"
fi

# Check deployment status
if echo "$DEPLOY_STATUS" | grep -q "Deployed"; then
    echo -e "${GREEN}✓ Deployment status confirmed${NC}"
else
    echo -e "${YELLOW}⚠ Deployment status unclear${NC}"
fi

echo ""
echo "Deployment testing complete!"