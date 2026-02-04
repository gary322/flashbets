#!/bin/bash

# Test script for quantum settlement functionality

echo "================================"
echo "Testing Quantum Settlement"
echo "================================"

BASE_URL="http://localhost:8081"

# Test 1: Create a quantum position first
echo -e "\n1. Creating quantum position..."
POSITION_RESPONSE=$(curl -s -X POST "$BASE_URL/api/quantum/create" \
  -H "Content-Type: application/json" \
  -d '{
    "wallet": "test-quantum-wallet",
    "market_id": 1001,
    "states": [
      {
        "outcome": 0,
        "probability": 0.6,
        "amount": 100000
      },
      {
        "outcome": 1,
        "probability": 0.4,
        "amount": 100000
      }
    ],
    "leverage": 2
  }')

echo "$POSITION_RESPONSE" | jq .

# Extract position ID (assuming it's returned)
POSITION_ID="test-quantum-pos-1"

# Test 2: Collapse the quantum position
echo -e "\n2. Collapsing quantum position..."
curl -s -X POST "$BASE_URL/quantum/collapse" \
  -H "Content-Type: application/json" \
  -d '{
    "position_id": "'$POSITION_ID'",
    "wallet": "test-quantum-wallet",
    "collapse_probability": 0.5,
    "signature": "test-signature"
  }' | jq .

# Test 3: Settle the collapsed position
echo -e "\n3. Settling quantum position..."
curl -s -X POST "$BASE_URL/api/quantum/settlement/position" \
  -H "Content-Type: application/json" \
  -d '{
    "position_id": "'$POSITION_ID'",
    "market_id": 1001,
    "winning_outcome": 0
  }' | jq .

# Test 4: Get quantum settlement status for market
echo -e "\n4. Getting quantum settlement status for market..."
curl -s "$BASE_URL/api/quantum/settlement/status/1001" | jq .

# Test 5: Get quantum settlement history
echo -e "\n5. Getting quantum settlement history..."
curl -s "$BASE_URL/api/quantum/settlement/history?wallet=test-quantum-wallet" | jq .

# Test 6: Settle all positions for a market
echo -e "\n6. Settling all quantum positions for market..."
curl -s -X POST "$BASE_URL/api/quantum/settlement/market" \
  -H "Content-Type: application/json" \
  -d '{
    "market_id": 1001,
    "winning_outcome": 0
  }' | jq .

# Test 7: Trigger automatic settlement
echo -e "\n7. Triggering automatic quantum settlement..."
curl -s -X POST "$BASE_URL/api/quantum/settlement/trigger" | jq .

echo -e "\n================================"
echo "Quantum settlement tests completed!"
echo "================================"
echo ""
echo "To test with a real quantum position:"
echo "1. Create a quantum position with multiple states"
echo "2. Wait for it to collapse (or force collapse)"
echo "3. Settle when market resolves"
echo ""
echo "Quantum settlement provides:"
echo "- Bonus rewards for maintaining superposition"
echo "- Coherence multipliers based on hold time"
echo "- Entanglement bonuses for correlated positions"