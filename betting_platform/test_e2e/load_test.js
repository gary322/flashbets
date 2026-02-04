import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';
import { randomIntBetween } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

// Custom metrics
const errorRate = new Rate('errors');
const marketLatency = new Trend('market_latency');
const tradeLatency = new Trend('trade_latency');
const positionLatency = new Trend('position_latency');
const successfulTrades = new Counter('successful_trades');
const failedTrades = new Counter('failed_trades');

// Test configuration for 1000+ users
export const options = {
  stages: [
    // Warm-up
    { duration: '1m', target: 50 },
    
    // Gradual ramp to 1000 users
    { duration: '2m', target: 100 },
    { duration: '2m', target: 250 },
    { duration: '2m', target: 500 },
    { duration: '2m', target: 750 },
    { duration: '2m', target: 1000 },
    
    // Sustained load at 1000+ users
    { duration: '10m', target: 1000 },
    
    // Spike test to 1500 users
    { duration: '2m', target: 1500 },
    { duration: '5m', target: 1500 },
    
    // Cool down
    { duration: '3m', target: 500 },
    { duration: '2m', target: 100 },
    { duration: '1m', target: 0 },
  ],
  
  thresholds: {
    // Response time thresholds
    'http_req_duration': [
      'p(50)<500',  // 50% of requests under 500ms
      'p(95)<2000', // 95% of requests under 2s
      'p(99)<5000', // 99% of requests under 5s
    ],
    
    // Error rate thresholds
    'http_req_failed': ['rate<0.05'], // Less than 5% errors
    'errors': ['rate<0.05'],
    
    // Custom metric thresholds
    'market_latency': ['p(95)<1000'],
    'trade_latency': ['p(95)<3000'],
    'position_latency': ['p(95)<1500'],
  },
  
  // Extended options for better performance
  batch: 20,
  batchPerHost: 5,
  httpDebug: 'false',
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8081';

// Helper functions
function generateWallet() {
  return `load-test-wallet-${__VU}-${randomIntBetween(1000, 9999)}`;
}

function selectRandomMarket(markets) {
  if (!markets || markets.length === 0) return null;
  return markets[randomIntBetween(0, markets.length - 1)];
}

// Test scenarios
export default function() {
  const wallet = generateWallet();
  
  // Scenario weights (adjust based on realistic usage)
  const scenario = randomIntBetween(1, 100);
  
  if (scenario <= 30) {
    // 30% - Browse markets
    browseMarkets();
  } else if (scenario <= 50) {
    // 20% - Place trades
    placeTrade(wallet);
  } else if (scenario <= 70) {
    // 20% - Check positions
    checkPositions(wallet);
  } else if (scenario <= 85) {
    // 15% - Complex trading (leveraged)
    leveragedTrading(wallet);
  } else if (scenario <= 95) {
    // 10% - Portfolio operations
    portfolioOperations(wallet);
  } else {
    // 5% - Heavy operations (quantum, DeFi)
    heavyOperations(wallet);
  }
  
  // Realistic think time between actions
  sleep(randomIntBetween(1, 5));
}

function browseMarkets() {
  group('Browse Markets', () => {
    const start = new Date();
    const res = http.get(`${BASE_URL}/api/markets`, {
      tags: { name: 'GetMarkets' },
    });
    
    const latency = new Date() - start;
    marketLatency.add(latency);
    
    const success = check(res, {
      'markets status is 200': (r) => r.status === 200,
      'markets returned': (r) => JSON.parse(r.body).length > 0,
    });
    
    errorRate.add(!success);
    
    // Randomly view market details
    if (success && randomIntBetween(1, 10) <= 3) {
      const markets = JSON.parse(res.body);
      const market = selectRandomMarket(markets);
      if (market) {
        http.get(`${BASE_URL}/api/markets/${market.id}`, {
          tags: { name: 'GetMarketDetail' },
        });
      }
    }
  });
}

function placeTrade(wallet) {
  group('Place Trade', () => {
    // First get markets
    const marketsRes = http.get(`${BASE_URL}/api/markets`);
    if (marketsRes.status !== 200) {
      failedTrades.add(1);
      return;
    }
    
    const markets = JSON.parse(marketsRes.body);
    const market = selectRandomMarket(markets);
    if (!market) {
      failedTrades.add(1);
      return;
    }
    
    // Place trade
    const payload = JSON.stringify({
      wallet: wallet,
      market_id: market.id,
      outcome: randomIntBetween(0, market.outcomes.length - 1),
      amount: randomIntBetween(100000, 10000000), // 0.1 to 10 SOL
      leverage: 1,
    });
    
    const start = new Date();
    const res = http.post(`${BASE_URL}/api/trade/place`, payload, {
      headers: { 'Content-Type': 'application/json' },
      tags: { name: 'PlaceTrade' },
    });
    
    const latency = new Date() - start;
    tradeLatency.add(latency);
    
    const success = check(res, {
      'trade status is 200': (r) => r.status === 200,
      'trade has signature': (r) => JSON.parse(r.body).signature !== undefined,
    });
    
    if (success) {
      successfulTrades.add(1);
    } else {
      failedTrades.add(1);
    }
    
    errorRate.add(!success);
  });
}

function checkPositions(wallet) {
  group('Check Positions', () => {
    const start = new Date();
    const res = http.get(`${BASE_URL}/api/positions/${wallet}`, {
      tags: { name: 'GetPositions' },
    });
    
    const latency = new Date() - start;
    positionLatency.add(latency);
    
    const success = check(res, {
      'positions status is 200': (r) => r.status === 200,
      'positions is array': (r) => Array.isArray(JSON.parse(r.body)),
    });
    
    errorRate.add(!success);
    
    // Check portfolio if positions exist
    if (success && randomIntBetween(1, 10) <= 5) {
      http.get(`${BASE_URL}/api/portfolio/${wallet}`, {
        tags: { name: 'GetPortfolio' },
      });
    }
  });
}

function leveragedTrading(wallet) {
  group('Leveraged Trading', () => {
    // Get markets first
    const marketsRes = http.get(`${BASE_URL}/api/markets`);
    if (marketsRes.status !== 200) return;
    
    const markets = JSON.parse(marketsRes.body);
    const market = selectRandomMarket(markets);
    if (!market) return;
    
    // Place leveraged trade
    const leverage = randomIntBetween(2, 10);
    const payload = JSON.stringify({
      wallet: wallet,
      market_id: market.id,
      outcome: 0,
      amount: randomIntBetween(50000, 1000000),
      leverage: leverage,
    });
    
    const res = http.post(`${BASE_URL}/api/trade/place`, payload, {
      headers: { 'Content-Type': 'application/json' },
      tags: { name: 'PlaceLeveragedTrade' },
    });
    
    const success = check(res, {
      'leveraged trade successful': (r) => r.status === 200,
    });
    
    if (success) {
      successfulTrades.add(1);
      
      // Check risk metrics
      http.get(`${BASE_URL}/api/risk/${wallet}`, {
        tags: { name: 'GetRiskMetrics' },
      });
    } else {
      failedTrades.add(1);
    }
  });
}

function portfolioOperations(wallet) {
  group('Portfolio Operations', () => {
    const batch = http.batch([
      ['GET', `${BASE_URL}/api/portfolio/${wallet}`, null, { tags: { name: 'GetPortfolio' } }],
      ['GET', `${BASE_URL}/api/positions/${wallet}`, null, { tags: { name: 'GetPositions' } }],
      ['GET', `${BASE_URL}/api/wallet/balance/${wallet}`, null, { tags: { name: 'GetBalance' } }],
    ]);
    
    batch.forEach(res => {
      check(res, {
        'portfolio request successful': (r) => r.status === 200,
      });
    });
  });
}

function heavyOperations(wallet) {
  group('Heavy Operations', () => {
    // Randomly choose between quantum and DeFi operations
    if (randomIntBetween(1, 2) === 1) {
      // Quantum operations
      const quantumPayload = JSON.stringify({
        wallet: wallet,
        market_id: 1,
        amount: 1000000,
        num_outcomes: 2,
        entanglement_level: 1,
      });
      
      const res = http.post(`${BASE_URL}/api/quantum/create`, quantumPayload, {
        headers: { 'Content-Type': 'application/json' },
        tags: { name: 'CreateQuantumPosition' },
      });
      
      check(res, {
        'quantum position created': (r) => r.status === 200,
      });
    } else {
      // DeFi operations
      const stakePayload = JSON.stringify({
        wallet: wallet,
        amount: 5000000,
        duration: 30,
      });
      
      const res = http.post(`${BASE_URL}/api/defi/stake`, stakePayload, {
        headers: { 'Content-Type': 'application/json' },
        tags: { name: 'StakeTokens' },
      });
      
      check(res, {
        'stake successful': (r) => r.status === 200,
      });
    }
  });
}

// Handle summary output
export function handleSummary(data) {
  return {
    'stdout': textSummary(data, { indent: ' ', enableColors: true }),
    './results/load_test/summary.json': JSON.stringify(data),
    './results/load_test/summary.html': htmlReport(data),
  };
}

// Custom summary function
function textSummary(data, options) {
  const { metrics } = data;
  const errorRateValue = metrics.errors ? metrics.errors.rate : 0;
  const successRate = (1 - errorRateValue) * 100;
  
  return `
Load Test Summary
=================
Total VUs: ${metrics.vus ? metrics.vus.value : 0}
Duration: ${metrics.iteration_duration ? (metrics.iteration_duration.avg / 1000).toFixed(2) : 0}s avg

Success Rate: ${successRate.toFixed(2)}%
Total Requests: ${metrics.http_reqs ? metrics.http_reqs.count : 0}
Failed Requests: ${metrics.http_req_failed ? metrics.http_req_failed.passes : 0}

Response Times:
- Median: ${metrics.http_req_duration ? metrics.http_req_duration.med.toFixed(2) : 0}ms
- 95th percentile: ${metrics.http_req_duration ? metrics.http_req_duration['p(95)'].toFixed(2) : 0}ms
- 99th percentile: ${metrics.http_req_duration ? metrics.http_req_duration['p(99)'].toFixed(2) : 0}ms

Custom Metrics:
- Successful Trades: ${metrics.successful_trades ? metrics.successful_trades.count : 0}
- Failed Trades: ${metrics.failed_trades ? metrics.failed_trades.count : 0}
- Market Latency (p95): ${metrics.market_latency ? metrics.market_latency['p(95)'].toFixed(2) : 0}ms
- Trade Latency (p95): ${metrics.trade_latency ? metrics.trade_latency['p(95)'].toFixed(2) : 0}ms
`;
}

// HTML report generator
function htmlReport(data) {
  return `<!DOCTYPE html>
<html>
<head>
    <title>Load Test Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .metric { margin: 10px 0; padding: 10px; background: #f0f0f0; }
        .success { color: green; }
        .warning { color: orange; }
        .error { color: red; }
    </style>
</head>
<body>
    <h1>Load Test Report - 1000+ Users</h1>
    <div class="metric">
        <h3>Test Summary</h3>
        <p>Duration: ${new Date(data.state.testRunDurationMs).toISOString()}</p>
        <p>Max VUs: ${data.metrics.vus ? data.metrics.vus.max : 0}</p>
    </div>
    <div class="metric">
        <h3>Performance Metrics</h3>
        <p>Total Requests: ${data.metrics.http_reqs ? data.metrics.http_reqs.count : 0}</p>
        <p>Request Rate: ${data.metrics.http_reqs ? (data.metrics.http_reqs.rate).toFixed(2) : 0} req/s</p>
        <p>Success Rate: ${((1 - (data.metrics.errors ? data.metrics.errors.rate : 0)) * 100).toFixed(2)}%</p>
    </div>
</body>
</html>`;
}
