#!/bin/bash

# State Management System Test Script
# Tests all state management endpoints and functionality

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

# Test state management endpoints
test_state_management() {
    log_info "Testing state management endpoints..."
    
    # 1. Test setting state (admin only)
    if [ -n "$ADMIN_TOKEN" ]; then
        log_info "Testing SET state..."
        RESPONSE=$(curl -s -X PUT "$API_URL/api/v1/state/test:key1" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "key": "test:key1",
                "value": {
                    "data": "test value",
                    "number": 42,
                    "array": [1, 2, 3]
                },
                "metadata": {
                    "purpose": "testing",
                    "created_by": "test_script"
                }
            }')
        
        if echo "$RESPONSE" | grep -q '"success":true'; then
            log_info "✓ Set state successful"
        else
            log_error "✗ Set state failed: $RESPONSE"
        fi
    fi
    
    # 2. Test getting state
    if [ -n "$USER_TOKEN" ]; then
        log_info "Testing GET state..."
        RESPONSE=$(curl -s -X GET "$API_URL/api/v1/state/test:key1?include_metadata=true" \
            -H "Authorization: Bearer $USER_TOKEN")
        
        if echo "$RESPONSE" | grep -q '"data":'; then
            log_info "✓ Get state successful"
            echo "$RESPONSE" | jq '.data' 2>/dev/null || echo "$RESPONSE"
        else
            log_error "✗ Get state failed: $RESPONSE"
        fi
    fi
    
    # 3. Test listing keys
    if [ -n "$USER_TOKEN" ]; then
        log_info "Testing LIST keys..."
        RESPONSE=$(curl -s -X GET "$API_URL/api/v1/state/keys?prefix=test:" \
            -H "Authorization: Bearer $USER_TOKEN")
        
        if echo "$RESPONSE" | grep -q '"keys":'; then
            log_info "✓ List keys successful"
            echo "$RESPONSE" | jq '.data' 2>/dev/null || echo "$RESPONSE"
        else
            log_error "✗ List keys failed: $RESPONSE"
        fi
    fi
    
    # 4. Test compare-and-swap
    if [ -n "$ADMIN_TOKEN" ]; then
        log_info "Testing COMPARE-AND-SWAP..."
        
        # First set a counter
        curl -s -X PUT "$API_URL/api/v1/state/test:counter" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{"key": "test:counter", "value": 10}' > /dev/null
        
        # Try CAS
        RESPONSE=$(curl -s -X POST "$API_URL/api/v1/state/cas" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "key": "test:counter",
                "expected": 10,
                "new_value": 11
            }')
        
        if echo "$RESPONSE" | grep -q '"success":true'; then
            log_info "✓ Compare-and-swap successful"
        else
            log_error "✗ Compare-and-swap failed: $RESPONSE"
        fi
        
        # Try failed CAS
        RESPONSE=$(curl -s -X POST "$API_URL/api/v1/state/cas" \
            -H "Authorization: Bearer $ADMIN_TOKEN" \
            -H "Content-Type: application/json" \
            -d '{
                "key": "test:counter",
                "expected": 10,
                "new_value": 12
            }')
        
        if echo "$RESPONSE" | grep -q '"success":false'; then
            log_info "✓ Failed CAS handled correctly"
        else
            log_error "✗ Failed CAS not handled properly: $RESPONSE"
        fi
    fi
    
    # 5. Test state statistics
    if [ -n "$USER_TOKEN" ]; then
        log_info "Testing STATE STATS..."
        RESPONSE=$(curl -s -X GET "$API_URL/api/v1/state/stats" \
            -H "Authorization: Bearer $USER_TOKEN")
        
        if echo "$RESPONSE" | grep -q '"total_keys":'; then
            log_info "✓ State stats successful"
            echo "$RESPONSE" | jq '.data' 2>/dev/null || echo "$RESPONSE"
        else
            log_error "✗ State stats failed: $RESPONSE"
        fi
    fi
    
    # 6. Test creating snapshot (admin only)
    if [ -n "$ADMIN_TOKEN" ]; then
        log_info "Testing CREATE SNAPSHOT..."
        RESPONSE=$(curl -s -X POST "$API_URL/api/v1/state/snapshot" \
            -H "Authorization: Bearer $ADMIN_TOKEN")
        
        if echo "$RESPONSE" | grep -q '"timestamp":'; then
            log_info "✓ Create snapshot successful"
        else
            log_error "✗ Create snapshot failed: $RESPONSE"
        fi
    fi
    
    # 7. Test removing state
    if [ -n "$ADMIN_TOKEN" ]; then
        log_info "Testing REMOVE state..."
        RESPONSE=$(curl -s -X DELETE "$API_URL/api/v1/state/test:key1" \
            -H "Authorization: Bearer $ADMIN_TOKEN")
        
        if echo "$RESPONSE" | grep -q '"success":true'; then
            log_info "✓ Remove state successful"
        else
            log_error "✗ Remove state failed: $RESPONSE"
        fi
        
        # Verify removal
        RESPONSE=$(curl -s -X GET "$API_URL/api/v1/state/test:key1" \
            -H "Authorization: Bearer $USER_TOKEN")
        
        if echo "$RESPONSE" | grep -q '"data":null'; then
            log_info "✓ State removed verified"
        else
            log_error "✗ State still exists after removal"
        fi
    fi
}

# Test WebSocket state events
test_websocket_events() {
    if [ -n "$USER_TOKEN" ] && command -v wscat &> /dev/null; then
        log_info "Testing WebSocket state events..."
        
        # Start WebSocket listener in background
        (
            echo "Connecting to WebSocket..."
            timeout 10 wscat -c "ws://localhost:8081/api/v1/state/events" \
                -H "Authorization: Bearer $USER_TOKEN" \
                -x '{"type":"ping"}' || true
        ) &
        WS_PID=$!
        
        sleep 2
        
        # Trigger state change
        if [ -n "$ADMIN_TOKEN" ]; then
            curl -s -X PUT "$API_URL/api/v1/state/test:websocket" \
                -H "Authorization: Bearer $ADMIN_TOKEN" \
                -H "Content-Type: application/json" \
                -d '{"key": "test:websocket", "value": {"event": "test"}}' > /dev/null
        fi
        
        wait $WS_PID 2>/dev/null || true
        log_info "✓ WebSocket test completed"
    else
        log_warn "wscat not installed, skipping WebSocket tests"
    fi
}

# Test state synchronization
test_state_sync() {
    log_info "Testing state synchronization..."
    
    if [ -n "$USER_TOKEN" ]; then
        # Make a request and check if it's tracked
        curl -s -X GET "$API_URL/api/markets" \
            -H "Authorization: Bearer $USER_TOKEN" \
            -H "X-Correlation-ID: test-correlation-123" > /dev/null
        
        sleep 1
        
        # Check if request was tracked
        if [ -n "$ADMIN_TOKEN" ]; then
            RESPONSE=$(curl -s -X GET "$API_URL/api/v1/state/request:test-correlation-123" \
                -H "Authorization: Bearer $ADMIN_TOKEN")
            
            if echo "$RESPONSE" | grep -q '"method":'; then
                log_info "✓ Request tracking working"
            else
                log_warn "Request tracking may not be working"
            fi
        fi
    fi
}

# Performance test
test_performance() {
    if [ -n "$ADMIN_TOKEN" ]; then
        log_info "Testing state management performance..."
        
        START_TIME=$(date +%s)
        
        # Set multiple keys
        for i in {1..100}; do
            curl -s -X PUT "$API_URL/api/v1/state/perf:test$i" \
                -H "Authorization: Bearer $ADMIN_TOKEN" \
                -H "Content-Type: application/json" \
                -d "{\"key\": \"perf:test$i\", \"value\": $i}" > /dev/null &
            
            # Limit concurrent requests
            if [ $((i % 10)) -eq 0 ]; then
                wait
            fi
        done
        
        wait
        
        END_TIME=$(date +%s)
        DURATION=$((END_TIME - START_TIME))
        
        log_info "✓ Set 100 keys in ${DURATION}s"
        
        # Get all keys
        START_TIME=$(date +%s)
        
        RESPONSE=$(curl -s -X GET "$API_URL/api/v1/state/keys?prefix=perf:" \
            -H "Authorization: Bearer $USER_TOKEN")
        
        END_TIME=$(date +%s)
        DURATION=$((END_TIME - START_TIME))
        
        KEY_COUNT=$(echo "$RESPONSE" | jq -r '.data.total' 2>/dev/null || echo "0")
        log_info "✓ Listed $KEY_COUNT keys in ${DURATION}s"
        
        # Cleanup
        for i in {1..100}; do
            curl -s -X DELETE "$API_URL/api/v1/state/perf:test$i" \
                -H "Authorization: Bearer $ADMIN_TOKEN" > /dev/null &
            
            if [ $((i % 10)) -eq 0 ]; then
                wait
            fi
        done
        
        wait
        log_info "✓ Cleanup completed"
    fi
}

# Main test execution
main() {
    log_info "Starting state management tests..."
    echo ""
    
    test_state_management
    echo ""
    
    test_websocket_events
    echo ""
    
    test_state_sync
    echo ""
    
    test_performance
    echo ""
    
    log_info "State management tests completed!"
}

# Run tests
main