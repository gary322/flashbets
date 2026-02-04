#!/bin/bash
# Comprehensive test runner for the betting platform

echo "=========================================="
echo "    Betting Platform Test Suite"
echo "=========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test categories
declare -a test_categories=(
    "test_full_system:Full System Integration"
    "test_amm:AMM Modules"
    "test_keeper_network:Keeper Network"
    "test_resolution:Resolution System"
    "test_security:Security Features"
    "test_mmt_token:MMT Token System"
    "test_tables:CDF/PDF Tables"
    "synthetics_tests:Synthetic Wrapper & Routing"
    "priority_queue_tests:Priority Queue & Anti-MEV"
    "user_journey_tests:User Journey Simulations"
    "integration_test_runner:Comprehensive Integration Tests"
    "production_readiness_check:Production Readiness Verification"
)

# Track results
total_tests=0
passed_tests=0
failed_tests=0

# Function to run a test category
run_test_category() {
    local test_file=$1
    local test_name=$2
    
    echo -e "${YELLOW}Running ${test_name} tests...${NC}"
    echo "----------------------------------------"
    
    if cargo test --test $test_file -- --nocapture --test-threads=1; then
        echo -e "${GREEN}✓ ${test_name} tests PASSED${NC}\n"
        ((passed_tests++))
    else
        echo -e "${RED}✗ ${test_name} tests FAILED${NC}\n"
        ((failed_tests++))
    fi
    
    ((total_tests++))
}

# Check if running specific test
if [ "$1" != "" ]; then
    echo -e "${YELLOW}Running specific test: $1${NC}\n"
    cargo test $1 -- --nocapture --test-threads=1
    exit $?
fi

# Run all test categories
echo "Starting comprehensive test suite..."
echo ""

start_time=$(date +%s)

for test_spec in "${test_categories[@]}"; do
    IFS=':' read -r test_file test_name <<< "$test_spec"
    run_test_category "$test_file" "$test_name"
done

end_time=$(date +%s)
duration=$((end_time - start_time))

# Summary
echo "=========================================="
echo "           TEST SUMMARY"
echo "=========================================="
echo ""
echo "Total test categories: $total_tests"
echo -e "Passed: ${GREEN}$passed_tests${NC}"
echo -e "Failed: ${RED}$failed_tests${NC}"
echo ""
echo "Duration: ${duration}s"
echo ""

if [ $failed_tests -eq 0 ]; then
    echo -e "${GREEN}All tests passed! ✓${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed! ✗${NC}"
    exit 1
fi