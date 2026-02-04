#!/bin/bash

echo "Testing Verse Matching Logic Directly"
echo "===================================="
echo

# Test with a simulated Biden approval market
curl -X POST http://localhost:8081/api/test/verse-match \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Will Biden approval rating per FiveThirtyEight be above 40% on March 1?",
    "category": "Politics",
    "description": "This market will resolve to Yes if President Biden's approval rating according to FiveThirtyEight polls is above 40% on March 1, 2024."
  }' | jq '.'

echo
echo "Testing with generic category but political keywords:"
echo

curl -X POST http://localhost:8081/api/test/verse-match \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Will Biden approval rating per FiveThirtyEight be above 40% on March 1?",
    "category": "General",
    "description": "This market will resolve to Yes if President Biden's approval rating according to FiveThirtyEight polls is above 40% on March 1, 2024."
  }' | jq '.'