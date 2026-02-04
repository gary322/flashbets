#!/bin/bash

echo "Testing verse endpoints..."

# Test catalog size
echo -e "\n1. Testing verse catalog endpoint:"
response=$(curl -s http://localhost:8081/api/verses)
if [ -z "$response" ] || [ "$response" = "[]" ]; then
    echo "   ❌ Empty response from /api/verses"
else
    verse_count=$(echo "$response" | grep -o '"id"' | wc -l)
    echo "   ✅ Found $verse_count verses"
fi

# Test a specific market
echo -e "\n2. Testing market verses:"
markets=$(curl -s http://localhost:8081/api/polymarket/markets | head -c 10000)
first_market=$(echo "$markets" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -n "$first_market" ]; then
    echo "   First market ID: $first_market"
    # Extract verses for first market
    verses=$(echo "$markets" | grep -A20 "\"id\":\"$first_market\"" | grep -o '"verses":\[[^]]*\]' | head -1)
    if [ -n "$verses" ]; then
        verse_count=$(echo "$verses" | grep -o '"id"' | wc -l)
        echo "   ✅ Market has $verse_count verses"
    else
        echo "   ❌ No verses found for market"
    fi
else
    echo "   ❌ No markets found"
fi

# Check health endpoint for debugging
echo -e "\n3. Checking server health:"
health=$(curl -s http://localhost:8081/health)
echo "   Response: $health"