#!/bin/bash

echo "=== API Throughput Benchmark ==="
echo "Testing optimized endpoints for high-throughput performance"
echo ""

# Check if API is running
if ! curl -s http://localhost:8081/health > /dev/null; then
    echo "ERROR: API is not running. Please start the API first."
    exit 1
fi

echo "API is running. Starting benchmark..."
echo ""

# Warm up the API
echo "Warming up..."
for i in {1..10}; do
    curl -s http://localhost:8081/api/markets > /dev/null &
done
wait

echo "Running throughput test..."
echo ""

# Test 1: Sequential requests
echo "Test 1: 100 sequential requests"
START=$(date +%s.%N)
for i in {1..100}; do
    curl -s http://localhost:8081/api/markets > /dev/null
done
END=$(date +%s.%N)
DURATION=$(echo "$END - $START" | bc)
RPS=$(echo "scale=2; 100 / $DURATION" | bc)
echo "Duration: ${DURATION}s"
echo "Requests per second: $RPS"
echo ""

# Test 2: Concurrent requests
echo "Test 2: 100 concurrent requests"
START=$(date +%s.%N)
for i in {1..100}; do
    curl -s http://localhost:8081/api/markets > /dev/null &
done
wait
END=$(date +%s.%N)
DURATION=$(echo "$END - $START" | bc)
RPS=$(echo "scale=2; 100 / $DURATION" | bc)
echo "Duration: ${DURATION}s"
echo "Requests per second: $RPS"
echo ""

# Test 3: Sustained load
echo "Test 3: Sustained load (1000 requests over 10 seconds)"
echo "Sending 100 requests per second..."
SUCCESS=0
FAILED=0

for second in {1..10}; do
    for req in {1..100}; do
        {
            if curl -s -o /dev/null -w "%{http_code}" http://localhost:8081/api/markets | grep -q "200"; then
                ((SUCCESS++))
            else
                ((FAILED++))
            fi
        } &
    done
    sleep 1
done
wait

echo "Completed: $SUCCESS successful, $FAILED failed"
TOTAL=$((SUCCESS + FAILED))
SUCCESS_RATE=$(echo "scale=2; $SUCCESS * 100 / $TOTAL" | bc)
echo "Success rate: ${SUCCESS_RATE}%"
echo ""

# Check compression
echo "Test 4: Compression check"
SIZE_UNCOMPRESSED=$(curl -s http://localhost:8081/api/markets | wc -c)
SIZE_COMPRESSED=$(curl -s -H "Accept-Encoding: gzip" http://localhost:8081/api/markets | wc -c)
COMPRESSION_RATIO=$(echo "scale=2; (1 - $SIZE_COMPRESSED / $SIZE_UNCOMPRESSED) * 100" | bc)
echo "Uncompressed size: $SIZE_UNCOMPRESSED bytes"
echo "Compressed size: $SIZE_COMPRESSED bytes"
echo "Compression ratio: ${COMPRESSION_RATIO}%"
echo ""

echo "=== Optimization Summary ==="
echo "1. TCP_NODELAY enabled for low latency"
echo "2. Compression enabled for bandwidth savings"
echo "3. Connection keep-alive for connection reuse"
echo "4. Pre-allocated JSON buffers for faster serialization"
echo "5. 32 worker threads for concurrent request handling"
echo "6. Optimized socket buffers (256KB send/recv)"