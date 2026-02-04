#!/bin/bash

echo "=== ACTUAL Solana Program Deployment ==="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Step 1: Check validator
echo -e "${YELLOW}Step 1: Checking local validator...${NC}"
if ! solana --url localhost cluster-version > /dev/null 2>&1; then
    echo -e "${RED}✗ Local validator not running${NC}"
    echo "Please start: solana-test-validator --reset"
    exit 1
fi
echo -e "${GREEN}✓ Validator is running${NC}"

# Step 2: Try to build for BPF/SBF
echo ""
echo -e "${YELLOW}Step 2: Building program for Solana...${NC}"

# First, let's check what Solana SDK we have
echo "Checking Solana SDK version..."
solana --version

# Try different build methods
if command -v cargo-build-sbf &> /dev/null; then
    echo "Using cargo-build-sbf..."
    cargo build-sbf
elif command -v cargo-build-bpf &> /dev/null; then
    echo "Using cargo-build-bpf..."
    cargo build-bpf
else
    echo -e "${YELLOW}BPF/SBF builder not found. Using alternative method...${NC}"
    
    # Create a simple deployable program
    mkdir -p target/deploy
    
    # For demonstration, we'll use the release build
    if [ -f target/release/libbetting_platform_native.so ]; then
        cp target/release/libbetting_platform_native.so target/deploy/betting_platform_native.so
        echo -e "${GREEN}✓ Using release build as deployment artifact${NC}"
    else
        # Create a minimal valid BPF program for testing
        echo -e "${YELLOW}Creating test program...${NC}"
        
        # This is just for demonstration - in production you need proper BPF compilation
        cat > target/deploy/betting_platform_native.so << 'EOF'
BPF_PROGRAM_PLACEHOLDER
EOF
        echo -e "${GREEN}✓ Test program created${NC}"
    fi
fi

# Step 3: Generate program keypair
echo ""
echo -e "${YELLOW}Step 3: Generating program keypair...${NC}"
PROGRAM_KEYPAIR="keypairs/program-keypair.json"
mkdir -p keypairs

if [ ! -f "$PROGRAM_KEYPAIR" ]; then
    solana-keygen new --outfile $PROGRAM_KEYPAIR --no-bip39-passphrase --force
fi

PROGRAM_ID=$(solana-keygen pubkey $PROGRAM_KEYPAIR)
echo -e "${BLUE}Program ID: $PROGRAM_ID${NC}"

# Step 4: Fund deployer
echo ""
echo -e "${YELLOW}Step 4: Funding deployer account...${NC}"
DEPLOYER_KEYPAIR="keypairs/deployer-keypair.json"

if [ ! -f "$DEPLOYER_KEYPAIR" ]; then
    solana-keygen new --outfile $DEPLOYER_KEYPAIR --no-bip39-passphrase --force
fi

DEPLOYER=$(solana-keygen pubkey $DEPLOYER_KEYPAIR)
echo "Deployer: $DEPLOYER"

solana airdrop 10 $DEPLOYER --url localhost
sleep 2
solana balance $DEPLOYER --url localhost

# Step 5: Actual deployment attempt
echo ""
echo -e "${YELLOW}Step 5: Deploying program...${NC}"

if [ -f "target/deploy/betting_platform_native.so" ]; then
    echo "Deploying betting_platform_native.so..."
    
    # Configure for localhost
    solana config set --url localhost
    solana config set --keypair $DEPLOYER_KEYPAIR
    
    # Deploy with error handling
    if solana program deploy \
        --program-id $PROGRAM_KEYPAIR \
        target/deploy/betting_platform_native.so \
        --url localhost \
        --keypair $DEPLOYER_KEYPAIR; then
        
        echo -e "${GREEN}✓ Program deployed successfully!${NC}"
        
        # Verify deployment
        echo ""
        echo -e "${YELLOW}Verifying deployment...${NC}"
        solana program show $PROGRAM_ID --url localhost
        
    else
        echo -e "${RED}✗ Deployment failed${NC}"
        echo ""
        echo "Common issues:"
        echo "1. Program too large - try: cargo build-sbf --features optimize"
        echo "2. Insufficient funds - try: solana airdrop 10"
        echo "3. Invalid program - ensure proper BPF compilation"
    fi
else
    echo -e "${RED}✗ No deployable program found${NC}"
    echo ""
    echo "To build for Solana BPF:"
    echo "1. Install Solana tools: sh -c \"\$(curl -sSfL https://release.solana.com/stable/install)\""
    echo "2. Install BPF tools: cargo install cargo-build-sbf"
    echo "3. Build: cargo build-sbf"
fi

# Step 6: Summary
echo ""
echo -e "${BLUE}=== Deployment Summary ===${NC}"
echo "Program ID: $PROGRAM_ID"
echo "Deployer: $DEPLOYER"
echo "Network: localhost"
echo ""

if [ -f "target/deploy/betting_platform_native.so" ]; then
    echo "Program size: $(ls -lh target/deploy/betting_platform_native.so | awk '{print $5}')"
fi

echo ""
echo "Next steps:"
echo "1. Initialize program: solana program invoke $PROGRAM_ID --data <INIT_DATA>"
echo "2. Monitor logs: solana logs $PROGRAM_ID --url localhost"
echo "3. Check account: solana account $PROGRAM_ID --url localhost"