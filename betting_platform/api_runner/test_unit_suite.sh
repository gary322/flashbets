#!/bin/bash

# Unit Test Suite for Betting Platform API
# Runs cargo tests without external dependencies

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Unit Test Suite${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "Start Time: $(date)"
echo ""

# Function to run cargo tests
run_cargo_tests() {
    local test_name=$1
    local test_command=$2
    
    echo -e "${YELLOW}Running: $test_name${NC}"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if $test_command; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        echo -e "${GREEN}✓ PASSED: $test_name${NC}"
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        echo -e "${RED}✗ FAILED: $test_name${NC}"
    fi
    echo ""
}

# Change to the correct directory
cd /Users/nishu/Downloads/betting/betting_platform/api_runner

echo -e "${BLUE}Building the project...${NC}"
cargo build --release

echo -e "\n${BLUE}Running Unit Tests...${NC}"
echo -e "${BLUE}========================================${NC}\n"

# Run different test categories
run_cargo_tests "Core Library Tests" "cargo test --lib --release"
run_cargo_tests "WebSocket Tests" "cargo test --lib websocket --release"
run_cargo_tests "Cache Tests" "cargo test --lib cache --release"
run_cargo_tests "Validation Tests" "cargo test --lib validation --release"
run_cargo_tests "Auth Tests" "cargo test --lib auth --release"
run_cargo_tests "Rate Limit Tests" "cargo test --lib rate_limit --release"
run_cargo_tests "Security Tests" "cargo test --lib security --release"
run_cargo_tests "Type Tests" "cargo test --lib types --release"
run_cargo_tests "Serialization Tests" "cargo test --lib serialization --release"

# Generate summary
echo -e "\n${BLUE}========================================${NC}"
echo -e "${BLUE}Test Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "End Time: $(date)"
echo ""
echo -e "Total Tests Run: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"

if [ $TOTAL_TESTS -gt 0 ]; then
    local success_rate=$(( (PASSED_TESTS * 100) / TOTAL_TESTS ))
    echo -e "Success Rate: ${success_rate}%"
fi

echo ""

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}✓ ALL UNIT TESTS PASSED!${NC}"
    exit 0
else
    echo -e "${RED}✗ SOME UNIT TESTS FAILED${NC}"
    exit 1
fi