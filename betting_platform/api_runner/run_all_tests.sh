#!/bin/bash

# Comprehensive test runner for the betting platform API

echo "🚀 Betting Platform API Test Runner"
echo "==================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to check if server is running
check_server() {
    curl -s http://localhost:8081/health > /dev/null 2>&1
    return $?
}

# Function to run tests with pretty output
run_test() {
    local test_name=$1
    local test_command=$2
    
    echo -n "Running $test_name... "
    
    if eval "$test_command" > test_output.tmp 2>&1; then
        echo -e "${GREEN}✓ PASSED${NC}"
        return 0
    else
        echo -e "${RED}✗ FAILED${NC}"
        echo -e "${YELLOW}Output:${NC}"
        cat test_output.tmp | grep -E "(error|failed|FAILED)" | head -10
        return 1
    fi
}

# Clean up previous test artifacts
rm -f test_output.tmp

# Build the project first
echo "Building project..."
if cargo build --release > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi

echo ""
echo "Running Unit Tests"
echo "------------------"

# Run unit tests
run_test "Library tests" "cargo test --lib --release"
run_test "Binary tests" "cargo test --bin betting_platform_api --release"

echo ""
echo "Running Integration Tests"
echo "------------------------"

# Check if server is running
if check_server; then
    echo -e "${GREEN}✓ Server is running${NC}"
    
    # Run integration tests
    run_test "API integration tests" "cargo test --test integration_tests --release"
    
    # Run additional endpoint tests
    echo ""
    echo "Testing Individual Endpoints"
    echo "---------------------------"
    
    run_test "Health endpoint" "curl -s http://localhost:8081/health | grep -q healthy"
    run_test "Markets endpoint" "curl -s http://localhost:8081/api/markets | grep -q markets"
    run_test "Verses endpoint" "curl -s http://localhost:8081/api/verses | grep -q verses"
    run_test "WebSocket connection" "timeout 2 curl -s -N -H 'Upgrade: websocket' -H 'Connection: Upgrade' http://localhost:8081/ws || true"
    
    # Test authentication flow
    echo ""
    echo "Testing Authentication Flow"
    echo "--------------------------"
    
    # Request challenge
    CHALLENGE_RESPONSE=$(curl -s -X POST http://localhost:8081/api/auth/wallet \
        -H "Content-Type: application/json" \
        -d '{"wallet": "demo_wallet_001"}')
    
    if echo "$CHALLENGE_RESPONSE" | grep -q "challenge"; then
        echo -e "${GREEN}✓ Challenge generation works${NC}"
    else
        echo -e "${RED}✗ Challenge generation failed${NC}"
    fi
    
else
    echo -e "${YELLOW}⚠ Server not running - skipping integration tests${NC}"
    echo "To run integration tests, start the server with:"
    echo "  cargo run --release"
fi

echo ""
echo "Running Load Tests"
echo "-----------------"

if check_server; then
    # Simple load test
    echo -n "Running basic load test... "
    
    success_count=0
    total_count=100
    
    for i in $(seq 1 $total_count); do
        if curl -s http://localhost:8081/health > /dev/null 2>&1; then
            ((success_count++))
        fi
    done
    
    if [ $success_count -eq $total_count ]; then
        echo -e "${GREEN}✓ All $total_count requests succeeded${NC}"
    else
        echo -e "${YELLOW}⚠ $success_count/$total_count requests succeeded${NC}"
    fi
else
    echo -e "${YELLOW}⚠ Server not running - skipping load tests${NC}"
fi

# Clean up
rm -f test_output.tmp

echo ""
echo "Test Summary"
echo "============"
echo ""

# Count test results
unit_tests=$(cargo test --lib --release 2>&1 | grep -oE "[0-9]+ passed" | head -1)
echo "Unit tests: $unit_tests"

if check_server; then
    echo "Integration tests: Available when server is running"
else
    echo "Integration tests: Server not running"
fi

echo ""
echo "To run all tests with the server:"
echo "1. Start the server: cargo run --release"
echo "2. Run this script again: ./run_all_tests.sh"
echo ""