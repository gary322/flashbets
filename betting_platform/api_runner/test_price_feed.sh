#!/bin/bash

# Test script for real-time price feed functionality

echo "Testing Real-time Price Feed Integration..."

# Start the server in the background
echo "Starting server..."
cargo run > /dev/null 2>&1 & 
SERVER_PID=$!

# Wait for server to start
sleep 5

# Test getting price for a market
echo -e "\n1. Testing price endpoint for a Polymarket market..."
MARKET_ID="0xe3b423dfad8c22ff75c9899c4e8176f628cf4ad4caa00481764d320e7415f7a9"
curl -s "http://localhost:8081/api/prices/$MARKET_ID" | python3 -m json.tool

# Test tracking a market for price updates
echo -e "\n2. Testing market price tracking..."
curl -s -X POST http://localhost:8081/api/prices/track \
  -H "Content-Type: application/json" \
  -d '{
    "polymarket_id": "'$MARKET_ID'",
    "internal_id": "1000"
  }' | python3 -m json.tool

# Test WebSocket connection (just connect and disconnect)
echo -e "\n3. Testing WebSocket price feed connection..."
(echo '{"type":"subscribe","market_id":"'$MARKET_ID'"}'; sleep 2) | \
  websocat ws://localhost:8081/api/prices/ws 2>&1 | head -5 || echo "WebSocket test requires 'websocat' tool"

# Kill the server
echo -e "\nStopping server..."
kill $SERVER_PID 2>/dev/null

echo "Test complete."