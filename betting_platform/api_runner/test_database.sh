#!/bin/bash

# Test database endpoints

API_URL="http://localhost:8081"

echo "Testing Database Endpoints..."
echo "============================="

# 1. Test database status
echo -e "\n1. Testing database status:"
curl -s "$API_URL/api/db/status" | jq '.'

# 2. Record a user login
echo -e "\n2. Recording user login:"
curl -s -X POST "$API_URL/api/db/user/login" \
  -H "Content-Type: application/json" \
  -d '{
    "wallet_address": "11111111111111111111111111111111"
  }' | jq '.'

# 3. Get user stats
echo -e "\n3. Getting user stats:"
curl -s "$API_URL/api/db/user/11111111111111111111111111111111/stats" | jq '.'

# 4. Record a trade
echo -e "\n4. Recording a trade:"
curl -s -X POST "$API_URL/api/db/trade/record" \
  -H "Content-Type: application/json" \
  -d '{
    "trade_id": "trade_'$(date +%s)'",
    "wallet_address": "11111111111111111111111111111111",
    "market_id": "market_test_001",
    "chain": "solana",
    "market_title": "Test Market",
    "market_description": "A test market for database integration",
    "market_end_time": "'$(date -u -d "+1 day" +"%Y-%m-%dT%H:%M:%SZ")'",
    "trade_type": "buy",
    "outcome": 1,
    "amount": 100000,
    "price": 0.65,
    "fee": 250,
    "signature": "sig_'$(date +%s)'"
  }' | jq '.'

# 5. Get user trades
echo -e "\n5. Getting user trades:"
curl -s "$API_URL/api/db/trades/11111111111111111111111111111111?limit=10" | jq '.'

echo -e "\nDatabase endpoint tests completed!"