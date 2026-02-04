#!/bin/bash

# Simple test summary script
# Runs various test categories and reports results

echo "========================================="
echo "Simple Test Summary"
echo "========================================="
echo "Date: $(date)"
echo ""

# Counter for results
TOTAL=0
PASSED=0
FAILED=0

# Function to run tests and report
run_test() {
    local name=$1
    local cmd=$2
    
    echo -n "Testing $name... "
    TOTAL=$((TOTAL + 1))
    
    if $cmd > /dev/null 2>&1; then
        echo "✓ PASSED"
        PASSED=$((PASSED + 1))
    else
        echo "✗ FAILED"
        FAILED=$((FAILED + 1))
    fi
}

# Run various test categories
run_test "WebSocket" "cargo test --lib websocket -- --quiet"
run_test "Auth" "cargo test --lib auth -- --quiet"
run_test "Types" "cargo test --lib types -- --quiet"
run_test "Cache" "cargo test --lib cache -- --quiet"
run_test "Validation" "cargo test --lib validation -- --quiet"
run_test "Config" "cargo test --lib config -- --quiet"

echo ""
echo "========================================="
echo "Summary:"
echo "Total: $TOTAL"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
echo "========================================="

if [ $FAILED -eq 0 ]; then
    echo "✓ All tests passed!"
    exit 0
else
    echo "✗ Some tests failed"
    exit 1
fi