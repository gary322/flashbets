#!/usr/bin/env node

/**
 * Test Real Polymarket Connection
 * Uses the actual Polymarket public API endpoints
 */

const https = require('https');

console.log('🔍 Testing Polymarket Public API...\n');

// Test Polymarket Gamma API (public markets data)
async function testGammaAPI() {
    const options = {
        hostname: 'gamma-api.polymarket.com',
        port: 443,
        path: '/markets?limit=5&active=true',
        method: 'GET',
        headers: {
            'Accept': 'application/json',
            'User-Agent': 'BettingPlatform/1.0'
        }
    };
    
    return new Promise((resolve, reject) => {
        const req = https.request(options, (res) => {
            let data = '';
            
            res.on('data', (chunk) => {
                data += chunk;
            });
            
            res.on('end', () => {
                console.log(`Gamma API Status: ${res.statusCode}`);
                if (res.statusCode === 200) {
                    try {
                        const markets = JSON.parse(data);
                        console.log(`✅ Found ${markets.length} active markets\n`);
                        
                        // Display first few markets
                        markets.slice(0, 3).forEach((market, i) => {
                            console.log(`Market ${i + 1}:`);
                            console.log(`  Title: ${market.title || market.question}`);
                            console.log(`  ID: ${market.id || market.condition_id}`);
                            console.log(`  Volume: $${market.volume || 0}`);
                            console.log(`  Liquidity: $${market.liquidity || 0}`);
                            console.log('');
                        });
                        resolve(markets);
                    } catch (e) {
                        console.log('Failed to parse response:', e.message);
                        reject(e);
                    }
                } else {
                    console.log('Response:', data);
                    reject(new Error(`HTTP ${res.statusCode}`));
                }
            });
        });
        
        req.on('error', (e) => {
            console.error('Connection error:', e.message);
            reject(e);
        });
        
        req.end();
    });
}

// Test Polymarket CLOB API (requires authentication)
async function testCLOBAPI() {
    // Generate a mock timestamp and nonce for testing
    const timestamp = Math.floor(Date.now() / 1000).toString();
    const nonce = '0';
    const address = '0x6540C23aa27D41322d170fe7ee4BD86893FfaC01';
    
    console.log('\nTesting CLOB API endpoints...\n');
    
    // Try different possible endpoints
    const endpoints = [
        { host: 'clob.polymarket.com', path: '/markets' },
        { host: 'api.polymarket.com', path: '/markets' },
        { host: 'clob-api.polymarket.com', path: '/markets' }
    ];
    
    for (const endpoint of endpoints) {
        console.log(`Testing ${endpoint.host}${endpoint.path}...`);
        
        const options = {
            hostname: endpoint.host,
            port: 443,
            path: endpoint.path,
            method: 'GET',
            headers: {
                'Accept': 'application/json',
                'POLY_ADDRESS': address,
                'POLY_TIMESTAMP': timestamp,
                'POLY_NONCE': nonce,
                'POLY_SIGNATURE': 'test_signature' // Would need real signature
            }
        };
        
        try {
            const result = await new Promise((resolve, reject) => {
                const req = https.request(options, (res) => {
                    console.log(`  Status: ${res.statusCode}`);
                    resolve(res.statusCode);
                });
                req.on('error', reject);
                req.setTimeout(5000, () => {
                    req.destroy();
                    reject(new Error('Timeout'));
                });
                req.end();
            });
            
            if (result === 200 || result === 401 || result === 403) {
                console.log(`  ✅ Endpoint exists (auth required)\n`);
            } else {
                console.log(`  ❌ Endpoint not available\n`);
            }
        } catch (e) {
            console.log(`  ❌ ${e.message}\n`);
        }
    }
}

// Test Strapi API (content/data)
async function testStrapiAPI() {
    console.log('Testing Strapi API...\n');
    
    const options = {
        hostname: 'strapi-matic.polymarket.com',
        port: 443,
        path: '/markets?_limit=3&active=true&closed=false',
        method: 'GET',
        headers: {
            'Accept': 'application/json'
        }
    };
    
    return new Promise((resolve, reject) => {
        const req = https.request(options, (res) => {
            let data = '';
            
            res.on('data', (chunk) => {
                data += chunk;
            });
            
            res.on('end', () => {
                console.log(`Strapi API Status: ${res.statusCode}`);
                if (res.statusCode === 200) {
                    console.log('✅ Strapi API accessible\n');
                    resolve(data);
                } else {
                    console.log('❌ Strapi API not accessible\n');
                    resolve(null);
                }
            });
        });
        
        req.on('error', (e) => {
            console.log('❌ Strapi API error:', e.message, '\n');
            resolve(null);
        });
        
        req.setTimeout(5000, () => {
            req.destroy();
            console.log('❌ Strapi API timeout\n');
            resolve(null);
        });
        
        req.end();
    });
}

// Main execution
async function main() {
    console.log('=' .repeat(60));
    console.log('POLYMARKET API CONNECTIVITY TEST');
    console.log('=' .repeat(60) + '\n');
    
    try {
        // Test public API
        console.log('1. Testing Gamma API (Public Markets)...\n');
        await testGammaAPI();
        
        // Test CLOB endpoints
        console.log('=' .repeat(60));
        console.log('2. Checking CLOB API Endpoints...');
        await testCLOBAPI();
        
        // Test Strapi
        console.log('=' .repeat(60));
        console.log('3. Testing Strapi API...\n');
        await testStrapiAPI();
        
        console.log('=' .repeat(60));
        console.log('✅ POLYMARKET PUBLIC API TEST COMPLETE');
        console.log('=' .repeat(60));
        console.log('\nSummary:');
        console.log('  • Gamma API (Public): ✅ Working');
        console.log('  • CLOB API: Requires proper authentication');
        console.log('  • Use Gamma API for public market data');
        console.log('  • CLOB API needed for trading operations');
        console.log('\nRecommendation:');
        console.log('  For production, register at polymarket.com');
        console.log('  and obtain proper API credentials.');
        console.log('=' .repeat(60));
        
    } catch (error) {
        console.error('\n❌ Test failed:', error.message);
        process.exit(1);
    }
}

// Run tests
main();