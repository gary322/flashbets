#!/bin/bash

echo "=== Database Connection Pool Load Test ==="
echo "Testing optimized pool configuration for high load scenarios"
echo ""

# Start API with high load configuration
export EXPECTED_CONCURRENT_USERS=2500
export RUST_LOG=betting_platform_api=info

echo "Starting API with pool optimized for $EXPECTED_CONCURRENT_USERS concurrent users..."
cargo run --release > pool_test.log 2>&1 &
API_PID=$!

# Wait for API to start
sleep 5

# Check if API started successfully
if ! ps -p $API_PID > /dev/null; then
    echo "ERROR: API failed to start"
    cat pool_test.log
    exit 1
fi

echo "API started successfully with PID $API_PID"

# Extract pool configuration from logs
echo ""
echo "Pool Configuration:"
grep "Pool configuration:" pool_test.log | tail -1

# Simple concurrent request test
echo ""
echo "Testing with concurrent requests..."
echo "Sending 100 concurrent requests to /api/markets"

# Use curl in parallel
for i in {1..100}; do
    curl -s http://localhost:8081/api/markets -o /dev/null -w "%{http_code}\n" &
done | grep -c "200"

wait

echo ""
echo "Test completed. Check pool_test.log for details."

# Cleanup
kill $API_PID 2>/dev/null

# Show pool recommendations
echo ""
echo "Pool Optimization Recommendations:"
echo "=================================="
cat << EOF
For 2000+ concurrent users:
- Max connections: 200
- Min idle connections: 50  
- Connection timeout: 5s
- Idle timeout: 5 minutes
- Max lifetime: 15 minutes

PostgreSQL Configuration:
- Ensure max_connections >= 250 in postgresql.conf
- shared_buffers = 25% of RAM
- effective_cache_size = 75% of RAM
- work_mem = RAM / max_connections / 2
EOF