#!/bin/bash

# Test Redis cache functionality

API_URL="http://localhost:8081"

echo "Testing Redis Cache Functionality..."
echo "==================================="

# 1. Check cache health
echo -e "\n1. Checking cache health:"
curl -s "$API_URL/api/cache/health" | jq '.'

# 2. Get cache statistics (should be empty initially)
echo -e "\n2. Getting initial cache statistics:"
curl -s "$API_URL/api/cache/stats" | jq '.'

# 3. Make some API calls to populate cache
echo -e "\n3. Making API calls to populate cache..."

# Get markets (should be cached)
echo -e "\n   - Fetching markets (1st call - MISS):"
START=$(date +%s%N)
curl -s -H "Accept: application/json" "$API_URL/api/markets" > /dev/null
END=$(date +%s%N)
DURATION=$((($END - $START)/1000000))
echo "     Duration: ${DURATION}ms"

echo -e "\n   - Fetching markets (2nd call - HIT):"
START=$(date +%s%N)
RESPONSE=$(curl -s -H "Accept: application/json" -D - "$API_URL/api/markets")
END=$(date +%s%N)
DURATION=$((($END - $START)/1000000))
echo "     Duration: ${DURATION}ms"
echo "$RESPONSE" | grep -i "x-cache" || echo "     No cache header found"

# Get verses (should be cached)
echo -e "\n   - Fetching verses:"
curl -s "$API_URL/api/verses" > /dev/null

# Get program info (should be cached)
echo -e "\n   - Fetching program info:"
curl -s "$API_URL/api/program/info" > /dev/null

# 4. Check cache statistics after calls
echo -e "\n4. Cache statistics after API calls:"
curl -s "$API_URL/api/cache/stats" | jq '.'

# 5. Warm cache
echo -e "\n5. Warming cache:"
curl -s -X POST "$API_URL/api/cache/warm" | jq '.'

# 6. Check specific cache key
echo -e "\n6. Checking specific cache key (markets:list):"
curl -s "$API_URL/api/cache/key/markets:list" | jq '.data | {exists: .exists, has_value: (if .value then true else false end)}'

# 7. Test cache invalidation
echo -e "\n7. Testing cache invalidation:"
curl -s -X POST "$API_URL/api/cache/invalidate" \
  -H "Content-Type: application/json" \
  -d '{
    "patterns": ["market", "verses:list"]
  }' | jq '.'

# 8. Final cache statistics
echo -e "\n8. Final cache statistics:"
curl -s "$API_URL/api/cache/stats" | jq '.'

# 9. Test cache TTL setting
echo -e "\n9. Testing cache TTL setting:"
curl -s -X POST "$API_URL/api/cache/ttl" \
  -H "Content-Type: application/json" \
  -d '{
    "key": "test:key",
    "ttl": 60
  }' | jq '.'

echo -e "\nCache functionality tests completed!"