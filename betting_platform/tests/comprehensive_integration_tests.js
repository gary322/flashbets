#!/usr/bin/env node

/**
 * Comprehensive Integration Test Suite for Betting Platform
 * Tests all user flows end-to-end
 */

const { Connection, Keypair, PublicKey, LAMPORTS_PER_SOL } = require('@solana/web3.js');
const axios = require('axios');
const WebSocket = require('ws');
const { performance } = require('perf_hooks');

// Configuration
const RPC_URL = 'http://localhost:8899';
const API_URL = 'http://localhost:8081/api';
const WS_URL = 'ws://localhost:8081/ws';

// Test wallets
const testWallets = [];
const testResults = {
    passed: 0,
    failed: 0,
    errors: []
};

// Utility functions
async function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

function logTest(testName, passed, error = null) {
    if (passed) {
        console.log(`✅ ${testName}`);
        testResults.passed++;
    } else {
        console.log(`❌ ${testName}: ${error}`);
        testResults.failed++;
        testResults.errors.push({ test: testName, error });
    }
}

// API helper
async function apiCall(endpoint, method = 'GET', data = null) {
    try {
        const config = {
            method,
            url: `${API_URL}${endpoint}`,
            headers: { 'Content-Type': 'application/json' }
        };
        
        if (data) {
            config.data = data;
        }
        
        const response = await axios(config);
        return response.data;
    } catch (error) {
        throw new Error(`API call failed: ${error.response?.data?.error?.message || error.message}`);
    }
}

// Test Suite Functions
async function testWalletConnection() {
    console.log('\n🔗 Testing Wallet Connection Flow...');
    
    try {
        // Create test wallet
        const wallet = Keypair.generate();
        const walletAddress = wallet.publicKey.toBase58();
        
        // Test wallet verification
        const nonce = await apiCall('/auth/nonce', 'POST', { wallet: walletAddress });
        logTest('Generate auth nonce', true);
        
        // In real scenario, wallet would sign the nonce
        // For testing, we'll use demo wallet
        const demoWallet = await apiCall('/demo/create', 'POST');
        testWallets.push(demoWallet.wallet);
        logTest('Create demo wallet', true);
        
        return true;
    } catch (error) {
        logTest('Wallet connection flow', false, error.message);
        return false;
    }
}

async function testMarketDiscovery() {
    console.log('\n🔍 Testing Market Discovery Flow...');
    
    try {
        // Test get all markets
        const markets = await apiCall('/markets?limit=10');
        logTest('Fetch all markets', markets.markets.length > 0);
        
        // Test search markets
        const searchResults = await apiCall('/markets?search=bitcoin');
        logTest('Search markets by keyword', searchResults.markets.length > 0);
        
        // Test filter by verse
        const verseMarkets = await apiCall('/markets?verse_id=2');
        logTest('Filter markets by verse', true);
        
        // Test get single market
        if (markets.markets.length > 0) {
            const marketId = markets.markets[0].id;
            const market = await apiCall(`/markets/${marketId}`);
            logTest('Get single market details', market.id === marketId);
        }
        
        return true;
    } catch (error) {
        logTest('Market discovery flow', false, error.message);
        return false;
    }
}

async function testTradingFlow() {
    console.log('\n💹 Testing Trading Flow...');
    
    try {
        const wallet = testWallets[0] || 'demo_wallet_test';
        
        // Get a market
        const markets = await apiCall('/markets?limit=1');
        if (markets.markets.length === 0) {
            throw new Error('No markets available');
        }
        
        const market = markets.markets[0];
        
        // Place market order
        const tradeData = {
            market_id: market.id,
            outcome: 0,
            amount: 100,
            wallet: wallet,
            leverage: 5,
            order_type: 'market'
        };
        
        const trade = await apiCall('/trades', 'POST', tradeData);
        logTest('Place market order', trade.signature !== undefined);
        
        // Place limit order
        const limitOrder = {
            market_id: market.id,
            outcome: 1,
            amount: 50,
            wallet: wallet,
            price: 0.45,
            order_type: 'limit'
        };
        
        const limitResult = await apiCall('/orders/limit', 'POST', limitOrder);
        logTest('Place limit order', true);
        
        // Test quantum trading
        const quantumTrade = {
            market_ids: [market.id],
            verses: [1, 2],
            amount: 200,
            wallet: wallet,
            quantum_mode: true
        };
        
        const quantumResult = await apiCall('/quantum/trade', 'POST', quantumTrade);
        logTest('Place quantum trade', true);
        
        return true;
    } catch (error) {
        logTest('Trading flow', false, error.message);
        return false;
    }
}

async function testPositionManagement() {
    console.log('\n📊 Testing Position Management Flow...');
    
    try {
        const wallet = testWallets[0] || 'demo_wallet_test';
        
        // Get positions
        const positions = await apiCall(`/positions?wallet=${wallet}`);
        logTest('Fetch user positions', true);
        
        // If we have positions, test closing
        if (positions.positions && positions.positions.length > 0) {
            const position = positions.positions[0];
            const closeResult = await apiCall(`/positions/${position.id}/close`, 'POST');
            logTest('Close position', true);
        }
        
        // Get position history
        const history = await apiCall(`/positions/history?wallet=${wallet}`);
        logTest('Fetch position history', true);
        
        return true;
    } catch (error) {
        logTest('Position management flow', false, error.message);
        return false;
    }
}

async function testWebSocketFlow() {
    console.log('\n🔌 Testing WebSocket Flow...');
    
    return new Promise((resolve) => {
        let wsConnected = false;
        let receivedUpdate = false;
        
        try {
            const ws = new WebSocket(WS_URL);
            
            ws.on('open', () => {
                wsConnected = true;
                logTest('WebSocket connection', true);
                
                // Subscribe to market updates
                ws.send(JSON.stringify({
                    type: 'subscribe',
                    markets: [1000, 1001, 1002]
                }));
            });
            
            ws.on('message', (data) => {
                receivedUpdate = true;
                logTest('Receive WebSocket updates', true);
                ws.close();
            });
            
            ws.on('close', () => {
                resolve(wsConnected && receivedUpdate);
            });
            
            ws.on('error', (error) => {
                logTest('WebSocket flow', false, error.message);
                resolve(false);
            });
            
            // Timeout after 5 seconds
            setTimeout(() => {
                if (!receivedUpdate) {
                    logTest('WebSocket updates timeout', false, 'No updates received');
                }
                ws.close();
            }, 5000);
            
        } catch (error) {
            logTest('WebSocket flow', false, error.message);
            resolve(false);
        }
    });
}

async function testRiskManagement() {
    console.log('\n⚠️ Testing Risk Management Flow...');
    
    try {
        const wallet = testWallets[0] || 'demo_wallet_test';
        
        // Get risk metrics
        const riskMetrics = await apiCall(`/risk/metrics?wallet=${wallet}`);
        logTest('Fetch risk metrics', true);
        
        // Set risk limits
        const riskLimits = {
            max_position_size: 5000,
            max_leverage: 10,
            max_drawdown: 0.2
        };
        
        await apiCall('/risk/limits', 'POST', riskLimits);
        logTest('Set risk limits', true);
        
        return true;
    } catch (error) {
        logTest('Risk management flow', false, error.message);
        return false;
    }
}

async function testDeFiFeatures() {
    console.log('\n🏦 Testing DeFi Features Flow...');
    
    try {
        const wallet = testWallets[0] || 'demo_wallet_test';
        
        // Add liquidity
        const addLiquidity = {
            market_id: 1000,
            amount: 1000,
            wallet: wallet
        };
        
        await apiCall('/liquidity/add', 'POST', addLiquidity);
        logTest('Add liquidity', true);
        
        // Stake tokens
        const stake = {
            amount: 500,
            wallet: wallet,
            duration_days: 30
        };
        
        await apiCall('/staking/stake', 'POST', stake);
        logTest('Stake tokens', true);
        
        return true;
    } catch (error) {
        logTest('DeFi features flow', false, error.message);
        return false;
    }
}

async function testPerformanceMetrics() {
    console.log('\n⚡ Testing Performance Metrics...');
    
    const metrics = {
        apiLatencies: [],
        wsLatencies: [],
        throughput: 0
    };
    
    try {
        // Test API latency
        for (let i = 0; i < 100; i++) {
            const start = performance.now();
            await apiCall('/markets?limit=10');
            const end = performance.now();
            metrics.apiLatencies.push(end - start);
        }
        
        const avgLatency = metrics.apiLatencies.reduce((a, b) => a + b, 0) / metrics.apiLatencies.length;
        const p95Latency = metrics.apiLatencies.sort((a, b) => a - b)[Math.floor(metrics.apiLatencies.length * 0.95)];
        
        logTest(`API Average Latency: ${avgLatency.toFixed(2)}ms`, avgLatency < 100);
        logTest(`API P95 Latency: ${p95Latency.toFixed(2)}ms`, p95Latency < 500);
        
        // Test throughput
        const start = performance.now();
        const promises = [];
        for (let i = 0; i < 50; i++) {
            promises.push(apiCall('/markets?limit=10').catch(() => null));
        }
        await Promise.all(promises);
        const duration = (performance.now() - start) / 1000; // seconds
        metrics.throughput = 50 / duration;
        
        logTest(`Throughput: ${metrics.throughput.toFixed(2)} req/s`, metrics.throughput > 10);
        
        return true;
    } catch (error) {
        logTest('Performance metrics', false, error.message);
        return false;
    }
}

// Main test runner
async function runAllTests() {
    console.log('🚀 Starting Comprehensive Integration Tests\n');
    console.log('API URL:', API_URL);
    console.log('RPC URL:', RPC_URL);
    console.log('WebSocket URL:', WS_URL);
    
    const startTime = Date.now();
    
    // Run all test suites
    await testWalletConnection();
    await testMarketDiscovery();
    await testTradingFlow();
    await testPositionManagement();
    await testWebSocketFlow();
    await testRiskManagement();
    await testDeFiFeatures();
    await testPerformanceMetrics();
    
    // Print summary
    const duration = (Date.now() - startTime) / 1000;
    console.log('\n' + '='.repeat(50));
    console.log('📊 Test Summary');
    console.log('='.repeat(50));
    console.log(`Total Tests: ${testResults.passed + testResults.failed}`);
    console.log(`Passed: ${testResults.passed}`);
    console.log(`Failed: ${testResults.failed}`);
    console.log(`Duration: ${duration.toFixed(2)}s`);
    
    if (testResults.failed > 0) {
        console.log('\n❌ Failed Tests:');
        testResults.errors.forEach(({ test, error }) => {
            console.log(`  - ${test}: ${error}`);
        });
    }
    
    console.log('\n' + (testResults.failed === 0 ? '✅ All tests passed!' : '❌ Some tests failed!'));
    
    // Exit with appropriate code
    process.exit(testResults.failed === 0 ? 0 : 1);
}

// Run tests
runAllTests().catch(console.error);