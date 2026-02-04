#!/bin/bash

# Test runner that starts the server and runs all tests

echo "🚀 Betting Platform API Full Test Suite"
echo "======================================"
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Kill any existing server
echo "Cleaning up existing processes..."
pkill -f "betting_platform_api" 2>/dev/null

# Build the project
echo -e "${BLUE}Building project...${NC}"
if cargo build --release; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi

# Start the server in background
echo -e "${BLUE}Starting API server...${NC}"
RUST_LOG=betting_platform_api=debug cargo run --release > server_test.log 2>&1 &
SERVER_PID=$!

# Wait for server to start
echo -n "Waiting for server to start"
for i in {1..30}; do
    if curl -s http://localhost:8081/health > /dev/null 2>&1; then
        echo -e " ${GREEN}✓${NC}"
        break
    fi
    echo -n "."
    sleep 1
done

# Check if server started
if ! curl -s http://localhost:8081/health > /dev/null 2>&1; then
    echo -e " ${RED}✗${NC}"
    echo "Server failed to start. Check server_test.log for details"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo ""
echo "Running Test Suite"
echo "=================="

# Run the comprehensive test script
./run_all_tests.sh

# Additional API tests
echo ""
echo "Extended API Tests"
echo "=================="

# Test trading endpoints
echo -e "${BLUE}Testing trading endpoints...${NC}"

# Place a trade
TRADE_RESPONSE=$(curl -s -X POST http://localhost:8081/api/trades \
    -H "Content-Type: application/json" \
    -d '{
        "market_id": 1000,
        "outcome": 0,
        "amount": 1000,
        "wallet": "demo_wallet_001",
        "leverage": 1
    }')

if echo "$TRADE_RESPONSE" | grep -q "success"; then
    echo -e "${GREEN}✓ Trade placement works${NC}"
else
    echo -e "${RED}✗ Trade placement failed${NC}"
    echo "$TRADE_RESPONSE"
fi

# Test positions endpoint
POSITIONS_RESPONSE=$(curl -s "http://localhost:8081/api/positions?wallet=demo_wallet_001")
if echo "$POSITIONS_RESPONSE" | grep -q "positions"; then
    echo -e "${GREEN}✓ Positions query works${NC}"
else
    echo -e "${RED}✗ Positions query failed${NC}"
fi

# Test quantum features
echo ""
echo -e "${BLUE}Testing quantum features...${NC}"

QUANTUM_RESPONSE=$(curl -s -X POST http://localhost:8081/api/quantum/trade \
    -H "Content-Type: application/json" \
    -d '{
        "verses": [0, 1, 2],
        "amount": 1000,
        "wallet": "demo_wallet_001"
    }')

if echo "$QUANTUM_RESPONSE" | grep -q "quantum_position_id"; then
    echo -e "${GREEN}✓ Quantum trading works${NC}"
else
    echo -e "${RED}✗ Quantum trading failed${NC}"
fi

# Test risk management
echo ""
echo -e "${BLUE}Testing risk management...${NC}"

RISK_RESPONSE=$(curl -s "http://localhost:8081/api/risk/metrics?wallet=demo_wallet_001")
if echo "$RISK_RESPONSE" | grep -q "total_exposure"; then
    echo -e "${GREEN}✓ Risk metrics work${NC}"
else
    echo -e "${RED}✗ Risk metrics failed${NC}"
fi

# Clean up
echo ""
echo "Cleaning up..."
kill $SERVER_PID 2>/dev/null
echo -e "${GREEN}✓ Server stopped${NC}"

echo ""
echo "Test Results Summary"
echo "==================="
echo ""
echo "Check server_test.log for detailed server output"
echo ""
echo -e "${GREEN}All tests completed!${NC}"