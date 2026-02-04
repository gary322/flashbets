#!/bin/bash

# Run all comprehensive tests for the betting platform

echo "🚀 Running Comprehensive Test Suite for Betting Platform"
echo "======================================================="

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to run a test suite
run_test_suite() {
    local test_name=$1
    local test_command=$2
    
    echo -e "\n${YELLOW}Running: ${test_name}${NC}"
    echo "----------------------------------------"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if eval "$test_command"; then
        echo -e "${GREEN}✅ ${test_name} PASSED${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}❌ ${test_name} FAILED${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
}

# Change to project root
cd "$(dirname "$0")/betting_platform" || exit 1

echo "📍 Working directory: $(pwd)"

# 1. Run Rust tests for CU verification
run_test_suite "CU Verification Tests" \
    "cd programs/betting_platform_native && cargo test test_cu_verification -- --nocapture"

# 2. Run Rust tests for enhanced sharding
run_test_suite "Enhanced Sharding Tests" \
    "cd programs/betting_platform_native && cargo test test_enhanced_sharding -- --nocapture"

# 3. Run Rust integration tests
run_test_suite "Integration Tests" \
    "cd programs/betting_platform_native && cargo test test_integration -- --nocapture"

# 4. Run TypeScript tests for UI components
run_test_suite "UI Component Tests (CurveEditor)" \
    "cd app && npm test -- src/ui/components/__tests__/CurveEditor.test.tsx --passWithNoTests"

run_test_suite "UI Component Tests (TradingWizard)" \
    "cd app && npm test -- src/ui/components/__tests__/TradingWizard.test.tsx --passWithNoTests"

# 5. Run specific feature tests
echo -e "\n${YELLOW}Running Feature-Specific Tests${NC}"
echo "----------------------------------------"

# Test PDA size validation
run_test_suite "PDA Size Validation" \
    "cd programs/betting_platform_native && cargo test test_pda_size_validation -- --nocapture"

# Test oracle integration
run_test_suite "Oracle Integration" \
    "cd programs/betting_platform_native && cargo test test_oracle_integration -- --nocapture"

# Test performance benchmarks
run_test_suite "Performance Benchmarks" \
    "cd programs/betting_platform_native && cargo test test_performance_benchmarks -- --nocapture"

# Summary
echo -e "\n======================================================="
echo -e "${YELLOW}📊 Test Summary${NC}"
echo "======================================================="
echo -e "Total Tests Run: ${TOTAL_TESTS}"
echo -e "${GREEN}Passed: ${PASSED_TESTS}${NC}"
echo -e "${RED}Failed: ${FAILED_TESTS}${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "\n${GREEN}🎉 All tests passed successfully!${NC}"
    exit 0
else
    echo -e "\n${RED}⚠️  Some tests failed. Please check the output above.${NC}"
    exit 1
fi