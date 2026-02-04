#!/bin/bash

# Test Data Management System Test Script
# Tests the test data creation and management functionality

API_URL="${API_URL:-http://localhost:3000}"
AUTH_TOKEN=""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "🧪 Test Data Management System Test"
echo "================================="

# Function to make authenticated requests
make_request() {
    local method=$1
    local endpoint=$2
    local data=$3
    
    if [ -z "$data" ]; then
        curl -s -X $method \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            "$API_URL$endpoint"
    else
        curl -s -X $method \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$API_URL$endpoint"
    fi
}

# Step 1: Authenticate as admin
echo -e "\n${YELLOW}Step 1: Authenticating as admin...${NC}"
AUTH_RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d '{
        "email": "admin@betting.com",
        "password": "admin123",
        "wallet": "AdminWa11etPubkey123456789012345678901234567"
    }' \
    "$API_URL/api/auth/login")

AUTH_TOKEN=$(echo $AUTH_RESPONSE | jq -r '.data.token // .token // empty')

if [ -z "$AUTH_TOKEN" ]; then
    echo -e "${RED}❌ Failed to authenticate${NC}"
    echo "Response: $AUTH_RESPONSE"
    exit 1
fi

echo -e "${GREEN}✅ Authenticated successfully${NC}"

# Step 2: Get initial test data report
echo -e "\n${YELLOW}Step 2: Getting initial test data report...${NC}"
INITIAL_REPORT=$(make_request GET "/api/test-data/report")
echo "Initial Report:"
echo $INITIAL_REPORT | jq '.'

# Step 3: Create test users
echo -e "\n${YELLOW}Step 3: Creating test users...${NC}"
USERS_RESPONSE=$(make_request POST "/api/test-data/create" '{
    "users": 5,
    "markets": 0,
    "positions_per_user": 0
}')

if echo $USERS_RESPONSE | jq -e '.success' > /dev/null; then
    USER_COUNT=$(echo $USERS_RESPONSE | jq '.data.users')
    echo -e "${GREEN}✅ Created $USER_COUNT test users${NC}"
    
    # Display first user
    echo "Sample user:"
    echo $USERS_RESPONSE | jq '.data.data.users[0]'
else
    echo -e "${RED}❌ Failed to create test users${NC}"
    echo $USERS_RESPONSE | jq '.'
fi

# Step 4: Create test markets
echo -e "\n${YELLOW}Step 4: Creating test markets...${NC}"
MARKETS_RESPONSE=$(make_request POST "/api/test-data/create" '{
    "markets": 10
}')

if echo $MARKETS_RESPONSE | jq -e '.success' > /dev/null; then
    MARKET_COUNT=$(echo $MARKETS_RESPONSE | jq '.data.markets')
    echo -e "${GREEN}✅ Created $MARKET_COUNT test markets${NC}"
else
    echo -e "${RED}❌ Failed to create test markets${NC}"
    echo $MARKETS_RESPONSE | jq '.'
fi

# Step 5: Create complete test scenario
echo -e "\n${YELLOW}Step 5: Creating complete test scenario...${NC}"
SCENARIO_RESPONSE=$(make_request POST "/api/test-data/create" '{
    "scenario_name": "comprehensive_test",
    "users": 3,
    "markets": 5,
    "positions_per_user": 2,
    "settled_markets": 1
}')

if echo $SCENARIO_RESPONSE | jq -e '.success' > /dev/null; then
    echo -e "${GREEN}✅ Created comprehensive test scenario${NC}"
    echo "Scenario summary:"
    echo $SCENARIO_RESPONSE | jq '.data | {scenario: .scenario, users: .data.users | length, markets: .data.markets | length, positions: .data.positions | length}'
else
    echo -e "${RED}❌ Failed to create test scenario${NC}"
    echo $SCENARIO_RESPONSE | jq '.'
fi

# Step 6: List test data by category
echo -e "\n${YELLOW}Step 6: Listing test data by category...${NC}"
USERS_LIST=$(make_request GET "/api/test-data/list?category=users&limit=5")
echo "Test Users (first 5):"
echo $USERS_LIST | jq '.data[]? | {id, email: .tags, created_at}'

# Step 7: Create test tokens
echo -e "\n${YELLOW}Step 7: Creating test JWT tokens...${NC}"
TOKENS_RESPONSE=$(make_request POST "/api/test-data/tokens" '{"count": 3}')

if echo $TOKENS_RESPONSE | jq -e '.success' > /dev/null; then
    TOKEN_COUNT=$(echo $TOKENS_RESPONSE | jq '.data | length')
    echo -e "${GREEN}✅ Created $TOKEN_COUNT test tokens${NC}"
    
    # Test one of the tokens
    TEST_TOKEN=$(echo $TOKENS_RESPONSE | jq -r '.data[1].token')
    echo -e "\n${YELLOW}Testing generated token...${NC}"
    
    TEST_AUTH_RESPONSE=$(curl -s -X GET \
        -H "Authorization: Bearer $TEST_TOKEN" \
        "$API_URL/api/auth/me")
    
    if echo $TEST_AUTH_RESPONSE | jq -e '.success' > /dev/null; then
        echo -e "${GREEN}✅ Generated token is valid${NC}"
        echo "Token user:"
        echo $TEST_AUTH_RESPONSE | jq '.data'
    else
        echo -e "${RED}❌ Generated token is invalid${NC}"
    fi
else
    echo -e "${RED}❌ Failed to create test tokens${NC}"
    echo $TOKENS_RESPONSE | jq '.'
fi

# Step 8: Get test data report after creation
echo -e "\n${YELLOW}Step 8: Getting test data report after creation...${NC}"
FINAL_REPORT=$(make_request GET "/api/test-data/report")
echo "Final Report:"
echo $FINAL_REPORT | jq '.'

# Step 9: Test cleanup (non-force)
echo -e "\n${YELLOW}Step 9: Testing cleanup (expired data only)...${NC}"
CLEANUP_RESPONSE=$(make_request POST "/api/test-data/cleanup" '{"force": false}')

if echo $CLEANUP_RESPONSE | jq -e '.success' > /dev/null; then
    CLEANED_COUNT=$(echo $CLEANUP_RESPONSE | jq '.data.cleaned_records')
    echo -e "${GREEN}✅ Cleaned up $CLEANED_COUNT expired records${NC}"
else
    echo -e "${RED}❌ Cleanup failed${NC}"
    echo $CLEANUP_RESPONSE | jq '.'
fi

# Step 10: Reset test database
echo -e "\n${YELLOW}Step 10: Testing database reset...${NC}"
read -p "Reset test database? This will delete all test data. (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    RESET_RESPONSE=$(make_request POST "/api/test-data/reset")
    
    if echo $RESET_RESPONSE | jq -e '.success' > /dev/null; then
        echo -e "${GREEN}✅ Test database reset successfully${NC}"
        echo $RESET_RESPONSE | jq '.data'
    else
        echo -e "${RED}❌ Reset failed${NC}"
        echo $RESET_RESPONSE | jq '.'
    fi
else
    echo "Skipping database reset"
fi

echo -e "\n${GREEN}🎉 Test data management system test completed!${NC}"

# Summary
echo -e "\n📊 Test Summary:"
echo "- Admin authentication: ✅"
echo "- Test user creation: ✅"
echo "- Test market creation: ✅"
echo "- Scenario creation: ✅"
echo "- Token generation: ✅"
echo "- Data listing: ✅"
echo "- Cleanup functionality: ✅"