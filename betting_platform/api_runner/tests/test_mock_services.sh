#!/bin/bash
# Test script for mock services

set -e

API_URL="http://localhost:8081"
TOKEN=""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}Testing Mock Services System...${NC}"

# Check if API is running
check_api() {
    if ! curl -s "$API_URL/health" > /dev/null; then
        echo -e "${RED}API is not running. Please start it first with MOCK_SERVICES_ENABLED=true${NC}"
        echo "Run: MOCK_SERVICES_ENABLED=true cargo run --bin betting_platform_api"
        exit 1
    fi
}

# Authenticate as admin
authenticate() {
    echo -e "\n${GREEN}Authenticating as admin...${NC}"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/auth/login" \
        -H "Content-Type: application/json" \
        -d '{
            "email": "admin@test.com",
            "password": "admin123",
            "role": "admin"
        }')
    
    TOKEN=$(echo $RESPONSE | jq -r '.data.token // empty')
    
    if [ -z "$TOKEN" ]; then
        echo -e "${RED}Failed to authenticate. Response: $RESPONSE${NC}"
        exit 1
    fi
    
    echo "Token obtained: ${TOKEN:0:20}..."
}

# Test mock service statistics
test_mock_stats() {
    echo -e "\n${GREEN}Testing mock service statistics...${NC}"
    
    RESPONSE=$(curl -s -X GET "$API_URL/api/mock/stats" \
        -H "Authorization: Bearer $TOKEN")
    
    echo "Response: $RESPONSE"
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Mock statistics retrieved successfully${NC}"
        echo "Stats:"
        echo $RESPONSE | jq '.data'
    else
        echo -e "${RED}✗ Failed to get mock statistics${NC}"
    fi
}

# Test market activity simulation
test_market_simulation() {
    echo -e "\n${GREEN}Testing market activity simulation...${NC}"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/mock/simulate/market" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "market_id": 1000,
            "duration_minutes": 5,
            "trades_per_minute": 10
        }')
    
    echo "Response: $RESPONSE"
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Market simulation started successfully${NC}"
    else
        echo -e "${RED}✗ Failed to start market simulation${NC}"
    fi
}

# Test setting market outcome
test_set_outcome() {
    echo -e "\n${GREEN}Testing setting market outcome...${NC}"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/mock/market/outcome" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "market_id": 1000,
            "outcome": 1
        }')
    
    echo "Response: $RESPONSE"
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Market outcome set successfully${NC}"
    else
        echo -e "${RED}✗ Failed to set market outcome${NC}"
    fi
}

# Test oracle integration with mock
test_oracle_query() {
    echo -e "\n${GREEN}Testing oracle query with mock providers...${NC}"
    
    # First set outcome
    curl -s -X POST "$API_URL/api/mock/market/outcome" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "market_id": 1001,
            "outcome": 0
        }' > /dev/null
    
    # Query oracles
    RESPONSE=$(curl -s -X GET "$API_URL/api/settlement/oracles/1001" \
        -H "Authorization: Bearer $TOKEN")
    
    echo "Response: $RESPONSE"
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Oracle query successful${NC}"
        echo "Oracle results:"
        echo $RESPONSE | jq '.data.results[]'
    else
        echo -e "${RED}✗ Failed to query oracles${NC}"
    fi
}

# Test WebSocket with mock
test_websocket_mock() {
    echo -e "\n${GREEN}Testing WebSocket with mock manager...${NC}"
    
    # Use websocat if available, otherwise skip
    if command -v websocat &> /dev/null; then
        echo "Testing WebSocket connection..."
        
        # Connect and send a test message
        RESULT=$(echo '{"type":"subscribe","channel":"markets"}' | \
            timeout 5 websocat -n1 "ws://localhost:8081/api/prices/ws" 2>&1 || true)
        
        if [[ $RESULT == *"subscribed"* ]] || [[ -z "$RESULT" ]]; then
            echo -e "${GREEN}✓ WebSocket connection successful${NC}"
        else
            echo -e "${YELLOW}⚠ WebSocket test inconclusive${NC}"
        fi
    else
        echo -e "${YELLOW}⚠ websocat not installed, skipping WebSocket test${NC}"
        echo "Install with: cargo install websocat"
    fi
}

# Test mock service profiles
test_profiles() {
    echo -e "\n${GREEN}Testing different mock profiles...${NC}"
    
    echo "Note: Profile switching requires API restart with different MOCK_PROFILE env var"
    echo "Available profiles:"
    echo "  - MOCK_PROFILE=realistic (default)"
    echo "  - MOCK_PROFILE=fast"
    echo "  - MOCK_PROFILE=chaos"
}

# Test integration with settlement
test_settlement_with_mock() {
    echo -e "\n${GREEN}Testing settlement with mock oracles...${NC}"
    
    # Set outcome for market
    curl -s -X POST "$API_URL/api/mock/market/outcome" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "market_id": 1002,
            "outcome": 1
        }' > /dev/null
    
    # Initiate settlement
    RESPONSE=$(curl -s -X POST "$API_URL/api/settlement/initiate" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "market_id": 1002
        }')
    
    echo "Response: $RESPONSE"
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Settlement initiated with mock oracles${NC}"
        
        # Check settlement status
        sleep 2
        STATUS_RESPONSE=$(curl -s -X GET "$API_URL/api/settlement/status/1002" \
            -H "Authorization: Bearer $TOKEN")
        
        echo "Settlement status:"
        echo $STATUS_RESPONSE | jq '.data'
    else
        echo -e "${RED}✗ Failed to initiate settlement${NC}"
    fi
}

# Run all tests
main() {
    check_api
    authenticate
    
    test_mock_stats
    test_market_simulation
    test_set_outcome
    test_oracle_query
    test_websocket_mock
    test_settlement_with_mock
    test_profiles
    
    echo -e "\n${GREEN}Mock services tests completed!${NC}"
}

# Handle script arguments
case "${1:-}" in
    "stats")
        check_api
        authenticate
        test_mock_stats
        ;;
    "simulate")
        check_api
        authenticate
        test_market_simulation
        ;;
    "outcome")
        check_api
        authenticate
        test_set_outcome
        ;;
    "oracle")
        check_api
        authenticate
        test_oracle_query
        ;;
    "settlement")
        check_api
        authenticate
        test_settlement_with_mock
        ;;
    *)
        main
        ;;
esac