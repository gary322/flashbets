#!/bin/bash

echo "=== JWT Authentication Test ==="
echo ""

# Test wallet credentials
WALLET="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
MESSAGE="Sign this message to authenticate with betting platform"
# This is a dummy signature for testing - in production, use actual wallet signature
SIGNATURE="5VpX6Y2V7b4v5vLKH2K5n3F5u8F6h8J9k3L4m6N7p8Q9r2S3t4U5v6W7x8Y9z"

API_URL="http://localhost:8081"

# Check if API is running
if ! curl -s $API_URL/health > /dev/null; then
    echo "ERROR: API is not running. Please start the API first."
    exit 1
fi

echo "1. Testing login endpoint..."
LOGIN_RESPONSE=$(curl -s -X POST $API_URL/api/auth/login \
    -H "Content-Type: application/json" \
    -d "{
        \"wallet\": \"$WALLET\",
        \"signature\": \"$SIGNATURE\",
        \"message\": \"$MESSAGE\"
    }")

echo "Login response: $LOGIN_RESPONSE"
echo ""

# Extract tokens (using grep since jq might not be available)
ACCESS_TOKEN=$(echo $LOGIN_RESPONSE | grep -o '"access_token":"[^"]*' | cut -d'"' -f4)
REFRESH_TOKEN=$(echo $LOGIN_RESPONSE | grep -o '"refresh_token":"[^"]*' | cut -d'"' -f4)

if [ -z "$ACCESS_TOKEN" ]; then
    echo "ERROR: Failed to get access token"
    exit 1
fi

echo "2. Testing authenticated endpoint..."
USER_INFO=$(curl -s -X GET $API_URL/api/auth/user \
    -H "Authorization: Bearer $ACCESS_TOKEN")

echo "User info: $USER_INFO"
echo ""

echo "3. Testing token validation..."
VALIDATION_RESPONSE=$(curl -s -X POST $API_URL/api/auth/validate \
    -H "Content-Type: application/json" \
    -d "{\"token\": \"$ACCESS_TOKEN\"}")

echo "Validation response: $VALIDATION_RESPONSE"
echo ""

echo "4. Testing refresh token..."
REFRESH_RESPONSE=$(curl -s -X POST $API_URL/api/auth/refresh \
    -H "Content-Type: application/json" \
    -d "{\"refresh_token\": \"$REFRESH_TOKEN\"}")

echo "Refresh response: $REFRESH_RESPONSE"
echo ""

echo "5. Testing logout..."
LOGOUT_RESPONSE=$(curl -s -X POST $API_URL/api/auth/logout \
    -H "Authorization: Bearer $ACCESS_TOKEN")

echo "Logout response: $LOGOUT_RESPONSE"
echo ""

echo "6. Testing expired token handling..."
# Send invalid/expired token
EXPIRED_RESPONSE=$(curl -s -X GET $API_URL/api/auth/user \
    -H "Authorization: Bearer invalid.token.here" \
    -w "\nHTTP Status: %{http_code}")

echo "Expired token response: $EXPIRED_RESPONSE"
echo ""

echo "=== JWT Configuration Summary ==="
echo "- Access token expiration: 60 minutes"
echo "- Refresh token expiration: 30 days"
echo "- Algorithm: HS256"
echo "- Token validation includes:"
echo "  - Expiration check (exp claim)"
echo "  - Not-before check (nbf claim)"
echo "  - Issuer validation"
echo "  - Signature verification"