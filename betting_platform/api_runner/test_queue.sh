#!/bin/bash

# Test script for message queue functionality

echo "===================="
echo "Testing Message Queue"
echo "===================="

BASE_URL="http://localhost:8081"

# Test 1: Get queue statistics
echo -e "\n1. Getting queue statistics..."
curl -s "$BASE_URL/api/queue/stats" | jq .

# Test 2: Get queue lengths
echo -e "\n2. Getting queue lengths..."
curl -s "$BASE_URL/api/queue/lengths" | jq .

# Test 3: Publish test trade message
echo -e "\n3. Publishing test trade message..."
curl -s -X POST "$BASE_URL/api/queue/publish/test" \
  -H "Content-Type: application/json" \
  -d '{
    "message_type": "trade",
    "wallet": "test-wallet-123",
    "market_id": "1001",
    "amount": 100000,
    "outcome": 0
  }' | jq .

# Test 4: Publish test market message
echo -e "\n4. Publishing test market message..."
curl -s -X POST "$BASE_URL/api/queue/publish/test" \
  -H "Content-Type: application/json" \
  -d '{
    "message_type": "market",
    "title": "Test Market Creation"
  }' | jq .

# Test 5: Publish test risk alert
echo -e "\n5. Publishing test risk alert..."
curl -s -X POST "$BASE_URL/api/queue/publish/test" \
  -H "Content-Type: application/json" \
  -d '{
    "message_type": "risk",
    "wallet": "high-risk-wallet",
    "alert_type": "leverage_exceeded",
    "severity": "high"
  }' | jq .

# Test 6: Publish test cache invalidation
echo -e "\n6. Publishing test cache invalidation..."
curl -s -X POST "$BASE_URL/api/queue/publish/test" \
  -H "Content-Type: application/json" \
  -d '{
    "message_type": "cache",
    "patterns": ["markets:*", "user:test-wallet-123:*"]
  }' | jq .

# Test 7: Publish delayed message
echo -e "\n7. Publishing delayed message (5 seconds)..."
curl -s -X POST "$BASE_URL/api/queue/publish/delayed" \
  -H "Content-Type: application/json" \
  -d '{
    "patterns": ["delayed:test:*"],
    "delay_seconds": 5
  }' | jq .

# Test 8: Get updated queue lengths
echo -e "\n8. Getting updated queue lengths..."
curl -s "$BASE_URL/api/queue/lengths" | jq .

# Test 9: Get updated statistics
echo -e "\n9. Getting updated statistics..."
curl -s "$BASE_URL/api/queue/stats" | jq .

echo -e "\n===================="
echo "Queue tests completed!"
echo "===================="
echo ""
echo "To clear a queue (admin only):"
echo "curl -X POST $BASE_URL/api/queue/clear/trades"
echo ""
echo "Available queues: trades, markets, settlements, risk_alerts, notifications, general, dead_letter"