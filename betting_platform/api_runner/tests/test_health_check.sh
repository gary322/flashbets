#!/bin/bash
# Test script for health check system

set -e

API_URL="http://localhost:8081"
TOKEN=""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}Testing Health Check System...${NC}"

# Check if API is running
check_api() {
    if ! curl -s "$API_URL/health" > /dev/null; then
        echo -e "${RED}API is not running. Please start it first${NC}"
        echo "Run: cargo run --bin betting_platform_api"
        exit 1
    fi
}

# Authenticate as admin for protected endpoints
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

# Test liveness probe
test_liveness() {
    echo -e "\n${GREEN}Testing liveness probe...${NC}"
    
    RESPONSE=$(curl -s -X GET "$API_URL/api/health/live")
    echo "Response: $RESPONSE"
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    HEALTH_STATUS=$(echo $RESPONSE | jq -r '.data.status // empty')
    
    if [ "$STATUS" = "success" ] && [ "$HEALTH_STATUS" = "Healthy" ]; then
        echo -e "${GREEN}✓ Liveness probe passed${NC}"
    else
        echo -e "${RED}✗ Liveness probe failed${NC}"
    fi
}

# Test readiness probe
test_readiness() {
    echo -e "\n${GREEN}Testing readiness probe...${NC}"
    
    RESPONSE=$(curl -s -X GET "$API_URL/api/health/ready")
    echo "Response: $RESPONSE"
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    HEALTH_STATUS=$(echo $RESPONSE | jq -r '.data.status // empty')
    
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Readiness probe passed - Status: $HEALTH_STATUS${NC}"
        
        if [ "$HEALTH_STATUS" = "Degraded" ]; then
            echo -e "${YELLOW}⚠ Service is degraded but operational${NC}"
        fi
    else
        echo -e "${RED}✗ Readiness probe failed${NC}"
    fi
}

# Test comprehensive health check
test_comprehensive() {
    echo -e "\n${GREEN}Testing comprehensive health check...${NC}"
    
    # Test with cached results
    echo "Getting cached health check..."
    RESPONSE=$(curl -s -X GET "$API_URL/api/health/check?detailed=true")
    echo "Cached response: $(echo $RESPONSE | jq -c '.data | {overall_status, timestamp, components: .components | length}')"
    
    # Test with force refresh
    echo -e "\nForcing fresh health check..."
    RESPONSE=$(curl -s -X GET "$API_URL/api/health/check?detailed=true&force_refresh=true")
    
    OVERALL_STATUS=$(echo $RESPONSE | jq -r '.data.overall_status // empty')
    COMPONENT_COUNT=$(echo $RESPONSE | jq '.data.components | length')
    
    echo -e "Overall status: ${OVERALL_STATUS}"
    echo -e "Components checked: ${COMPONENT_COUNT}"
    
    # Show component statuses
    echo -e "\nComponent Health:"
    echo $RESPONSE | jq -r '.data.components[] | "\(.name): \(.status) - \(.message) (\(.response_time_ms)ms)"'
    
    if [ "$OVERALL_STATUS" = "Healthy" ]; then
        echo -e "${GREEN}✓ All components healthy${NC}"
    elif [ "$OVERALL_STATUS" = "Degraded" ]; then
        echo -e "${YELLOW}⚠ Some components degraded${NC}"
        echo $RESPONSE | jq -r '.data.components[] | select(.status != "Healthy") | "  - \(.name): \(.message)"'
    else
        echo -e "${RED}✗ System unhealthy${NC}"
        echo $RESPONSE | jq -r '.data.components[] | select(.status == "Unhealthy") | "  - \(.name): \(.message)"'
    fi
}

# Test individual component health
test_component() {
    echo -e "\n${GREEN}Testing individual component health...${NC}"
    
    COMPONENTS=("database" "trading_engine" "solana_rpc" "websocket" "circuit_breakers" "external_apis")
    
    for COMPONENT in "${COMPONENTS[@]}"; do
        RESPONSE=$(curl -s -X GET "$API_URL/api/health/component/$COMPONENT")
        STATUS=$(echo $RESPONSE | jq -r '.status // empty')
        
        if [ "$STATUS" = "success" ]; then
            HEALTH=$(echo $RESPONSE | jq -r '.data.status // empty')
            RESPONSE_TIME=$(echo $RESPONSE | jq -r '.data.response_time_ms // empty')
            echo -e "  $COMPONENT: $HEALTH (${RESPONSE_TIME}ms)"
        else
            echo -e "  $COMPONENT: ${RED}Not found${NC}"
        fi
    done
}

# Test Prometheus metrics
test_metrics() {
    echo -e "\n${GREEN}Testing Prometheus metrics endpoint...${NC}"
    
    RESPONSE=$(curl -s -X GET "$API_URL/api/health/metrics")
    
    # Check if response looks like Prometheus format
    if [[ $RESPONSE == *"# HELP"* ]] && [[ $RESPONSE == *"# TYPE"* ]]; then
        echo -e "${GREEN}✓ Metrics endpoint working${NC}"
        
        # Extract some key metrics
        HEALTH_STATUS=$(echo "$RESPONSE" | grep "^health_status " | awk '{print $2}')
        UPTIME=$(echo "$RESPONSE" | grep "^uptime_seconds " | awk '{print $2}')
        
        echo "  Health status: $HEALTH_STATUS (0=healthy, 1=degraded, 2=unhealthy)"
        echo "  Uptime: $(echo "scale=2; $UPTIME / 3600" | bc) hours"
        
        # Show component health
        echo -e "\n  Component health:"
        echo "$RESPONSE" | grep "^component_.*_healthy " | while read line; do
            COMPONENT=$(echo $line | sed 's/component_\(.*\)_healthy.*/\1/')
            VALUE=$(echo $line | awk '{print $2}')
            echo "    $COMPONENT: $([ "$VALUE" = "1" ] && echo "healthy" || echo "unhealthy")"
        done
    else
        echo -e "${RED}✗ Metrics endpoint not working properly${NC}"
    fi
}

# Test admin trigger endpoint
test_trigger() {
    echo -e "\n${GREEN}Testing admin trigger endpoint...${NC}"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/health/trigger" \
        -H "Authorization: Bearer $TOKEN")
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Health check triggered successfully${NC}"
        OVERALL_STATUS=$(echo $RESPONSE | jq -r '.data.overall_status // empty')
        echo "  Overall status: $OVERALL_STATUS"
    else
        echo -e "${RED}✗ Failed to trigger health check${NC}"
        echo "Response: $RESPONSE"
    fi
}

# Test health history
test_history() {
    echo -e "\n${GREEN}Testing health history endpoint...${NC}"
    
    RESPONSE=$(curl -s -X GET "$API_URL/api/health/history?limit=10" \
        -H "Authorization: Bearer $TOKEN")
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Health history retrieved${NC}"
        CHECK_COUNT=$(echo $RESPONSE | jq '.data.checks | length')
        echo "  Historical checks: $CHECK_COUNT"
    else
        echo -e "${RED}✗ Failed to get health history${NC}"
    fi
}

# Load test health checks
load_test() {
    echo -e "\n${GREEN}Running health check load test...${NC}"
    
    echo "Making 10 concurrent health check requests..."
    
    START_TIME=$(date +%s%N)
    
    for i in {1..10}; do
        curl -s -X GET "$API_URL/api/health/check" > /dev/null &
    done
    
    wait
    
    END_TIME=$(date +%s%N)
    DURATION=$((($END_TIME - $START_TIME) / 1000000))
    
    echo -e "${GREEN}✓ Completed 10 concurrent requests in ${DURATION}ms${NC}"
}

# Test Kubernetes compatibility
test_kubernetes() {
    echo -e "\n${GREEN}Testing Kubernetes probe compatibility...${NC}"
    
    # Test liveness with curl options Kubernetes uses
    LIVENESS=$(curl -s -o /dev/null -w "%{http_code}" \
        --max-time 5 \
        "$API_URL/api/health/live")
    
    # Test readiness with curl options Kubernetes uses
    READINESS=$(curl -s -o /dev/null -w "%{http_code}" \
        --max-time 3 \
        "$API_URL/api/health/ready")
    
    echo "Liveness probe HTTP status: $LIVENESS"
    echo "Readiness probe HTTP status: $READINESS"
    
    if [ "$LIVENESS" = "200" ] && [ "$READINESS" = "200" ]; then
        echo -e "${GREEN}✓ Kubernetes probes compatible${NC}"
    else
        echo -e "${RED}✗ Kubernetes probe issues detected${NC}"
    fi
}

# Run all tests
main() {
    check_api
    
    test_liveness
    test_readiness
    test_comprehensive
    test_component
    test_metrics
    
    authenticate
    test_trigger
    test_history
    
    test_kubernetes
    load_test
    
    echo -e "\n${GREEN}Health check tests completed!${NC}"
}

# Handle script arguments
case "${1:-}" in
    "live")
        check_api
        test_liveness
        ;;
    "ready")
        check_api
        test_readiness
        ;;
    "check")
        check_api
        test_comprehensive
        ;;
    "metrics")
        check_api
        test_metrics
        ;;
    "admin")
        check_api
        authenticate
        test_trigger
        test_history
        ;;
    "load")
        check_api
        load_test
        ;;
    *)
        main
        ;;
esac