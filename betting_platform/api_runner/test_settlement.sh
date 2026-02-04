#!/bin/bash

# Test script for settlement endpoints

echo "Testing Settlement Endpoints..."

# Start the server in the background
echo "Starting server..."
cargo run > /dev/null 2>&1 & 
SERVER_PID=$!

# Wait for server to start
sleep 5

# Test settlement status for a Polymarket market
echo -e "\n1. Testing settlement status endpoint..."
# Using a real Polymarket condition ID (this is an example)
curl -s "http://localhost:8081/api/settlement/status/0x1234567890abcdef" | python3 -m json.tool

# Test pending settlements
echo -e "\n2. Testing pending settlements endpoint..."
curl -s "http://localhost:8081/api/settlement/pending?limit=10" | python3 -m json.tool

# Test user settlements
echo -e "\n3. Testing user settlements..."
TEST_WALLET="0x742d35Cc6634C0532925a3b844Bc9e7595f8b9d0"
curl -s "http://localhost:8081/api/settlement/user/$TEST_WALLET" | python3 -m json.tool

# Test historical settlements
echo -e "\n4. Testing historical settlements..."
curl -s "http://localhost:8081/api/settlement/historical?days=7" | python3 -m json.tool

# Test oracle information
echo -e "\n5. Testing oracle information..."
curl -s "http://localhost:8081/api/settlement/oracle/0x1234567890abcdef" | python3 -m json.tool

# Test settlement webhook (simulated)
echo -e "\n6. Testing settlement webhook..."
curl -s -X POST http://localhost:8081/api/settlement/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "event_type": "market.resolved",
    "condition_id": "0x1234567890abcdef",
    "resolution": 1,
    "timestamp": 1700000000,
    "signature": "mock_signature"
  }' | python3 -m json.tool

# Kill the server
echo -e "\nStopping server..."
kill $SERVER_PID 2>/dev/null

echo "Test complete."