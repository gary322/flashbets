#!/bin/bash

# Test script for live Polymarket API integration

echo "Testing Polymarket Live API Integration..."

# Start the server in the background
echo "Starting server..."
cargo run &
SERVER_PID=$!

# Wait for server to start
sleep 5

# Test integration status endpoint
echo -e "\n1. Testing integration status..."
curl -s http://localhost:8081/api/integration/status | jq .

# Test Polymarket markets endpoint
echo -e "\n2. Testing live Polymarket markets..."
curl -s http://localhost:8081/api/integration/polymarket/markets | jq .

# Kill the server
echo -e "\nStopping server..."
kill $SERVER_PID

echo "Test complete."