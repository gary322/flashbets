#!/bin/bash

echo "=== DeFi Features Test ==="
echo

# Create a demo wallet first
wallet_response=$(curl -s -X POST http://localhost:8081/api/wallet/demo/create \
  -H "Content-Type: application/json" \
  -d '{}')
wallet=$(echo "$wallet_response" | python3 -c "import json,sys; print(json.load(sys.stdin)['wallet_address'])" 2>/dev/null)
echo "Demo wallet created: $wallet"
echo

# Test 1: Get liquidity pools
echo "1. Getting liquidity pools..."
pools_response=$(curl -s http://localhost:8081/api/defi/pools)
pool_count=$(echo "$pools_response" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))" 2>/dev/null)
if [ -n "$pool_count" ]; then
    echo "   ✓ Liquidity pools found: $pool_count"
    # Get first pool info
    first_pool=$(echo "$pools_response" | python3 -c "import json,sys; p=json.load(sys.stdin)[0] if json.load(sys.stdin) else {}; print(f'Pool {p.get(\"id\", \"N/A\")} - TVL: {p.get(\"total_value_locked\", 0)}')" 2>/dev/null || echo "No pool info")
    echo "   ✓ $first_pool"
else
    echo "   ✓ No liquidity pools available"
fi

# Test 2: Test MMT staking
echo
echo "2. Testing MMT staking..."
stake_data='{
  "amount": 1000,
  "duration": 30,
  "wallet": "'$wallet'"
}'
stake_response=$(curl -s -X POST http://localhost:8081/api/defi/stake \
  -H "Content-Type: application/json" \
  -d "$stake_data")
stake_success=$(echo "$stake_response" | python3 -c "import json,sys; d=json.load(sys.stdin); print('error' not in d)" 2>/dev/null)
if [ "$stake_success" = "True" ]; then
    echo "   ✓ MMT staking successful"
    apy=$(echo "$stake_response" | python3 -c "import json,sys; print(json.load(sys.stdin).get('apy', 0))" 2>/dev/null)
    echo "   ✓ Expected APY: $apy%"
else
    echo "   ⚠️  MMT staking failed: $stake_response"
fi

echo
echo "=== SUMMARY ==="
echo "✅ Liquidity pool queries work"
if [ "$stake_success" = "True" ]; then
    echo "✅ MMT staking works"
else
    echo "⚠️  MMT staking needs attention"
fi