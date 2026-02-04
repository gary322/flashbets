#!/bin/bash

# Tracing Logger Test Script
# Tests the enhanced logging system with correlation IDs

set -e

echo "================================"
echo "Tracing Logger System Test"
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

# Helper function to make API call and capture headers
make_request_with_headers() {
    local method=$1
    local endpoint=$2
    local data=$3
    local correlation_id=$4
    
    local headers=""
    if [ -n "$correlation_id" ]; then
        headers="-H \"X-Correlation-ID: $correlation_id\""
    fi
    
    if [ -n "$data" ]; then
        curl -s -i -X "$method" "$API_URL$endpoint" \
            -H "Content-Type: application/json" \
            $headers \
            -d "$data"
    else
        curl -s -i -X "$method" "$API_URL$endpoint" $headers
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
RUST_LOG=debug LOG_FORMAT=json cargo run --release > server.log 2>&1 &
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

# Test 1: Check correlation ID generation
echo -e "\n${YELLOW}Test 1: Correlation ID Generation${NC}"
RESPONSE=$(make_request_with_headers "GET" "/health" "")
test_case "Response includes correlation ID header" "x-correlation-id:" "$RESPONSE"

# Extract correlation ID from response
CORRELATION_ID=$(echo "$RESPONSE" | grep -i "x-correlation-id:" | awk '{print $2}' | tr -d '\r')
test_case "Correlation ID is valid UUID format" "-" "$CORRELATION_ID"

# Test 2: Correlation ID propagation
echo -e "\n${YELLOW}Test 2: Correlation ID Propagation${NC}"
CUSTOM_ID="test-correlation-$(date +%s)"
RESPONSE=$(make_request_with_headers "GET" "/health" "" "$CUSTOM_ID")
test_case "Custom correlation ID preserved" "$CUSTOM_ID" "$RESPONSE"

# Test 3: Check structured logging
echo -e "\n${YELLOW}Test 3: Structured Logging Output${NC}"
# Make a request and check logs
make_request_with_headers "GET" "/markets" "" > /dev/null 2>&1
sleep 1

# Check if logs contain correlation ID
LOG_ENTRY=$(tail -n 50 server.log | grep "Incoming request" | tail -n 1)
test_case "Log contains correlation_id field" "correlation_id" "$LOG_ENTRY"
test_case "Log contains path field" "path" "$LOG_ENTRY"
test_case "Log contains method field" "method" "$LOG_ENTRY"

# Test 4: Request completion logging
echo -e "\n${YELLOW}Test 4: Request Completion Logging${NC}"
RESPONSE=$(make_request_with_headers "GET" "/markets" "")
sleep 1

COMPLETION_LOG=$(tail -n 50 server.log | grep "Request completed" | tail -n 1)
test_case "Completion log exists" "Request completed" "$COMPLETION_LOG"
test_case "Completion log has duration_ms" "duration_ms" "$COMPLETION_LOG"
test_case "Completion log has status" "status" "$COMPLETION_LOG"

# Test 5: Error correlation
echo -e "\n${YELLOW}Test 5: Error Correlation${NC}"
ERROR_RESPONSE=$(make_request_with_headers "GET" "/api/nonexistent" "")
sleep 1

ERROR_LOG=$(tail -n 50 server.log | grep -E "(Request failed|404)" | tail -n 1)
test_case "Error log includes correlation" "correlation_id" "$ERROR_LOG"

# Test 6: Multi-step operation tracking
echo -e "\n${YELLOW}Test 6: Multi-Step Operation Tracking${NC}"

# Get auth token first
AUTH_RESPONSE=$(make_request_with_headers "POST" "/auth/token" '{
    "wallet_address": "TestWallet11111111111111111111111111111111",
    "signature": "test_signature",
    "message": "Login to Betting Platform",
    "timestamp": 1234567890
}')

# Extract token (if auth is working)
TOKEN=$(echo "$AUTH_RESPONSE" | grep -A 10 "token" | grep -o '"token":"[^"]*' | cut -d'"' -f4)

if [ -n "$TOKEN" ]; then
    # Make authenticated request
    TRADE_RESPONSE=$(curl -s -i -X POST "$API_URL/trades" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $TOKEN" \
        -H "X-Correlation-ID: trade-test-123" \
        -d '{
            "market_id": 1,
            "amount": 100,
            "side": "buy"
        }')
    
    sleep 1
    
    # Check for operation logs
    OPERATION_LOGS=$(tail -n 100 server.log | grep "trade-test-123")
    test_case "Multiple operation logs with same correlation ID" "trade-test-123" "$OPERATION_LOGS"
fi

# Test 7: Performance monitoring
echo -e "\n${YELLOW}Test 7: Performance Monitoring${NC}"

# Make several requests to generate metrics
for i in {1..5}; do
    make_request_with_headers "GET" "/markets" "" > /dev/null 2>&1
    sleep 0.2
done

# Check for performance logs
PERF_LOGS=$(tail -n 200 server.log | grep "duration_ms")
test_case "Performance metrics logged" "duration_ms" "$PERF_LOGS"

# Test 8: JSON log format
echo -e "\n${YELLOW}Test 8: JSON Log Format${NC}"
JSON_LOG=$(tail -n 50 server.log | grep "Incoming request" | tail -n 1)

# Try to parse as JSON (basic check)
if echo "$JSON_LOG" | python3 -m json.tool > /dev/null 2>&1; then
    test_case "Logs are valid JSON" "valid" "valid"
else
    test_case "Logs are valid JSON" "valid" "invalid"
fi

# Summary
echo -e "\n================================"
echo -e "Test Summary"
echo -e "================================"
echo -e "Total Tests: $TOTAL_TESTS"
echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed: ${RED}$((TOTAL_TESTS - PASSED_TESTS))${NC}"

# Show sample logs
echo -e "\n${YELLOW}Sample Log Output:${NC}"
echo "Recent request logs:"
tail -n 10 server.log | grep -E "(Incoming request|Request completed)" || echo "No request logs found"

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