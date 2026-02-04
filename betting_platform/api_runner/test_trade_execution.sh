#!/bin/bash

# Trade Execution System Test Script
# Tests the trade execution functionality

set -e

echo "================================"
echo "Trade Execution System Test"
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

# Get Trader token
TRADER_AUTH=$(make_request "POST" "/auth/token" '{
    "wallet_address": "TraderWallet11111111111111111111111111111111",
    "signature": "trader_signature",
    "message": "Login to Betting Platform",
    "timestamp": 1234567890
}')

TRADER_TOKEN=$(echo "$TRADER_AUTH" | grep -o '"token":"[^"]*' | cut -d'"' -f4)

if [ -z "$TRADER_TOKEN" ]; then
    echo -e "${RED}Failed to get trader token${NC}"
    echo "Response: $TRADER_AUTH"
    kill $SERVER_PID
    exit 1
fi

echo -e "${GREEN}Got auth tokens${NC}"

# Test 1: Execute limit order
echo -e "\n${YELLOW}Test 1: Execute Limit Order${NC}"

LIMIT_ORDER_RESPONSE=$(make_request "POST" "/trades/execute" "{
    \"market_id\": 100000001,
    \"side\": \"buy\",
    \"outcome\": 0,
    \"amount\": 10000000,
    \"order_type\": \"limit\",
    \"limit_price\": 0.65,
    \"slippage_tolerance\": 0.01,
    \"time_in_force\": \"GTC\",
    \"reduce_only\": false,
    \"post_only\": false
}" "$TRADER_TOKEN")

test_case "Limit order returns trade_id" "trade_id" "$LIMIT_ORDER_RESPONSE"
test_case "Limit order returns order_id" "order_id" "$LIMIT_ORDER_RESPONSE"
test_case "Limit order returns fees" "platform_fee" "$LIMIT_ORDER_RESPONSE"

# Extract order ID for later tests
ORDER_ID=$(echo "$LIMIT_ORDER_RESPONSE" | grep -o '"order_id":"[^"]*' | cut -d'"' -f4)

if [ -n "$ORDER_ID" ]; then
    echo "Created order with ID: $ORDER_ID"
fi

# Test 2: Execute market order
echo -e "\n${YELLOW}Test 2: Execute Market Order${NC}"

MARKET_ORDER_RESPONSE=$(make_request "POST" "/trades/execute" "{
    \"market_id\": 100000001,
    \"side\": \"sell\",
    \"outcome\": 1,
    \"amount\": 5000000,
    \"order_type\": \"market\",
    \"slippage_tolerance\": 0.02,
    \"time_in_force\": \"IOC\",
    \"reduce_only\": false,
    \"post_only\": false
}" "$TRADER_TOKEN")

test_case "Market order executes immediately" "executed_amount" "$MARKET_ORDER_RESPONSE"
test_case "Market order has average_price" "average_price" "$MARKET_ORDER_RESPONSE"

# Test 3: Invalid order validation
echo -e "\n${YELLOW}Test 3: Order Validation${NC}"

# Test zero amount
INVALID_AMOUNT_RESPONSE=$(make_request "POST" "/trades/execute" "{
    \"market_id\": 100000001,
    \"side\": \"buy\",
    \"outcome\": 0,
    \"amount\": 0,
    \"order_type\": \"market\"
}" "$TRADER_TOKEN")

test_case "Zero amount validation" "amount must be greater than 0" "$INVALID_AMOUNT_RESPONSE"

# Test invalid outcome
INVALID_OUTCOME_RESPONSE=$(make_request "POST" "/trades/execute" "{
    \"market_id\": 100000001,
    \"side\": \"buy\",
    \"outcome\": 10,
    \"amount\": 1000000,
    \"order_type\": \"market\"
}" "$TRADER_TOKEN")

test_case "Invalid outcome validation" "Invalid outcome" "$INVALID_OUTCOME_RESPONSE"

# Test 4: Get user orders
echo -e "\n${YELLOW}Test 4: Get User Orders${NC}"

ORDERS_RESPONSE=$(make_request "GET" "/trades/orders?limit=10" "" "$TRADER_TOKEN")
test_case "Get orders returns array" "[" "$ORDERS_RESPONSE"

if [ -n "$ORDER_ID" ]; then
    test_case "Orders include created order" "$ORDER_ID" "$ORDERS_RESPONSE"
fi

# Test 5: Cancel order
echo -e "\n${YELLOW}Test 5: Cancel Order${NC}"

if [ -n "$ORDER_ID" ]; then
    CANCEL_STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE "$API_URL/trades/orders/$ORDER_ID/cancel" \
        -H "Authorization: Bearer $TRADER_TOKEN")
    
    test_case "Order cancellation succeeds" "204" "$CANCEL_STATUS"
    
    # Verify order is cancelled
    CANCELLED_ORDERS=$(make_request "GET" "/trades/orders?status=cancelled" "" "$TRADER_TOKEN")
    test_case "Cancelled order appears in list" "$ORDER_ID" "$CANCELLED_ORDERS"
fi

# Test 6: Get trade history
echo -e "\n${YELLOW}Test 6: Trade History${NC}"

HISTORY_RESPONSE=$(make_request "GET" "/trades/history?limit=20" "" "$TRADER_TOKEN")
test_case "Trade history returns response" "trades" "$HISTORY_RESPONSE"
test_case "Trade history has total count" "total" "$HISTORY_RESPONSE"
test_case "Trade history has pagination" "limit" "$HISTORY_RESPONSE"

# Test 7: Get order book
echo -e "\n${YELLOW}Test 7: Order Book${NC}"

ORDERBOOK_RESPONSE=$(make_request "GET" "/trades/order-book/100000001?outcome=0&depth=5")
test_case "Order book returns bids" "bids" "$ORDERBOOK_RESPONSE"
test_case "Order book returns asks" "asks" "$ORDERBOOK_RESPONSE"
test_case "Order book has spread" "spread" "$ORDERBOOK_RESPONSE"
test_case "Order book has mid_price" "mid_price" "$ORDERBOOK_RESPONSE"

# Test 8: Get execution statistics
echo -e "\n${YELLOW}Test 8: Execution Statistics${NC}"

STATS_RESPONSE=$(make_request "GET" "/trades/stats")
test_case "Stats include total trades" "total_trades_24h" "$STATS_RESPONSE"
test_case "Stats include volume" "total_volume_24h" "$STATS_RESPONSE"
test_case "Stats include unique traders" "unique_traders_24h" "$STATS_RESPONSE"
test_case "Stats include fees collected" "total_fees_24h" "$STATS_RESPONSE"

# Test 9: Time in force options
echo -e "\n${YELLOW}Test 9: Time in Force Options${NC}"

# Test IOC order
IOC_ORDER_RESPONSE=$(make_request "POST" "/trades/execute" "{
    \"market_id\": 100000001,
    \"side\": \"buy\",
    \"outcome\": 0,
    \"amount\": 1000000,
    \"order_type\": \"limit\",
    \"limit_price\": 0.01,
    \"time_in_force\": \"IOC\"
}" "$TRADER_TOKEN")

test_case "IOC order processes correctly" "order_id" "$IOC_ORDER_RESPONSE"

# Test FOK order
FOK_ORDER_RESPONSE=$(make_request "POST" "/trades/execute" "{
    \"market_id\": 100000001,
    \"side\": \"buy\",
    \"outcome\": 0,
    \"amount\": 100000000,
    \"order_type\": \"limit\",
    \"limit_price\": 0.50,
    \"time_in_force\": \"FOK\"
}" "$TRADER_TOKEN")

test_case "FOK order processes correctly" "order_id" "$FOK_ORDER_RESPONSE"

# Test 10: Risk limits
echo -e "\n${YELLOW}Test 10: Risk Limit Checks${NC}"

# Try to place very large order
LARGE_ORDER_RESPONSE=$(make_request "POST" "/trades/execute" "{
    \"market_id\": 100000001,
    \"side\": \"buy\",
    \"outcome\": 0,
    \"amount\": 1000000000000,
    \"order_type\": \"market\"
}" "$TRADER_TOKEN")

# This should fail due to risk limits
test_case "Large order blocked by risk limits" "limit exceeded" "$LARGE_ORDER_RESPONSE"

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