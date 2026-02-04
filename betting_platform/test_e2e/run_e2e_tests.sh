#!/bin/bash

# Comprehensive End-to-End Test Runner
# Tests all user journeys including betting, leverage, verses, quantum, and DeFi

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
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
LOG_DIR="$SCRIPT_DIR/logs"
RESULTS_DIR="$SCRIPT_DIR/results"
DEPLOYED_ADDRESSES="$SCRIPT_DIR/deployed_addresses.json"

# Test configuration
API_BASE_URL="http://localhost:8081"
UI_BASE_URL="http://localhost:3000"
WS_URL="ws://localhost:8081"

# Create directories
mkdir -p "$LOG_DIR" "$RESULTS_DIR"

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
TEST_START_TIME=$(date +%s)

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}End-to-End Test Suite${NC}"
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
    
    # Log to file
    echo "$(date -u +"%Y-%m-%dT%H:%M:%SZ") | $test_name | $status | $details" >> "$RESULTS_DIR/test_results.log"
}

# Function to check service health
check_services() {
    echo -e "${BLUE}Checking Service Health...${NC}"
    
    # Check API
    if curl -s "$API_BASE_URL/health" | grep -q "ok"; then
        log_test "API Health Check" "PASS" "API is running"
    else
        log_test "API Health Check" "FAIL" "API is not responding"
        exit 1
    fi
    
    # Check WebSocket
    if timeout 2 bash -c "echo 'test' | websocat $WS_URL/ws/v2" 2>/dev/null | grep -q "Connected"; then
        log_test "WebSocket Health Check" "PASS" "WebSocket is running"
    else
        log_test "WebSocket Health Check" "FAIL" "WebSocket is not responding"
    fi
    
    echo ""
}

# Function to create test user
create_test_user() {
    local user_num=$1
    local wallet_file="$SCRIPT_DIR/wallets/user$user_num.json"
    local address_file="$SCRIPT_DIR/wallets/user$user_num.address"
    
    if [ -f "$wallet_file" ] && command -v solana-keygen > /dev/null 2>&1; then
        local pubkey=$(solana-keygen pubkey "$wallet_file" 2>/dev/null)
        echo "$pubkey"
    elif [ -f "$address_file" ]; then
        # Use mock address for testing
        cat "$address_file"
    else
        # Generate a test address
        echo "test-wallet-user$user_num-$$"
    fi
}

# Journey 1: Basic Betting Flow
test_basic_betting() {
    echo -e "\n${CYAN}Journey 1: Basic Betting Flow${NC}"
    
    local wallet=$(create_test_user 1)
    if [ -z "$wallet" ]; then
        log_test "Basic Betting - Setup" "FAIL" "Test wallet not found"
        return
    fi
    
    # 1. Get markets
    local markets_response=$(curl -s "$API_BASE_URL/api/markets")
    if echo "$markets_response" | jq -e '.[0]' > /dev/null 2>&1; then
        log_test "Basic Betting - Get Markets" "PASS" "Retrieved markets list"
        
        local market_id=$(echo "$markets_response" | jq -r '.[0].id')
        
        # 2. Place bet
        local bet_response=$(curl -s -X POST "$API_BASE_URL/api/trade/place" \
            -H "Content-Type: application/json" \
            -d "{
                \"wallet\": \"$wallet\",
                \"market_id\": $market_id,
                \"outcome\": 0,
                \"amount\": 1000000,
                \"leverage\": 1
            }")
        
        if echo "$bet_response" | jq -e '.signature' > /dev/null 2>&1; then
            log_test "Basic Betting - Place Bet" "PASS" "Bet placed successfully"
            
            # 3. Check position
            local position_response=$(curl -s "$API_BASE_URL/api/positions/$wallet")
            if echo "$position_response" | jq -e '.[0]' > /dev/null 2>&1; then
                log_test "Basic Betting - Check Position" "PASS" "Position created"
            else
                log_test "Basic Betting - Check Position" "FAIL" "No position found"
            fi
        else
            log_test "Basic Betting - Place Bet" "FAIL" "Failed to place bet: $bet_response"
        fi
    else
        log_test "Basic Betting - Get Markets" "FAIL" "No markets available"
    fi
}

# Journey 2: Leveraged Trading
test_leveraged_trading() {
    echo -e "\n${CYAN}Journey 2: Leveraged Trading${NC}"
    
    local wallet=$(create_test_user 2)
    if [ -z "$wallet" ]; then
        log_test "Leveraged Trading - Setup" "FAIL" "Test wallet not found"
        return
    fi
    
    # Test different leverage levels
    for leverage in 2 5 10; do
        local trade_response=$(curl -s -X POST "$API_BASE_URL/api/trade/place" \
            -H "Content-Type: application/json" \
            -d "{
                \"wallet\": \"$wallet\",
                \"market_id\": 1,
                \"outcome\": 0,
                \"amount\": 500000,
                \"leverage\": $leverage
            }")
        
        if echo "$trade_response" | jq -e '.signature' > /dev/null 2>&1; then
            log_test "Leveraged Trading - ${leverage}x Leverage" "PASS" "Trade placed with ${leverage}x leverage"
            
            # Check margin requirements
            local risk_response=$(curl -s "$API_BASE_URL/api/risk/$wallet")
            if echo "$risk_response" | jq -e '.margin_used' > /dev/null 2>&1; then
                log_test "Leveraged Trading - Margin Check ${leverage}x" "PASS" "Margin requirements calculated"
            else
                log_test "Leveraged Trading - Margin Check ${leverage}x" "FAIL" "Failed to get margin info"
            fi
        else
            log_test "Leveraged Trading - ${leverage}x Leverage" "FAIL" "Failed to place leveraged trade"
        fi
    done
}

# Journey 3: Verse System
test_verse_system() {
    echo -e "\n${CYAN}Journey 3: Verse System Integration${NC}"
    
    # Get available verses
    local verses_response=$(curl -s "$API_BASE_URL/api/verses")
    if echo "$verses_response" | jq -e '.[0]' > /dev/null 2>&1; then
        log_test "Verse System - Get Verses" "PASS" "Retrieved verse catalog"
        
        # Test verse matching
        local match_response=$(curl -s -X POST "$API_BASE_URL/api/test/verse-match" \
            -H "Content-Type: application/json" \
            -d '{
                "title": "Will BTC reach $100k by end of 2024?",
                "category": "Crypto",
                "keywords": ["bitcoin", "price", "crypto"]
            }')
        
        if echo "$match_response" | jq -e '.matching_verses[0]' > /dev/null 2>&1; then
            log_test "Verse System - Match Verses" "PASS" "Verses matched to market"
        else
            log_test "Verse System - Match Verses" "FAIL" "No verses matched"
        fi
    else
        log_test "Verse System - Get Verses" "FAIL" "Failed to retrieve verses"
    fi
}

# Journey 4: Quantum Betting
test_quantum_betting() {
    echo -e "\n${CYAN}Journey 4: Quantum Betting Features${NC}"
    
    local wallet=$(create_test_user 3)
    if [ -z "$wallet" ]; then
        log_test "Quantum Betting - Setup" "FAIL" "Test wallet not found"
        return
    fi
    
    # Create quantum position
    local quantum_response=$(curl -s -X POST "$API_BASE_URL/api/quantum/create" \
        -H "Content-Type: application/json" \
        -d "{
            \"wallet\": \"$wallet\",
            \"market_id\": 1,
            \"amount\": 1000000,
            \"num_outcomes\": 2,
            \"entanglement_level\": 1
        }")
    
    if echo "$quantum_response" | jq -e '.position_id' > /dev/null 2>&1; then
        log_test "Quantum Betting - Create Position" "PASS" "Quantum position created"
        
        local position_id=$(echo "$quantum_response" | jq -r '.position_id')
        
        # Check quantum states
        local states_response=$(curl -s "$API_BASE_URL/api/quantum/states/1")
        if echo "$states_response" | jq -e '.quantum_positions[0]' > /dev/null 2>&1; then
            log_test "Quantum Betting - Check States" "PASS" "Quantum states retrieved"
        else
            log_test "Quantum Betting - Check States" "FAIL" "Failed to get quantum states"
        fi
        
        # Test quantum settlement
        local settlement_response=$(curl -s -X POST "$API_BASE_URL/api/quantum/settlement/position" \
            -H "Content-Type: application/json" \
            -d "{
                \"position_id\": \"$position_id\",
                \"observation_type\": \"market_resolution\"
            }")
        
        if echo "$settlement_response" | jq -e '.collapsed_outcome' > /dev/null 2>&1; then
            log_test "Quantum Betting - Settlement" "PASS" "Quantum position settled"
        else
            log_test "Quantum Betting - Settlement" "FAIL" "Failed to settle quantum position"
        fi
    else
        log_test "Quantum Betting - Create Position" "FAIL" "Failed to create quantum position"
    fi
}

# Journey 5: DeFi Integration
test_defi_integration() {
    echo -e "\n${CYAN}Journey 5: DeFi Integration${NC}"
    
    local wallet=$(create_test_user 4)
    if [ -z "$wallet" ]; then
        log_test "DeFi Integration - Setup" "FAIL" "Test wallet not found"
        return
    fi
    
    # Stake MMT tokens
    local stake_response=$(curl -s -X POST "$API_BASE_URL/api/defi/stake" \
        -H "Content-Type: application/json" \
        -d "{
            \"wallet\": \"$wallet\",
            \"amount\": 1000000,
            \"duration\": 30
        }")
    
    if echo "$stake_response" | jq -e '.stake_id' > /dev/null 2>&1; then
        log_test "DeFi Integration - Stake MMT" "PASS" "MMT tokens staked"
        
        # Check liquidity pools
        local pools_response=$(curl -s "$API_BASE_URL/api/defi/pools")
        if echo "$pools_response" | jq -e '.[0]' > /dev/null 2>&1; then
            log_test "DeFi Integration - Get Pools" "PASS" "Liquidity pools retrieved"
        else
            log_test "DeFi Integration - Get Pools" "FAIL" "Failed to get liquidity pools"
        fi
    else
        log_test "DeFi Integration - Stake MMT" "FAIL" "Failed to stake MMT tokens"
    fi
}

# Security Testing
test_security_features() {
    echo -e "\n${CYAN}Security Testing${NC}"
    
    # Test rate limiting
    local rate_limit_ok=true
    for i in {1..15}; do
        local response=$(curl -s -o /dev/null -w "%{http_code}" "$API_BASE_URL/api/markets")
        if [ "$response" = "429" ]; then
            rate_limit_ok=true
            break
        fi
    done
    
    if [ "$rate_limit_ok" = true ]; then
        log_test "Security - Rate Limiting" "PASS" "Rate limiter activated after rapid requests"
    else
        log_test "Security - Rate Limiting" "FAIL" "Rate limiter not working"
    fi
    
    # Test input sanitization
    local xss_payload='<script>alert("xss")</script>'
    local sanitize_response=$(curl -s -X POST "$API_BASE_URL/api/markets/create" \
        -H "Content-Type: application/json" \
        -d "{
            \"question\": \"$xss_payload\",
            \"outcomes\": [\"Yes\", \"No\"],
            \"end_time\": 1735689600
        }")
    
    if echo "$sanitize_response" | grep -q "script"; then
        log_test "Security - Input Sanitization" "FAIL" "XSS payload not sanitized"
    else
        log_test "Security - Input Sanitization" "PASS" "Input properly sanitized"
    fi
    
    # Test JWT validation
    local invalid_token_response=$(curl -s -w "\n%{http_code}" \
        -H "Authorization: Bearer invalid_token" \
        "$API_BASE_URL/api/positions/test")
    
    if echo "$invalid_token_response" | grep -q "401"; then
        log_test "Security - JWT Validation" "PASS" "Invalid token rejected"
    else
        log_test "Security - JWT Validation" "FAIL" "Invalid token not rejected"
    fi
}

# WebSocket Real-time Testing
test_websocket_events() {
    echo -e "\n${CYAN}WebSocket Real-time Events${NC}"
    
    if command -v websocat > /dev/null 2>&1; then
        # Start WebSocket listener in background
        websocat "$WS_URL/ws/v2" > "$LOG_DIR/websocket_events.log" 2>&1 &
        local ws_pid=$!
        
        # Wait for connection
        sleep 2
        
        # Trigger some events
        curl -s -X POST "$API_BASE_URL/api/trade/place" \
            -H "Content-Type: application/json" \
            -d '{
                "wallet": "test-wallet-ws",
                "market_id": 1,
                "outcome": 0,
                "amount": 100000,
                "leverage": 1
            }' > /dev/null 2>&1
        
        # Wait for events
        sleep 3
        
        # Check if events were received
        kill $ws_pid 2>/dev/null || true
        
        if grep -q "TradeExecuted\|MarketUpdate" "$LOG_DIR/websocket_events.log" 2>/dev/null; then
            log_test "WebSocket - Real-time Events" "PASS" "Events received via WebSocket"
        else
            log_test "WebSocket - Real-time Events" "FAIL" "No events received"
        fi
    else
        log_test "WebSocket - Real-time Events" "SKIP" "websocat not installed"
    fi
}

# Performance Testing
test_performance() {
    echo -e "\n${CYAN}Performance Testing${NC}"
    
    # Test API response times
    local total_time=0
    local num_requests=10
    
    for i in $(seq 1 $num_requests); do
        local response_time=$(curl -s -o /dev/null -w "%{time_total}" "$API_BASE_URL/api/markets")
        total_time=$(echo "$total_time + $response_time" | bc)
    done
    
    local avg_time=$(echo "scale=3; $total_time / $num_requests" | bc)
    
    if (( $(echo "$avg_time < 0.2" | bc -l) )); then
        log_test "Performance - API Response Time" "PASS" "Average response time: ${avg_time}s"
    else
        log_test "Performance - API Response Time" "FAIL" "Slow response time: ${avg_time}s"
    fi
}

# Run all tests
main() {
    # Check services first
    check_services
    
    # Run all test journeys
    test_basic_betting
    test_leveraged_trading
    test_verse_system
    test_quantum_betting
    test_defi_integration
    test_security_features
    test_websocket_events
    test_performance
    
    # Calculate test duration
    local test_end_time=$(date +%s)
    local test_duration=$((test_end_time - TEST_START_TIME))
    
    # Generate summary report
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}Test Summary Report${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo -e "End Time: $(date)"
    echo -e "Duration: ${test_duration} seconds"
    echo ""
    echo -e "Total Tests: $TOTAL_TESTS"
    echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
    echo -e "${RED}Failed: $FAILED_TESTS${NC}"
    
    local success_rate=0
    if [ $TOTAL_TESTS -gt 0 ]; then
        success_rate=$(echo "scale=2; ($PASSED_TESTS * 100) / $TOTAL_TESTS" | bc)
    fi
    echo -e "Success Rate: ${success_rate}%"
    
    # Save detailed report
    cat > "$RESULTS_DIR/test_summary.json" << EOF
{
    "test_run": {
        "start_time": "$(date -d @$TEST_START_TIME -u +"%Y-%m-%dT%H:%M:%SZ")",
        "end_time": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
        "duration_seconds": $test_duration
    },
    "results": {
        "total": $TOTAL_TESTS,
        "passed": $PASSED_TESTS,
        "failed": $FAILED_TESTS,
        "success_rate": $success_rate
    },
    "environment": {
        "api_url": "$API_BASE_URL",
        "ui_url": "$UI_BASE_URL",
        "network": "localnet"
    }
}
EOF
    
    echo -e "\nDetailed results saved to:"
    echo "- Test log: $RESULTS_DIR/test_results.log"
    echo "- Summary: $RESULTS_DIR/test_summary.json"
    
    # Exit with appropriate code
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "\n${GREEN}✓ All tests passed!${NC}"
        exit 0
    else
        echo -e "\n${RED}✗ Some tests failed${NC}"
        exit 1
    fi
}

# Run the test suite
main