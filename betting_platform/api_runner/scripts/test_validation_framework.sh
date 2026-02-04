#!/bin/bash

# Data Validation Framework Test Script
# Tests all validation functionality including schemas, validators, and middleware

set -e

API_URL="${API_URL:-http://localhost:8081}"
ADMIN_TOKEN="${ADMIN_TOKEN:-}"
USER_TOKEN="${USER_TOKEN:-}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Check if tokens are provided
if [ -z "$ADMIN_TOKEN" ]; then
    log_warn "ADMIN_TOKEN not set. Some tests will be skipped."
fi

if [ -z "$USER_TOKEN" ]; then
    log_warn "USER_TOKEN not set. Some tests will be skipped."
fi

# Test schema registration
test_schema_registration() {
    log_info "Testing schema registration..."
    
    if [ -n "$ADMIN_TOKEN" ]; then
        # Register a test schema
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/schemas" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "name": "test_user_profile",
                "rules": [
                    {
                        "field": "username",
                        "rule_type": "Required",
                        "message": null,
                        "severity": "Error"
                    },
                    {
                        "field": "username",
                        "rule_type": {
                            "MinLength": 3
                        },
                        "message": "Username must be at least 3 characters",
                        "severity": "Error"
                    },
                    {
                        "field": "age",
                        "rule_type": {
                            "Range": {
                                "min": 18,
                                "max": 120
                            }
                        },
                        "message": "Age must be between 18 and 120",
                        "severity": "Error"
                    },
                    {
                        "field": "email",
                        "rule_type": "Email",
                        "message": null,
                        "severity": "Error"
                    },
                    {
                        "field": "role",
                        "rule_type": {
                            "OneOf": ["user", "admin", "moderator"]
                        },
                        "message": null,
                        "severity": "Error"
                    }
                ]
            }')
        
        if echo "$RESPONSE" | grep -q '"success":true'; then
            log_info "✓ Schema registration successful"
        else
            log_error "✗ Schema registration failed: $RESPONSE"
        fi
    fi
}

# Test data validation
test_data_validation() {
    log_info "Testing data validation..."
    
    if [ -n "$USER_TOKEN" ]; then
        # Test valid data
        log_info "Testing valid data..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/validate/test_user_profile" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "username": "john_doe",
                "age": 25,
                "email": "john@example.com",
                "role": "user"
            }')
        
        if echo "$RESPONSE" | grep -q '"is_valid":true'; then
            log_info "✓ Valid data passed validation"
        else
            log_error "✗ Valid data failed validation: $RESPONSE"
        fi
        
        # Test invalid data
        log_info "Testing invalid data..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/validate/test_user_profile" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "username": "jd",
                "age": 15,
                "email": "not-an-email",
                "role": "superuser"
            }')
        
        if echo "$RESPONSE" | grep -q '"is_valid":false'; then
            log_info "✓ Invalid data failed validation correctly"
            echo "$RESPONSE" | jq '.data.report.errors' 2>/dev/null || echo "$RESPONSE"
        else
            log_error "✗ Invalid data passed validation incorrectly"
        fi
    fi
}

# Test endpoint validation configuration
test_endpoint_validation() {
    log_info "Testing endpoint validation configuration..."
    
    if [ -n "$ADMIN_TOKEN" ]; then
        # Configure endpoint validation
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/endpoints" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "method": "POST",
                "path_pattern": "/api/test/profile",
                "schema_name": "test_user_profile",
                "extract_params": false,
                "validate_query": false
            }')
        
        if echo "$RESPONSE" | grep -q '"success":true'; then
            log_info "✓ Endpoint validation configured"
        else
            log_error "✗ Endpoint validation configuration failed: $RESPONSE"
        fi
    fi
}

# Test middleware configuration
test_middleware_config() {
    log_info "Testing middleware configuration..."
    
    if [ -n "$ADMIN_TOKEN" ]; then
        RESPONSE=$(curl -s -X PUT "$API_URL/api/validation/config" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "enabled": true,
                "log_violations": true,
                "fail_on_warning": false,
                "endpoints": [
                    {
                        "method": "POST",
                        "path_pattern": "/api/markets/create",
                        "schema_name": "market_creation",
                        "extract_params": false,
                        "validate_query": false
                    },
                    {
                        "method": "POST",
                        "path_pattern": "/api/trade/place",
                        "schema_name": "trade_execution",
                        "extract_params": false,
                        "validate_query": false
                    }
                ]
            }')
        
        if echo "$RESPONSE" | grep -q '"success":true'; then
            log_info "✓ Middleware configuration updated"
        else
            log_error "✗ Middleware configuration failed: $RESPONSE"
        fi
    fi
}

# Test validation statistics
test_validation_stats() {
    log_info "Testing validation statistics..."
    
    if [ -n "$USER_TOKEN" ]; then
        RESPONSE=$(curl -s -X GET "$API_URL/api/validation/stats" \
            -H "Authorization: Bearer $USER_TOKEN")
        
        if echo "$RESPONSE" | grep -q '"data":{'; then
            log_info "✓ Validation statistics retrieved"
            echo "$RESPONSE" | jq '.data' 2>/dev/null || echo "$RESPONSE"
        else
            log_error "✗ Failed to get validation statistics: $RESPONSE"
        fi
    fi
}

# Test domain validators
test_domain_validators() {
    log_info "Testing domain-specific validators..."
    
    if [ -n "$USER_TOKEN" ]; then
        # Test position validation
        log_info "Testing position validation..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/validate/position" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "size": 100000,
                "leverage": 10,
                "collateral": 10000
            }')
        
        echo "Position validation response: $RESPONSE"
        
        # Test order validation
        log_info "Testing order validation..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/validate/order" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "market_id": "btc-100k-2024",
                "side": "buy",
                "order_type": "limit",
                "size": 1000000,
                "price": 0.65,
                "time_in_force": "GTC"
            }')
        
        echo "Order validation response: $RESPONSE"
    fi
}

# Test security validation
test_security_validation() {
    log_info "Testing security validation..."
    
    if [ -n "$USER_TOKEN" ]; then
        # Test SQL injection detection
        log_info "Testing SQL injection detection..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/validate/test_user_profile" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "username": "admin\"; DROP TABLE users; --",
                "age": 25,
                "email": "test@example.com",
                "role": "user"
            }')
        
        if echo "$RESPONSE" | grep -q '"is_valid":false' || echo "$RESPONSE" | grep -q "security"; then
            log_info "✓ SQL injection detected"
        else
            log_warn "SQL injection may not have been detected"
        fi
        
        # Test XSS detection
        log_info "Testing XSS detection..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/validate/test_user_profile" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "username": "<script>alert(\"XSS\")</script>",
                "age": 25,
                "email": "test@example.com",
                "role": "user"
            }')
        
        if echo "$RESPONSE" | grep -q '"is_valid":false' || echo "$RESPONSE" | grep -q "security"; then
            log_info "✓ XSS attempt detected"
        else
            log_warn "XSS may not have been detected"
        fi
    fi
}

# Test cache functionality
test_cache_functionality() {
    log_info "Testing cache functionality..."
    
    if [ -n "$ADMIN_TOKEN" ]; then
        # Clear cache
        RESPONSE=$(curl -s -X POST "$API_URL/api/validation/cache/clear" \
            -H "Authorization: Bearer $ADMIN_TOKEN")
        
        if echo "$RESPONSE" | grep -q '"success":true'; then
            log_info "✓ Cache cleared successfully"
        else
            log_error "✗ Cache clear failed: $RESPONSE"
        fi
        
        # Validate same data twice to test caching
        if [ -n "$USER_TOKEN" ]; then
            DATA='{
                "username": "cache_test_user",
                "age": 30,
                "email": "cache@test.com",
                "role": "user"
            }'
            
            # First validation (should miss cache)
            START_TIME=$(date +%s%N)
            curl -s -X POST "$API_URL/api/validation/validate/test_user_profile" \
                -H "Authorization: Bearer $USER_TOKEN" \
                -H "Content-Type: application/json" \
                -d "$DATA" > /dev/null
            END_TIME=$(date +%s%N)
            FIRST_TIME=$((($END_TIME - $START_TIME) / 1000000))
            
            # Second validation (should hit cache)
            START_TIME=$(date +%s%N)
            curl -s -X POST "$API_URL/api/validation/validate/test_user_profile" \
                -H "Authorization: Bearer $USER_TOKEN" \
                -H "Content-Type: application/json" \
                -d "$DATA" > /dev/null
            END_TIME=$(date +%s%N)
            SECOND_TIME=$((($END_TIME - $START_TIME) / 1000000))
            
            log_info "First validation: ${FIRST_TIME}ms, Second validation: ${SECOND_TIME}ms"
            
            if [ $SECOND_TIME -lt $FIRST_TIME ]; then
                log_info "✓ Cache appears to be working (second request faster)"
            else
                log_warn "Cache may not be working effectively"
            fi
        fi
    fi
}

# Test middleware validation on actual endpoints
test_middleware_validation() {
    log_info "Testing middleware validation on actual endpoints..."
    
    if [ -n "$USER_TOKEN" ]; then
        # Test market creation with invalid data
        log_info "Testing market creation with invalid data..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/markets/create" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "title": "Short",
                "category": "invalid_category",
                "outcomes": ["Yes"]
            }')
        
        if echo "$RESPONSE" | grep -q "Validation failed" || echo "$RESPONSE" | grep -q "400"; then
            log_info "✓ Invalid market creation rejected by validation"
        else
            log_warn "Invalid market creation may not have been validated"
        fi
        
        # Test trade placement with invalid data
        log_info "Testing trade placement with invalid data..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/trade/place" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "side": "invalid_side",
                "amount": -100
            }')
        
        if echo "$RESPONSE" | grep -q "Validation failed" || echo "$RESPONSE" | grep -q "400"; then
            log_info "✓ Invalid trade rejected by validation"
        else
            log_warn "Invalid trade may not have been validated"
        fi
    fi
}

# Performance test
test_validation_performance() {
    log_info "Testing validation performance..."
    
    if [ -n "$USER_TOKEN" ]; then
        ITERATIONS=100
        
        START_TIME=$(date +%s)
        
        for i in $(seq 1 $ITERATIONS); do
            curl -s -X POST "$API_URL/api/validation/validate/test_user_profile" \
                -H "Authorization: Bearer $USER_TOKEN" \
                -H "Content-Type: application/json" \
                -d "{
                    \"username\": \"perf_test_$i\",
                    \"age\": $((18 + i % 50)),
                    \"email\": \"user$i@test.com\",
                    \"role\": \"user\"
                }" > /dev/null &
            
            # Limit concurrent requests
            if [ $((i % 10)) -eq 0 ]; then
                wait
            fi
        done
        
        wait
        
        END_TIME=$(date +%s)
        DURATION=$((END_TIME - START_TIME))
        RATE=$((ITERATIONS / DURATION))
        
        log_info "✓ Validated $ITERATIONS requests in ${DURATION}s (${RATE} req/s)"
    fi
}

# Main test execution
main() {
    log_info "Starting data validation framework tests..."
    echo ""
    
    test_schema_registration
    echo ""
    
    test_data_validation
    echo ""
    
    test_endpoint_validation
    echo ""
    
    test_middleware_config
    echo ""
    
    test_validation_stats
    echo ""
    
    test_domain_validators
    echo ""
    
    test_security_validation
    echo ""
    
    test_cache_functionality
    echo ""
    
    test_middleware_validation
    echo ""
    
    test_validation_performance
    echo ""
    
    log_info "Data validation framework tests completed!"
}

# Run tests
main