#!/bin/bash

echo "=== RBAC Authorization Test ==="
echo ""

API_URL="http://localhost:8081"

# Test wallets with different roles
USER_WALLET="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
ADMIN_WALLET="Admin5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
MARKET_MAKER_WALLET="MMaker5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"

MESSAGE="Sign this message to authenticate with betting platform"
SIGNATURE="5VpX6Y2V7b4v5vLKH2K5n3F5u8F6h8J9k3L4m6N7p8Q9r2S3t4U5v6W7x8Y9z"

# Check if API is running
if ! curl -s $API_URL/health > /dev/null; then
    echo "ERROR: API is not running. Please start the API first."
    exit 1
fi

echo "1. Login as regular user..."
USER_LOGIN=$(curl -s -X POST $API_URL/api/auth/login \
    -H "Content-Type: application/json" \
    -d "{
        \"wallet\": \"$USER_WALLET\",
        \"signature\": \"$SIGNATURE\",
        \"message\": \"$MESSAGE\"
    }")

USER_TOKEN=$(echo $USER_LOGIN | grep -o '"access_token":"[^"]*' | cut -d'"' -f4)
echo "User token obtained"
echo ""

echo "2. Check user permissions..."
USER_PERMS=$(curl -s -X GET $API_URL/api/rbac/permissions \
    -H "Authorization: Bearer $USER_TOKEN")
echo "User permissions: $USER_PERMS"
echo ""

echo "3. Try to access admin endpoint (should fail)..."
ADMIN_ACCESS=$(curl -s -w "\nHTTP Status: %{http_code}" \
    -X GET $API_URL/api/admin/positions/all \
    -H "Authorization: Bearer $USER_TOKEN")
echo "Admin access attempt: $ADMIN_ACCESS"
echo ""

echo "4. Try to create market without permission (should fail)..."
MARKET_CREATE=$(curl -s -w "\nHTTP Status: %{http_code}" \
    -X POST $API_URL/api/markets/create-authorized \
    -H "Authorization: Bearer $USER_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{
        "question": "Test market",
        "outcomes": ["Yes", "No"],
        "end_time": 1735689600
    }')
echo "Market creation attempt: $MARKET_CREATE"
echo ""

# In a real scenario, you would have different tokens for different roles
echo "=== RBAC System Summary ==="
echo ""
echo "Roles:"
echo "- User: Basic viewing permissions"
echo "- Trader: Can place and close trades"
echo "- MarketMaker: Can create markets and provide liquidity"
echo "- Admin: Full system access"
echo "- Support: Customer support access"
echo "- Auditor: Read-only access to all data"
echo ""
echo "Permission Examples:"
echo "- ViewMarkets: All roles"
echo "- PlaceTrades: Trader and above"
echo "- CreateMarkets: MarketMaker and above"
echo "- UpdateSystemConfig: Admin only"
echo "- ViewAllPositions: Support, Auditor, Admin"
echo ""
echo "Authorization Methods:"
echo "1. Role-based: Automatic permissions based on role"
echo "2. Permission-based: Check specific permissions"
echo "3. Custom permissions: Grant additional permissions per user"