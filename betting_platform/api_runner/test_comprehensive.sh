#!/bin/bash

# Comprehensive Test Suite for Betting Platform API
# This script runs all tests in a systematic order and generates a report

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0
TEST_RESULTS=()

# Log file
LOG_FILE="test_results_$(date +%Y%m%d_%H%M%S).log"
SUMMARY_FILE="test_summary_$(date +%Y%m%d_%H%M%S).txt"

echo -e "${BLUE}========================================${NC}" | tee -a $LOG_FILE
echo -e "${BLUE}Comprehensive Test Suite${NC}" | tee -a $LOG_FILE
echo -e "${BLUE}========================================${NC}" | tee -a $LOG_FILE
echo -e "Start Time: $(date)" | tee -a $LOG_FILE
echo "" | tee -a $LOG_FILE

# Function to run a test
run_test() {
    local test_name=$1
    local test_script=$2
    local test_category=$3
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    echo -e "${YELLOW}[$test_category] Running: $test_name${NC}" | tee -a $LOG_FILE
    
    # Create a temporary file for test output
    local temp_output=$(mktemp)
    
    # Run the test and capture output
    if timeout 300 bash $test_script > $temp_output 2>&1; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        echo -e "${GREEN}✓ PASSED: $test_name${NC}" | tee -a $LOG_FILE
        TEST_RESULTS+=("PASS|$test_category|$test_name")
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        echo -e "${RED}✗ FAILED: $test_name${NC}" | tee -a $LOG_FILE
        echo -e "${RED}Error output:${NC}" | tee -a $LOG_FILE
        tail -n 20 $temp_output | tee -a $LOG_FILE
        TEST_RESULTS+=("FAIL|$test_category|$test_name")
    fi
    
    # Clean up
    rm -f $temp_output
    echo "" | tee -a $LOG_FILE
}

# Function to check if server is running
check_server() {
    echo -e "${YELLOW}Checking if API server is running...${NC}" | tee -a $LOG_FILE
    if curl -s http://localhost:8081/health > /dev/null 2>&1; then
        echo -e "${GREEN}API server is running${NC}" | tee -a $LOG_FILE
        return 0
    else
        echo -e "${RED}API server is not running${NC}" | tee -a $LOG_FILE
        echo -e "${YELLOW}Starting API server...${NC}" | tee -a $LOG_FILE
        
        # Start the server in background
        cd /Users/nishu/Downloads/betting/betting_platform/api_runner
        cargo run --release > server.log 2>&1 &
        SERVER_PID=$!
        
        # Wait for server to start
        local max_attempts=30
        local attempt=0
        while [ $attempt -lt $max_attempts ]; do
            if curl -s http://localhost:8081/health > /dev/null 2>&1; then
                echo -e "${GREEN}API server started successfully${NC}" | tee -a $LOG_FILE
                return 0
            fi
            sleep 2
            attempt=$((attempt + 1))
        done
        
        echo -e "${RED}Failed to start API server${NC}" | tee -a $LOG_FILE
        return 1
    fi
}

# Function to check dependencies
check_dependencies() {
    echo -e "${YELLOW}Checking dependencies...${NC}" | tee -a $LOG_FILE
    
    local deps_ok=true
    
    # Check PostgreSQL
    if pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
        echo -e "${GREEN}✓ PostgreSQL is running${NC}" | tee -a $LOG_FILE
    else
        echo -e "${RED}✗ PostgreSQL is not running${NC}" | tee -a $LOG_FILE
        deps_ok=false
    fi
    
    # Check Redis
    if redis-cli ping > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Redis is running${NC}" | tee -a $LOG_FILE
    else
        echo -e "${RED}✗ Redis is not running${NC}" | tee -a $LOG_FILE
        deps_ok=false
    fi
    
    # Check required tools
    for tool in curl jq websocat; do
        if command -v $tool > /dev/null 2>&1; then
            echo -e "${GREEN}✓ $tool is installed${NC}" | tee -a $LOG_FILE
        else
            echo -e "${YELLOW}⚠ $tool is not installed (some tests may be skipped)${NC}" | tee -a $LOG_FILE
        fi
    done
    
    if [ "$deps_ok" = false ]; then
        echo -e "${RED}Some dependencies are missing. Please start PostgreSQL and Redis.${NC}" | tee -a $LOG_FILE
        return 1
    fi
    
    return 0
}

# Main test execution
main() {
    # Check dependencies
    if ! check_dependencies; then
        echo -e "${RED}Dependency check failed. Exiting.${NC}" | tee -a $LOG_FILE
        exit 1
    fi
    
    # Check/start server
    if ! check_server; then
        echo -e "${RED}Server check failed. Exiting.${NC}" | tee -a $LOG_FILE
        exit 1
    fi
    
    echo "" | tee -a $LOG_FILE
    echo -e "${BLUE}Running Test Suite...${NC}" | tee -a $LOG_FILE
    echo -e "${BLUE}========================================${NC}" | tee -a $LOG_FILE
    
    # Phase 1: Security Tests
    echo -e "\n${BLUE}Phase 1: Security Tests${NC}" | tee -a $LOG_FILE
    run_test "JWT Security" "./test_jwt_security.sh" "Security"
    run_test "Rate Limiting" "./test_rate_limiter.sh" "Security"
    
    # Phase 2: Real Data Tests
    echo -e "\n${BLUE}Phase 2: Real Data Integration Tests${NC}" | tee -a $LOG_FILE
    run_test "Polymarket Live API" "./test_polymarket_live.sh" "Integration"
    run_test "Price Feed Service" "./test_price_feed.sh" "Integration"
    run_test "Polygon Wallet" "./test_polygon_wallet.sh" "Integration"
    
    # Phase 3: Betting Tests
    echo -e "\n${BLUE}Phase 3: Betting Mechanism Tests${NC}" | tee -a $LOG_FILE
    run_test "Settlement System" "./test_settlement.sh" "Betting"
    run_test "Market Trading" "./test_market_trading.sh" "Betting"
    run_test "Order Types" "./test_order_types.sh" "Betting"
    
    # Phase 4: Blockchain Tests
    echo -e "\n${BLUE}Phase 4: Blockchain Integration Tests${NC}" | tee -a $LOG_FILE
    run_test "Wallet Authentication" "./test_wallet_auth.sh" "Blockchain"
    run_test "Verse Matching" "./test_verse_matching_directly.sh" "Blockchain"
    
    # Phase 5: Infrastructure Tests
    echo -e "\n${BLUE}Phase 5: Infrastructure Tests${NC}" | tee -a $LOG_FILE
    run_test "Database Operations" "./test_database.sh" "Infrastructure"
    run_test "Cache Service" "./test_cache.sh" "Infrastructure"
    run_test "Message Queue" "./test_queue.sh" "Infrastructure"
    
    # Phase 6: Feature Tests
    echo -e "\n${BLUE}Phase 6: Feature Tests${NC}" | tee -a $LOG_FILE
    run_test "Quantum Settlement" "./test_quantum_settlement.sh" "Features"
    run_test "WebSocket Real Events" "./test_websocket_real_events.sh" "Features"
    run_test "Quantum Features" "./test_quantum_features.sh" "Features"
    run_test "DeFi Features" "./test_defi_features.sh" "Features"
    
    # Phase 7: End-to-End Tests
    echo -e "\n${BLUE}Phase 7: End-to-End Tests${NC}" | tee -a $LOG_FILE
    run_test "API Endpoints" "./test_endpoints.sh" "E2E"
    run_test "User Journey" "./exhaustive_user_journey_test.sh" "E2E"
    
    # Generate test report
    generate_report
}

# Function to generate test report
generate_report() {
    echo "" | tee -a $SUMMARY_FILE
    echo -e "${BLUE}========================================${NC}" | tee -a $SUMMARY_FILE
    echo -e "${BLUE}Test Summary Report${NC}" | tee -a $SUMMARY_FILE
    echo -e "${BLUE}========================================${NC}" | tee -a $SUMMARY_FILE
    echo -e "End Time: $(date)" | tee -a $SUMMARY_FILE
    echo "" | tee -a $SUMMARY_FILE
    
    # Overall statistics
    echo -e "Total Tests: $TOTAL_TESTS" | tee -a $SUMMARY_FILE
    echo -e "${GREEN}Passed: $PASSED_TESTS${NC}" | tee -a $SUMMARY_FILE
    echo -e "${RED}Failed: $FAILED_TESTS${NC}" | tee -a $SUMMARY_FILE
    if [ $SKIPPED_TESTS -gt 0 ]; then
        echo -e "${YELLOW}Skipped: $SKIPPED_TESTS${NC}" | tee -a $SUMMARY_FILE
    fi
    
    # Success rate
    if [ $TOTAL_TESTS -gt 0 ]; then
        local success_rate=$(( (PASSED_TESTS * 100) / TOTAL_TESTS ))
        echo -e "Success Rate: ${success_rate}%" | tee -a $SUMMARY_FILE
    fi
    
    echo "" | tee -a $SUMMARY_FILE
    echo -e "${BLUE}Test Results by Category:${NC}" | tee -a $SUMMARY_FILE
    echo -e "${BLUE}------------------------${NC}" | tee -a $SUMMARY_FILE
    
    # Group results by category
    declare -A category_pass
    declare -A category_fail
    
    for result in "${TEST_RESULTS[@]}"; do
        IFS='|' read -r status category name <<< "$result"
        if [ "$status" = "PASS" ]; then
            category_pass[$category]=$((${category_pass[$category]:-0} + 1))
        else
            category_fail[$category]=$((${category_fail[$category]:-0} + 1))
        fi
    done
    
    # Display category results
    for category in "${!category_pass[@]}" "${!category_fail[@]}"; do
        local pass=${category_pass[$category]:-0}
        local fail=${category_fail[$category]:-0}
        local total=$((pass + fail))
        echo -e "$category: ${GREEN}$pass passed${NC}, ${RED}$fail failed${NC} (Total: $total)" | tee -a $SUMMARY_FILE
    done
    
    # Failed tests details
    if [ $FAILED_TESTS -gt 0 ]; then
        echo "" | tee -a $SUMMARY_FILE
        echo -e "${RED}Failed Tests:${NC}" | tee -a $SUMMARY_FILE
        echo -e "${RED}-------------${NC}" | tee -a $SUMMARY_FILE
        
        for result in "${TEST_RESULTS[@]}"; do
            IFS='|' read -r status category name <<< "$result"
            if [ "$status" = "FAIL" ]; then
                echo -e "- [$category] $name" | tee -a $SUMMARY_FILE
            fi
        done
    fi
    
    echo "" | tee -a $SUMMARY_FILE
    echo -e "${BLUE}========================================${NC}" | tee -a $SUMMARY_FILE
    
    # Final verdict
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "${GREEN}✓ ALL TESTS PASSED!${NC}" | tee -a $SUMMARY_FILE
        echo -e "${GREEN}The system is ready for deployment.${NC}" | tee -a $SUMMARY_FILE
    else
        echo -e "${RED}✗ SOME TESTS FAILED${NC}" | tee -a $SUMMARY_FILE
        echo -e "${RED}Please fix the failing tests before deployment.${NC}" | tee -a $SUMMARY_FILE
    fi
    
    echo "" | tee -a $SUMMARY_FILE
    echo "Full test log: $LOG_FILE" | tee -a $SUMMARY_FILE
    echo "Summary report: $SUMMARY_FILE" | tee -a $SUMMARY_FILE
}

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}Cleaning up...${NC}"
    
    # Kill server if we started it
    if [ ! -z "${SERVER_PID:-}" ]; then
        echo "Stopping API server..."
        kill $SERVER_PID 2>/dev/null || true
    fi
}

# Set up cleanup on exit
trap cleanup EXIT

# Run the test suite
main

# Exit with appropriate code
if [ $FAILED_TESTS -eq 0 ]; then
    exit 0
else
    exit 1
fi