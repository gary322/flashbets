#!/bin/bash

# Test script for Polygon wallet integration

echo "Testing Polygon Wallet Integration..."

# Test wallet address (example Polygon address)
TEST_ADDRESS="0x742d35Cc6634C0532925a3b844Bc9e7595f8b9d0"

# Start the server in the background
echo "Starting server..."
cargo run > /dev/null 2>&1 & 
SERVER_PID=$!

# Wait for server to start
sleep 5

# Test getting wallet balance
echo -e "\n1. Testing wallet balance endpoint..."
curl -s "http://localhost:8081/api/wallet/polygon/balance/$TEST_ADDRESS" | python3 -m json.tool

# Test getting gas price
echo -e "\n2. Testing gas price endpoint..."
curl -s "http://localhost:8081/api/wallet/polygon/gas-price" | python3 -m json.tool

# Test getting nonce
echo -e "\n3. Testing wallet nonce endpoint..."
curl -s "http://localhost:8081/api/wallet/polygon/nonce/$TEST_ADDRESS" | python3 -m json.tool

# Test outcome token balance (with example token ID)
echo -e "\n4. Testing outcome token balance..."
curl -s "http://localhost:8081/api/wallet/polygon/outcome-balance/$TEST_ADDRESS?token_id=0x1234567890abcdef" | python3 -m json.tool

# Test gas estimation for USDC approval
echo -e "\n5. Testing gas estimation..."
curl -s -X POST http://localhost:8081/api/wallet/polygon/estimate-gas \
  -H "Content-Type: application/json" \
  -d '{
    "from": "'$TEST_ADDRESS'",
    "spender": "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E",
    "amount": "1000000"
  }' | python3 -m json.tool

# Kill the server
echo -e "\nStopping server..."
kill $SERVER_PID 2>/dev/null

echo "Test complete."