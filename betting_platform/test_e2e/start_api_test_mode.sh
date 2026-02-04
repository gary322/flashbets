#!/bin/bash

# Start API Server in Test Mode (without PostgreSQL)
# Uses in-memory storage for testing

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
API_DIR="$PROJECT_ROOT/api_runner"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Starting API Server in Test Mode${NC}"
echo -e "${BLUE}========================================${NC}"

cd "$API_DIR"

# Create test environment file
cat > "$API_DIR/.env.test" << EOF
# Test Mode Configuration - No PostgreSQL Required
SERVER_HOST=127.0.0.1
SERVER_PORT=8081
CORS_ORIGINS=http://localhost:3000,http://localhost:3001

# Disable database for testing
DATABASE_ENABLED=false
USE_IN_MEMORY_STORAGE=true

# Redis (optional)
REDIS_URL=redis://localhost:6379
CACHE_ENABLED=false
QUEUE_ENABLED=false

# Solana
SOLANA_RPC_URL=http://localhost:8899
SOLANA_WS_URL=ws://localhost:8900
PROGRAM_ID=HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca

# Security (TEST ONLY)
JWT_SECRET=test-secret-key-minimum-32-characters-long-for-testing
SECURITY_LOG_TO_FILE=false

# External Services
POLYMARKET_ENABLED=false
KALSHI_ENABLED=false

# Seeded Markets
USE_SEEDED_MARKETS=true
SEEDED_MARKETS_COUNT=10

# Enable Demo Mode
ENABLE_DEMO_MODE=true
ENABLE_AUTO_FUNDING=true

# Logging
LOG_LEVEL=info
EOF

echo -e "${GREEN}Created test environment configuration${NC}"

# Build the API server with test features
echo -e "\n${BLUE}Building API server with test features...${NC}"
cargo build --release --features "test-mode" 2>&1 | grep -E "(error|warning|Finished)" | tail -20 || true

# Start the API server
echo -e "\n${BLUE}Starting API server on port 8081...${NC}"
echo -e "${YELLOW}Press Ctrl+C to stop${NC}"
echo ""

# Export the test environment
export ENV_FILE=.env.test
export RUST_LOG=info

# Run the server
cargo run --release --features "test-mode"