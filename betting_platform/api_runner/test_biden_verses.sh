#!/bin/bash

echo "Testing Biden Approval Market Verses"
echo "===================================="

# Fetch markets and find Biden approval market
response=$(curl -s http://localhost:8081/api/polymarket/markets)

# Find Biden approval market - more flexible search
biden_market=$(echo "$response" | grep -B5 -A100 -i "biden" | grep -B5 -A100 -i "approval\|rating" | head -200)

if [ -n "$biden_market" ]; then
    echo "Found Biden approval market:"
    echo "$biden_market" | grep -o '"question":"[^"]*"' | head -1
    echo
    
    # Extract verses
    verses=$(echo "$biden_market" | grep -A50 '"verses":\[' | sed -n '/"verses":/,/\]/p')
    
    if [ -n "$verses" ]; then
        echo "Verses assigned:"
        echo "$verses" | grep -o '"name":"[^"]*"' | sed 's/"name":"//g' | sed 's/"//g'
        echo
        echo "Verse levels:"
        echo "$verses" | grep -o '"level":[0-9]' | sort | uniq -c
        echo
        echo "Multipliers:"
        echo "$verses" | grep -o '"multiplier":[0-9.]*' | sed 's/"multiplier"://g'
    else
        echo "NO VERSES FOUND!"
    fi
else
    echo "Biden approval market not found"
    echo "First few markets:"
    echo "$response" | grep -o '"question":"[^"]*"' | head -5
fi