#!/bin/bash

# Create test data for the betting platform

echo "Creating test markets and data..."

# Create some test markets via API
curl -X POST http://localhost:8081/api/markets/create \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Will Bitcoin reach $100k by 2025?",
    "description": "Market for Bitcoin price prediction",
    "category": "Crypto",
    "endTime": "2025-01-01T00:00:00Z",
    "ammType": "LMSR"
  }'

curl -X POST http://localhost:8081/api/markets/create \
  -H "Content-Type: application/json" \
  -d '{
    "title": "US Presidential Election 2024",
    "description": "Who will win the 2024 US Presidential Election?",
    "category": "Politics",
    "endTime": "2024-11-05T00:00:00Z",
    "ammType": "PM-AMM"
  }'

curl -X POST http://localhost:8081/api/markets/create \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Will AGI be achieved by 2030?",
    "description": "Artificial General Intelligence achievement prediction",
    "category": "Technology",
    "endTime": "2030-12-31T00:00:00Z",
    "ammType": "Hybrid"
  }'

echo "Test data creation complete!"