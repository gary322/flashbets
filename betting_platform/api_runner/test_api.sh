#!/bin/bash
# API Test Script for Boom Platform

API_URL="http://localhost:8081/api"
WS_URL="ws://localhost:8081"

echo "=== Boom Platform API Test Suite ==="
echo

# Health check
echo "1. Testing health endpoint..."
curl -s "${API_URL%/api}/health" | jq .
echo

# Program info
echo "2. Testing program info..."
curl -s "$API_URL/program/info" | jq .
echo

# Markets
echo "3. Testing markets endpoint..."
curl -s "$API_URL/markets" | jq '.[] | {id, title, total_liquidity, total_volume}' | head -20
echo

# Polymarket proxy
echo "4. Testing Polymarket integration..."
curl -s "$API_URL/polymarket/markets" | jq '.[0]' 
echo

# Integration status
echo "5. Testing integration status..."
curl -s "$API_URL/integration/status" | jq .
echo

# Portfolio for test wallet
TEST_WALLET="11111111111111111111111111111111"
echo "6. Testing portfolio endpoint..."
curl -s "$API_URL/portfolio/$TEST_WALLET" | jq '.summary'
echo

# Risk metrics
echo "7. Testing risk metrics..."
curl -s "$API_URL/risk/$TEST_WALLET" | jq '.risk_metrics | {var_95, var_99, risk_score, risk_rating}'
echo

# Create demo account
echo "8. Testing demo account creation..."
curl -s -X POST "$API_URL/wallet/demo/create" -H "Content-Type: application/json" -d '{}' | jq .
echo

# WebSocket test
echo "9. Testing WebSocket connection..."
echo "Connecting to $WS_URL/ws/v2 ..."
timeout 5 websocat "$WS_URL/ws/v2" <<EOF
{"action":"subscribe","subscriptions":[{"type":"SystemStatus"}]}
{"action":"ping","timestamp":$(date +%s)}
EOF
echo

echo "=== Test Suite Complete ==="