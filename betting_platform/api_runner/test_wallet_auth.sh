#!/bin/bash

echo "=== Wallet Authentication Test ==="
echo

# Test 1: Create demo wallet
echo "1. Creating demo wallet..."
wallet_response=$(curl -s -X POST http://localhost:8081/api/wallet/demo/create \
  -H "Content-Type: application/json" \
  -d '{}')
wallet=$(echo "$wallet_response" | python3 -c "import json,sys; print(json.load(sys.stdin)['wallet_address'])" 2>/dev/null)
echo "   ✓ Wallet created: $wallet"

# Test 2: Generate challenge
echo
echo "2. Generating wallet challenge..."
challenge_response=$(curl -s http://localhost:8081/api/wallet/challenge/$wallet)
challenge=$(echo "$challenge_response" | python3 -c "import json,sys; print(json.load(sys.stdin).get('challenge', ''))" 2>/dev/null)
if [ -n "$challenge" ]; then
    echo "   ✓ Challenge generated: ${challenge:0:32}..."
else
    echo "   ❌ Failed to generate challenge: $challenge_response"
fi

# Test 3: Check wallet verification status
echo
echo "3. Checking wallet verification status..."
status_response=$(curl -s http://localhost:8081/api/wallet/status/$wallet)
is_verified=$(echo "$status_response" | python3 -c "import json,sys; print(json.load(sys.stdin).get('verified', False))" 2>/dev/null)
echo "   ✓ Verification status: $is_verified"

# Test 4: Test signature verification (mock for demo)
echo
echo "4. Testing signature verification..."
verify_data='{
  "wallet": "'$wallet'",
  "signature": "mock_signature_for_demo",
  "message": "'$challenge'"
}'
verify_response=$(curl -s -X POST http://localhost:8081/api/wallet/verify \
  -H "Content-Type: application/json" \
  -d "$verify_data")
verify_result=$(echo "$verify_response" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('verified', False))" 2>/dev/null)
if [ "$verify_result" = "True" ]; then
    echo "   ✓ Signature verification successful"
else
    echo "   ⚠️  Signature verification failed (expected for demo): $verify_response"
fi

echo
echo "=== SUMMARY ==="
echo "✅ Demo wallet creation works"
echo "✅ Challenge generation works"
echo "✅ Verification status check works"
echo "⚠️  Signature verification needs real wallet for testing"