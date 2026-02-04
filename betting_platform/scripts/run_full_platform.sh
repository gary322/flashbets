#!/bin/bash

# Quantum Betting Platform - Full Stack Runner
# This script starts all components and runs automated tests

set -e

echo "🚀 Starting Quantum Betting Platform Full Stack..."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check prerequisites
check_prerequisites() {
    echo -e "${BLUE}Checking prerequisites...${NC}"
    
    # Check Node.js
    if ! command -v node &> /dev/null; then
        echo -e "${RED}Node.js is not installed${NC}"
        exit 1
    fi
    
    # Check Rust
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Rust is not installed${NC}"
        exit 1
    fi
    
    # Check Solana CLI
    if ! command -v solana &> /dev/null; then
        echo -e "${RED}Solana CLI is not installed${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ All prerequisites met${NC}"
}

# Start local Solana validator
start_validator() {
    echo -e "${BLUE}Starting Solana validator...${NC}"
    
    # Check if validator is already running
    if pgrep -x "solana-test-validator" > /dev/null; then
        echo -e "${YELLOW}Validator already running${NC}"
    else
        solana-test-validator \
            --rpc-port 8899 \
            --ws-port 8900 \
            --bind-address 0.0.0.0 \
            --ledger ./test-ledger \
            --quiet &
        
        VALIDATOR_PID=$!
        echo "Validator PID: $VALIDATOR_PID"
        
        # Wait for validator to start
        sleep 5
        
        # Airdrop SOL to test accounts
        echo -e "${BLUE}Airdropping SOL to test accounts...${NC}"
        solana airdrop 100 7g9N4kFHqr7nXpYBrKcVgD48AU --url localhost
    fi
}

# Deploy smart contracts
deploy_contracts() {
    echo -e "${BLUE}Deploying smart contracts...${NC}"
    
    cd programs/betting_platform_native
    
    # Build program
    cargo build-sbf
    
    # Deploy
    PROGRAM_ID=$(solana program deploy \
        target/deploy/betting_platform_native.so \
        --url localhost \
        --keypair ../../keypairs/deployer.json \
        | grep "Program Id:" | awk '{print $3}')
    
    echo -e "${GREEN}✓ Program deployed: $PROGRAM_ID${NC}"
    
    # Export for API
    export PROGRAM_ID=$PROGRAM_ID
    
    cd ../..
}

# Start API server
start_api_server() {
    echo -e "${BLUE}Starting API server...${NC}"
    
    cd api_runner
    
    # Build API
    cargo build --release
    
    # Start API server
    PROGRAM_ID=$PROGRAM_ID \
    RPC_URL=http://localhost:8899 \
    cargo run --release &
    
    API_PID=$!
    echo "API Server PID: $API_PID"
    
    # Wait for API to start
    sleep 10
    
    # Health check
    if curl -f http://localhost:8080/health > /dev/null 2>&1; then
        echo -e "${GREEN}✓ API server is healthy${NC}"
    else
        echo -e "${RED}API server health check failed${NC}"
        exit 1
    fi
    
    cd ..
}

# Start UI server
start_ui_server() {
    echo -e "${BLUE}Starting UI server...${NC}"
    
    cd programs/betting_platform_native/ui_demo
    
    # Use the real app.js
    cp app_real.js app.js
    cp index_real.html index.html
    
    # Start UI server
    node server.js &
    
    UI_PID=$!
    echo "UI Server PID: $UI_PID"
    
    sleep 3
    
    echo -e "${GREEN}✓ UI server running at http://localhost:8080${NC}"
    
    cd ../../..
}

# Run automated tests
run_tests() {
    echo -e "${BLUE}Running automated tests...${NC}"
    
    cd tests/playwright
    
    # Install dependencies
    npm install
    
    # Install Playwright browsers
    npx playwright install
    
    # Run tests
    echo -e "${YELLOW}Running user journey tests...${NC}"
    npm run test:user-journeys
    
    echo -e "${YELLOW}Running exhaustive tests...${NC}"
    npm run test:exhaustive
    
    cd ../..
}

# Monitor system
monitor_system() {
    echo -e "${BLUE}Starting system monitoring...${NC}"
    
    # Create monitoring dashboard
    cat << EOF > monitor.sh
#!/bin/bash
while true; do
    clear
    echo "🎮 Quantum Betting Platform Monitor"
    echo "=================================="
    echo ""
    echo "📊 Services Status:"
    ps aux | grep -E "solana-test-validator|cargo run|node server.js" | grep -v grep
    echo ""
    echo "🌐 Endpoints:"
    echo "- UI: http://localhost:8080"
    echo "- API: http://localhost:8080/api"
    echo "- WebSocket: ws://localhost:8080/ws"
    echo ""
    echo "📈 API Metrics:"
    curl -s http://localhost:8080/health | jq '.'
    echo ""
    echo "Press Ctrl+C to stop monitoring"
    sleep 5
done
EOF
    
    chmod +x monitor.sh
}

# Cleanup function
cleanup() {
    echo -e "${YELLOW}Cleaning up...${NC}"
    
    # Kill processes
    [ ! -z "$VALIDATOR_PID" ] && kill $VALIDATOR_PID 2>/dev/null
    [ ! -z "$API_PID" ] && kill $API_PID 2>/dev/null
    [ ! -z "$UI_PID" ] && kill $UI_PID 2>/dev/null
    
    # Clean test ledger
    rm -rf ./test-ledger
    
    echo -e "${GREEN}✓ Cleanup complete${NC}"
}

# Trap cleanup on exit
trap cleanup EXIT

# Main execution
main() {
    echo -e "${GREEN}"
    echo "╔═══════════════════════════════════════════╗"
    echo "║     Quantum Betting Platform Runner       ║"
    echo "╚═══════════════════════════════════════════╝"
    echo -e "${NC}"
    
    check_prerequisites
    start_validator
    deploy_contracts
    start_api_server
    start_ui_server
    
    echo -e "${GREEN}"
    echo "╔═══════════════════════════════════════════╗"
    echo "║        Platform Ready! 🚀                 ║"
    echo "║                                           ║"
    echo "║  UI: http://localhost:8080                ║"
    echo "║  API: http://localhost:8080/api           ║"
    echo "║  WS: ws://localhost:8080/ws               ║"
    echo "║                                           ║"
    echo "║  Press 'T' to run tests                   ║"
    echo "║  Press 'M' for monitoring                 ║"
    echo "║  Press 'Q' to quit                        ║"
    echo "╚═══════════════════════════════════════════╝"
    echo -e "${NC}"
    
    # Interactive menu
    while true; do
        read -n 1 -s key
        case $key in
            t|T)
                run_tests
                ;;
            m|M)
                monitor_system
                ./monitor.sh
                ;;
            q|Q)
                echo -e "${YELLOW}Shutting down...${NC}"
                break
                ;;
        esac
    done
}

# Run main function
main