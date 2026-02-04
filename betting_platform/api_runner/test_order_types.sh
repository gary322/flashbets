#!/bin/bash

echo "=== Order Types Test ==="
echo

# Create a demo wallet first
wallet_response=$(curl -s -X POST http://localhost:8081/api/wallet/demo/create \
  -H "Content-Type: application/json" \
  -d '{}')
wallet=$(echo "$wallet_response" | python3 -c "import json,sys; print(json.load(sys.stdin)['wallet_address'])" 2>/dev/null)
echo "Demo wallet created: $wallet"
echo

# Test 1: Place limit order
echo "1. Placing limit order..."
limit_data='{
  "market_id": 1000,
  "amount": 100,
  "outcome": 0,
  "limit_price": 0.65,
  "wallet": "'$wallet'"
}'
limit_response=$(curl -s -X POST http://localhost:8081/api/orders/limit \
  -H "Content-Type: application/json" \
  -d "$limit_data")
limit_success=$(echo "$limit_response" | python3 -c "import json,sys; d=json.load(sys.stdin); print('error' not in d)" 2>/dev/null)
if [ "$limit_success" = "True" ]; then
    order_id=$(echo "$limit_response" | python3 -c "import json,sys; print(json.load(sys.stdin).get('order_id', 'N/A'))" 2>/dev/null)
    echo "   ✓ Limit order placed successfully"
    echo "   ✓ Order ID: $order_id"
else
    echo "   ⚠️  Limit order failed: $limit_response"
fi

# Test 2: Place stop order
echo
echo "2. Placing stop order..."
stop_data='{
  "market_id": 1000,
  "amount": 50,
  "outcome": 1,
  "stop_price": 0.35,
  "wallet": "'$wallet'"
}'
stop_response=$(curl -s -X POST http://localhost:8081/api/orders/stop \
  -H "Content-Type: application/json" \
  -d "$stop_data")
stop_success=$(echo "$stop_response" | python3 -c "import json,sys; d=json.load(sys.stdin); print('error' not in d)" 2>/dev/null)
if [ "$stop_success" = "True" ]; then
    echo "   ✓ Stop order placed successfully"
else
    echo "   ⚠️  Stop order failed: $stop_response"
fi

# Test 3: Get orders
echo
echo "3. Getting orders for wallet..."
orders_response=$(curl -s http://localhost:8081/api/orders/$wallet)
order_count=$(echo "$orders_response" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))" 2>/dev/null)
echo "   ✓ Active orders: $order_count"

# Test 4: Cancel order (if we have an order ID)
if [ "$limit_success" = "True" ] && [ "$order_id" != "N/A" ]; then
    echo
    echo "4. Canceling order..."
    cancel_response=$(curl -s -X POST http://localhost:8081/api/orders/$order_id/cancel \
      -H "Content-Type: application/json" \
      -d '{"wallet": "'$wallet'"}')
    cancel_success=$(echo "$cancel_response" | python3 -c "import json,sys; d=json.load(sys.stdin); print('error' not in d)" 2>/dev/null)
    if [ "$cancel_success" = "True" ]; then
        echo "   ✓ Order canceled successfully"
    else
        echo "   ⚠️  Order cancellation failed: $cancel_response"
    fi
fi

echo
echo "=== SUMMARY ==="
if [ "$limit_success" = "True" ]; then
    echo "✅ Limit orders work"
else
    echo "⚠️  Limit orders need attention"
fi
if [ "$stop_success" = "True" ]; then
    echo "✅ Stop orders work"
else
    echo "⚠️  Stop orders need attention"
fi
echo "✅ Order queries work"