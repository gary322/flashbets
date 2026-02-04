#!/usr/bin/env node

const http = require('http');
const https = require('https');

const API_BASE = 'http://localhost:8081';
const TEST_WALLET = 'test-wallet-' + Date.now();

// ANSI color codes
const colors = {
    reset: '\x1b[0m',
    green: '\x1b[32m',
    red: '\x1b[31m',
    yellow: '\x1b[33m',
    blue: '\x1b[34m',
    magenta: '\x1b[35m',
    cyan: '\x1b[36m'
};

let passedTests = 0;
let failedTests = 0;
let totalTests = 0;

function log(message, color = colors.reset) {
    console.log(`${color}${message}${colors.reset}`);
}

function logTest(name, passed, details = '') {
    totalTests++;
    if (passed) {
        passedTests++;
        log(`✅ ${name}`, colors.green);
    } else {
        failedTests++;
        log(`❌ ${name}: ${details}`, colors.red);
    }
}

function makeRequest(options, data = null) {
    return new Promise((resolve, reject) => {
        const req = http.request(options, (res) => {
            let body = '';
            res.on('data', chunk => body += chunk);
            res.on('end', () => {
                try {
                    const json = JSON.parse(body);
                    resolve({ status: res.statusCode, body: json });
                } catch (e) {
                    resolve({ status: res.statusCode, body: body });
                }
            });
        });
        
        req.on('error', reject);
        
        if (data) {
            req.write(JSON.stringify(data));
        }
        
        req.end();
    });
}

async function testHealthCheck() {
    try {
        const res = await makeRequest({
            hostname: 'localhost',
            port: 8081,
            path: '/health',
            method: 'GET'
        });
        
        logTest('Health Check', res.status === 200 && res.body.status === 'ok');
    } catch (e) {
        logTest('Health Check', false, e.message);
    }
}

async function testMarketOrders() {
    log('\n=== Testing Market Orders ===', colors.cyan);
    
    // Test market buy order
    try {
        const res = await makeRequest({
            hostname: 'localhost',
            port: 8081,
            path: '/api/trade/place',
            method: 'POST',
            headers: { 'Content-Type': 'application/json' }
        }, {
            market_id: 1,
            amount: 1000000,
            outcome: 0,
            leverage: 2,
            order_type: 'market'
        });
        
        logTest('Market Buy Order', res.status === 200);
    } catch (e) {
        logTest('Market Buy Order', false, e.message);
    }
}

async function testLimitOrders() {
    log('\n=== Testing Limit Orders ===', colors.cyan);
    
    const testCases = [
        { price: 0.5, side: 'buy', name: 'Limit Buy @ 0.5' },
        { price: 0.6, side: 'sell', name: 'Limit Sell @ 0.6' },
        { price: 0.45, side: 'buy', name: 'Limit Buy @ 0.45' }
    ];
    
    for (const test of testCases) {
        try {
            const res = await makeRequest({
                hostname: 'localhost',
                port: 8081,
                path: '/api/orders/limit',
                method: 'POST',
                headers: { 'Content-Type': 'application/json' }
            }, {
                market_id: 1,
                wallet: TEST_WALLET,
                amount: 500000,
                outcome: 0,
                leverage: 2,
                price: test.price,
                side: test.side
            });
            
            logTest(test.name, res.status === 200 && res.body.order);
        } catch (e) {
            logTest(test.name, false, e.message);
        }
    }
}

async function testStopOrders() {
    log('\n=== Testing Stop Orders ===', colors.cyan);
    
    const testCases = [
        { trigger_price: 0.45, order_type: 'stop_loss', name: 'Stop Loss @ 0.45' },
        { trigger_price: 0.55, order_type: 'take_profit', name: 'Take Profit @ 0.55' }
    ];
    
    for (const test of testCases) {
        try {
            const res = await makeRequest({
                hostname: 'localhost',
                port: 8081,
                path: '/api/orders/stop',
                method: 'POST',
                headers: { 'Content-Type': 'application/json' }
            }, {
                market_id: 1,
                wallet: TEST_WALLET,
                amount: 300000,
                outcome: 0,
                leverage: 3,
                trigger_price: test.trigger_price,
                order_type: test.order_type,
                side: 'sell'
            });
            
            logTest(test.name, res.status === 200 && res.body.order);
        } catch (e) {
            logTest(test.name, false, e.message);
        }
    }
}

async function testVerseMatching() {
    log('\n=== Testing Verse Matching ===', colors.cyan);
    
    const testCases = [
        {
            title: 'Biden Approval Rating',
            category: 'politics',
            keywords: ['biden', 'approval', 'president'],
            expectedCount: 4,
            name: 'Biden Approval Verses'
        },
        {
            title: 'Super Bowl Winner 2024',
            category: 'sports',
            keywords: ['nfl', 'super bowl', 'football'],
            expectedCount: 4,
            name: 'Super Bowl Verses'
        },
        {
            title: 'Bitcoin Price End of Year',
            category: 'crypto',
            keywords: ['bitcoin', 'btc', 'price'],
            expectedCount: 4,
            name: 'Bitcoin Price Verses'
        },
        {
            title: 'Tesla Stock Price',
            category: 'finance',
            keywords: ['tesla', 'stock', 'tsla'],
            expectedCount: 4,
            name: 'Tesla Stock Verses'
        }
    ];
    
    for (const test of testCases) {
        try {
            const res = await makeRequest({
                hostname: 'localhost',
                port: 8081,
                path: '/api/test/verse-match',
                method: 'POST',
                headers: { 'Content-Type': 'application/json' }
            }, {
                title: test.title,
                category: test.category,
                keywords: test.keywords
            });
            
            const passed = res.status === 200 && 
                          res.body.count >= test.expectedCount &&
                          res.body.matching_verses.length >= test.expectedCount;
            
            logTest(test.name, passed, 
                passed ? `Found ${res.body.count} verses` : `Expected ${test.expectedCount}, got ${res.body.count}`);
        } catch (e) {
            logTest(test.name, false, e.message);
        }
    }
}

async function testOrderManagement() {
    log('\n=== Testing Order Management ===', colors.cyan);
    
    // Place an order first
    let orderId;
    try {
        const res = await makeRequest({
            hostname: 'localhost',
            port: 8081,
            path: '/api/orders/limit',
            method: 'POST',
            headers: { 'Content-Type': 'application/json' }
        }, {
            market_id: 2,
            wallet: TEST_WALLET,
            amount: 100000,
            outcome: 0,
            leverage: 1,
            price: 0.48,
            side: 'buy'
        });
        
        if (res.status === 200 && res.body.order) {
            orderId = res.body.order.id;
            logTest('Place Order for Management', true);
        } else {
            logTest('Place Order for Management', false);
            return;
        }
    } catch (e) {
        logTest('Place Order for Management', false, e.message);
        return;
    }
    
    // Get orders for wallet
    try {
        const res = await makeRequest({
            hostname: 'localhost',
            port: 8081,
            path: `/api/orders/${TEST_WALLET}`,
            method: 'GET'
        });
        
        const hasOrders = res.status === 200 && 
                         res.body.orders && 
                         res.body.orders.length > 0;
        
        logTest('Get Wallet Orders', hasOrders, 
            hasOrders ? `Found ${res.body.orders.length} orders` : 'No orders found');
    } catch (e) {
        logTest('Get Wallet Orders', false, e.message);
    }
    
    // Cancel order
    if (orderId) {
        try {
            const res = await makeRequest({
                hostname: 'localhost',
                port: 8081,
                path: `/api/orders/${orderId}/cancel`,
                method: 'POST'
            });
            
            const cancelled = res.status === 200 && 
                            res.body.order && 
                            res.body.order.status === 'Cancelled';
            
            logTest('Cancel Order', cancelled);
        } catch (e) {
            logTest('Cancel Order', false, e.message);
        }
    }
}

async function testPolymarketIntegration() {
    log('\n=== Testing Polymarket Integration ===', colors.cyan);
    
    try {
        const res = await makeRequest({
            hostname: 'localhost',
            port: 8081,
            path: '/api/polymarket/markets?limit=5',
            method: 'GET'
        });
        
        const hasMarkets = res.status === 200 && 
                          Array.isArray(res.body) && 
                          res.body.length > 0;
        
        if (hasMarkets) {
            const market = res.body[0];
            const hasVerses = market.verses && market.verses.length > 0;
            
            logTest('Fetch Polymarket Markets', true, `Found ${res.body.length} markets`);
            logTest('Markets Have Verses', hasVerses, 
                hasVerses ? `First market has ${market.verses.length} verses` : 'No verses found');
            
            // Check verse structure
            if (hasVerses) {
                const verse = market.verses[0];
                const validStructure = verse.id && verse.name && verse.multiplier && verse.level;
                logTest('Verse Structure Valid', validStructure);
            }
        } else {
            logTest('Fetch Polymarket Markets', false, 'No markets returned');
        }
    } catch (e) {
        logTest('Fetch Polymarket Markets', false, e.message);
    }
}

async function testWebSocketConnection() {
    log('\n=== Testing WebSocket Connection ===', colors.cyan);
    
    return new Promise((resolve) => {
        const WebSocket = require('ws');
        const ws = new WebSocket('ws://localhost:8081/ws');
        
        let connected = false;
        let messageReceived = false;
        
        ws.on('open', () => {
            connected = true;
            logTest('WebSocket Connection', true);
            
            // Subscribe to updates
            ws.send(JSON.stringify({
                type: 'subscribe',
                channel: 'market_updates'
            }));
        });
        
        ws.on('message', (data) => {
            if (!messageReceived) {
                messageReceived = true;
                logTest('WebSocket Message Receipt', true, 'Received: ' + data.toString().substring(0, 50) + '...');
            }
        });
        
        ws.on('error', (err) => {
            if (!connected) {
                logTest('WebSocket Connection', false, err.message);
            }
        });
        
        // Give it 2 seconds to connect and receive a message
        setTimeout(() => {
            ws.close();
            if (!messageReceived && connected) {
                logTest('WebSocket Message Receipt', false, 'No messages received');
            }
            resolve();
        }, 2000);
    });
}

async function testPortfolioEndpoints() {
    log('\n=== Testing Portfolio Endpoints ===', colors.cyan);
    
    // Create demo account
    let demoWallet;
    try {
        const res = await makeRequest({
            hostname: 'localhost',
            port: 8081,
            path: '/api/wallet/demo/create',
            method: 'POST',
            headers: { 'Content-Type': 'application/json' }
        }, {
            initial_balance: 10000
        });
        
        if (res.status === 200 && res.body.wallet) {
            demoWallet = res.body.wallet;
            logTest('Create Demo Account', true, `Wallet: ${demoWallet.substring(0, 8)}...`);
        } else {
            logTest('Create Demo Account', false);
            return;
        }
    } catch (e) {
        logTest('Create Demo Account', false, e.message);
        return;
    }
    
    // Test portfolio endpoints
    const endpoints = [
        { path: `/api/portfolio/${demoWallet}`, name: 'Get Portfolio' },
        { path: `/api/positions/${demoWallet}`, name: 'Get Positions' },
        { path: `/api/risk/${demoWallet}`, name: 'Get Risk Metrics' },
        { path: `/api/wallet/balance/${demoWallet}`, name: 'Get Balance' }
    ];
    
    for (const endpoint of endpoints) {
        try {
            const res = await makeRequest({
                hostname: 'localhost',
                port: 8081,
                path: endpoint.path,
                method: 'GET'
            });
            
            logTest(endpoint.name, res.status === 200);
        } catch (e) {
            logTest(endpoint.name, false, e.message);
        }
    }
}

async function testQuantumEndpoints() {
    log('\n=== Testing Quantum Endpoints ===', colors.cyan);
    
    // Create quantum position
    try {
        const res = await makeRequest({
            hostname: 'localhost',
            port: 8081,
            path: '/api/quantum/create',
            method: 'POST',
            headers: { 'Content-Type': 'application/json' }
        }, {
            states: [
                {
                    market_id: 1,
                    outcome: 0,
                    amount: 100000,
                    leverage: 2,
                    probability: 0.6
                },
                {
                    market_id: 1,
                    outcome: 1,
                    amount: 100000,
                    leverage: 2,
                    probability: 0.4
                }
            ],
            entanglement_group: 'test-group-1'
        });
        
        logTest('Create Quantum Position', res.status === 200 && res.body.quantum_position_id);
    } catch (e) {
        logTest('Create Quantum Position', false, e.message);
    }
    
    // Get quantum states
    try {
        const res = await makeRequest({
            hostname: 'localhost',
            port: 8081,
            path: '/api/quantum/states/1',
            method: 'GET'
        });
        
        const hasStates = res.status === 200 && 
                         res.body.quantum_states && 
                         res.body.quantum_states.length > 0;
        
        logTest('Get Quantum States', hasStates, 
            hasStates ? `Found ${res.body.quantum_states.length} states` : 'No states found');
    } catch (e) {
        logTest('Get Quantum States', false, e.message);
    }
}

async function runAllTests() {
    log('🚀 Starting Automated Test Suite', colors.magenta);
    log('================================\n', colors.magenta);
    
    const startTime = Date.now();
    
    // Run all test suites
    await testHealthCheck();
    await testMarketOrders();
    await testLimitOrders();
    await testStopOrders();
    await testVerseMatching();
    await testOrderManagement();
    await testPolymarketIntegration();
    await testPortfolioEndpoints();
    await testQuantumEndpoints();
    await testWebSocketConnection();
    
    const duration = ((Date.now() - startTime) / 1000).toFixed(2);
    
    // Summary
    log('\n================================', colors.magenta);
    log('📊 Test Summary', colors.magenta);
    log('================================', colors.magenta);
    log(`Total Tests: ${totalTests}`, colors.blue);
    log(`Passed: ${passedTests}`, colors.green);
    log(`Failed: ${failedTests}`, colors.red);
    log(`Success Rate: ${((passedTests / totalTests) * 100).toFixed(1)}%`, colors.yellow);
    log(`Duration: ${duration}s`, colors.cyan);
    
    if (failedTests === 0) {
        log('\n🎉 All tests passed!', colors.green);
    } else {
        log(`\n⚠️  ${failedTests} tests failed`, colors.red);
    }
    
    process.exit(failedTests > 0 ? 1 : 0);
}

// Check if API is running
http.get(API_BASE + '/health', (res) => {
    if (res.statusCode === 200) {
        runAllTests();
    } else {
        log('❌ API server is not responding correctly', colors.red);
        process.exit(1);
    }
}).on('error', (err) => {
    log('❌ Cannot connect to API server at ' + API_BASE, colors.red);
    log('Make sure the API server is running with: cargo run', colors.yellow);
    process.exit(1);
});