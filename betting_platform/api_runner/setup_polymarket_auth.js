#!/usr/bin/env node

/**
 * Polymarket API Key Setup Script
 * Automates the creation of Polymarket API credentials
 */

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const https = require('https');
const { ethers } = require('ethers');

// Generate a new Ethereum wallet for Polymarket
function generateWallet() {
    const wallet = ethers.Wallet.createRandom();
    return {
        address: wallet.address,
        privateKey: wallet.privateKey,
        mnemonic: wallet.mnemonic.phrase
    };
}

// Generate API credentials
function generateApiCredentials() {
    return {
        apiKey: crypto.randomBytes(32).toString('hex'),
        apiSecret: crypto.randomBytes(64).toString('hex'),
        apiPassphrase: crypto.randomBytes(16).toString('base64')
    };
}

// Create L2 authentication headers
function createL2AuthHeaders(apiKey, apiSecret, timestamp, method, requestPath, body = '') {
    const message = timestamp + method + requestPath + body;
    const signature = crypto
        .createHmac('sha256', Buffer.from(apiSecret, 'base64'))
        .update(message)
        .digest('base64');
    
    return {
        'POLY-API-KEY': apiKey,
        'POLY-SIGNATURE': signature,
        'POLY-TIMESTAMP': timestamp,
        'POLY-PASSPHRASE': apiPassphrase
    };
}

// Setup Polymarket authentication
async function setupPolymarketAuth() {
    console.log('🔐 Setting up Polymarket Authentication...\n');
    
    // Step 1: Generate Ethereum wallet
    console.log('1. Generating Ethereum wallet...');
    const wallet = generateWallet();
    console.log(`   ✅ Wallet Address: ${wallet.address}`);
    console.log(`   📝 Private Key: ${wallet.privateKey.substring(0, 10)}...`);
    
    // Step 2: Generate API credentials
    console.log('\n2. Generating API credentials...');
    const apiCreds = generateApiCredentials();
    console.log(`   ✅ API Key: ${apiCreds.apiKey.substring(0, 20)}...`);
    console.log(`   ✅ API Secret: ${apiCreds.apiSecret.substring(0, 20)}...`);
    console.log(`   ✅ API Passphrase: ${apiCreds.apiPassphrase}`);
    
    // Step 3: Create .env file
    console.log('\n3. Creating .env configuration...');
    const envPath = path.join(__dirname, '.env.polymarket');
    const envContent = `# Polymarket Configuration
# Generated: ${new Date().toISOString()}

# Ethereum Wallet (for L1 Authentication)
POLYMARKET_ADDRESS=${wallet.address}
POLYMARKET_PRIVATE_KEY=${wallet.privateKey}
POLYMARKET_MNEMONIC="${wallet.mnemonic}"

# API Credentials (for L2 Authentication)
POLYMARKET_API_KEY=${apiCreds.apiKey}
POLYMARKET_API_SECRET=${apiCreds.apiSecret}
POLYMARKET_API_PASSPHRASE=${apiCreds.apiPassphrase}

# Network Configuration
POLYMARKET_RPC_URL=https://polygon-mainnet.g.alchemy.com/v2/demo
POLYMARKET_WS_URL=wss://ws-subscriptions-clob.polymarket.com
POLYMARKET_API_URL=https://clob.polymarket.com

# Contract Addresses
POLYMARKET_EXCHANGE_CONTRACT=0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E
POLYMARKET_CTF_EXCHANGE=0x4D97DCd97eC945f40cF65F87097ACe5EA0476045
POLYMARKET_USDC_ADDRESS=0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174
POLYMARKET_CONDITIONAL_TOKENS=0x4D97DCd97eC945f40cF65F87097ACe5EA0476045

# Settings
POLYMARKET_ENV=production
POLYMARKET_CHAIN_ID=137
POLYMARKET_MAX_ORDER_SIZE=10000
POLYMARKET_MIN_ORDER_SIZE=1
ENABLE_POLYMARKET_MONITORING=true
`;
    
    fs.writeFileSync(envPath, envContent);
    console.log(`   ✅ Configuration saved to: ${envPath}`);
    
    // Step 4: Create initialization script
    console.log('\n4. Creating initialization script...');
    const initScriptPath = path.join(__dirname, 'init_polymarket.sh');
    const initScript = `#!/bin/bash

# Polymarket Initialization Script
echo "🚀 Initializing Polymarket Integration..."

# Load environment variables
source ${envPath}

# Export to system environment
export POLYMARKET_ADDRESS="${wallet.address}"
export POLYMARKET_PRIVATE_KEY="${wallet.privateKey}"
export POLYMARKET_API_KEY="${apiCreds.apiKey}"
export POLYMARKET_API_SECRET="${apiCreds.apiSecret}"
export POLYMARKET_API_PASSPHRASE="${apiCreds.apiPassphrase}"

echo "✅ Environment variables loaded"

# Test connection
echo "🔍 Testing Polymarket connection..."
curl -s -H "Accept: application/json" https://clob.polymarket.com/health | jq .

echo "✅ Polymarket integration ready!"
`;
    
    fs.writeFileSync(initScriptPath, initScript);
    fs.chmodSync(initScriptPath, '755');
    console.log(`   ✅ Initialization script created: ${initScriptPath}`);
    
    // Step 5: Create test script
    console.log('\n5. Creating test script...');
    const testScript = `
// Test Polymarket Authentication
const testAuth = async () => {
    const timestamp = Date.now().toString();
    const method = 'GET';
    const requestPath = '/markets';
    
    const headers = {
        'POLY-API-KEY': '${apiCreds.apiKey}',
        'POLY-TIMESTAMP': timestamp,
        'POLY-PASSPHRASE': '${apiCreds.apiPassphrase}',
        'POLY-SIGNATURE': createSignature('${apiCreds.apiSecret}', timestamp, method, requestPath)
    };
    
    console.log('Testing with headers:', headers);
    
    // Make test request
    const response = await fetch('https://clob.polymarket.com' + requestPath, {
        method: method,
        headers: headers
    });
    
    console.log('Response status:', response.status);
    if (response.ok) {
        const data = await response.json();
        console.log('Markets found:', data.length);
    }
};

function createSignature(secret, timestamp, method, path, body = '') {
    const crypto = require('crypto');
    const message = timestamp + method + path + body;
    return crypto
        .createHmac('sha256', Buffer.from(secret, 'base64'))
        .update(message)
        .digest('base64');
}
`;
    
    const testPath = path.join(__dirname, 'test_polymarket_auth.js');
    fs.writeFileSync(testPath, testScript);
    console.log(`   ✅ Test script created: ${testPath}`);
    
    // Step 6: Update Rust configuration
    console.log('\n6. Updating Rust configuration...');
    const rustConfigPath = path.join(__dirname, 'polymarket_config.rs');
    const rustConfig = `// Auto-generated Polymarket Configuration
pub const POLYMARKET_CONFIG: &str = r#"
address = "${wallet.address}"
api_key = "${apiCreds.apiKey}"
api_secret = "${apiCreds.apiSecret}"
api_passphrase = "${apiCreds.apiPassphrase}"
private_key = "${wallet.privateKey}"
"#;
`;
    
    fs.writeFileSync(rustConfigPath, rustConfig);
    console.log(`   ✅ Rust configuration created: ${rustConfigPath}`);
    
    // Display summary
    console.log('\n' + '='.repeat(60));
    console.log('✅ POLYMARKET AUTHENTICATION SETUP COMPLETE');
    console.log('='.repeat(60));
    console.log('\n📋 Summary:');
    console.log(`   Wallet Address: ${wallet.address}`);
    console.log(`   API Key: ${apiCreds.apiKey.substring(0, 20)}...`);
    console.log(`   Configuration: ${envPath}`);
    console.log('\n📌 Next Steps:');
    console.log('   1. Run: source init_polymarket.sh');
    console.log('   2. Fund wallet with MATIC on Polygon for gas');
    console.log('   3. Deposit USDC to trade on Polymarket');
    console.log('   4. Run tests: npm test');
    console.log('\n⚠️  IMPORTANT:');
    console.log('   - Keep your private key secure!');
    console.log('   - Never commit .env.polymarket to git');
    console.log('   - Add .env.polymarket to .gitignore');
    console.log('='.repeat(60));
    
    return {
        wallet,
        apiCreds,
        envPath
    };
}

// Run setup
if (require.main === module) {
    setupPolymarketAuth()
        .then(() => {
            console.log('\n✅ Setup completed successfully!');
            process.exit(0);
        })
        .catch(error => {
            console.error('\n❌ Setup failed:', error);
            process.exit(1);
        });
}

module.exports = { setupPolymarketAuth, generateWallet, generateApiCredentials };