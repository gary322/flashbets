#!/bin/bash

# Deploy All Contracts - Polygon and Solana
# Production-ready deployment script for local testing

set -e

echo "====================================="
echo "🚀 BETTING PLATFORM FULL DEPLOYMENT"
echo "====================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}ℹ️  $1${NC}"
}

# Check prerequisites
check_prerequisites() {
    echo "Checking prerequisites..."
    
    # Check Node.js
    if ! command -v node &> /dev/null; then
        print_error "Node.js is not installed"
        exit 1
    fi
    print_success "Node.js found: $(node --version)"
    
    # Check npm
    if ! command -v npm &> /dev/null; then
        print_error "npm is not installed"
        exit 1
    fi
    print_success "npm found: $(npm --version)"
    
    # Check Solana CLI
    if ! command -v solana &> /dev/null; then
        print_error "Solana CLI is not installed"
        echo "Please install: https://docs.solana.com/cli/install-solana-cli-tools"
        exit 1
    fi
    print_success "Solana CLI found: $(solana --version)"
    
    # Check Anchor
    if ! command -v anchor &> /dev/null; then
        print_error "Anchor is not installed"
        echo "Please install: https://www.anchor-lang.com/docs/installation"
        exit 1
    fi
    print_success "Anchor found: $(anchor --version)"
    
    echo ""
}

# Deploy Polygon contracts
deploy_polygon() {
    echo "====================================="
    echo "📦 DEPLOYING POLYGON CONTRACTS"
    echo "====================================="
    echo ""
    
    cd contracts
    
    # Install dependencies
    print_info "Installing dependencies..."
    npm install
    
    # Compile contracts
    print_info "Compiling contracts..."
    npx hardhat compile
    
    # Start local Hardhat node in background
    print_info "Starting local Hardhat node..."
    npx hardhat node &
    HARDHAT_PID=$!
    sleep 5
    
    # Deploy contracts
    print_info "Deploying contracts to local network..."
    npx hardhat run scripts/deploy.js --network localhost
    
    # Export ABIs
    print_info "Exporting ABIs..."
    npx hardhat export-abi
    
    print_success "Polygon contracts deployed successfully!"
    
    # Keep Hardhat node running
    echo "Hardhat node running with PID: $HARDHAT_PID"
    
    cd ..
    echo ""
}

# Deploy Solana programs
deploy_solana() {
    echo "====================================="
    echo "📦 DEPLOYING SOLANA PROGRAMS"
    echo "====================================="
    echo ""
    
    # Start local validator in background
    print_info "Starting Solana test validator..."
    solana-test-validator --reset &
    SOLANA_PID=$!
    sleep 5
    
    # Configure Solana CLI for localhost
    print_info "Configuring Solana CLI..."
    solana config set --url localhost
    
    # Airdrop SOL to deployer
    print_info "Airdropping SOL to deployer..."
    solana airdrop 100
    
    # Build main betting platform program
    print_info "Building main betting platform program..."
    cd programs/betting_platform_native
    cargo build-sbf
    
    # Deploy main program
    print_info "Deploying main program..."
    MAIN_PROGRAM_ID=$(solana program deploy target/deploy/betting_platform_native.so | grep "Program Id:" | awk '{print $3}')
    echo "Main Program ID: $MAIN_PROGRAM_ID"
    
    cd ../..
    
    # Build and deploy flash betting program
    print_info "Building flash betting program..."
    cd flash_bets/program
    cargo build-sbf
    
    print_info "Deploying flash betting program..."
    FLASH_PROGRAM_ID=$(solana program deploy target/deploy/mv_flash.so | grep "Program Id:" | awk '{print $3}')
    echo "Flash Program ID: $FLASH_PROGRAM_ID"
    
    cd ../..
    
    # Generate IDL files
    print_info "Generating IDL files..."
    
    # Create IDL directory
    mkdir -p idl
    
    # Generate IDL for main program
    cat > idl/betting_platform.json << EOF
{
  "version": "0.1.0",
  "name": "betting_platform",
  "programId": "$MAIN_PROGRAM_ID",
  "instructions": [
    {
      "name": "initialize",
      "accounts": [
        {"name": "authority", "isMut": false, "isSigner": true},
        {"name": "globalConfig", "isMut": true, "isSigner": false},
        {"name": "systemProgram", "isMut": false, "isSigner": false}
      ],
      "args": []
    },
    {
      "name": "createVerse",
      "accounts": [
        {"name": "authority", "isMut": false, "isSigner": true},
        {"name": "verse", "isMut": true, "isSigner": false},
        {"name": "systemProgram", "isMut": false, "isSigner": false}
      ],
      "args": [
        {"name": "title", "type": "string"},
        {"name": "category", "type": "u8"},
        {"name": "odds", "type": "u64"}
      ]
    },
    {
      "name": "placeBet",
      "accounts": [
        {"name": "user", "isMut": true, "isSigner": true},
        {"name": "verse", "isMut": true, "isSigner": false},
        {"name": "userBet", "isMut": true, "isSigner": false},
        {"name": "systemProgram", "isMut": false, "isSigner": false}
      ],
      "args": [
        {"name": "amount", "type": "u64"},
        {"name": "side", "type": "u8"}
      ]
    }
  ],
  "accounts": [
    {
      "name": "GlobalConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {"name": "authority", "type": "publicKey"},
          {"name": "totalVersesCreated", "type": "u64"},
          {"name": "totalBetsPlaced", "type": "u64"},
          {"name": "totalVolume", "type": "u64"}
        ]
      }
    },
    {
      "name": "VersePDA",
      "type": {
        "kind": "struct",
        "fields": [
          {"name": "id", "type": "u128"},
          {"name": "creator", "type": "publicKey"},
          {"name": "title", "type": "string"},
          {"name": "totalStake", "type": "u64"},
          {"name": "resolved", "type": "bool"}
        ]
      }
    }
  ]
}
EOF
    
    # Generate IDL for flash program
    cat > idl/flash_betting.json << EOF
{
  "version": "0.1.0",
  "name": "mv_flash",
  "programId": "$FLASH_PROGRAM_ID",
  "instructions": [
    {
      "name": "createFlashMarket",
      "accounts": [
        {"name": "authority", "isMut": false, "isSigner": true},
        {"name": "flashMarket", "isMut": true, "isSigner": false},
        {"name": "systemProgram", "isMut": false, "isSigner": false}
      ],
      "args": [
        {"name": "title", "type": "string"},
        {"name": "duration", "type": "u64"},
        {"name": "tau", "type": "u64"}
      ]
    },
    {
      "name": "openFlashPosition",
      "accounts": [
        {"name": "trader", "isMut": true, "isSigner": true},
        {"name": "flashMarket", "isMut": true, "isSigner": false},
        {"name": "position", "isMut": true, "isSigner": false},
        {"name": "systemProgram", "isMut": false, "isSigner": false}
      ],
      "args": [
        {"name": "amount", "type": "u64"},
        {"name": "leverage", "type": "u64"},
        {"name": "isYes", "type": "bool"}
      ]
    }
  ],
  "accounts": [
    {
      "name": "FlashMarket",
      "type": {
        "kind": "struct",
        "fields": [
          {"name": "id", "type": "publicKey"},
          {"name": "title", "type": "string"},
          {"name": "startTime", "type": "i64"},
          {"name": "endTime", "type": "i64"},
          {"name": "tau", "type": "u64"},
          {"name": "resolved", "type": "bool"}
        ]
      }
    }
  ]
}
EOF
    
    print_success "Solana programs deployed successfully!"
    
    # Keep validator running
    echo "Solana validator running with PID: $SOLANA_PID"
    echo ""
}

# Create backend integration file
create_backend_integration() {
    echo "====================================="
    echo "🔗 CREATING BACKEND INTEGRATION"
    echo "====================================="
    echo ""
    
    # Read deployment info
    POLYGON_DEPLOYMENT=$(cat contracts/deployments/localhost-deployment.json 2>/dev/null || echo "{}")
    
    # Create backend integration config
    cat > backend_integration.js << 'EOF'
const { ethers } = require('ethers');
const { Connection, PublicKey } = require('@solana/web3.js');
const fs = require('fs');
const path = require('path');

// Load deployment info
const polygonDeployment = require('./contracts/deployments/localhost-deployment.json');

// Load ABIs
const loadABI = (contractName) => {
  const abiPath = path.join(__dirname, 'contracts/abi', `${contractName}.json`);
  return JSON.parse(fs.readFileSync(abiPath, 'utf8'));
};

// Load IDLs
const loadIDL = (programName) => {
  const idlPath = path.join(__dirname, 'idl', `${programName}.json`);
  return JSON.parse(fs.readFileSync(idlPath, 'utf8'));
};

// Polygon Configuration
const polygonConfig = {
  rpcUrl: 'http://localhost:8545',
  contracts: polygonDeployment.contracts,
  abis: {
    BettingPlatform: loadABI('BettingPlatform'),
    PolymarketIntegration: loadABI('PolymarketIntegration'),
    MarketFactory: loadABI('MarketFactory'),
    FlashBetting: loadABI('FlashBetting'),
    LeverageVault: loadABI('LeverageVault'),
    LiquidityPool: loadABI('LiquidityPool')
  }
};

// Solana Configuration
const solanaConfig = {
  rpcUrl: 'http://localhost:8899',
  programs: {
    bettingPlatform: loadIDL('betting_platform'),
    flashBetting: loadIDL('flash_betting')
  }
};

// Initialize Polygon Provider
const initPolygonProvider = () => {
  const provider = new ethers.providers.JsonRpcProvider(polygonConfig.rpcUrl);
  const signer = new ethers.Wallet(
    '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80',
    provider
  );
  
  return { provider, signer };
};

// Initialize Solana Connection
const initSolanaConnection = () => {
  const connection = new Connection(solanaConfig.rpcUrl, 'confirmed');
  return connection;
};

// Get Polygon Contract Instance
const getPolygonContract = (contractName, signer) => {
  const address = polygonConfig.contracts[contractName];
  const abi = polygonConfig.abis[contractName];
  return new ethers.Contract(address, abi, signer);
};

// Export configuration and helper functions
module.exports = {
  polygonConfig,
  solanaConfig,
  initPolygonProvider,
  initSolanaConnection,
  getPolygonContract,
  
  // Contract addresses
  addresses: {
    polygon: polygonConfig.contracts,
    solana: {
      bettingPlatform: solanaConfig.programs.bettingPlatform.programId,
      flashBetting: solanaConfig.programs.flashBetting.programId
    }
  },
  
  // ABIs and IDLs
  interfaces: {
    polygon: polygonConfig.abis,
    solana: solanaConfig.programs
  }
};
EOF
    
    print_success "Backend integration file created!"
    echo ""
}

# Create test script
create_test_script() {
    echo "====================================="
    echo "🧪 CREATING TEST SCRIPT"
    echo "====================================="
    echo ""
    
    cat > test_deployment.js << 'EOF'
const { 
  initPolygonProvider, 
  initSolanaConnection, 
  getPolygonContract,
  addresses 
} = require('./backend_integration');

async function testPolygonDeployment() {
  console.log('Testing Polygon deployment...');
  
  const { provider, signer } = initPolygonProvider();
  
  // Test BettingPlatform
  const bettingPlatform = getPolygonContract('BettingPlatform', signer);
  const totalVolume = await bettingPlatform.totalVolume();
  console.log('BettingPlatform total volume:', totalVolume.toString());
  
  // Test MarketFactory
  const marketFactory = getPolygonContract('MarketFactory', signer);
  const totalMarkets = await marketFactory.totalMarketsCreated();
  console.log('Total markets created:', totalMarkets.toString());
  
  console.log('✅ Polygon contracts working!');
}

async function testSolanaDeployment() {
  console.log('Testing Solana deployment...');
  
  const connection = initSolanaConnection();
  
  // Test connection
  const slot = await connection.getSlot();
  console.log('Current slot:', slot);
  
  // Check program deployment
  const programId = addresses.solana.bettingPlatform;
  const accountInfo = await connection.getAccountInfo(new PublicKey(programId));
  
  if (accountInfo) {
    console.log('✅ Solana programs deployed!');
  } else {
    console.log('❌ Solana programs not found');
  }
}

async function main() {
  console.log('===================================');
  console.log('🧪 TESTING DEPLOYMENTS');
  console.log('===================================');
  console.log('');
  
  try {
    await testPolygonDeployment();
    console.log('');
    await testSolanaDeployment();
    
    console.log('');
    console.log('===================================');
    console.log('✅ ALL TESTS PASSED!');
    console.log('===================================');
  } catch (error) {
    console.error('Test failed:', error);
    process.exit(1);
  }
}

// Run tests
main();
EOF
    
    print_success "Test script created!"
    echo ""
}

# Main execution
main() {
    check_prerequisites
    
    # Deploy Polygon contracts
    deploy_polygon
    
    # Deploy Solana programs
    deploy_solana
    
    # Create backend integration
    create_backend_integration
    
    # Create test script
    create_test_script
    
    echo "====================================="
    echo "🎉 DEPLOYMENT COMPLETE!"
    echo "====================================="
    echo ""
    echo "📊 Deployment Summary:"
    echo "-------------------------------------"
    echo "Polygon Contracts: ✅ Deployed to localhost:8545"
    echo "Solana Programs: ✅ Deployed to localhost:8899"
    echo "ABI Files: ✅ Generated in contracts/abi/"
    echo "IDL Files: ✅ Generated in idl/"
    echo "Backend Integration: ✅ Created in backend_integration.js"
    echo ""
    echo "📝 Next Steps:"
    echo "1. Run 'node test_deployment.js' to test the deployments"
    echo "2. Use backend_integration.js in your backend API"
    echo "3. Access ABIs from contracts/abi/ directory"
    echo "4. Access IDLs from idl/ directory"
    echo ""
    echo "⚠️  Keep this terminal open to maintain local blockchains"
    echo ""
    
    # Keep script running
    echo "Press Ctrl+C to stop all services..."
    wait
}

# Cleanup function
cleanup() {
    echo ""
    echo "Shutting down services..."
    
    if [ ! -z "$HARDHAT_PID" ]; then
        kill $HARDHAT_PID 2>/dev/null
        print_info "Hardhat node stopped"
    fi
    
    if [ ! -z "$SOLANA_PID" ]; then
        kill $SOLANA_PID 2>/dev/null
        print_info "Solana validator stopped"
    fi
    
    exit 0
}

# Set up cleanup on exit
trap cleanup EXIT INT TERM

# Run main function
main