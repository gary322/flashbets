#!/bin/bash

# Load test runner script
# Requires k6 to be installed: brew install k6

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

BASE_URL=${BASE_URL:-http://localhost:8081}

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Betting Platform Load Test${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "Target URL: ${YELLOW}$BASE_URL${NC}"
echo -e "Start Time: $(date)"
echo ""

# Check if k6 is installed
if ! command -v k6 &> /dev/null; then
    echo -e "${RED}k6 is not installed${NC}"
    echo "Install with: brew install k6"
    exit 1
fi

# Check if server is running
echo -e "${YELLOW}Checking if API server is running...${NC}"
if curl -s $BASE_URL/health > /dev/null 2>&1; then
    echo -e "${GREEN}API server is running${NC}"
else
    echo -e "${RED}API server is not running at $BASE_URL${NC}"
    echo "Please start the server before running load tests"
    exit 1
fi

# Run load test
echo ""
echo -e "${BLUE}Running load test...${NC}"
echo -e "${YELLOW}This will ramp up to 1000+ concurrent users over ~30 minutes${NC}"
echo ""

# Create results directory
mkdir -p results
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_FILE="results/load_test_$TIMESTAMP.json"

# Run k6 with JSON output
k6 run \
    --out json=$RESULTS_FILE \
    --summary-export=results/summary_$TIMESTAMP.json \
    -e BASE_URL=$BASE_URL \
    load_test.js

# Check if test passed
if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ Load test completed successfully!${NC}"
    echo -e "Results saved to: $RESULTS_FILE"
else
    echo ""
    echo -e "${RED}✗ Load test failed${NC}"
    echo -e "Check the results in: $RESULTS_FILE"
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "End Time: $(date)"