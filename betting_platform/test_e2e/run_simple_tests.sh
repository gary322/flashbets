#!/bin/bash

# Simplified E2E Test Runner - Works without PostgreSQL
# Tests API endpoints with basic functionality

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
API_BASE_URL="http://localhost:8081"

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Simplified E2E Test Suite${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "Start Time: $(date)"
echo ""

# Function to log test result
log_test() {
    local test_name=$1
    local status=$2
    local details=$3
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if [ "$status" = "PASS" ]; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        echo -e "${GREEN}✓ $test_name${NC}"
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        echo -e "${RED}✗ $test_name${NC}"
        echo -e "  ${YELLOW}Details: $details${NC}"
    fi
}

# Test 1: API Health Check
echo -e "\n${CYAN}Test 1: API Health Check${NC}"
if curl -s "$API_BASE_URL/health" | grep -q "ok"; then
    log_test "API Health" "PASS" "API is responsive"
else
    log_test "API Health" "FAIL" "API not responding"
    echo -e "\n${RED}API server not running. Please start it first.${NC}"
    exit 1
fi

# Test 2: Seeded Markets
echo -e "\n${CYAN}Test 2: Seeded Markets${NC}"
MARKETS=$(curl -s "$API_BASE_URL/api/markets/seeded")
if echo "$MARKETS" | grep -q "BTC"; then
    log_test "Seeded Markets Available" "PASS" "Found seeded markets"
    
    # Test market details
    MARKET_ID=$(echo "$MARKETS" | grep -o '"id":[0-9]*' | head -1 | cut -d: -f2)
    if [ ! -z "$MARKET_ID" ]; then
        MARKET_DETAIL=$(curl -s "$API_BASE_URL/api/markets/seeded/$MARKET_ID")
        if echo "$MARKET_DETAIL" | grep -q "outcomes"; then
            log_test "Market Detail Access" "PASS" "Can retrieve individual market"
        else
            log_test "Market Detail Access" "FAIL" "Cannot retrieve market details"
        fi
    fi
else
    log_test "Seeded Markets Available" "FAIL" "No seeded markets found"
fi

# Test 3: Demo Trading
echo -e "\n${CYAN}Test 3: Demo Trading${NC}"
# Create demo wallet
DEMO_RESPONSE=$(curl -s -X POST "$API_BASE_URL/api/wallet/demo/create" \
    -H "Content-Type: application/json" \
    -d '{"initial_balance": 10000000}')

if echo "$DEMO_RESPONSE" | grep -q "wallet"; then
    DEMO_WALLET=$(echo "$DEMO_RESPONSE" | grep -o '"wallet":"[^"]*"' | cut -d'"' -f4)
    log_test "Demo Wallet Creation" "PASS" "Created wallet: ${DEMO_WALLET:0:8}..."
    
    # Place demo trade
    TRADE_RESPONSE=$(curl -s -X POST "$API_BASE_URL/api/trade/demo" \
        -H "Content-Type: application/json" \
        -d "{
            \"wallet\": \"$DEMO_WALLET\",
            \"market_id\": 1,
            \"outcome\": 0,
            \"amount\": 1000000,
            \"leverage\": 1
        }")
    
    if echo "$TRADE_RESPONSE" | grep -q "position_id"; then
        log_test "Demo Trade Placement" "PASS" "Trade executed successfully"
    else
        log_test "Demo Trade Placement" "FAIL" "Failed to place demo trade"
    fi
    
    # Check demo position
    POSITION_RESPONSE=$(curl -s "$API_BASE_URL/api/positions/demo/$DEMO_WALLET")
    if echo "$POSITION_RESPONSE" | grep -q "market_id"; then
        log_test "Demo Position Query" "PASS" "Position retrieved"
    else
        log_test "Demo Position Query" "FAIL" "No positions found"
    fi
else
    log_test "Demo Wallet Creation" "FAIL" "Failed to create demo wallet"
fi

# Test 4: Rate Limiting
echo -e "\n${CYAN}Test 4: Rate Limiting${NC}"
RATE_LIMITED=false
for i in {1..20}; do
    RESPONSE_CODE=$(curl -s -o /dev/null -w "%{http_code}" "$API_BASE_URL/api/markets/seeded")
    if [ "$RESPONSE_CODE" = "429" ]; then
        RATE_LIMITED=true
        break
    fi
done

if [ "$RATE_LIMITED" = true ]; then
    log_test "Rate Limiting" "PASS" "Rate limiter activated after rapid requests"
else
    log_test "Rate Limiting" "FAIL" "Rate limiter not working"
fi

# Test 5: WebSocket Connection
echo -e "\n${CYAN}Test 5: WebSocket Testing${NC}"
if command -v websocat > /dev/null 2>&1; then
    # Test WebSocket connection
    if timeout 2 echo '{"type":"ping"}' | websocat -n1 "ws://localhost:8081/ws" 2>/dev/null | grep -q "pong"; then
        log_test "WebSocket Connection" "PASS" "WebSocket server responding"
    else
        log_test "WebSocket Connection" "FAIL" "WebSocket not responding"
    fi
else
    log_test "WebSocket Connection" "SKIP" "websocat not installed"
fi

# Test 6: API Response Times
echo -e "\n${CYAN}Test 6: Performance Testing${NC}"
total_time=0
num_requests=5

for i in $(seq 1 $num_requests); do
    response_time=$(curl -s -o /dev/null -w "%{time_total}" "$API_BASE_URL/api/markets/seeded")
    total_time=$(echo "$total_time + $response_time" | bc)
done

avg_time=$(echo "scale=3; $total_time / $num_requests" | bc)

if (( $(echo "$avg_time < 0.5" | bc -l) )); then
    log_test "API Response Time" "PASS" "Average: ${avg_time}s"
else
    log_test "API Response Time" "FAIL" "Slow response: ${avg_time}s"
fi

# Test 7: Verse System
echo -e "\n${CYAN}Test 7: Verse System${NC}"
VERSES_RESPONSE=$(curl -s "$API_BASE_URL/api/verses")
if echo "$VERSES_RESPONSE" | grep -q "verse"; then
    log_test "Verse Catalog" "PASS" "Verses available"
    
    # Test verse matching
    MATCH_RESPONSE=$(curl -s -X POST "$API_BASE_URL/api/test/verse-match" \
        -H "Content-Type: application/json" \
        -d '{
            "title": "Will BTC reach $100k?",
            "category": "Crypto",
            "keywords": ["bitcoin", "price"]
        }')
    
    if echo "$MATCH_RESPONSE" | grep -q "matching_verses"; then
        log_test "Verse Matching" "PASS" "Verse matching algorithm working"
    else
        log_test "Verse Matching" "FAIL" "Verse matching not working"
    fi
else
    log_test "Verse Catalog" "FAIL" "No verses available"
fi

# Summary
echo -e "\n${BLUE}========================================${NC}"
echo -e "${BLUE}Test Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "Total Tests: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"

success_rate=0
if [ $TOTAL_TESTS -gt 0 ]; then
    success_rate=$(echo "scale=2; ($PASSED_TESTS * 100) / $TOTAL_TESTS" | bc)
fi
echo -e "Success Rate: ${success_rate}%"

# Exit with appropriate code
if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "\n${GREEN}✓ All tests passed!${NC}"
    exit 0
else
    echo -e "\n${RED}✗ Some tests failed${NC}"
    exit 1
fi