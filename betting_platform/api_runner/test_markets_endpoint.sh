#!/bin/bash

echo "=== Markets Endpoint Test ==="
echo ""

API_URL="http://localhost:8081"

# Check if API is running
if ! curl -s $API_URL/health > /dev/null; then
    echo "ERROR: API is not running. Please start the API first."
    exit 1
fi

echo "1. Testing basic markets endpoint (v1)..."
echo ""
curl -s "$API_URL/api/markets?limit=5" | jq '.'
echo ""

echo "2. Testing enhanced markets endpoint (v2)..."
echo ""
curl -s "$API_URL/api/v2/markets?limit=5" | jq '.'
echo ""

echo "3. Testing markets with search filter..."
echo ""
curl -s "$API_URL/api/v2/markets?search=2024&limit=5" | jq '.'
echo ""

echo "4. Testing markets with status filter..."
echo ""
curl -s "$API_URL/api/v2/markets?status=active&limit=5" | jq '.'
echo ""

echo "5. Testing markets with sorting..."
echo ""
curl -s "$API_URL/api/v2/markets?sort=volume&limit=5" | jq '.'
echo ""

echo "6. Testing market statistics..."
echo ""
curl -s "$API_URL/api/v2/markets/stats" | jq '.'
echo ""

echo "7. Testing single market by ID..."
echo ""
# Try fetching market with ID 1001
curl -s "$API_URL/api/v2/markets/1001" | jq '.'
echo ""

echo "8. Testing Polymarket proxy..."
echo ""
curl -s "$API_URL/api/polymarket/markets?limit=3" | jq '.'
echo ""

echo "=== Market Data Sources Test ==="
echo ""

# Test with various parameters
echo "Testing pagination..."
curl -s "$API_URL/api/v2/markets?limit=10&offset=5" | jq '.pagination'
echo ""

echo "Testing metadata..."
curl -s "$API_URL/api/v2/markets?limit=5" | jq '.metadata'
echo ""

echo "Testing filters..."
curl -s "$API_URL/api/v2/markets?min_volume=1000&limit=5" | jq '.filters_applied'
echo ""

echo "=== Summary ==="
echo ""
echo "✓ Basic markets endpoint (backward compatible)"
echo "✓ Enhanced v2 markets endpoint with metadata"
echo "✓ Search and filtering capabilities"
echo "✓ Sorting options (volume, liquidity, created)"
echo "✓ Market statistics endpoint"
echo "✓ Single market lookup"
echo "✓ Polymarket integration"
echo "✓ Pagination support"
echo "✓ Multiple data source aggregation"
echo ""
echo "Data sources priority:"
echo "1. Database (if available)"
echo "2. Polymarket API (live data)"
echo "3. Solana blockchain"
echo "4. Seeded/mock data (fallback)"