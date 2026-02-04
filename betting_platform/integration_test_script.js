#!/usr/bin/env node

/**
 * Comprehensive Integration Test Script
 * Tests all components of the betting platform
 */

const http = require('http');
const https = require('https');
const { URL } = require('url');

// Test configuration
const config = {
  api: 'http://localhost:8081',
  ui: 'http://localhost:3000',
  solana: 'http://localhost:8899',
  ws: 'ws://localhost:8081/ws'
};

// Color codes for output
const colors = {
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  reset: '\x1b[0m'
};

// Test results
const results = {
  passed: 0,
  failed: 0,
  tests: []
};

// Helper function to make HTTP requests
function httpRequest(url, options = {}) {
  return new Promise((resolve, reject) => {
    const parsedUrl = new URL(url);
    const client = parsedUrl.protocol === 'https:' ? https : http;
    
    const req = client.request(url, options, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        resolve({
          status: res.statusCode,
          headers: res.headers,
          data: data
        });
      });
    });
    
    req.on('error', reject);
    
    if (options.body) {
      req.write(options.body);
    }
    
    req.end();
  });
}

// Test functions
async function testSolanaValidator() {
  console.log(`\n${colors.blue}Testing Solana Validator...${colors.reset}`);
  
  try {
    const response = await httpRequest(config.solana, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'getHealth'
      })
    });
    
    const data = JSON.parse(response.data);
    const passed = response.status === 200 && data.result === 'ok';
    
    return {
      name: 'Solana Validator Health',
      passed,
      message: passed ? 'Validator is healthy' : `Validator unhealthy: ${data.error?.message || 'Unknown error'}`
    };
  } catch (error) {
    return {
      name: 'Solana Validator Health',
      passed: false,
      message: `Connection failed: ${error.message}`
    };
  }
}

async function testAPIHealth() {
  console.log(`\n${colors.blue}Testing API Backend...${colors.reset}`);
  
  try {
    const response = await httpRequest(`${config.api}/health`);
    const data = JSON.parse(response.data);
    const passed = response.status === 200 && data.status === 'ok';
    
    return {
      name: 'API Health Check',
      passed,
      message: passed ? 'API is healthy' : 'API unhealthy'
    };
  } catch (error) {
    return {
      name: 'API Health Check',
      passed: false,
      message: `Connection failed: ${error.message}`
    };
  }
}

async function testMarketsEndpoint() {
  try {
    const start = Date.now();
    const response = await httpRequest(`${config.api}/api/markets`);
    const elapsed = Date.now() - start;
    const data = JSON.parse(response.data);
    
    const passed = response.status === 200 && 
                   data.count !== undefined && 
                   Array.isArray(data.markets);
    
    return {
      name: 'Markets API Endpoint',
      passed,
      message: passed ? 
        `Found ${data.count} markets (${elapsed}ms)` : 
        'Invalid response format',
      metrics: { responseTime: elapsed, marketCount: data.count }
    };
  } catch (error) {
    return {
      name: 'Markets API Endpoint',
      passed: false,
      message: `Request failed: ${error.message}`
    };
  }
}

async function testVersesEndpoint() {
  try {
    const response = await httpRequest(`${config.api}/api/verses`);
    const data = JSON.parse(response.data);
    const passed = response.status === 200 && Array.isArray(data);
    
    return {
      name: 'Verses API Endpoint',
      passed,
      message: passed ? `Found ${data.length} verses` : 'Invalid response'
    };
  } catch (error) {
    return {
      name: 'Verses API Endpoint',
      passed: false,
      message: `Request failed: ${error.message}`
    };
  }
}

async function testDemoWalletCreation() {
  try {
    const response = await httpRequest(`${config.api}/api/wallet/demo/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: 'Test User' })
    });
    
    const data = JSON.parse(response.data);
    const passed = response.status === 200 && data.wallet && data.privateKey;
    
    return {
      name: 'Demo Wallet Creation',
      passed,
      message: passed ? 
        `Created wallet: ${data.wallet.substring(0, 8)}...` : 
        'Failed to create wallet'
    };
  } catch (error) {
    return {
      name: 'Demo Wallet Creation',
      passed: false,
      message: `Request failed: ${error.message}`
    };
  }
}

async function testUIServer() {
  console.log(`\n${colors.blue}Testing UI Frontend...${colors.reset}`);
  
  try {
    const response = await httpRequest(config.ui);
    const passed = response.status === 200;
    
    return {
      name: 'UI Server Status',
      passed,
      message: passed ? 'UI server is running' : `UI server error: ${response.status}`
    };
  } catch (error) {
    return {
      name: 'UI Server Status',
      passed: false,
      message: `Connection failed: ${error.message}`
    };
  }
}

async function testWebSocket() {
  console.log(`\n${colors.blue}Testing WebSocket...${colors.reset}`);
  
  return new Promise((resolve) => {
    try {
      const WebSocket = require('ws');
      const ws = new WebSocket(config.ws);
      let connected = false;
      
      const timeout = setTimeout(() => {
        if (!connected) {
          ws.close();
          resolve({
            name: 'WebSocket Connection',
            passed: false,
            message: 'Connection timeout'
          });
        }
      }, 5000);
      
      ws.on('open', () => {
        connected = true;
        clearTimeout(timeout);
        ws.send(JSON.stringify({ type: 'subscribe', channel: 'markets' }));
        
        setTimeout(() => {
          ws.close();
          resolve({
            name: 'WebSocket Connection',
            passed: true,
            message: 'WebSocket connected successfully'
          });
        }, 1000);
      });
      
      ws.on('error', (error) => {
        clearTimeout(timeout);
        resolve({
          name: 'WebSocket Connection',
          passed: false,
          message: `Connection error: ${error.message}`
        });
      });
    } catch (error) {
      resolve({
        name: 'WebSocket Connection',
        passed: false,
        message: `WebSocket module not available: ${error.message}`
      });
    }
  });
}

// Run a test and update results
async function runTest(testFn) {
  const result = await testFn();
  results.tests.push(result);
  
  if (result.passed) {
    results.passed++;
    console.log(`${colors.green}✅ ${result.name}${colors.reset}`);
    console.log(`   ${result.message}`);
  } else {
    results.failed++;
    console.log(`${colors.red}❌ ${result.name}${colors.reset}`);
    console.log(`   ${result.message}`);
  }
  
  if (result.metrics) {
    console.log(`   Metrics:`, result.metrics);
  }
}

// Main test runner
async function runAllTests() {
  console.log(`${colors.yellow}🧪 Betting Platform Integration Tests${colors.reset}`);
  console.log('=' .repeat(50));
  
  // Infrastructure tests
  await runTest(testSolanaValidator);
  await runTest(testAPIHealth);
  await runTest(testUIServer);
  
  // API endpoint tests
  console.log(`\n${colors.blue}Testing API Endpoints...${colors.reset}`);
  await runTest(testMarketsEndpoint);
  await runTest(testVersesEndpoint);
  await runTest(testDemoWalletCreation);
  
  // Real-time tests
  await runTest(testWebSocket);
  
  // Summary
  console.log('\n' + '=' .repeat(50));
  console.log(`${colors.yellow}Test Summary:${colors.reset}`);
  console.log(`${colors.green}Passed: ${results.passed}${colors.reset}`);
  console.log(`${colors.red}Failed: ${results.failed}${colors.reset}`);
  console.log(`Total: ${results.tests.length}`);
  
  if (results.failed === 0) {
    console.log(`\n${colors.green}✨ All tests passed! Full stack integration verified.${colors.reset}`);
  } else {
    console.log(`\n${colors.red}⚠️  Some tests failed. Please check the errors above.${colors.reset}`);
  }
  
  // Exit with appropriate code
  process.exit(results.failed > 0 ? 1 : 0);
}

// Check if ws module is available
try {
  require('ws');
} catch (e) {
  console.log(`${colors.yellow}Note: WebSocket tests skipped (ws module not installed)${colors.reset}`);
}

// Run tests
runAllTests().catch(error => {
  console.error(`${colors.red}Test runner error:${colors.reset}`, error);
  process.exit(1);
});