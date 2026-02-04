#!/bin/bash

# Test script for betting platform API endpoints

API_BASE="http://localhost:8081/api"
WALLET="5EPjFE65uKGGnXbdXPBFGSEkZqGN2pyQ6xjzbTh8gKLu"

echo "=== Testing Betting Platform API Endpoints ==="
echo ""

# Test 1: Program Info
echo "1. Testing Program Info..."
curl -s "$API_BASE/program/info" | jq '.'
echo ""

# Test 2: Markets
echo "2. Testing Get Markets..."
curl -s "$API_BASE/markets" | jq '.'
echo ""

# Test 3: Polymarket Integration
echo "3. Testing Polymarket Markets..."
curl -s "$API_BASE/polymarket/markets" | head -c 500
echo "..."
echo ""

# Test 4: Place Trade (Market Order)
echo "4. Testing Place Trade (Market Order)..."
curl -s -X POST "$API_BASE/trade" \
  -H "Content-Type: application/json" \
  -d '{
    "market_id": 1,
    "amount": 1000000000,
    "outcome": 0,
    "leverage": 10,
    "order_type": "market"
  }' | jq '.'
echo ""

# Test 5: Place Trade (Limit Order with Stop Loss)
echo "5. Testing Place Trade (Limit Order with Stop Loss)..."
curl -s -X POST "$API_BASE/trade" \
  -H "Content-Type: application/json" \
  -d '{
    "market_id": 1,
    "amount": 1000000000,
    "outcome": 1,
    "leverage": 5,
    "order_type": "limit",
    "limit_price": 0.65,
    "stop_loss": 0.55
  }' | jq '.'
echo ""

# Test 6: Get Positions
echo "6. Testing Get Positions..."
curl -s "$API_BASE/positions/$WALLET" | jq '.'
echo ""

# Test 7: Close Position
echo "7. Testing Close Position..."
curl -s -X POST "$API_BASE/positions/close" \
  -H "Content-Type: application/json" \
  -d '{
    "market_id": 1,
    "position_index": 0
  }' | jq '.'
echo ""

# Test 8: Get Quantum Positions
echo "8. Testing Get Quantum Positions..."
curl -s "$API_BASE/quantum/positions/$WALLET" | jq '.'
echo ""

# Test 9: Create Quantum Position
echo "9. Testing Create Quantum Position..."
curl -s -X POST "$API_BASE/quantum/create" \
  -H "Content-Type: application/json" \
  -d '{
    "market_id": 1,
    "amount": 2000000000,
    "outcomes": [0, 1],
    "leverage": 20
  }' | jq '.'
echo ""

# Test 10: Get Portfolio
echo "10. Testing Get Portfolio..."
curl -s "$API_BASE/portfolio/$WALLET" | jq '.'
echo ""

# Test 11: Get Balance
echo "11. Testing Get Balance..."
curl -s "$API_BASE/balance/$WALLET" | jq '.'
echo ""

# Test 12: Create Demo Account
echo "12. Testing Create Demo Account..."
curl -s -X POST "$API_BASE/demo-account" \
  -H "Content-Type: application/json" \
  -d '{}' | jq '.'
echo ""

echo "=== All Tests Completed ==="