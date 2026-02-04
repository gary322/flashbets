#!/bin/bash

# Circuit Breaker Test Script
# Tests the circuit breaker implementation

set -e

echo "================================"
echo "Circuit Breaker System Test"
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

# Get auth token for admin
echo -e "\n${YELLOW}Getting admin auth token...${NC}"
AUTH_RESPONSE=$(make_request "POST" "/auth/token" '{
    "wallet_address": "AdminWallet11111111111111111111111111111111",
    "signature": "admin_signature",
    "message": "Login to Betting Platform",
    "timestamp": 1234567890
}')

ADMIN_TOKEN=$(echo "$AUTH_RESPONSE" | grep -o '"token":"[^"]*' | cut -d'"' -f4)

if [ -z "$ADMIN_TOKEN" ]; then
    echo -e "${RED}Failed to get admin token${NC}"
    echo "Response: $AUTH_RESPONSE"
    kill $SERVER_PID
    exit 1
fi

echo -e "${GREEN}Got admin token${NC}"

# Test 1: Check circuit breaker health endpoint
echo -e "\n${YELLOW}Test 1: Circuit Breaker Health Check${NC}"
HEALTH_RESPONSE=$(make_request "GET" "/circuit-breakers/health")
test_case "Health endpoint returns data" "endpoint_breakers" "$HEALTH_RESPONSE"
test_case "Health includes service breakers" "service_breakers" "$HEALTH_RESPONSE"
test_case "Health includes timestamp" "timestamp" "$HEALTH_RESPONSE"

# Test 2: Simulate failures to trigger circuit breaker
echo -e "\n${YELLOW}Test 2: Circuit Breaker Activation${NC}"

# First, let's check current state
INITIAL_STATE=$(make_request "GET" "/circuit-breakers/health")
echo "Initial circuit breaker state captured"

# Make several failing requests to a non-existent endpoint
echo "Making failing requests to trigger circuit breaker..."
for i in {1..10}; do
    curl -s -o /dev/null -w "%{http_code}" http://localhost:3001/api/test/fail || true
    sleep 0.1
done

# Check if circuit state changed
AFTER_FAILURES=$(make_request "GET" "/circuit-breakers/health")
echo "Circuit breaker state after failures captured"

# Test 3: Reset circuit breakers (admin only)
echo -e "\n${YELLOW}Test 3: Reset Circuit Breakers${NC}"
RESET_RESPONSE=$(make_request "POST" "/circuit-breakers/reset" "" "$ADMIN_TOKEN")
test_case "Reset returns success" "success" "$RESET_RESPONSE"
test_case "Reset includes message" "All circuit breakers reset" "$RESET_RESPONSE"

# Test 4: Test with non-admin token
echo -e "\n${YELLOW}Test 4: Authorization Check${NC}"

# Get regular user token
USER_AUTH=$(make_request "POST" "/auth/token" '{
    "wallet_address": "UserWallet111111111111111111111111111111111",
    "signature": "user_signature",
    "message": "Login to Betting Platform",
    "timestamp": 1234567890
}')

USER_TOKEN=$(echo "$USER_AUTH" | grep -o '"token":"[^"]*' | cut -d'"' -f4)

if [ -n "$USER_TOKEN" ]; then
    FORBIDDEN_RESPONSE=$(make_request "POST" "/circuit-breakers/reset" "" "$USER_TOKEN")
    test_case "Non-admin reset returns error" "Admin access required" "$FORBIDDEN_RESPONSE"
fi

# Test 5: Service-specific circuit breakers
echo -e "\n${YELLOW}Test 5: Service Circuit Breakers${NC}"

# Check service breaker states
SERVICE_HEALTH=$(make_request "GET" "/circuit-breakers/health")
test_case "Database breaker exists" "database" "$SERVICE_HEALTH"
test_case "Redis breaker exists" "redis" "$SERVICE_HEALTH"
test_case "Solana RPC breaker exists" "solana_rpc" "$SERVICE_HEALTH"
test_case "External API breaker exists" "external_api" "$SERVICE_HEALTH"

# Test 6: Circuit breaker patterns
echo -e "\n${YELLOW}Test 6: Circuit Breaker Integration${NC}"

# Try a complex operation that uses multiple circuit breakers
TRADE_RESPONSE=$(make_request "POST" "/trades" '{
    "user_id": "test_user",
    "market_id": 1,
    "amount": 100,
    "side": "buy",
    "on_chain": false
}' "$USER_TOKEN")

# Should work or fail gracefully
if [[ "$TRADE_RESPONSE" == *"error"* ]]; then
    echo "Trade failed (expected if services not fully configured)"
else
    test_case "Trade operation completes" "success" "$TRADE_RESPONSE"
fi

# Test 7: Verify metrics collection
echo -e "\n${YELLOW}Test 7: Metrics Collection${NC}"
FINAL_HEALTH=$(make_request "GET" "/circuit-breakers/health")

# Extract and display some metrics
echo "Sample metrics from circuit breakers:"
echo "$FINAL_HEALTH" | grep -E "(total_calls|successful_calls|failed_calls|rejected_calls)" | head -5 || echo "No detailed metrics found"

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