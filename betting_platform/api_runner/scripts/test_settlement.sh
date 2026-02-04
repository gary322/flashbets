#!/bin/bash

# Settlement System Test Script
# Tests the settlement functionality with oracle integration

API_URL="${API_URL:-http://localhost:3000}"
AUTH_TOKEN=""
MARKET_ID=""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "🏁 Settlement System Test Script"
echo "================================"

# Function to make authenticated requests
make_request() {
    local method=$1
    local endpoint=$2
    local data=$3
    
    if [ -z "$data" ]; then
        curl -s -X $method \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            "$API_URL$endpoint"
    else
        curl -s -X $method \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$API_URL$endpoint"
    fi
}

# Step 1: Authenticate as admin
echo -e "\n${YELLOW}Step 1: Authenticating as admin...${NC}"
AUTH_RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{
        "email": "admin@betting.com",
        "password": "admin123",
        "wallet": "AdminWa11etPubkey123456789012345678901234567"
    }' \
    "$API_URL/api/auth/login")

AUTH_TOKEN=$(echo $AUTH_RESPONSE | jq -r '.data.token // .token // empty')

if [ -z "$AUTH_TOKEN" ]; then
    echo -e "${RED}❌ Failed to authenticate${NC}"
    echo "Response: $AUTH_RESPONSE"
    exit 1
fi

echo -e "${GREEN}✅ Authenticated successfully${NC}"

# Step 2: Get an active market
echo -e "\n${YELLOW}Step 2: Finding an active market...${NC}"
MARKETS_RESPONSE=$(make_request GET "/api/markets?status=open&limit=1")
MARKET_ID=$(echo $MARKETS_RESPONSE | jq -r '.data.markets[0].id // .markets[0].id // empty')

if [ -z "$MARKET_ID" ]; then
    echo -e "${RED}❌ No active markets found${NC}"
    echo "Response: $MARKETS_RESPONSE"
    exit 1
fi

echo -e "${GREEN}✅ Found market ID: $MARKET_ID${NC}"

# Step 3: Query oracles for the market
echo -e "\n${YELLOW}Step 3: Querying oracles for market resolution...${NC}"
ORACLE_RESPONSE=$(make_request GET "/api/settlement/oracles/$MARKET_ID")

echo "Oracle Response:"
echo $ORACLE_RESPONSE | jq '.'

CONSENSUS_OUTCOME=$(echo $ORACLE_RESPONSE | jq -r '.consensus_outcome // empty')
CAN_SETTLE=$(echo $ORACLE_RESPONSE | jq -r '.can_settle // false')

if [ "$CAN_SETTLE" != "true" ]; then
    echo -e "${YELLOW}⚠️  Market cannot be settled yet${NC}"
    REASON=$(echo $ORACLE_RESPONSE | jq -r '.reason // "Unknown reason"')
    echo "Reason: $REASON"
fi

# Step 4: Check settlement status
echo -e "\n${YELLOW}Step 4: Checking settlement status...${NC}"
STATUS_RESPONSE=$(make_request GET "/api/settlement/status/$MARKET_ID")
echo "Settlement Status:"
echo $STATUS_RESPONSE | jq '.'

# Step 5: Attempt to initiate settlement (admin only)
if [ "$CAN_SETTLE" == "true" ]; then
    echo -e "\n${YELLOW}Step 5: Initiating settlement...${NC}"
    
    SETTLEMENT_DATA=$(cat <<EOF
{
    "market_id": $MARKET_ID,
    "oracle_results": [
        {
            "oracle_name": "TestOracle",
            "outcome": $CONSENSUS_OUTCOME,
            "confidence": 0.95,
            "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
            "proof_url": "https://test-oracle.com/proof/$MARKET_ID"
        }
    ]
}
EOF
)
    
    SETTLEMENT_RESPONSE=$(make_request POST "/api/settlement/initiate" "$SETTLEMENT_DATA")
    
    if echo $SETTLEMENT_RESPONSE | jq -e '.error' > /dev/null; then
        echo -e "${RED}❌ Settlement failed${NC}"
        echo $SETTLEMENT_RESPONSE | jq '.'
    else
        echo -e "${GREEN}✅ Settlement initiated successfully${NC}"
        echo $SETTLEMENT_RESPONSE | jq '.'
        
        SETTLEMENT_ID=$(echo $SETTLEMENT_RESPONSE | jq -r '.settlement_id // .data.settlement_id // empty')
        echo -e "Settlement ID: ${GREEN}$SETTLEMENT_ID${NC}"
    fi
else
    echo -e "\n${YELLOW}Step 5: Skipping settlement initiation (market not ready)${NC}"
fi

# Step 6: Get user settlement history
echo -e "\n${YELLOW}Step 6: Getting user settlement history...${NC}"
HISTORY_RESPONSE=$(make_request GET "/api/settlement/user?limit=5")
echo "Recent Settlements:"
echo $HISTORY_RESPONSE | jq '.settlements[]? | {
    market_title: .market_title,
    winning_outcome: .winning_outcome,
    payout: .payout,
    pnl: .pnl,
    settled_at: .settled_at
}'

# Step 7: Get system-wide settlement history (admin)
echo -e "\n${YELLOW}Step 7: Getting system settlement history...${NC}"
SYSTEM_HISTORY=$(make_request GET "/api/settlement/history?limit=5")
echo "System Settlement History:"
echo $SYSTEM_HISTORY | jq '.settlements[]? | {
    settlement_id: .settlement_id,
    market_title: .market_title,
    total_positions: .total_positions,
    total_payout: .total_payout,
    oracle_consensus: .oracle_consensus,
    settled_at: .settled_at
}'

echo -e "\n${GREEN}🎉 Settlement system test completed!${NC}"

# Summary
echo -e "\n📊 Test Summary:"
echo "- Market ID tested: $MARKET_ID"
echo "- Oracle consensus outcome: $CONSENSUS_OUTCOME"
echo "- Can settle: $CAN_SETTLE"
if [ -n "$SETTLEMENT_ID" ]; then
    echo "- Settlement ID: $SETTLEMENT_ID"
fi