#!/usr/bin/env node

/**
 * Performance Test Suite for Betting Platform
 * Tests API response times, throughput, and WebSocket performance
 */

const http = require('http');

const API_BASE = 'http://localhost:8081';

// Performance test configuration
const perfTests = {
  markets: {
    endpoint: '/api/markets',
    concurrent: [1, 10, 50, 100],
    iterations: 100
  },
  verses: {
    endpoint: '/api/verses',
    concurrent: [1, 10, 50],
    iterations: 100
  },
  walletCreation: {
    endpoint: '/api/wallet/demo/create',
    method: 'POST',
    body: JSON.stringify({ name: 'Perf Test User' }),
    concurrent: [1, 5, 10],
    iterations: 50
  }
};

// Helper to make HTTP request
function makeRequest(path, options = {}) {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    
    const req = http.request(`${API_BASE}${path}`, {
      method: options.method || 'GET',
      headers: options.headers || {}
    }, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        const duration = Date.now() - start;
        resolve({
          status: res.statusCode,
          duration,
          size: data.length
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

// Run concurrent requests
async function runConcurrentTest(endpoint, concurrent, iterations, options = {}) {
  const results = [];
  const errors = [];
  
  console.log(`Testing ${endpoint} with ${concurrent} concurrent requests...`);
  
  const totalRequests = concurrent * iterations;
  let completed = 0;
  
  const startTime = Date.now();
  
  // Create batches of concurrent requests
  for (let i = 0; i < iterations; i++) {
    const batch = [];
    
    for (let j = 0; j < concurrent; j++) {
      batch.push(
        makeRequest(endpoint, options)
          .then(result => {
            results.push(result);
            completed++;
            if (completed % 100 === 0) {
              process.stdout.write(`\r  Progress: ${completed}/${totalRequests}`);
            }
          })
          .catch(error => {
            errors.push(error);
            completed++;
          })
      );
    }
    
    await Promise.all(batch);
  }
  
  const totalDuration = Date.now() - startTime;
  
  console.log(`\r  Completed: ${completed}/${totalRequests}`);
  
  // Calculate statistics
  const durations = results.map(r => r.duration).sort((a, b) => a - b);
  const avgDuration = durations.reduce((a, b) => a + b, 0) / durations.length;
  const p50 = durations[Math.floor(durations.length * 0.5)];
  const p95 = durations[Math.floor(durations.length * 0.95)];
  const p99 = durations[Math.floor(durations.length * 0.99)];
  const minDuration = Math.min(...durations);
  const maxDuration = Math.max(...durations);
  
  const rps = (completed / totalDuration) * 1000;
  
  return {
    endpoint,
    concurrent,
    totalRequests,
    successfulRequests: results.length,
    failedRequests: errors.length,
    avgResponseTime: avgDuration.toFixed(2),
    p50ResponseTime: p50,
    p95ResponseTime: p95,
    p99ResponseTime: p99,
    minResponseTime: minDuration,
    maxResponseTime: maxDuration,
    requestsPerSecond: rps.toFixed(2),
    totalDuration: (totalDuration / 1000).toFixed(2)
  };
}

// Format results as table
function printResults(results) {
  console.log('\n┌─────────────────────────────────────────────────────────────────────────┐');
  console.log('│                          Performance Test Results                         │');
  console.log('├─────────────────────────────────────────────────────────────────────────┤');
  console.log('│ Endpoint            │ Conc │ RPS    │ Avg    │ P50   │ P95   │ P99    │');
  console.log('├─────────────────────────────────────────────────────────────────────────┤');
  
  results.forEach(r => {
    const endpoint = r.endpoint.padEnd(20).substring(0, 20);
    const conc = r.concurrent.toString().padEnd(5);
    const rps = r.requestsPerSecond.padEnd(7);
    const avg = `${r.avgResponseTime}ms`.padEnd(7);
    const p50 = `${r.p50ResponseTime}ms`.padEnd(6);
    const p95 = `${r.p95ResponseTime}ms`.padEnd(6);
    const p99 = `${r.p99ResponseTime}ms`.padEnd(7);
    
    console.log(`│ ${endpoint}│ ${conc}│ ${rps}│ ${avg}│ ${p50}│ ${p95}│ ${p99}│`);
  });
  
  console.log('└─────────────────────────────────────────────────────────────────────────┘');
}

// Main test runner
async function runPerformanceTests() {
  console.log('🚀 Starting Performance Tests\n');
  
  const allResults = [];
  
  // Test each endpoint
  for (const [name, config] of Object.entries(perfTests)) {
    console.log(`\n📊 Testing ${name}:`);
    
    for (const concurrent of config.concurrent) {
      const options = {};
      
      if (config.method) {
        options.method = config.method;
        options.headers = { 'Content-Type': 'application/json' };
        options.body = config.body;
      }
      
      const result = await runConcurrentTest(
        config.endpoint,
        concurrent,
        config.iterations,
        options
      );
      
      allResults.push(result);
      
      // Add delay between tests to let server recover
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }
  
  // Print summary
  printResults(allResults);
  
  // Performance analysis
  console.log('\n📈 Performance Analysis:');
  
  const marketsResults = allResults.filter(r => r.endpoint === '/api/markets');
  const bestMarketRPS = Math.max(...marketsResults.map(r => parseFloat(r.requestsPerSecond)));
  
  console.log(`\n✅ API can handle ${bestMarketRPS} requests/second for market data`);
  
  // Check for performance degradation
  const degradation = marketsResults.map(r => ({
    concurrent: r.concurrent,
    avgTime: parseFloat(r.avgResponseTime)
  }));
  
  const singleThreadTime = degradation.find(d => d.concurrent === 1)?.avgTime || 0;
  const highConcurrencyTime = degradation[degradation.length - 1]?.avgTime || 0;
  
  if (highConcurrencyTime > singleThreadTime * 10) {
    console.log('⚠️  Significant performance degradation under high load detected');
  } else {
    console.log('✅ Performance scales well under concurrent load');
  }
  
  // Check response time targets
  const allP95s = allResults.map(r => r.p95ResponseTime);
  const maxP95 = Math.max(...allP95s);
  
  if (maxP95 < 100) {
    console.log('✅ Excellent response times - P95 under 100ms');
  } else if (maxP95 < 500) {
    console.log('✅ Good response times - P95 under 500ms');
  } else {
    console.log('⚠️  Some endpoints have high P95 response times');
  }
}

// Run tests
runPerformanceTests().catch(error => {
  console.error('Performance test error:', error);
  process.exit(1);
});