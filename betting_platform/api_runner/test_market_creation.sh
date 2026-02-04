#!/bin/bash

# Market Creation System Test Script
# Tests the market creation functionality

set -e

echo "================================"
echo "Market Creation System Test"
echo "================================"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# API base URL
API_URL="http://localhost:3001/api"

# Test counter
TOTAL_TESTS=0
PASSED_TESTS=0

# Helper function to make API call
make_request() {
    local method=$1
    local endpoint=$2
    local data=$3
    local token=$4
    
    if [ -n "$token" ]; then
        if [ -n "$data" ]; then
            curl -s -X "$method" "$API_URL$endpoint" \
                -H "Content-Type: application/json" \
                -H "Authorization: Bearer $token" \
                -d "$data"
        else
            curl -s -X "$method" "$API_URL$endpoint" \
                -H "Authorization: Bearer $token"
        fi
    else
        if [ -n "$data" ]; then
            curl -s -X "$method" "$API_URL$endpoint" \
                -H "Content-Type: application/json" \
                -d "$data"
        else
            curl -s -X "$method" "$API_URL$endpoint"
        fi
    fi
}

# Test function
test_case() {
    local test_name=$1
    local expected=$2
    local actual=$3
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if [[ "$actual" == *"$expected"* ]]; then
        echo -e "${GREEN}✓${NC} $test_name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}✗${NC} $test_name"
        echo "  Expected: $expected"
        echo "  Actual: $actual"
    fi
}

echo -e "\n${YELLOW}Starting API server...${NC}"
cd "$(dirname "$0")"

# Kill any existing server
pkill -f "cargo run" || true
sleep 2

# Start server in background
cargo run --release > server.log 2>&1 &
SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to start..."
sleep 10

# Check if server is running
if ! curl -s http://localhost:3001/health > /dev/null; then
    echo -e "${RED}Server failed to start${NC}"
    cat server.log
    exit 1
fi

echo -e "${GREEN}Server started successfully${NC}"

# Get auth tokens
echo -e "\n${YELLOW}Getting auth tokens...${NC}"

# Get MarketMaker token
MARKET_MAKER_AUTH=$(make_request "POST" "/auth/token" '{
    "wallet_address": "MarketMakerWallet1111111111111111111111111",
    "signature": "market_maker_signature",
    "message": "Login to Betting Platform",
    "timestamp": 1234567890
}')

MARKET_MAKER_TOKEN=$(echo "$MARKET_MAKER_AUTH" | grep -o '"token":"[^"]*' | cut -d'"' -f4)

if [ -z "$MARKET_MAKER_TOKEN" ]; then
    echo -e "${RED}Failed to get market maker token${NC}"
    echo "Response: $MARKET_MAKER_AUTH"
    kill $SERVER_PID
    exit 1
fi

# Get regular user token
USER_AUTH=$(make_request "POST" "/auth/token" '{
    "wallet_address": "UserWallet111111111111111111111111111111111",
    "signature": "user_signature",
    "message": "Login to Betting Platform",
    "timestamp": 1234567890
}')

USER_TOKEN=$(echo "$USER_AUTH" | grep -o '"token":"[^"]*' | cut -d'"' -f4)

echo -e "${GREEN}Got auth tokens${NC}"

# Test 1: Create market with valid data
echo -e "\n${YELLOW}Test 1: Create Market${NC}"

CURRENT_TIME=$(date -u +%s)
END_TIME=$((CURRENT_TIME + 86400)) # 24 hours from now
RESOLUTION_TIME=$((END_TIME + 3600)) # 1 hour after end

CREATE_RESPONSE=$(make_request "POST" "/markets/create" "{
    \"title\": \"Will Bitcoin reach \$50,000 by tomorrow?\",
    \"description\": \"This market will resolve YES if Bitcoin price reaches or exceeds \$50,000 USD on any major exchange\",
    \"outcomes\": [\"YES\", \"NO\"],
    \"end_time\": \"$(date -u -d @$END_TIME +%Y-%m-%dT%H:%M:%SZ)\",
    \"resolution_time\": \"$(date -u -d @$RESOLUTION_TIME +%Y-%m-%dT%H:%M:%SZ)\",
    \"category\": \"Crypto\",
    \"tags\": [\"bitcoin\", \"price\", \"cryptocurrency\"],
    \"amm_type\": \"Cpmm\",
    \"initial_liquidity\": 10000000,
    \"creator_fee_bps\": 250,
    \"platform_fee_bps\": 100,
    \"min_bet_amount\": 1000000,
    \"max_bet_amount\": 100000000,
    \"oracle_sources\": [
        {
            \"name\": \"Manual\",
            \"url\": \"https://admin.platform.com/resolve\",
            \"weight\": 100
        }
    ]
}" "$MARKET_MAKER_TOKEN")

test_case "Market creation returns market_id" "market_id" "$CREATE_RESPONSE"
test_case "Market creation returns transaction_signature" "transaction_signature" "$CREATE_RESPONSE"

# Extract market ID
MARKET_ID=$(echo "$CREATE_RESPONSE" | grep -o '"market_id":[0-9]*' | cut -d':' -f2)

if [ -n "$MARKET_ID" ]; then
    echo "Created market with ID: $MARKET_ID"
fi

# Test 2: Try to create market without permission
echo -e "\n${YELLOW}Test 2: Permission Check${NC}"

FORBIDDEN_RESPONSE=$(make_request "POST" "/markets/create" "{
    \"title\": \"Unauthorized market\",
    \"description\": \"This should fail due to permissions\",
    \"outcomes\": [\"YES\", \"NO\"],
    \"end_time\": \"$(date -u -d @$END_TIME +%Y-%m-%dT%H:%M:%SZ)\",
    \"resolution_time\": \"$(date -u -d @$RESOLUTION_TIME +%Y-%m-%dT%H:%M:%SZ)\",
    \"category\": \"Test\",
    \"tags\": [\"test\"],
    \"amm_type\": \"Cpmm\",
    \"initial_liquidity\": 10000000,
    \"creator_fee_bps\": 250,
    \"platform_fee_bps\": 100,
    \"min_bet_amount\": 1000000,
    \"max_bet_amount\": 100000000,
    \"oracle_sources\": [{\"name\": \"Manual\", \"url\": \"test\", \"weight\": 100}]
}" "$USER_TOKEN")

test_case "Regular user cannot create market" "Insufficient permissions" "$FORBIDDEN_RESPONSE"

# Test 3: Validation tests
echo -e "\n${YELLOW}Test 3: Validation Tests${NC}"

# Test short title
VALIDATION_RESPONSE=$(make_request "POST" "/markets/create" "{
    \"title\": \"Short\",
    \"description\": \"This has a title that is too short\",
    \"outcomes\": [\"YES\", \"NO\"],
    \"end_time\": \"$(date -u -d @$END_TIME +%Y-%m-%dT%H:%M:%SZ)\",
    \"resolution_time\": \"$(date -u -d @$RESOLUTION_TIME +%Y-%m-%dT%H:%M:%SZ)\",
    \"category\": \"Test\",
    \"tags\": [\"test\"],
    \"amm_type\": \"Cpmm\",
    \"initial_liquidity\": 10000000,
    \"creator_fee_bps\": 250,
    \"platform_fee_bps\": 100,
    \"min_bet_amount\": 1000000,
    \"max_bet_amount\": 100000000,
    \"oracle_sources\": [{\"name\": \"Manual\", \"url\": \"test\", \"weight\": 100}]
}" "$MARKET_MAKER_TOKEN")

test_case "Title validation works" "Title must be at least" "$VALIDATION_RESPONSE"

# Test invalid oracle weights
ORACLE_RESPONSE=$(make_request "POST" "/markets/create" "{
    \"title\": \"Market with invalid oracle weights\",
    \"description\": \"This market has oracle weights that don't sum to 100\",
    \"outcomes\": [\"YES\", \"NO\"],
    \"end_time\": \"$(date -u -d @$END_TIME +%Y-%m-%dT%H:%M:%SZ)\",
    \"resolution_time\": \"$(date -u -d @$RESOLUTION_TIME +%Y-%m-%dT%H:%M:%SZ)\",
    \"category\": \"Test\",
    \"tags\": [\"test\"],
    \"amm_type\": \"Cpmm\",
    \"initial_liquidity\": 10000000,
    \"creator_fee_bps\": 250,
    \"platform_fee_bps\": 100,
    \"min_bet_amount\": 1000000,
    \"max_bet_amount\": 100000000,
    \"oracle_sources\": [
        {\"name\": \"Oracle1\", \"url\": \"test1\", \"weight\": 40},
        {\"name\": \"Oracle2\", \"url\": \"test2\", \"weight\": 30}
    ]
}" "$MARKET_MAKER_TOKEN")

test_case "Oracle weight validation works" "Oracle weights must sum to 100" "$ORACLE_RESPONSE"

# Test 4: Get market details
echo -e "\n${YELLOW}Test 4: Get Market Details${NC}"

if [ -n "$MARKET_ID" ]; then
    MARKET_DETAILS=$(make_request "GET" "/markets/$MARKET_ID")
    test_case "Get market returns title" "Will Bitcoin reach" "$MARKET_DETAILS"
    test_case "Get market returns outcomes" "YES" "$MARKET_DETAILS"
    test_case "Get market returns category" "Crypto" "$MARKET_DETAILS"
fi

# Test 5: Update market
echo -e "\n${YELLOW}Test 5: Update Market${NC}"

if [ -n "$MARKET_ID" ]; then
    UPDATE_RESPONSE=$(make_request "PUT" "/markets/$MARKET_ID/update" '{
        "title": "Will Bitcoin reach $50,000 by tomorrow? (Updated)",
        "tags": ["bitcoin", "price", "updated"]
    }' "$MARKET_MAKER_TOKEN")
    
    # Check status code (should be 204 No Content)
    UPDATE_STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "$API_URL/markets/$MARKET_ID/update" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $MARKET_MAKER_TOKEN" \
        -d '{"title": "Updated title"}')
    
    test_case "Market update succeeds" "204" "$UPDATE_STATUS"
fi

# Test 6: List markets
echo -e "\n${YELLOW}Test 6: List Markets${NC}"

LIST_RESPONSE=$(make_request "GET" "/markets/list?category=Crypto&limit=10")
test_case "List markets returns array" "markets" "$LIST_RESPONSE"
test_case "List includes total count" "total" "$LIST_RESPONSE"
test_case "List includes pagination" "limit" "$LIST_RESPONSE"

# Test 7: Get market statistics
echo -e "\n${YELLOW}Test 7: Market Statistics${NC}"

if [ -n "$MARKET_ID" ]; then
    STATS_RESPONSE=$(make_request "GET" "/markets/$MARKET_ID/stats")
    test_case "Stats include unique traders" "unique_traders" "$STATS_RESPONSE"
    test_case "Stats include total trades" "total_trades" "$STATS_RESPONSE"
    test_case "Stats include volume" "total_volume" "$STATS_RESPONSE"
fi

# Summary
echo -e "\n================================"
echo -e "Test Summary"
echo -e "================================"
echo -e "Total Tests: $TOTAL_TESTS"
echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed: ${RED}$((TOTAL_TESTS - PASSED_TESTS))${NC}"

# Cleanup
echo -e "\n${YELLOW}Cleaning up...${NC}"
kill $SERVER_PID 2>/dev/null || true

if [ $PASSED_TESTS -eq $TOTAL_TESTS ]; then
    echo -e "\n${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "\n${RED}Some tests failed${NC}"
    exit 1
fi