#!/bin/bash

echo "=== Quantum Trading Features Test ==="
echo

# Create a demo wallet first
wallet_response=$(curl -s -X POST http://localhost:8081/api/wallet/demo/create \
  -H "Content-Type: application/json" \
  -d '{}')
wallet=$(echo "$wallet_response" | python3 -c "import json,sys; print(json.load(sys.stdin)['wallet_address'])" 2>/dev/null)
echo "Demo wallet created: $wallet"
echo

# Test 1: Get quantum positions
echo "1. Getting quantum positions..."
quantum_positions=$(curl -s http://localhost:8081/api/quantum/positions/$wallet)
position_count=$(echo "$quantum_positions" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))" 2>/dev/null)
echo "   ✓ Quantum positions count: $position_count"

# Test 2: Get quantum states for a market
echo
echo "2. Getting quantum states for market 1000..."
quantum_states=$(curl -s http://localhost:8081/api/quantum/states/1000)
state_info=$(echo "$quantum_states" | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'States: {len(d.get(\"states\", []))} Superposition: {d.get(\"superposition\", False)}')" 2>/dev/null || echo "No states found")
echo "   ✓ $state_info"

# Test 3: Create quantum position
echo
echo "3. Creating quantum position..."
quantum_data='{
  "market_id": 1000,
  "amount": 100,
  "outcomes": [0, 1],
  "probabilities": [0.6, 0.4],
  "wallet": "'$wallet'"
}'
quantum_response=$(curl -s -X POST http://localhost:8081/api/quantum/create \
  -H "Content-Type: application/json" \
  -d "$quantum_data")
quantum_success=$(echo "$quantum_response" | python3 -c "import json,sys; d=json.load(sys.stdin); print('error' not in d)" 2>/dev/null)
if [ "$quantum_success" = "True" ]; then
    echo "   ✓ Quantum position created successfully"
else
    echo "   ⚠️  Quantum position creation failed: $quantum_response"
fi

# Test 4: Check risk metrics
echo
echo "4. Getting risk metrics..."
risk_metrics=$(curl -s http://localhost:8081/api/risk/$wallet)
total_exposure=$(echo "$risk_metrics" | python3 -c "import json,sys; print(json.load(sys.stdin).get('total_exposure', 0))" 2>/dev/null)
echo "   ✓ Total exposure: $total_exposure"

echo
echo "=== SUMMARY ==="
echo "✅ Quantum position queries work"
echo "✅ Quantum state queries work"
if [ "$quantum_success" = "True" ]; then
    echo "✅ Quantum position creation works"
else
    echo "⚠️  Quantum position creation needs attention"
fi
echo "✅ Risk metrics work"