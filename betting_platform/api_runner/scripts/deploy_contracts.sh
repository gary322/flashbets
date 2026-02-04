#!/bin/bash

# Smart Contract Deployment Script
# Deploys the betting platform smart contracts to Solana

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
API_URL="${API_URL:-http://localhost:8081}"
PROGRAM_PATH="${PROGRAM_PATH:-../../programs/betting_platform/target/deploy/betting_platform.so}"
KEYPAIR_PATH="${KEYPAIR_PATH:-~/.config/solana/id.json}"
NETWORK="${NETWORK:-devnet}"

echo -e "${GREEN}=== Betting Platform Smart Contract Deployment ===${NC}"
echo ""

# Check if API is running
echo "Checking API availability..."
if ! curl -s $API_URL/health > /dev/null; then
    echo -e "${RED}ERROR: API is not running at $API_URL${NC}"
    exit 1
fi
echo -e "${GREEN}✓ API is running${NC}"
echo ""

# Check if program file exists
if [ ! -f "$PROGRAM_PATH" ]; then
    echo -e "${YELLOW}Program file not found at $PROGRAM_PATH${NC}"
    echo "Building program..."
    
    # Build the program
    cd ../../programs/betting_platform
    anchor build
    cd -
    
    if [ ! -f "$PROGRAM_PATH" ]; then
        echo -e "${RED}ERROR: Failed to build program${NC}"
        exit 1
    fi
fi
echo -e "${GREEN}✓ Program file found${NC}"
echo ""

# Get admin JWT token (in production, this would be done securely)
echo "Authenticating as admin..."
ADMIN_WALLET="AdminWallet11111111111111111111111111111111"
ADMIN_TOKEN=$(curl -s -X POST $API_URL/api/auth/login \
    -H "Content-Type: application/json" \
    -d "{
        \"wallet\": \"$ADMIN_WALLET\",
        \"signature\": \"admin_signature\",
        \"message\": \"Sign this message to authenticate with betting platform\"
    }" | grep -o '"access_token":"[^"]*' | cut -d'"' -f4)

if [ -z "$ADMIN_TOKEN" ]; then
    echo -e "${YELLOW}Warning: Could not get admin token, proceeding without auth${NC}"
fi

# Register the program for deployment
echo "1. Registering program for deployment..."
curl -X POST $API_URL/api/deployment/register \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{
        \"program_name\": \"betting_platform\",
        \"program_path\": \"$PROGRAM_PATH\",
        \"use_upgradeable_loader\": true,
        \"skip_fee_check\": false
    }" | python3 -m json.tool || echo "Registration response"
echo ""

# Get deployer keypair (base58 encoded)
if [ -f "$KEYPAIR_PATH" ]; then
    DEPLOYER_KEYPAIR=$(cat $KEYPAIR_PATH | python3 -c "
import json
import sys
data = json.load(sys.stdin)
bytes_data = bytes(data[:64])  # Private key is first 64 bytes
import base58
print(base58.b58encode(bytes_data).decode())
" 2>/dev/null || echo "")
else
    echo -e "${YELLOW}Warning: Keypair not found, using dummy key${NC}"
    DEPLOYER_KEYPAIR="5JYkFjmEJHBF4D8cFkKMqQjVUacCQ7ei2fPmvoU2SAqfvdKBKEhZ8WvnC8jbstfn5JQBNckarCL79kKEXTRpXXt"
fi

# Deploy the program
echo "2. Deploying program to $NETWORK..."
DEPLOY_RESPONSE=$(curl -s -X POST $API_URL/api/deployment/deploy \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{
        \"program_name\": \"betting_platform\",
        \"deployer_keypair\": \"$DEPLOYER_KEYPAIR\"
    }")

echo "$DEPLOY_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$DEPLOY_RESPONSE"
echo ""

# Extract program ID from response
PROGRAM_ID=$(echo "$DEPLOY_RESPONSE" | grep -o '"program_id":"[^"]*' | cut -d'"' -f4)

if [ -z "$PROGRAM_ID" ]; then
    echo -e "${RED}ERROR: Failed to get program ID from deployment${NC}"
    echo "Checking deployment status..."
    
    # Check deployment status
    curl -s $API_URL/api/deployment/status/betting_platform | python3 -m json.tool
    exit 1
fi

echo -e "${GREEN}✓ Program deployed successfully!${NC}"
echo "Program ID: $PROGRAM_ID"
echo ""

# Verify deployment
echo "3. Verifying deployment..."
VERIFY_RESPONSE=$(curl -s $API_URL/api/deployment/verify/$PROGRAM_ID)
echo "$VERIFY_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$VERIFY_RESPONSE"
echo ""

# Initialize the program
echo "4. Initializing program..."

# Generate config account
CONFIG_ACCOUNT="ConfigAccount11111111111111111111111111111"
ORACLE_PUBKEY="OracleAccount1111111111111111111111111111111"

INIT_RESPONSE=$(curl -s -X POST $API_URL/api/deployment/initialize \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{
        \"program_id\": \"$PROGRAM_ID\",
        \"initializer_keypair\": \"$DEPLOYER_KEYPAIR\",
        \"config_account\": \"$CONFIG_ACCOUNT\",
        \"fee_rate\": 250,
        \"min_bet_amount\": 1000000,
        \"max_bet_amount\": 1000000000000,
        \"settlement_delay\": 3600,
        \"oracle_pubkey\": \"$ORACLE_PUBKEY\"
    }")

echo "$INIT_RESPONSE" | python3 -m json.tool 2>/dev/null || echo "$INIT_RESPONSE"
echo ""

# Save deployment info
echo "5. Saving deployment information..."
cat > deployment_info.json << EOF
{
    "network": "$NETWORK",
    "program_id": "$PROGRAM_ID",
    "deployed_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "config_account": "$CONFIG_ACCOUNT",
    "oracle_pubkey": "$ORACLE_PUBKEY",
    "api_url": "$API_URL"
}
EOF

echo -e "${GREEN}Deployment information saved to deployment_info.json${NC}"
echo ""

# Get program IDL
echo "6. Fetching program IDL..."
curl -s $API_URL/api/deployment/idl/betting_platform > betting_platform.idl.json
echo -e "${GREEN}✓ IDL saved to betting_platform.idl.json${NC}"
echo ""

# Final status check
echo "7. Final deployment status..."
curl -s $API_URL/api/deployment/manager/status | python3 -m json.tool
echo ""

echo -e "${GREEN}=== Deployment Complete ===${NC}"
echo ""
echo "Summary:"
echo "- Program Name: betting_platform"
echo "- Program ID: $PROGRAM_ID"
echo "- Network: $NETWORK"
echo "- Config Account: $CONFIG_ACCOUNT"
echo "- Oracle: $ORACLE_PUBKEY"
echo ""
echo "Next steps:"
echo "1. Update .env file with PROGRAM_ID=$PROGRAM_ID"
echo "2. Test the deployment with test_deployment.sh"
echo "3. Create initial markets using the API"
echo ""

# Update .env file if it exists
if [ -f "../../.env" ]; then
    echo "Updating .env file..."
    if grep -q "PROGRAM_ID=" ../../.env; then
        sed -i.bak "s/PROGRAM_ID=.*/PROGRAM_ID=$PROGRAM_ID/" ../../.env
    else
        echo "PROGRAM_ID=$PROGRAM_ID" >> ../../.env
    fi
    echo -e "${GREEN}✓ .env file updated${NC}"
fi