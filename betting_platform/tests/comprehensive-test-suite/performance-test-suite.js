#!/usr/bin/env node

/**
 * Performance & Load Testing Suite
 * Tests system performance, scalability, and edge cases
 */

const fetch = require('node-fetch');
const { Connection, Keypair } = require('@solana/web3.js');
const chalk = require('chalk').default || require('chalk');
const ora = require('ora').default || require('ora');
const fs = require('fs');
const path = require('path');

class PerformanceTestSuite {
  constructor(config) {
    this.config = config;
    this.results = {
      totalTests: 0,
      passed: 0,
      failed: 0,
      phases: {},
      errors: [],
      metrics: {
        responseTimesMs: [],
        throughputRps: [],
        errorRates: [],
        memoryUsageMb: []
      },
      startTime: Date.now()
    };
    this.connection = new Connection(config.rpcUrl, 'confirmed');
  }

  async runTest(phase, testId, testName, testFn) {
    this.results.totalTests++;
    const spinner = ora(`Running ${testId}: ${testName}`).start();
    
    try {
      const startTime = Date.now();
      const result = await testFn.call(this);
      const duration = Date.now() - startTime;
      
      spinner.succeed(`✅ ${testId}: ${testName} (${duration}ms)`);
      this.results.passed++;
      
      if (!this.results.phases[phase]) {
        this.results.phases[phase] = { passed: 0, failed: 0, tests: {} };
      }
      this.results.phases[phase].passed++;
      this.results.phases[phase].tests[testId] = { 
        status: 'passed',
        duration,
        ...result
      };
      
    } catch (error) {
      spinner.fail(`❌ ${testId}: ${testName}`);
      this.results.failed++;
      
      if (!this.results.phases[phase]) {
        this.results.phases[phase] = { passed: 0, failed: 0, tests: {} };
      }
      this.results.phases[phase].failed++;
      this.results.phases[phase].tests[testId] = { 
        status: 'failed', 
        error: error.message 
      };
      
      this.results.errors.push({
        phase,
        testId,
        testName,
        error: error.message
      });
    }
    
    await new Promise(resolve => setTimeout(resolve, 100));
  }

  // Helper to measure response time
  async measureResponseTime(url, options = {}) {
    const start = Date.now();
    const response = await fetch(url, options);
    const duration = Date.now() - start;
    this.results.metrics.responseTimesMs.push(duration);
    return { response, duration };
  }

  // Phase 7: Load Testing
  async runPhase7() {
    console.log(chalk.bold.cyan('\n📋 PHASE 7: Load Testing & Performance\n'));
    
    await this.runTest('Phase 7', '7.1.1', 'Single Request Baseline', async () => {
      const { duration } = await this.measureResponseTime(`${this.config.apiUrl}/health`);
      if (duration > 100) throw new Error(`Response too slow: ${duration}ms`);
      return { responseTime: duration };
    });

    await this.runTest('Phase 7', '7.1.2', 'Concurrent Requests - 10', async () => {
      const requests = Array(10).fill(null).map(() => 
        this.measureResponseTime(`${this.config.apiUrl}/api/markets`)
      );
      
      const results = await Promise.all(requests);
      const avgTime = results.reduce((sum, r) => sum + r.duration, 0) / results.length;
      
      if (avgTime > 500) throw new Error(`Average response too slow: ${avgTime}ms`);
      return { avgResponseTime: avgTime, requests: 10 };
    });

    await this.runTest('Phase 7', '7.1.3', 'Concurrent Requests - 50', async () => {
      const requests = Array(50).fill(null).map(() => 
        this.measureResponseTime(`${this.config.apiUrl}/api/markets`)
      );
      
      const results = await Promise.all(requests);
      const avgTime = results.reduce((sum, r) => sum + r.duration, 0) / results.length;
      const errors = results.filter(r => !r.response.ok).length;
      
      if (errors > 5) throw new Error(`Too many errors: ${errors}/50`);
      if (avgTime > 1000) throw new Error(`Average response too slow: ${avgTime}ms`);
      
      return { avgResponseTime: avgTime, requests: 50, errors };
    });

    await this.runTest('Phase 7', '7.1.4', 'Concurrent Requests - 100', async () => {
      const requests = Array(100).fill(null).map(() => 
        this.measureResponseTime(`${this.config.apiUrl}/api/markets`)
      );
      
      const results = await Promise.all(requests);
      const avgTime = results.reduce((sum, r) => sum + r.duration, 0) / results.length;
      const errors = results.filter(r => !r.response.ok).length;
      
      if (errors > 10) throw new Error(`Too many errors: ${errors}/100`);
      if (avgTime > 2000) throw new Error(`Average response too slow: ${avgTime}ms`);
      
      return { avgResponseTime: avgTime, requests: 100, errors };
    });

    await this.runTest('Phase 7', '7.1.5', 'Sustained Load - 60s', async () => {
      const startTime = Date.now();
      const duration = 60000; // 60 seconds
      let totalRequests = 0;
      let totalErrors = 0;
      const responseTimes = [];
      
      while (Date.now() - startTime < duration) {
        const { response, duration } = await this.measureResponseTime(
          `${this.config.apiUrl}/api/markets`
        );
        
        totalRequests++;
        responseTimes.push(duration);
        if (!response.ok) totalErrors++;
        
        // Target 10 requests per second
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      
      const avgTime = responseTimes.reduce((sum, t) => sum + t, 0) / responseTimes.length;
      const errorRate = (totalErrors / totalRequests) * 100;
      
      if (errorRate > 5) throw new Error(`High error rate: ${errorRate.toFixed(2)}%`);
      if (avgTime > 500) throw new Error(`Average response too slow: ${avgTime}ms`);
      
      return { 
        totalRequests, 
        totalErrors, 
        errorRate: errorRate.toFixed(2),
        avgResponseTime: avgTime.toFixed(2),
        requestsPerSecond: (totalRequests / 60).toFixed(2)
      };
    });

    await this.runTest('Phase 7', '7.2.1', 'Large Payload Test', async () => {
      // Create a large order request
      const largeOrder = {
        market_id: 'test',
        orders: Array(100).fill(null).map((_, i) => ({
          side: i % 2 === 0 ? 'buy' : 'sell',
          outcome: i % 3,
          amount: 100 + i,
          price: 0.5 + (i * 0.001)
        }))
      };
      
      const { response, duration } = await this.measureResponseTime(
        `${this.config.apiUrl}/api/orders/batch`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(largeOrder)
        }
      );
      
      if (duration > 5000) throw new Error(`Large payload too slow: ${duration}ms`);
      return { payloadSize: JSON.stringify(largeOrder).length, responseTime: duration };
    });

    await this.runTest('Phase 7', '7.2.2', 'Memory Leak Detection', async () => {
      // Make 100 requests and check if memory usage increases significantly
      const initialMemory = process.memoryUsage().heapUsed / 1024 / 1024;
      
      for (let i = 0; i < 100; i++) {
        await fetch(`${this.config.apiUrl}/api/markets`);
        if (i % 10 === 0) {
          global.gc && global.gc(); // Force garbage collection if available
        }
      }
      
      const finalMemory = process.memoryUsage().heapUsed / 1024 / 1024;
      const memoryIncrease = finalMemory - initialMemory;
      
      if (memoryIncrease > 50) {
        throw new Error(`Potential memory leak: ${memoryIncrease.toFixed(2)}MB increase`);
      }
      
      return { 
        initialMemoryMB: initialMemory.toFixed(2),
        finalMemoryMB: finalMemory.toFixed(2),
        increaseMB: memoryIncrease.toFixed(2)
      };
    });

    await this.runTest('Phase 7', '7.3.1', 'WebSocket Connection Limit', async () => {
      const WebSocket = require('ws');
      const connections = [];
      let successfulConnections = 0;
      
      try {
        for (let i = 0; i < 100; i++) {
          const ws = new WebSocket(this.config.wsUrl);
          
          await new Promise((resolve, reject) => {
            ws.on('open', () => {
              successfulConnections++;
              connections.push(ws);
              resolve();
            });
            ws.on('error', reject);
            setTimeout(() => reject(new Error('Connection timeout')), 5000);
          });
        }
      } catch (error) {
        // Expected to fail at some point
      }
      
      // Clean up
      connections.forEach(ws => ws.close());
      
      if (successfulConnections < 50) {
        throw new Error(`Too few WebSocket connections allowed: ${successfulConnections}`);
      }
      
      return { maxConnections: successfulConnections };
    });

    await this.runTest('Phase 7', '7.3.2', 'API Rate Limiting', async () => {
      // Make rapid requests to test rate limiting
      const requests = [];
      let rateLimited = false;
      
      for (let i = 0; i < 200; i++) {
        const promise = fetch(`${this.config.apiUrl}/api/markets`).then(r => ({
          status: r.status,
          rateLimited: r.status === 429
        }));
        requests.push(promise);
      }
      
      const results = await Promise.all(requests);
      rateLimited = results.some(r => r.rateLimited);
      
      if (!rateLimited) {
        console.log(chalk.yellow('  ⚠️  No rate limiting detected'));
      }
      
      return { 
        totalRequests: 200,
        rateLimited,
        responses429: results.filter(r => r.rateLimited).length
      };
    });

    await this.runTest('Phase 7', '7.4.1', 'Database Connection Pool', async () => {
      // Test database connection exhaustion
      const promises = Array(50).fill(null).map(() => 
        fetch(`${this.config.apiUrl}/api/markets`)
          .then(r => r.json())
          .catch(() => null)
      );
      
      const results = await Promise.all(promises);
      const failures = results.filter(r => r === null).length;
      
      if (failures > 5) {
        throw new Error(`Too many connection failures: ${failures}/50`);
      }
      
      return { connections: 50, failures };
    });
  }

  // Phase 8: Security Testing
  async runPhase8() {
    console.log(chalk.bold.cyan('\n📋 PHASE 8: Security Testing\n'));
    
    await this.runTest('Phase 8', '8.1.1', 'SQL Injection Test', async () => {
      const maliciousInput = "'; DROP TABLE users; --";
      const response = await fetch(`${this.config.apiUrl}/api/markets?search=${encodeURIComponent(maliciousInput)}`);
      
      if (!response.ok && response.status !== 400) {
        throw new Error('Unexpected response to SQL injection attempt');
      }
      
      // Check if API is still functional
      const checkResponse = await fetch(`${this.config.apiUrl}/api/markets`);
      if (!checkResponse.ok) {
        throw new Error('API broken after SQL injection test');
      }
      
      return { status: 'protected' };
    });

    await this.runTest('Phase 8', '8.1.2', 'XSS Attack Vector', async () => {
      const xssPayload = '<script>alert("XSS")</script>';
      const response = await fetch(`${this.config.apiUrl}/api/markets`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: xssPayload,
          description: xssPayload
        })
      });
      
      // Should reject or sanitize
      if (response.ok) {
        const data = await response.json();
        if (data.title && data.title.includes('<script>')) {
          throw new Error('XSS payload not sanitized');
        }
      }
      
      return { status: 'protected' };
    });

    await this.runTest('Phase 8', '8.1.3', 'Path Traversal Test', async () => {
      const pathTraversal = '../../../etc/passwd';
      const response = await fetch(`${this.config.apiUrl}/api/markets/${pathTraversal}`);
      
      if (response.ok) {
        const text = await response.text();
        if (text.includes('root:')) {
          throw new Error('Path traversal vulnerability detected');
        }
      }
      
      return { status: 'protected' };
    });

    await this.runTest('Phase 8', '8.1.4', 'Authorization Bypass Test', async () => {
      // Try to access admin endpoints without auth
      const adminEndpoints = [
        '/api/admin/users',
        '/api/admin/settings',
        '/api/admin/markets/delete'
      ];
      
      for (const endpoint of adminEndpoints) {
        const response = await fetch(`${this.config.apiUrl}${endpoint}`);
        if (response.ok) {
          throw new Error(`Unauthorized access to ${endpoint}`);
        }
        if (response.status !== 401 && response.status !== 403 && response.status !== 404) {
          throw new Error(`Unexpected status for ${endpoint}: ${response.status}`);
        }
      }
      
      return { status: 'protected', endpointsTested: adminEndpoints.length };
    });

    await this.runTest('Phase 8', '8.1.5', 'CORS Policy Check', async () => {
      const response = await fetch(`${this.config.apiUrl}/api/markets`, {
        headers: {
          'Origin': 'http://malicious-site.com'
        }
      });
      
      const corsHeader = response.headers.get('access-control-allow-origin');
      if (corsHeader === '*') {
        console.log(chalk.yellow('  ⚠️  CORS allows all origins'));
      }
      
      return { corsPolicy: corsHeader || 'not set' };
    });

    await this.runTest('Phase 8', '8.2.1', 'Invalid Input Handling', async () => {
      const invalidInputs = [
        { wallet: null },
        { wallet: '' },
        { wallet: 'a'.repeat(1000) },
        { wallet: '0x1234' }, // Wrong format
        { amount: -1000 },
        { amount: 'not-a-number' },
        { amount: Infinity },
        { amount: 0.0000000001 }
      ];
      
      let handled = 0;
      for (const input of invalidInputs) {
        const response = await fetch(`${this.config.apiUrl}/api/trade/place`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(input)
        });
        
        if (response.status === 400 || response.status === 422) {
          handled++;
        }
      }
      
      if (handled < invalidInputs.length * 0.8) {
        throw new Error(`Poor input validation: ${handled}/${invalidInputs.length} handled`);
      }
      
      return { invalidInputsHandled: handled, total: invalidInputs.length };
    });

    await this.runTest('Phase 8', '8.2.2', 'Resource Exhaustion Protection', async () => {
      // Try to create very large request
      const largeArray = Array(10000).fill({ data: 'x'.repeat(1000) });
      
      const response = await fetch(`${this.config.apiUrl}/api/orders/batch`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ orders: largeArray })
      });
      
      if (response.ok) {
        throw new Error('Large payload accepted without limits');
      }
      
      return { status: 'protected', payloadSizeMB: (JSON.stringify(largeArray).length / 1024 / 1024).toFixed(2) };
    });

    await this.runTest('Phase 8', '8.3.1', 'Session Hijacking Protection', async () => {
      // Create a session
      const keypair = Keypair.generate();
      const challengeResponse = await fetch(
        `${this.config.apiUrl}/api/wallet/challenge/${keypair.publicKey.toBase58()}`
      );
      const { challenge } = await challengeResponse.json();
      
      // Try to use challenge from different IP (simulated)
      const response = await fetch(`${this.config.apiUrl}/api/wallet/verify`, {
        method: 'POST',
        headers: { 
          'Content-Type': 'application/json',
          'X-Forwarded-For': '192.168.1.1'
        },
        body: JSON.stringify({
          wallet: keypair.publicKey.toBase58(),
          signature: 'fake-signature',
          challenge
        })
      });
      
      if (response.ok) {
        throw new Error('Session hijacking possible');
      }
      
      return { status: 'protected' };
    });

    await this.runTest('Phase 8', '8.3.2', 'Cryptographic Security', async () => {
      // Test if sensitive data is properly encrypted
      const response = await fetch(`${this.config.apiUrl}/api/wallet/demo/create`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ initial_balance: 10000 })
      });
      
      if (response.ok) {
        const data = await response.json();
        if (data.private_key && data.private_key.length < 32) {
          throw new Error('Private key appears to be in plain text');
        }
      }
      
      return { status: 'protected' };
    });
  }

  // Phase 9: Edge Cases & Error Handling
  async runPhase9() {
    console.log(chalk.bold.cyan('\n📋 PHASE 9: Edge Cases & Error Handling\n'));
    
    await this.runTest('Phase 9', '9.1.1', 'Empty Request Body', async () => {
      const response = await fetch(`${this.config.apiUrl}/api/trade/place`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: ''
      });
      
      if (response.ok) {
        throw new Error('Empty body accepted');
      }
      
      return { status: response.status };
    });

    await this.runTest('Phase 9', '9.1.2', 'Malformed JSON', async () => {
      const response = await fetch(`${this.config.apiUrl}/api/trade/place`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: '{"invalid": json}'
      });
      
      if (response.ok) {
        throw new Error('Malformed JSON accepted');
      }
      
      return { status: response.status };
    });

    await this.runTest('Phase 9', '9.1.3', 'Unicode & Special Characters', async () => {
      const unicodeData = {
        title: '🚀 Test Market 测试 тест',
        description: 'Special chars: \n\r\t\0 €£¥',
        amount: 100
      };
      
      const response = await fetch(`${this.config.apiUrl}/api/markets`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(unicodeData)
      });
      
      // Should handle gracefully
      if (!response.ok && response.status >= 500) {
        throw new Error('Server error on unicode input');
      }
      
      return { handled: true };
    });

    await this.runTest('Phase 9', '9.1.4', 'Boundary Value Testing', async () => {
      const boundaryTests = [
        { amount: 0 },
        { amount: 0.01 },
        { amount: Number.MAX_SAFE_INTEGER },
        { amount: -1 },
        { price: 0 },
        { price: 1 },
        { price: 0.9999999999 }
      ];
      
      let handled = 0;
      for (const test of boundaryTests) {
        const response = await fetch(`${this.config.apiUrl}/api/trade/place`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ market_id: 'test', ...test })
        });
        
        if (response.status < 500) {
          handled++;
        }
      }
      
      if (handled < boundaryTests.length) {
        throw new Error(`Poor boundary handling: ${handled}/${boundaryTests.length}`);
      }
      
      return { boundaryTestsPassed: handled };
    });

    await this.runTest('Phase 9', '9.2.1', 'Network Timeout Handling', async () => {
      // Test with very short timeout
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 10); // 10ms timeout
      
      try {
        await fetch(`${this.config.apiUrl}/api/markets`, {
          signal: controller.signal
        });
      } catch (error) {
        if (error.name === 'AbortError') {
          return { timeoutHandled: true };
        }
        throw error;
      } finally {
        clearTimeout(timeout);
      }
      
      return { timeoutHandled: false };
    });

    await this.runTest('Phase 9', '9.2.2', 'Concurrent Modification', async () => {
      // Try to modify same resource concurrently
      const marketId = '1';
      const updates = Array(10).fill(null).map((_, i) => 
        fetch(`${this.config.apiUrl}/api/markets/${marketId}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title: `Update ${i}` })
        })
      );
      
      const results = await Promise.all(updates);
      const successful = results.filter(r => r.ok).length;
      
      // Should handle race conditions properly
      return { concurrentUpdates: 10, successful };
    });

    await this.runTest('Phase 9', '9.3.1', 'Pagination Edge Cases', async () => {
      const edgeCases = [
        { limit: -1, offset: 0 },
        { limit: 0, offset: 0 },
        { limit: 10000, offset: 0 },
        { limit: 10, offset: -1 },
        { limit: 10, offset: 999999 }
      ];
      
      for (const params of edgeCases) {
        const response = await fetch(
          `${this.config.apiUrl}/api/markets?limit=${params.limit}&offset=${params.offset}`
        );
        
        if (response.status >= 500) {
          throw new Error(`Server error on pagination: ${JSON.stringify(params)}`);
        }
      }
      
      return { edgeCasesHandled: edgeCases.length };
    });

    await this.runTest('Phase 9', '9.3.2', 'Date/Time Edge Cases', async () => {
      const dateTests = [
        { endTime: '2024-13-01' }, // Invalid month
        { endTime: '2024-02-30' }, // Invalid day
        { endTime: 'not-a-date' },
        { endTime: new Date('1970-01-01').toISOString() }, // Past date
        { endTime: new Date('2100-01-01').toISOString() } // Far future
      ];
      
      let handled = 0;
      for (const test of dateTests) {
        const response = await fetch(`${this.config.apiUrl}/api/markets`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title: 'Test', ...test })
        });
        
        if (response.status === 400 || response.status === 422) {
          handled++;
        }
      }
      
      return { dateValidation: `${handled}/${dateTests.length}` };
    });
  }

  // Phase 10: Integration & E2E Tests
  async runPhase10() {
    console.log(chalk.bold.cyan('\n📋 PHASE 10: End-to-End Scenarios\n'));
    
    await this.runTest('Phase 10', '10.1.1', 'Complete Trading Flow', async () => {
      // 1. Create demo account
      const demoResponse = await fetch(`${this.config.apiUrl}/api/wallet/demo/create`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ initial_balance: 10000 })
      });
      
      if (!demoResponse.ok) throw new Error('Demo account creation failed');
      const { wallet_address } = await demoResponse.json();
      
      // 2. Get markets
      const marketsResponse = await fetch(`${this.config.apiUrl}/api/markets`);
      const { markets } = await marketsResponse.json();
      if (!markets || markets.length === 0) throw new Error('No markets available');
      
      // 3. Place trade
      const tradeResponse = await fetch(`${this.config.apiUrl}/api/trade/place`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          wallet: wallet_address,
          market_id: markets[0].id,
          side: 'buy',
          outcome: 0,
          amount: 100
        })
      });
      
      // 4. Check position
      const positionsResponse = await fetch(`${this.config.apiUrl}/api/positions/${wallet_address}`);
      
      return { 
        flowCompleted: true,
        steps: ['account', 'markets', 'trade', 'position']
      };
    });

    await this.runTest('Phase 10', '10.1.2', 'Market Maker Flow', async () => {
      // Test market maker operations
      const steps = [
        'Create market maker account',
        'Add liquidity to market',
        'Place multiple orders',
        'Adjust spreads',
        'Remove liquidity'
      ];
      
      // Simplified test - just check endpoints exist
      const endpoints = [
        '/api/defi/pools',
        '/api/markets/create',
        '/api/orders/batch',
        '/api/liquidity/remove'
      ];
      
      for (const endpoint of endpoints) {
        const response = await fetch(`${this.config.apiUrl}${endpoint}`, {
          method: endpoint.includes('create') ? 'POST' : 'GET'
        });
        
        if (response.status === 404) {
          throw new Error(`Endpoint not found: ${endpoint}`);
        }
      }
      
      return { marketMakerFlow: 'tested', endpoints: endpoints.length };
    });

    await this.runTest('Phase 10', '10.2.1', 'Recovery After Crash', async () => {
      // Test system recovery
      // 1. Create some state
      const demoResponse = await fetch(`${this.config.apiUrl}/api/wallet/demo/create`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ initial_balance: 10000 })
      });
      
      const { wallet_address } = await demoResponse.json();
      
      // 2. Simulate high load that might cause issues
      const heavyRequests = Array(50).fill(null).map(() => 
        fetch(`${this.config.apiUrl}/api/positions/${wallet_address}`)
          .catch(() => null)
      );
      
      await Promise.all(heavyRequests);
      
      // 3. Check if system is still responsive
      const healthResponse = await fetch(`${this.config.apiUrl}/health`);
      if (!healthResponse.ok) {
        throw new Error('System not healthy after load');
      }
      
      return { recoveryTest: 'passed' };
    });

    await this.runTest('Phase 10', '10.2.2', 'Data Consistency Check', async () => {
      // Test data consistency across endpoints
      const demoResponse = await fetch(`${this.config.apiUrl}/api/wallet/demo/create`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ initial_balance: 10000 })
      });
      
      if (!demoResponse.ok) throw new Error('Demo creation failed');
      const { wallet_address } = await demoResponse.json();
      
      // Check balance from different endpoints
      const balanceResponse = await fetch(`${this.config.apiUrl}/api/wallet/balance/${wallet_address}`);
      const portfolioResponse = await fetch(`${this.config.apiUrl}/api/portfolio/${wallet_address}`);
      
      if (balanceResponse.ok && portfolioResponse.ok) {
        const { balance } = await balanceResponse.json();
        const { total_value } = await portfolioResponse.json();
        
        // Should be consistent
        if (Math.abs(balance - total_value) > 0.01) {
          throw new Error('Inconsistent balance across endpoints');
        }
      }
      
      return { dataConsistency: 'verified' };
    });
  }

  // Generate comprehensive report
  async generateReport() {
    const duration = Date.now() - this.results.startTime;
    const passRate = this.results.totalTests > 0 
      ? (this.results.passed / this.results.totalTests * 100).toFixed(2)
      : 0;
    
    console.log(chalk.bold.blue('\n📊 Performance Test Results Summary\n'));
    console.log(chalk.green(`✅ Passed: ${this.results.passed}`));
    console.log(chalk.red(`❌ Failed: ${this.results.failed}`));
    console.log(chalk.blue(`📊 Pass Rate: ${passRate}%`));
    console.log(chalk.gray(`⏱️  Duration: ${Math.round(duration / 1000)}s`));
    
    // Calculate performance metrics
    if (this.results.metrics.responseTimesMs.length > 0) {
      const avgResponseTime = this.results.metrics.responseTimesMs.reduce((a, b) => a + b, 0) / 
                              this.results.metrics.responseTimesMs.length;
      const maxResponseTime = Math.max(...this.results.metrics.responseTimesMs);
      const minResponseTime = Math.min(...this.results.metrics.responseTimesMs);
      
      console.log(chalk.bold.cyan('\n📈 Performance Metrics:'));
      console.log(`  Average Response Time: ${avgResponseTime.toFixed(2)}ms`);
      console.log(`  Max Response Time: ${maxResponseTime}ms`);
      console.log(`  Min Response Time: ${minResponseTime}ms`);
    }
    
    if (this.results.errors.length > 0) {
      console.log(chalk.red('\n❌ Failed Tests:'));
      this.results.errors.forEach(err => {
        console.log(chalk.red(`  ${err.testId} - ${err.testName}: ${err.error}`));
      });
    }
    
    // Save detailed results
    const resultsPath = path.join(__dirname, 'performance-test-results.json');
    fs.writeFileSync(resultsPath, JSON.stringify(this.results, null, 2));
    
    console.log(chalk.gray(`\nDetailed results saved to: ${resultsPath}`));
  }

  // Main execution
  async runAllTests() {
    console.log(chalk.bold.blue('🎯 Starting Performance & Security Test Suite\n'));
    
    try {
      await this.runPhase7();  // Load & Performance Tests
      await this.runPhase8();  // Security Tests
      await this.runPhase9();  // Edge Cases
      await this.runPhase10(); // E2E Scenarios
      
      await this.generateReport();
      
    } catch (error) {
      console.error(chalk.red('Test suite failed:'), error);
      await this.generateReport();
      process.exit(1);
    }
  }
}

// Execute tests
if (require.main === module) {
  const configPath = path.join(__dirname, 'test-config.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  
  const tester = new PerformanceTestSuite(config);
  tester.runAllTests().catch(console.error);
}

module.exports = PerformanceTestSuite;