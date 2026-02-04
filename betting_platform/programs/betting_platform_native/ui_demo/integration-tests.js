// Comprehensive Integration Test Suite for Boom Platform
// Tests all user journeys and component connections

const API_BASE_URL = 'http://localhost:8081/api';
const WS_URL = 'ws://localhost:8081/ws';

// Test results tracking
let testResults = {
    passed: 0,
    failed: 0,
    total: 0,
    details: []
};

// Utility functions
async function runTest(testName, testFn) {
    testResults.total++;
    console.log(`\n🧪 Running: ${testName}`);
    try {
        await testFn();
        testResults.passed++;
        testResults.details.push({ name: testName, status: 'PASSED', error: null });
        console.log(`✅ PASSED: ${testName}`);
    } catch (error) {
        testResults.failed++;
        testResults.details.push({ name: testName, status: 'FAILED', error: error.message });
        console.error(`❌ FAILED: ${testName}`, error.message);
    }
}

async function delay(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

// Test Suite
async function runIntegrationTests() {
    console.log('🚀 Starting Boom Platform Integration Tests\n');
    
    // 1. Backend API Health Check
    await runTest('Backend API Health Check', async () => {
        const response = await fetch(`http://localhost:8081/health`);
        if (!response.ok) throw new Error(`Health check failed: ${response.status}`);
        const data = await response.json();
        if (data.status !== 'ok') throw new Error('Health status not ok');
    });
    
    // 2. Program Info Endpoint
    await runTest('Program Info Endpoint', async () => {
        const response = await fetch(`${API_BASE_URL}/program/info`);
        if (!response.ok) throw new Error(`Program info failed: ${response.status}`);
        const data = await response.json();
        if (!data.programId && !data.program_id) throw new Error('No program ID returned');
    });
    
    // 3. Markets List
    await runTest('Fetch Markets List', async () => {
        const response = await fetch(`${API_BASE_URL}/markets`);
        if (!response.ok) throw new Error(`Markets fetch failed: ${response.status}`);
        const markets = await response.json();
        if (!Array.isArray(markets)) throw new Error('Markets not an array');
        // It's OK if no markets exist yet
        console.log(`    Found ${markets.length} markets`);
    });
    
    // 4. Individual Market Details
    await runTest('Fetch Individual Market', async () => {
        const marketsResponse = await fetch(`${API_BASE_URL}/markets`);
        const markets = await marketsResponse.json();
        if (markets.length === 0) {
            console.log('    No markets available to test individual fetch');
            return;
        }
        
        const marketId = markets[0].id;
        const response = await fetch(`${API_BASE_URL}/markets/${marketId}`);
        if (!response.ok) throw new Error(`Market detail failed: ${response.status}`);
        const market = await response.json();
        if (!market.id) throw new Error('Market has no ID');
    });
    
    // 5. Market Order Book
    await runTest('Fetch Market Order Book', async () => {
        const marketsResponse = await fetch(`${API_BASE_URL}/markets`);
        const markets = await marketsResponse.json();
        if (markets.length === 0) {
            // Test with a dummy market ID
            const response = await fetch(`${API_BASE_URL}/markets/1/orderbook`);
            if (!response.ok) throw new Error(`Order book failed: ${response.status}`);
            const orderbook = await response.json();
            if (!orderbook.asks || !orderbook.bids) throw new Error('Invalid order book structure');
            return;
        }
        
        const marketId = markets[0].id;
        const response = await fetch(`${API_BASE_URL}/markets/${marketId}/orderbook`);
        if (!response.ok) throw new Error(`Order book failed: ${response.status}`);
        const orderbook = await response.json();
        if (!orderbook.asks || !orderbook.bids) throw new Error('Invalid order book structure');
    });
    
    // 6. Verses/Stages Endpoint
    await runTest('Fetch Verses/Stages', async () => {
        const response = await fetch(`${API_BASE_URL}/verses`);
        if (!response.ok) throw new Error(`Verses fetch failed: ${response.status}`);
        const verses = await response.json();
        if (!Array.isArray(verses)) throw new Error('Verses not an array');
    });
    
    // 7. Quantum/Groups States
    await runTest('Fetch Quantum/Groups States', async () => {
        // Test with a dummy market ID since we may not have markets
        const marketId = 1;
        const response = await fetch(`${API_BASE_URL}/quantum/states/${marketId}`);
        if (!response.ok) throw new Error(`Quantum states failed: ${response.status}`);
        const states = await response.json();
        if (!states.quantum_states) throw new Error('No quantum states returned');
    });
    
    // 8. WebSocket Connection
    await runTest('WebSocket Connection', async () => {
        return new Promise((resolve, reject) => {
            const ws = new WebSocket(WS_URL);
            const timeout = setTimeout(() => {
                ws.close();
                reject(new Error('WebSocket connection timeout'));
            }, 5000);
            
            ws.onopen = () => {
                clearTimeout(timeout);
                ws.close();
                resolve();
            };
            
            ws.onerror = (error) => {
                clearTimeout(timeout);
                reject(new Error('WebSocket connection error'));
            };
        });
    });
    
    // 9. WebSocket Market Updates
    await runTest('WebSocket Market Updates', async () => {
        return new Promise((resolve, reject) => {
            const ws = new WebSocket(WS_URL);
            const timeout = setTimeout(() => {
                ws.close();
                reject(new Error('No market updates received'));
            }, 10000);
            
            ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    if (data.MarketUpdate || data.OrderBookUpdate) {
                        clearTimeout(timeout);
                        ws.close();
                        resolve();
                    }
                } catch (e) {
                    // Ignore parse errors
                }
            };
            
            ws.onerror = () => {
                clearTimeout(timeout);
                reject(new Error('WebSocket error'));
            };
        });
    });
    
    // 10. Portfolio Endpoint (with test wallet)
    await runTest('Portfolio Endpoint', async () => {
        const testWallet = '11111111111111111111111111111111';
        const response = await fetch(`${API_BASE_URL}/portfolio/${testWallet}`);
        if (!response.ok) throw new Error(`Portfolio fetch failed: ${response.status}`);
        const portfolio = await response.json();
        if (portfolio.total_value === undefined && portfolio.totalValue === undefined) {
            throw new Error('Portfolio missing total value');
        }
    });
    
    // 11. Risk Metrics Endpoint
    await runTest('Risk Metrics Endpoint', async () => {
        const testWallet = '11111111111111111111111111111111';
        const response = await fetch(`${API_BASE_URL}/risk/${testWallet}`);
        if (!response.ok) throw new Error(`Risk metrics failed: ${response.status}`);
        const risk = await response.json();
        if (!risk.portfolio_metrics) throw new Error('Risk metrics missing portfolio_metrics');
    });
    
    // 12. Polymarket Integration
    await runTest('Polymarket Markets Proxy', async () => {
        const response = await fetch(`${API_BASE_URL}/polymarket/markets`);
        if (!response.ok) throw new Error(`Polymarket proxy failed: ${response.status}`);
        const data = await response.json();
        const markets = data.data || data;
        if (!Array.isArray(markets)) throw new Error('Polymarket markets not an array');
    });
    
    // 13. Integration Status
    await runTest('Integration Status', async () => {
        const response = await fetch(`${API_BASE_URL}/integration/status`);
        if (!response.ok) throw new Error(`Integration status failed: ${response.status}`);
        const status = await response.json();
        if (status.polymarket === undefined) throw new Error('Integration status missing polymarket');
    });
    
    // 14. Trade Placement (dry run)
    await runTest('Trade Placement Endpoint', async () => {
        const tradeData = {
            market_id: 1,
            outcome: 0,
            amount: 100,
            leverage: 1,
            wallet: '11111111111111111111111111111111'
        };
        
        const response = await fetch(`${API_BASE_URL}/trade/place`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(tradeData)
        });
        
        // We expect this to fail with proper error (no real wallet)
        // But it should be a 400-level error, not 500
        if (response.status >= 500) {
            throw new Error(`Server error on trade: ${response.status}`);
        }
    });
    
    // 15. Position Close Endpoint
    await runTest('Position Close Endpoint', async () => {
        const closeData = {
            market_id: 1,
            position_index: 0
        };
        
        const response = await fetch(`${API_BASE_URL}/trade/close`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(closeData)
        });
        
        // We expect this to fail with proper error
        if (response.status >= 500) {
            throw new Error(`Server error on close: ${response.status}`);
        }
    });
    
    // 16. Quantum Position Creation
    await runTest('Quantum Position Creation', async () => {
        const quantumData = {
            market_id: 1,
            states: ['yes', 'no'],
            amplitudes: [0.707, 0.707],
            wallet: '11111111111111111111111111111111'
        };
        
        const response = await fetch(`${API_BASE_URL}/quantum/create`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(quantumData)
        });
        
        if (response.status >= 500) {
            throw new Error(`Server error on quantum create: ${response.status}`);
        }
    });
    
    // 17. DeFi Staking Endpoint
    await runTest('DeFi Staking Endpoint', async () => {
        const stakeData = {
            amount: 1000,
            wallet: '11111111111111111111111111111111'
        };
        
        const response = await fetch(`${API_BASE_URL}/defi/stake`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(stakeData)
        });
        
        if (response.status >= 500) {
            throw new Error(`Server error on stake: ${response.status}`);
        }
    });
    
    // 18. Liquidity Pools
    await runTest('Liquidity Pools Endpoint', async () => {
        const response = await fetch(`${API_BASE_URL}/defi/pools`);
        if (!response.ok) throw new Error(`Pools fetch failed: ${response.status}`);
        const data = await response.json();
        const pools = data.pools || data;
        if (!Array.isArray(pools)) throw new Error('Pools not an array');
    });
    
    // Print results
    console.log('\n' + '='.repeat(60));
    console.log('📊 TEST RESULTS SUMMARY');
    console.log('='.repeat(60));
    console.log(`Total Tests: ${testResults.total}`);
    console.log(`✅ Passed: ${testResults.passed}`);
    console.log(`❌ Failed: ${testResults.failed}`);
    console.log(`Success Rate: ${((testResults.passed / testResults.total) * 100).toFixed(1)}%`);
    
    if (testResults.failed > 0) {
        console.log('\n❌ FAILED TESTS:');
        testResults.details.filter(t => t.status === 'FAILED').forEach(test => {
            console.log(`  - ${test.name}: ${test.error}`);
        });
    }
    
    console.log('\n' + '='.repeat(60));
    
    return testResults;
}

// Run tests if called directly
if (typeof window === 'undefined') {
    runIntegrationTests().then(results => {
        process.exit(results.failed > 0 ? 1 : 0);
    });
} else {
    window.runIntegrationTests = runIntegrationTests;
}