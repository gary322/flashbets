#!/bin/bash
# Test script for environment configuration system

set -e

API_URL="http://localhost:8081"
TOKEN=""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}Testing Environment Configuration System...${NC}"

# Check if API is running
check_api() {
    if ! curl -s "$API_URL/health" > /dev/null; then
        echo -e "${RED}API is not running. Please start it first${NC}"
        echo "Run: cargo run --bin betting_platform_api"
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

# Test get configuration
test_get_config() {
    echo -e "\n${GREEN}Testing get configuration...${NC}"
    
    # Without sensitive data
    echo "Getting config without sensitive data..."
    RESPONSE=$(curl -s -X GET "$API_URL/api/config" \
        -H "Authorization: Bearer $TOKEN")
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    ENVIRONMENT=$(echo $RESPONSE | jq -r '.data.environment // empty')
    
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Configuration retrieved${NC}"
        echo "  Environment: $ENVIRONMENT"
        
        # Check if sensitive data is redacted
        JWT_SECRET=$(echo $RESPONSE | jq -r '.data.config.security.jwt_secret // empty')
        if [[ $JWT_SECRET == *"REDACTED"* ]]; then
            echo -e "${GREEN}✓ Sensitive data properly redacted${NC}"
        else
            echo -e "${YELLOW}⚠ Sensitive data may not be redacted${NC}"
        fi
    else
        echo -e "${RED}✗ Failed to get configuration${NC}"
        echo "Response: $RESPONSE"
    fi
    
    # With sensitive data
    echo -e "\nGetting config with sensitive data..."
    RESPONSE=$(curl -s -X GET "$API_URL/api/config?include_sensitive=true" \
        -H "Authorization: Bearer $TOKEN")
    
    JWT_SECRET=$(echo $RESPONSE | jq -r '.data.config.security.jwt_secret // empty')
    if [[ $JWT_SECRET != *"REDACTED"* ]] && [ ! -z "$JWT_SECRET" ]; then
        echo -e "${GREEN}✓ Sensitive data included when requested${NC}"
    fi
}

# Test get specific config value
test_get_config_value() {
    echo -e "\n${GREEN}Testing get specific config value...${NC}"
    
    # Test various paths
    PATHS=("server.port" "database.max_connections" "features.enable_mock_services")
    
    for PATH in "${PATHS[@]}"; do
        RESPONSE=$(curl -s -X GET "$API_URL/api/config/$PATH" \
            -H "Authorization: Bearer $TOKEN")
        
        STATUS=$(echo $RESPONSE | jq -r '.status // empty')
        VALUE=$(echo $RESPONSE | jq -r '.data // empty')
        
        if [ "$STATUS" = "success" ]; then
            echo -e "  $PATH: $VALUE"
        else
            echo -e "${RED}  Failed to get $PATH${NC}"
        fi
    done
    
    # Test non-existent path
    echo -e "\nTesting non-existent path..."
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/non.existent.path" \
        -H "Authorization: Bearer $TOKEN")
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    if [ "$STATUS" = "error" ]; then
        echo -e "${GREEN}✓ Properly handles non-existent paths${NC}"
    fi
}

# Test configuration override
test_config_override() {
    echo -e "\n${GREEN}Testing configuration override...${NC}"
    
    # Set an override
    RESPONSE=$(curl -s -X POST "$API_URL/api/config/override" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d '{
            "key": "server.workers",
            "value": 16,
            "reason": "Testing override functionality"
        }')
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Configuration override set${NC}"
        
        # Verify the override took effect
        RESPONSE=$(curl -s -X GET "$API_URL/api/config/server.workers" \
            -H "Authorization: Bearer $TOKEN")
        
        VALUE=$(echo $RESPONSE | jq -r '.data // empty')
        if [ "$VALUE" = "16" ]; then
            echo -e "${GREEN}✓ Override value confirmed${NC}"
        else
            echo -e "${RED}✗ Override value not applied${NC}"
        fi
    else
        echo -e "${RED}✗ Failed to set override${NC}"
        echo "Response: $RESPONSE"
    fi
}

# Test configuration reload
test_config_reload() {
    echo -e "\n${GREEN}Testing configuration reload...${NC}"
    
    RESPONSE=$(curl -s -X POST "$API_URL/api/config/reload" \
        -H "Authorization: Bearer $TOKEN")
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    
    if [ "$STATUS" = "success" ]; then
        echo -e "${GREEN}✓ Configuration reloaded successfully${NC}"
    else
        echo -e "${RED}✗ Failed to reload configuration${NC}"
        echo "Response: $RESPONSE"
    fi
}

# Test configuration export
test_config_export() {
    echo -e "\n${GREEN}Testing configuration export...${NC}"
    
    # Test different formats
    FORMATS=("toml" "json" "yaml")
    
    for FORMAT in "${FORMATS[@]}"; do
        echo -e "\nExporting as $FORMAT..."
        RESPONSE=$(curl -s -X GET "$API_URL/api/config/export?format=$FORMAT" \
            -H "Authorization: Bearer $TOKEN")
        
        # Check if response looks valid for format
        case $FORMAT in
            "toml")
                if [[ $RESPONSE == *"[server]"* ]]; then
                    echo -e "${GREEN}✓ Valid TOML export${NC}"
                else
                    echo -e "${RED}✗ Invalid TOML export${NC}"
                fi
                ;;
            "json")
                if echo "$RESPONSE" | jq . > /dev/null 2>&1; then
                    echo -e "${GREEN}✓ Valid JSON export${NC}"
                else
                    echo -e "${RED}✗ Invalid JSON export${NC}"
                fi
                ;;
            "yaml")
                if [[ $RESPONSE == *"server:"* ]]; then
                    echo -e "${GREEN}✓ Valid YAML export${NC}"
                else
                    echo -e "${RED}✗ Invalid YAML export${NC}"
                fi
                ;;
        esac
    done
}

# Test configuration diff
test_config_diff() {
    echo -e "\n${GREEN}Testing configuration diff...${NC}"
    
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/diff" \
        -H "Authorization: Bearer $TOKEN")
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    
    if [ "$STATUS" = "success" ]; then
        TOTAL_CHANGES=$(echo $RESPONSE | jq -r '.data.total_changes // 0')
        echo -e "${GREEN}✓ Configuration diff retrieved${NC}"
        echo "  Total changes from default: $TOTAL_CHANGES"
        
        # Show first few changes
        echo $RESPONSE | jq -r '.data.changes[:3][] | "  - \(.key): \(.current) (default: \(.default))"' 2>/dev/null || true
    else
        echo -e "${RED}✗ Failed to get configuration diff${NC}"
    fi
}

# Test configuration validation
test_config_validate() {
    echo -e "\n${GREEN}Testing configuration validation...${NC}"
    
    RESPONSE=$(curl -s -X GET "$API_URL/api/config/validate" \
        -H "Authorization: Bearer $TOKEN")
    
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    
    if [ "$STATUS" = "success" ]; then
        VALID=$(echo $RESPONSE | jq -r '.data.valid // false')
        echo -e "${GREEN}✓ Configuration validation completed${NC}"
        echo "  Configuration valid: $VALID"
        
        # Show validation results
        echo -e "\n  Validation results:"
        echo $RESPONSE | jq -r '.data.results[] | "    \(.component): \(.valid) \(.message // "")"'
    else
        echo -e "${RED}✗ Failed to validate configuration${NC}"
    fi
}

# Test access control
test_access_control() {
    echo -e "\n${GREEN}Testing access control...${NC}"
    
    # Try to access without token
    RESPONSE=$(curl -s -X GET "$API_URL/api/config")
    STATUS=$(echo $RESPONSE | jq -r '.status // empty')
    
    if [ "$STATUS" = "error" ]; then
        echo -e "${GREEN}✓ Properly rejects unauthenticated requests${NC}"
    else
        echo -e "${RED}✗ Allowed unauthenticated access${NC}"
    fi
    
    # Try with non-admin token (if available)
    # This would require creating a non-admin user first
}

# Test environment-specific loading
test_environment_loading() {
    echo -e "\n${GREEN}Testing environment-specific configuration...${NC}"
    
    # Check current environment
    RESPONSE=$(curl -s -X GET "$API_URL/api/config" \
        -H "Authorization: Bearer $TOKEN")
    
    ENVIRONMENT=$(echo $RESPONSE | jq -r '.data.environment // empty')
    echo "  Current environment: $ENVIRONMENT"
    
    # Check if environment-specific values are loaded
    case $ENVIRONMENT in
        "development")
            MOCK_ENABLED=$(echo $RESPONSE | jq -r '.data.config.features.enable_mock_services // false')
            if [ "$MOCK_ENABLED" = "true" ]; then
                echo -e "${GREEN}✓ Development features enabled${NC}"
            fi
            ;;
        "production")
            TEST_ENDPOINTS=$(echo $RESPONSE | jq -r '.data.config.features.enable_test_endpoints // true')
            if [ "$TEST_ENDPOINTS" = "false" ]; then
                echo -e "${GREEN}✓ Production safety features active${NC}"
            fi
            ;;
    esac
}

# Test configuration file creation
create_test_config() {
    echo -e "\n${GREEN}Creating test configuration file...${NC}"
    
    CONFIG_DIR="${CONFIG_DIR:-./config}"
    TEST_CONFIG="$CONFIG_DIR/config.test.toml"
    
    if [ ! -d "$CONFIG_DIR" ]; then
        mkdir -p "$CONFIG_DIR"
        echo "Created config directory: $CONFIG_DIR"
    fi
    
    cat > "$TEST_CONFIG" << 'EOF'
[server]
port = 9999
workers = 2

[features]
enable_mock_services = false
enable_test_endpoints = false

[monitoring]
log_level = "debug"
EOF
    
    echo "Created test config: $TEST_CONFIG"
}

# Test hot reload
test_hot_reload() {
    echo -e "\n${GREEN}Testing hot reload functionality...${NC}"
    
    CONFIG_DIR="${CONFIG_DIR:-./config}"
    TEST_CONFIG="$CONFIG_DIR/config.test_hotreload.toml"
    
    # Create initial config
    cat > "$TEST_CONFIG" << 'EOF'
[server]
workers = 4
EOF
    
    echo "Created test config for hot reload"
    
    # Wait for potential reload
    sleep 2
    
    # Modify the config
    cat > "$TEST_CONFIG" << 'EOF'
[server]
workers = 8
EOF
    
    echo "Modified config file"
    echo "Check server logs for reload messages"
    
    # Clean up
    rm -f "$TEST_CONFIG"
}

# Performance test
test_performance() {
    echo -e "\n${GREEN}Testing configuration access performance...${NC}"
    
    START_TIME=$(date +%s%N)
    
    # Make 100 config value requests
    for i in {1..100}; do
        curl -s -X GET "$API_URL/api/config/server.port" \
            -H "Authorization: Bearer $TOKEN" > /dev/null
    done
    
    END_TIME=$(date +%s%N)
    DURATION=$((($END_TIME - $START_TIME) / 1000000))
    AVG_TIME=$((DURATION / 100))
    
    echo -e "${GREEN}✓ Completed 100 config requests in ${DURATION}ms${NC}"
    echo "  Average time per request: ${AVG_TIME}ms"
}

# Run all tests
main() {
    check_api
    authenticate
    
    test_get_config
    test_get_config_value
    test_config_override
    test_config_reload
    test_config_export
    test_config_diff
    test_config_validate
    test_access_control
    test_environment_loading
    # test_hot_reload  # Commented out as it modifies files
    test_performance
    
    echo -e "\n${GREEN}Environment configuration tests completed!${NC}"
}

# Handle script arguments
case "${1:-}" in
    "get")
        check_api
        authenticate
        test_get_config
        ;;
    "value")
        check_api
        authenticate
        test_get_config_value
        ;;
    "override")
        check_api
        authenticate
        test_config_override
        ;;
    "export")
        check_api
        authenticate
        test_config_export
        ;;
    "validate")
        check_api
        authenticate
        test_config_validate
        ;;
    "create")
        create_test_config
        ;;
    *)
        main
        ;;
esac