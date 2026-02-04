#!/bin/bash

# Test external API integration endpoints

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
API_URL="${API_URL:-http://localhost:8081}"

echo -e "${GREEN}=== Testing External API Integration ===${NC}"
echo ""

# 1. Check external API health
echo "1. Checking external API health..."
HEALTH=$(curl -s $API_URL/api/external/health)
echo "$HEALTH" | python3 -m json.tool || echo "$HEALTH"
echo ""

# 2. Test Polymarket connectivity
echo "2. Testing Polymarket connectivity..."
POLYMARKET_TEST=$(curl -s $API_URL/api/external/test/polymarket)
echo "$POLYMARKET_TEST" | python3 -m json.tool || echo "$POLYMARKET_TEST"
echo ""

# 3. Test Kalshi connectivity
echo "3. Testing Kalshi connectivity..."
KALSHI_TEST=$(curl -s $API_URL/api/external/test/kalshi)
echo "$KALSHI_TEST" | python3 -m json.tool || echo "$KALSHI_TEST"
echo ""

# 4. Fetch markets from all platforms
echo "4. Fetching markets from all platforms (limit=5)..."
MARKETS=$(curl -s "$API_URL/api/external/markets?limit=5")
echo "$MARKETS" | python3 -m json.tool | head -50 || echo "$MARKETS"
echo ""

# 5. Fetch markets from specific platform
echo "5. Fetching Polymarket markets only..."
POLY_MARKETS=$(curl -s "$API_URL/api/external/markets?platform=polymarket&limit=3")
echo "$POLY_MARKETS" | python3 -m json.tool | head -30 || echo "$POLY_MARKETS"
echo ""

# 6. Check sync status
echo "6. Checking market sync status..."
SYNC_STATUS=$(curl -s $API_URL/api/external/sync/status)
echo "$SYNC_STATUS" | python3 -m json.tool || echo "$SYNC_STATUS"
echo ""

# 7. Get cached prices
echo "7. Getting cached prices..."
CACHED_PRICES=$(curl -s $API_URL/api/external/cache/prices)
if [ "$CACHED_PRICES" != "{}" ] && [ -n "$CACHED_PRICES" ]; then
    echo "$CACHED_PRICES" | python3 -m json.tool | head -30 || echo "$CACHED_PRICES"
else
    echo "No cached prices yet"
fi
echo ""

# 8. Test fetching specific market prices
echo "8. Testing price fetch for specific markets..."
if echo "$POLY_MARKETS" | grep -q "condition_id"; then
    # Extract first market ID
    MARKET_ID=$(echo "$POLY_MARKETS" | python3 -c "
import json
import sys
data = json.load(sys.stdin)
if data and isinstance(data, list) and len(data) > 0:
    print(data[0].get('id', ''))
" 2>/dev/null || echo "")
    
    if [ -n "$MARKET_ID" ]; then
        echo "Fetching prices for market: $MARKET_ID"
        PRICES=$(curl -s -X POST $API_URL/api/external/prices/polymarket \
            -H "Content-Type: application/json" \
            -d "[\"$MARKET_ID\"]")
        echo "$PRICES" | python3 -m json.tool || echo "$PRICES"
    else
        echo "Could not extract market ID"
    fi
else
    echo "No Polymarket markets available for price testing"
fi
echo ""

# 9. Test market comparison
echo "9. Testing market comparison..."
COMPARISON=$(curl -s "$API_URL/api/external/compare?threshold=0.7")
echo "$COMPARISON" | python3 -m json.tool || echo "$COMPARISON"
echo ""

# Summary
echo -e "${GREEN}=== Test Summary ===${NC}"
echo ""

# Check Polymarket health
if echo "$POLYMARKET_TEST" | grep -q '"success":true'; then
    echo -e "${GREEN}✓ Polymarket API is working${NC}"
else
    echo -e "${RED}✗ Polymarket API issues detected${NC}"
fi

# Check Kalshi health
if echo "$KALSHI_TEST" | grep -q '"success":true'; then
    echo -e "${GREEN}✓ Kalshi API is working${NC}"
else
    echo -e "${YELLOW}⚠ Kalshi API may have issues (check if API keys are configured)${NC}"
fi

# Check sync service
if echo "$SYNC_STATUS" | grep -q "is_running"; then
    echo -e "${GREEN}✓ Market sync service is active${NC}"
else
    echo -e "${YELLOW}⚠ Market sync service status unclear${NC}"
fi

echo ""
echo "External API integration test complete!"