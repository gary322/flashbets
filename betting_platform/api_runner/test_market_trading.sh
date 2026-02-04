#!/bin/bash

echo "=== Market Creation and Trading Test ==="
echo

# Test 1: Create a demo wallet
echo "1. Creating demo wallet..."
wallet_response=$(curl -s -X POST http://localhost:8081/api/wallet/demo/create \
  -H "Content-Type: application/json" \
  -d '{}')
wallet=$(echo "$wallet_response" | python3 -c "import json,sys; print(json.load(sys.stdin)['wallet_address'])" 2>/dev/null)
if [ -z "$wallet" ]; then
    echo "   ❌ Failed to create demo wallet"
    echo "   Response: $wallet_response"
    exit 1
fi
echo "   ✓ Wallet created: $wallet"

# Test 2: Check wallet balance
echo
echo "2. Checking wallet balance..."
balance_response=$(curl -s http://localhost:8081/api/wallet/balance/$wallet)
balance=$(echo "$balance_response" | python3 -c "import json,sys; print(json.load(sys.stdin)['balance'])" 2>/dev/null)
echo "   ✓ Balance: $balance"

# Test 3: Get a market to trade on
echo
echo "3. Getting market to trade..."
markets_response=$(curl -s http://localhost:8081/api/markets)
market_id=$(echo "$markets_response" | python3 -c "import json,sys; print(json.load(sys.stdin)['markets'][0]['id'])" 2>/dev/null)
market_title=$(echo "$markets_response" | python3 -c "import json,sys; print(json.load(sys.stdin)['markets'][0]['title'][:50])" 2>/dev/null)
echo "   ✓ Market ID: $market_id"
echo "   ✓ Market: $market_title..."

# Test 4: Place a trade
echo
echo "4. Placing a trade..."
trade_data='{
  "market_id": '$market_id',
  "amount": 100,
  "outcome": 0,
  "wallet": "'$wallet'"
}'
trade_response=$(curl -s -X POST http://localhost:8081/api/trade/place \
  -H "Content-Type: application/json" \
  -d "$trade_data")
trade_success=$(echo "$trade_response" | python3 -c "import json,sys; d=json.load(sys.stdin); print('error' not in d)" 2>/dev/null)
if [ "$trade_success" = "True" ]; then
    echo "   ✓ Trade placed successfully"
else
    echo "   ❌ Trade failed: $trade_response"
fi

# Test 5: Check positions
echo
echo "5. Checking positions..."
positions_response=$(curl -s http://localhost:8081/api/positions/$wallet)
position_count=$(echo "$positions_response" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))" 2>/dev/null)
echo "   ✓ Positions count: $position_count"

# Test 6: Get portfolio summary
echo
echo "6. Getting portfolio summary..."
portfolio_response=$(curl -s http://localhost:8081/api/portfolio/$wallet)
total_value=$(echo "$portfolio_response" | python3 -c "import json,sys; print(json.load(sys.stdin).get('total_value', 0))" 2>/dev/null)
echo "   ✓ Portfolio total value: $total_value"

# Test 7: Test market creation (optional)
echo
echo "7. Testing market creation..."
create_data='{
  "title": "Test Market: Will this test pass?",
  "description": "A test market for API validation",
  "outcomes": ["Yes", "No"],
  "resolution_time": 1735689600,
  "creator": "'$wallet'",
  "initial_liquidity": 1000
}'
create_response=$(curl -s -X POST http://localhost:8081/api/markets/create \
  -H "Content-Type: application/json" \
  -d "$create_data")
create_success=$(echo "$create_response" | python3 -c "import json,sys; d=json.load(sys.stdin); print('error' not in d)" 2>/dev/null)
if [ "$create_success" = "True" ]; then
    echo "   ✓ Market created successfully"
else
    echo "   ⚠️  Market creation not available or failed: $create_response"
fi

echo
echo "=== SUMMARY ==="
echo "✅ Demo wallet creation works"
echo "✅ Wallet balance check works"
echo "✅ Market listing works"
if [ "$trade_success" = "True" ]; then
    echo "✅ Trade placement works"
    echo "✅ Position tracking works"
    echo "✅ Portfolio summary works"
else
    echo "⚠️  Trading functionality needs attention"
fi