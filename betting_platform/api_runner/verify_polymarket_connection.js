#!/usr/bin/env node

/**
 * Verify Polymarket Connection
 * Tests the generated API credentials with real Polymarket endpoints
 */

const https = require('https');
const http = require('http');
const { URL } = require('url');
const crypto = require('crypto');

// Load credentials from environment variables (do not hardcode secrets).
// Note: POLYMARKET_API_SECRET must be base64-encoded (matches Rust implementation).
const API_KEY = process.env.POLYMARKET_API_KEY;
const API_SECRET = process.env.POLYMARKET_API_SECRET;
const API_PASSPHRASE = process.env.POLYMARKET_API_PASSPHRASE;
const WALLET_ADDRESS = process.env.POLYMARKET_ADDRESS || process.env.POLYMARKET_WALLET_ADDRESS;

const clobUrl = new URL(process.env.POLYMARKET_CLOB_BASE_URL || 'https://clob.polymarket.com');
const httpClient = clobUrl.protocol === 'http:' ? http : https;
const CLOB_HOST = clobUrl.hostname;
const CLOB_PORT = clobUrl.port ? Number(clobUrl.port) : (clobUrl.protocol === 'http:' ? 80 : 443);
const CLOB_BASE_PATH = clobUrl.pathname.replace(/\/+$/, '');

console.log('🔍 Verifying Polymarket Connection...\n');
console.log('Configuration:');
console.log(`  Wallet: ${WALLET_ADDRESS || '(not set)'}`);
console.log(`  API Key: ${API_KEY ? `${API_KEY.substring(0, 8)}...` : '(not set)'}`);
console.log(`  API URL: ${clobUrl.origin}\n`);

if (!API_KEY || !API_SECRET || !API_PASSPHRASE) {
    console.error('❌ Missing required env vars: POLYMARKET_API_KEY, POLYMARKET_API_SECRET (base64), POLYMARKET_API_PASSPHRASE');
    process.exit(1);
}

// Create HMAC signature for L2 auth
function createSignature(secret, timestamp, method, path, body = '') {
    const message = timestamp + method + path + body;
    const hmac = crypto.createHmac('sha256', Buffer.from(secret, 'base64'));
    hmac.update(message);
    return hmac.digest('base64');
}

// Test API connection
async function testConnection() {
    const timestamp = Math.floor(Date.now() / 1000).toString();
    const method = 'GET';
    const path = '/health';
    
    const signature = createSignature(API_SECRET, timestamp, method, path);
    
    const options = {
        hostname: CLOB_HOST,
        port: CLOB_PORT,
        path: `${CLOB_BASE_PATH}${path}`,
        method: method,
        headers: {
            'Accept': 'application/json',
            'Content-Type': 'application/json',
            'POLY_API_KEY': API_KEY,
            'POLY_SIGNATURE': signature,
            'POLY_TIMESTAMP': timestamp,
            'POLY_PASSPHRASE': API_PASSPHRASE,
            // Some docs use dash-separated header names; send both for compatibility.
            'POLY-API-KEY': API_KEY,
            'POLY-SIGNATURE': signature,
            'POLY-TIMESTAMP': timestamp,
            'POLY-PASSPHRASE': API_PASSPHRASE,
        }
    };
    
    return new Promise((resolve, reject) => {
        const req = httpClient.request(options, (res) => {
            let data = '';
            
            res.on('data', (chunk) => {
                data += chunk;
            });
            
            res.on('end', () => {
                console.log(`Status: ${res.statusCode}`);
                if (res.statusCode === 200) {
                    console.log('✅ Health check passed');
                    resolve(JSON.parse(data));
                } else {
                    console.log('❌ Health check failed');
                    console.log('Response:', data);
                    reject(new Error(`HTTP ${res.statusCode}: ${data}`));
                }
            });
        });
        
        req.on('error', (e) => {
            console.error('❌ Connection error:', e.message);
            reject(e);
        });
        
        req.end();
    });
}

// Test market data fetch
async function testMarkets() {
    const timestamp = Math.floor(Date.now() / 1000).toString();
    const method = 'GET';
    const path = '/markets?limit=5';
    
    const signature = createSignature(API_SECRET, timestamp, method, path);
    
    const options = {
        hostname: CLOB_HOST,
        port: CLOB_PORT,
        path: `${CLOB_BASE_PATH}${path}`,
        method: method,
        headers: {
            'Accept': 'application/json',
            'POLY_API_KEY': API_KEY,
            'POLY_SIGNATURE': signature,
            'POLY_TIMESTAMP': timestamp,
            'POLY_PASSPHRASE': API_PASSPHRASE,
            'POLY-API-KEY': API_KEY,
            'POLY-SIGNATURE': signature,
            'POLY-TIMESTAMP': timestamp,
            'POLY-PASSPHRASE': API_PASSPHRASE,
        }
    };
    
    return new Promise((resolve, reject) => {
        const req = httpClient.request(options, (res) => {
            let data = '';
            
            res.on('data', (chunk) => {
                data += chunk;
            });
            
            res.on('end', () => {
                if (res.statusCode === 200) {
                    const markets = JSON.parse(data);
                    console.log(`\n✅ Found ${markets.length} markets`);
                    
                    // Display first market
                    if (markets.length > 0) {
                        const market = markets[0];
                        console.log('\nSample Market:');
                        console.log(`  Title: ${market.question || market.title || 'N/A'}`);
                        console.log(`  ID: ${market.condition_id || market.id}`);
                        console.log(`  Volume: $${market.volume || 0}`);
                    }
                    resolve(markets);
                } else {
                    console.log('❌ Failed to fetch markets');
                    console.log('Response:', data);
                    reject(new Error(`HTTP ${res.statusCode}`));
                }
            });
        });
        
        req.on('error', reject);
        req.end();
    });
}

// Test order book fetch
async function testOrderBook() {
    // Using a popular market token ID
    const tokenId = '48331043336612883890938759509493159234755048973500640148014422747788308965671';
    const timestamp = Math.floor(Date.now() / 1000).toString();
    const method = 'GET';
    const path = `/book?token_id=${tokenId}`;
    
    const signature = createSignature(API_SECRET, timestamp, method, path);
    
    const options = {
        hostname: CLOB_HOST,
        port: CLOB_PORT,
        path: `${CLOB_BASE_PATH}${path}`,
        method: method,
        headers: {
            'Accept': 'application/json',
            'POLY_API_KEY': API_KEY,
            'POLY_SIGNATURE': signature,
            'POLY_TIMESTAMP': timestamp,
            'POLY_PASSPHRASE': API_PASSPHRASE,
            'POLY-API-KEY': API_KEY,
            'POLY-SIGNATURE': signature,
            'POLY-TIMESTAMP': timestamp,
            'POLY-PASSPHRASE': API_PASSPHRASE,
        }
    };
    
    return new Promise((resolve, reject) => {
        const req = httpClient.request(options, (res) => {
            let data = '';
            
            res.on('data', (chunk) => {
                data += chunk;
            });
            
            res.on('end', () => {
                if (res.statusCode === 200) {
                    const book = JSON.parse(data);
                    console.log('\n✅ Order book fetched');
                    console.log(`  Bids: ${book.bids?.length || 0}`);
                    console.log(`  Asks: ${book.asks?.length || 0}`);
                    
                    if (book.bids && book.bids.length > 0) {
                        console.log(`  Best Bid: $${book.bids[0].price}`);
                    }
                    if (book.asks && book.asks.length > 0) {
                        console.log(`  Best Ask: $${book.asks[0].price}`);
                    }
                    resolve(book);
                } else {
                    console.log('⚠️  Order book not available (market may be inactive)');
                    resolve(null);
                }
            });
        });
        
        req.on('error', reject);
        req.end();
    });
}

// Main execution
async function main() {
    try {
        console.log('1. Testing API Health...');
        await testConnection();
        
        console.log('\n2. Fetching Markets...');
        await testMarkets();
        
        console.log('\n3. Testing Order Book...');
        await testOrderBook();
        
        console.log('\n' + '='.repeat(60));
        console.log('✅ POLYMARKET CONNECTION VERIFIED');
        console.log('='.repeat(60));
        console.log('\nYour Polymarket integration is ready to use!');
        console.log('\nNext steps:');
        console.log('  1. Fund wallet with MATIC for gas fees');
        console.log('  2. Deposit USDC to start trading');
        console.log('  3. Run: cargo run --release');
        console.log('='.repeat(60));
        
    } catch (error) {
        console.error('\n❌ Verification failed:', error.message);
        console.log('\nTroubleshooting:');
        console.log('  1. Check your internet connection');
        console.log('  2. Verify API credentials are correct');
        console.log('  3. Try again in a few moments');
        process.exit(1);
    }
}

// Run verification
main();
