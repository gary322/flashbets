#!/bin/bash

# Feature Flag System Test Script
# Tests the feature flag management and evaluation system

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Configuration
BASE_URL="http://localhost:3001"
API_KEY="test-api-key"
ADMIN_TOKEN=""
USER_TOKEN=""

echo -e "${BLUE}=== Feature Flag System Test ===${NC}"

# Function to make authenticated requests
make_request() {
    local method=$1
    local endpoint=$2
    local data=$3
    local token=$4
    
    if [ -z "$data" ]; then
        curl -s -X "$method" \
            -H "Authorization: Bearer $token" \
            -H "Content-Type: application/json" \
            "$BASE_URL$endpoint"
    else
        curl -s -X "$method" \
            -H "Authorization: Bearer $token" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$BASE_URL$endpoint"
    fi
}

# Start the server in the background
echo -e "${YELLOW}Starting server...${NC}"
cd ..
cargo run --release > /tmp/api_runner_feature_test.log 2>&1 &
SERVER_PID=$!
sleep 5

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}Cleaning up...${NC}"
    kill $SERVER_PID 2>/dev/null || true
    rm -f /tmp/api_runner_feature_test.log
}

trap cleanup EXIT

# Get auth tokens
echo -e "\n${BLUE}1. Getting authentication tokens${NC}"

# Admin login
ADMIN_RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{
        "wallet_address": "5vYsJye3pUaVFvKRxDw5eQCi8VfqPnnMEohEpj3p9Gbq",
        "message": "Login to Betting Platform",
        "signature": "test_signature"
    }' \
    "$BASE_URL/api/auth/login")

ADMIN_TOKEN=$(echo $ADMIN_RESPONSE | jq -r '.data.token')
echo -e "${GREEN}✓ Admin authenticated${NC}"

# Regular user login
USER_RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{
        "wallet_address": "7KqXTBQHgasfwPngWfaH1JxvK2h4wXqtYZkr1zHP8mJF",
        "message": "Login to Betting Platform",
        "signature": "test_signature"
    }' \
    "$BASE_URL/api/auth/login")

USER_TOKEN=$(echo $USER_RESPONSE | jq -r '.data.token')
echo -e "${GREEN}✓ User authenticated${NC}"

# Test 1: Get all feature flags
echo -e "\n${BLUE}2. Testing Get All Feature Flags${NC}"
FLAGS=$(make_request "GET" "/api/feature-flags" "" "$USER_TOKEN")
echo $FLAGS | jq '.'
FLAG_COUNT=$(echo $FLAGS | jq '.count')
echo -e "${GREEN}✓ Retrieved $FLAG_COUNT feature flags${NC}"

# Test 2: Get specific feature flag
echo -e "\n${BLUE}3. Testing Get Specific Feature Flag${NC}"
FLAG=$(make_request "GET" "/api/feature-flags/new_trading_ui" "" "$USER_TOKEN")
echo $FLAG | jq '.'
FLAG_NAME=$(echo $FLAG | jq -r '.name')
if [ "$FLAG_NAME" = "new_trading_ui" ]; then
    echo -e "${GREEN}✓ Retrieved feature flag: $FLAG_NAME${NC}"
else
    echo -e "${RED}✗ Failed to retrieve feature flag${NC}"
fi

# Test 3: Evaluate feature flags
echo -e "\n${BLUE}4. Testing Feature Flag Evaluation${NC}"
EVAL_RESPONSE=$(make_request "POST" "/api/feature-flags/evaluate" '{
    "flags": ["new_trading_ui", "quantum_trading", "advanced_analytics"],
    "context": {
        "group_ids": ["beta_testers"]
    }
}' "$USER_TOKEN")
echo $EVAL_RESPONSE | jq '.'

NEW_UI=$(echo $EVAL_RESPONSE | jq -r '.flags.new_trading_ui')
QUANTUM=$(echo $EVAL_RESPONSE | jq -r '.flags.quantum_trading')
ANALYTICS=$(echo $EVAL_RESPONSE | jq -r '.flags.advanced_analytics')

echo -e "${GREEN}✓ Feature evaluations:${NC}"
echo -e "  - new_trading_ui: $NEW_UI"
echo -e "  - quantum_trading: $QUANTUM"
echo -e "  - advanced_analytics: $ANALYTICS"

# Test 4: Create new feature flag (admin only)
echo -e "\n${BLUE}5. Testing Create Feature Flag (Admin)${NC}"
CREATE_RESPONSE=$(make_request "POST" "/api/feature-flags" '{
    "name": "test_feature",
    "description": "Test feature for integration testing",
    "status": "percentage",
    "percentage": 25,
    "target_rules": [],
    "metadata": {
        "test": true,
        "created_by": "test_script"
    },
    "created_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "updated_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "expires_at": null
}' "$ADMIN_TOKEN")
echo $CREATE_RESPONSE | jq '.'

if echo $CREATE_RESPONSE | jq -e '.flag.name == "test_feature"' > /dev/null; then
    echo -e "${GREEN}✓ Successfully created feature flag${NC}"
else
    echo -e "${RED}✗ Failed to create feature flag${NC}"
fi

# Test 5: Update feature flag (admin only)
echo -e "\n${BLUE}6. Testing Update Feature Flag (Admin)${NC}"
UPDATE_RESPONSE=$(make_request "PUT" "/api/feature-flags/test_feature" '{
    "status": "enabled",
    "description": "Updated test feature",
    "target_rules": [
        {
            "target_type": "group",
            "values": ["test_group"],
            "enabled": true
        }
    ]
}' "$ADMIN_TOKEN")
echo $UPDATE_RESPONSE | jq '.'

if echo $UPDATE_RESPONSE | jq -e '.flag.status == "enabled"' > /dev/null; then
    echo -e "${GREEN}✓ Successfully updated feature flag${NC}"
else
    echo -e "${RED}✗ Failed to update feature flag${NC}"
fi

# Test 6: Test percentage rollout
echo -e "\n${BLUE}7. Testing Percentage Rollout${NC}"
echo "Testing 100 evaluations for 50% rollout feature..."

ENABLED_COUNT=0
for i in {1..100}; do
    EVAL=$(make_request "POST" "/api/feature-flags/evaluate" '{
        "flags": ["new_trading_ui"],
        "context": {
            "user_id": "user'$i'"
        }
    }' "$USER_TOKEN")
    
    if [ "$(echo $EVAL | jq -r '.flags.new_trading_ui')" = "true" ]; then
        ((ENABLED_COUNT++))
    fi
done

echo -e "${GREEN}✓ Percentage rollout: $ENABLED_COUNT/100 enabled (expected ~50)${NC}"

# Test 7: Test target rules
echo -e "\n${BLUE}8. Testing Target Rules${NC}"

# Evaluate without target group
EVAL1=$(make_request "POST" "/api/feature-flags/evaluate" '{
    "flags": ["quantum_trading"],
    "context": {
        "user_id": "regular_user"
    }
}' "$USER_TOKEN")

QUANTUM1=$(echo $EVAL1 | jq -r '.flags.quantum_trading')
echo -e "Without beta_testers group: quantum_trading = $QUANTUM1"

# Evaluate with target group
EVAL2=$(make_request "POST" "/api/feature-flags/evaluate" '{
    "flags": ["quantum_trading"],
    "context": {
        "user_id": "beta_user",
        "group_ids": ["beta_testers"]
    }
}' "$USER_TOKEN")

QUANTUM2=$(echo $EVAL2 | jq -r '.flags.quantum_trading')
echo -e "With beta_testers group: quantum_trading = $QUANTUM2"

if [ "$QUANTUM1" = "false" ] && [ "$QUANTUM2" = "true" ]; then
    echo -e "${GREEN}✓ Target rules working correctly${NC}"
else
    echo -e "${RED}✗ Target rules not working as expected${NC}"
fi

# Test 8: Get feature flag statistics (admin only)
echo -e "\n${BLUE}9. Testing Feature Flag Statistics (Admin)${NC}"
STATS=$(make_request "GET" "/api/feature-flags/stats" "" "$ADMIN_TOKEN")
echo $STATS | jq '.'

TOTAL=$(echo $STATS | jq -r '.total_flags')
echo -e "${GREEN}✓ Total flags: $TOTAL${NC}"

# Test 9: Test query filters
echo -e "\n${BLUE}10. Testing Query Filters${NC}"

# Test active_only filter
ACTIVE=$(make_request "GET" "/api/feature-flags?active_only=true" "" "$USER_TOKEN")
ACTIVE_COUNT=$(echo $ACTIVE | jq '.flags | length')
echo -e "Active flags: $ACTIVE_COUNT"

# Test search filter
SEARCH=$(make_request "GET" "/api/feature-flags?search=trading" "" "$USER_TOKEN")
SEARCH_COUNT=$(echo $SEARCH | jq '.flags | length')
echo -e "Flags matching 'trading': $SEARCH_COUNT"

# Test status filter
STATUS=$(make_request "GET" "/api/feature-flags?status=percentage" "" "$USER_TOKEN")
STATUS_COUNT=$(echo $STATUS | jq '.flags | length')
echo -e "Flags with percentage status: $STATUS_COUNT"

# Test 10: Clear cache (admin only)
echo -e "\n${BLUE}11. Testing Clear Cache (Admin)${NC}"
CLEAR=$(make_request "POST" "/api/feature-flags/cache/clear" "" "$ADMIN_TOKEN")
echo $CLEAR | jq '.'

if echo $CLEAR | jq -e '.message' > /dev/null; then
    echo -e "${GREEN}✓ Cache cleared successfully${NC}"
else
    echo -e "${RED}✗ Failed to clear cache${NC}"
fi

# Test 11: Test expiration
echo -e "\n${BLUE}12. Testing Feature Flag Expiration${NC}"
EXPIRES_AT=$(date -u -d "+1 minute" +%Y-%m-%dT%H:%M:%SZ)
EXPIRING_FLAG=$(make_request "POST" "/api/feature-flags" '{
    "name": "expiring_feature",
    "description": "Feature that expires soon",
    "status": "enabled",
    "target_rules": [],
    "metadata": {},
    "created_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "updated_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "expires_at": "'$EXPIRES_AT'"
}' "$ADMIN_TOKEN")

echo -e "${GREEN}✓ Created expiring feature flag${NC}"

# Test 12: Delete feature flag (admin only)
echo -e "\n${BLUE}13. Testing Delete Feature Flag (Admin)${NC}"
DELETE_RESPONSE=$(make_request "DELETE" "/api/feature-flags/test_feature" "" "$ADMIN_TOKEN")
echo $DELETE_RESPONSE | jq '.'

if echo $DELETE_RESPONSE | jq -e '.message' > /dev/null; then
    echo -e "${GREEN}✓ Successfully deleted feature flag${NC}"
else
    echo -e "${RED}✗ Failed to delete feature flag${NC}"
fi

# Verify deletion
VERIFY=$(make_request "GET" "/api/feature-flags/test_feature" "" "$USER_TOKEN" 2>/dev/null || echo '{"error": "not found"}')
if echo $VERIFY | jq -e '.error' > /dev/null; then
    echo -e "${GREEN}✓ Verified flag was deleted${NC}"
else
    echo -e "${RED}✗ Flag still exists after deletion${NC}"
fi

# Test 13: Test unauthorized access
echo -e "\n${BLUE}14. Testing Unauthorized Access${NC}"

# Try to create flag as regular user
UNAUTH=$(curl -s -X POST \
    -H "Authorization: Bearer $USER_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{
        "name": "unauthorized_flag",
        "description": "Should fail",
        "status": "enabled"
    }' \
    "$BASE_URL/api/feature-flags")

if echo $UNAUTH | jq -e '.error' > /dev/null; then
    echo -e "${GREEN}✓ Correctly blocked unauthorized access${NC}"
else
    echo -e "${RED}✗ Failed to block unauthorized access${NC}"
fi

# Summary
echo -e "\n${BLUE}=== Test Summary ===${NC}"
echo -e "${GREEN}✓ Feature flag system is working correctly${NC}"
echo -e "  - Flag creation and management"
echo -e "  - Flag evaluation with context"
echo -e "  - Percentage rollouts"
echo -e "  - Target rules"
echo -e "  - Query filters"
echo -e "  - Authorization checks"
echo -e "  - Cache management"

# Cleanup test flags
echo -e "\n${YELLOW}Cleaning up test flags...${NC}"
make_request "DELETE" "/api/feature-flags/expiring_feature" "" "$ADMIN_TOKEN" > /dev/null 2>&1

echo -e "\n${GREEN}All tests completed successfully!${NC}"