const { ethers } = require('ethers');
const axios = require('axios');

// Wallet credentials (DO NOT hardcode secrets in this repo).
// Required:
//   - POLYMARKET_PRIVATE_KEY (0x... 64 hex)
// Optional:
//   - POLYMARKET_ADDRESS (asserts private key matches)
const WALLET_ADDRESS = process.env.POLYMARKET_ADDRESS || process.env.POLYMARKET_WALLET_ADDRESS;
const PRIVATE_KEY = process.env.POLYMARKET_PRIVATE_KEY;

// Polymarket CLOB endpoint
const CLOB_ENDPOINT = process.env.POLYMARKET_CLOB_BASE_URL || 'https://clob.polymarket.com';
const OUT_FILE = process.env.POLYMARKET_CREDENTIALS_OUT || 'polymarket_credentials.json';

function requireEnv(name) {
    const value = process.env[name];
    if (!value) {
        console.error(`\n❌ Missing required env var: ${name}\n`);
        process.exit(1);
    }
    return value;
}

async function generatePolymarketAPIKey() {
    try {
        requireEnv('POLYMARKET_PRIVATE_KEY');

        // Create wallet instance
        const wallet = new ethers.Wallet(PRIVATE_KEY);
        
        // Verify wallet address matches
        if (WALLET_ADDRESS && wallet.address.toLowerCase() !== WALLET_ADDRESS.toLowerCase()) {
            throw new Error('Private key does not match wallet address');
        }
        
        console.log('Using wallet:', wallet.address);
        
        // Create timestamp and nonce
        const timestamp = Math.floor(Date.now() / 1000);
        const nonce = 0;
        
        // EIP-712 Domain
        const domain = {
            name: 'ClobAuthDomain',
            version: '1',
            chainId: 137, // Polygon mainnet
        };
        
        // EIP-712 Types
        const types = {
            ClobAuth: [
                { name: 'address', type: 'address' },
                { name: 'timestamp', type: 'uint256' },
                { name: 'nonce', type: 'uint256' },
                { name: 'message', type: 'string' },
            ],
        };
        
        // Message to sign
        const message = {
            address: wallet.address,
            timestamp: timestamp.toString(),
            nonce: nonce.toString(),
            message: 'This message attests that I control the given wallet',
        };
        
        // Sign the message (ethers v6 syntax)
        const signature = await wallet.signTypedData(domain, types, message);
        
        console.log('\nGenerated signature:', signature);
        console.log('Timestamp:', timestamp);
        
        // Prepare headers
        const headers = {
            'POLY_ADDRESS': wallet.address,
            'POLY_SIGNATURE': signature,
            'POLY_TIMESTAMP': timestamp.toString(),
            'POLY_NONCE': nonce.toString(),
            'Content-Type': 'application/json',
        };
        
        console.log('\nSending request to Polymarket...');
        
        // Make API request
        const response = await axios.post(
            `${CLOB_ENDPOINT}/auth/api-key`,
            {},
            { headers }
        );
        
        console.log('\n✅ Success! Your Polymarket API credentials:');
        console.log('API Key:', response.data.apiKey || response.data.key);
        console.log('Secret:', response.data.secret);
        console.log('Passphrase:', response.data.passphrase);
        
        // Save to file
        const credentials = {
            apiKey: response.data.apiKey || response.data.key,
            secret: response.data.secret,
            passphrase: response.data.passphrase,
            created: new Date().toISOString(),
            wallet: wallet.address,
        };
        
        require('fs').writeFileSync(OUT_FILE, JSON.stringify(credentials, null, 2));
        
        console.log(`\nCredentials saved to ${OUT_FILE}`);
        console.log('⚠️  Never commit generated credential files. They are ignored by .gitignore.');
        
        return credentials;
        
    } catch (error) {
        console.error('\n❌ Error generating API key:');
        
        if (error.response) {
            console.error('Status:', error.response.status);
            console.error('Data:', error.response.data);
        } else {
            console.error(error.message);
        }
        
        throw error;
    }
}

// Run the function
generatePolymarketAPIKey()
    .then(() => process.exit(0))
    .catch(() => process.exit(1));
