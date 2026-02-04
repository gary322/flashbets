import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');

// Test configuration
export const options = {
  stages: [
    { duration: '2m', target: 100 },   // Ramp up to 100 users
    { duration: '5m', target: 100 },   // Stay at 100 users
    { duration: '2m', target: 500 },   // Ramp up to 500 users
    { duration: '5m', target: 500 },   // Stay at 500 users
    { duration: '2m', target: 1000 },  // Ramp up to 1000 users
    { duration: '10m', target: 1000 }, // Stay at 1000 users
    { duration: '5m', target: 0 },     // Ramp down to 0 users
  ],
  thresholds: {
    http_req_duration: ['p(95)<2000'], // 95% of requests must complete below 2s
    http_req_failed: ['rate<0.1'],     // Error rate must be below 10%
    errors: ['rate<0.1'],              // Custom error rate must be below 10%
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8081';

// Test scenarios
const scenarios = [
  { name: 'health_check', weight: 0.1 },
  { name: 'get_markets', weight: 0.3 },
  { name: 'get_market_detail', weight: 0.2 },
  { name: 'place_trade', weight: 0.2 },
  { name: 'get_positions', weight: 0.1 },
  { name: 'get_portfolio', weight: 0.1 },
];

// Helper to select scenario based on weights
function selectScenario() {
  const random = Math.random();
  let cumulative = 0;
  
  for (const scenario of scenarios) {
    cumulative += scenario.weight;
    if (random < cumulative) {
      return scenario.name;
    }
  }
  
  return scenarios[0].name;
}

// Generate demo wallet
function generateWallet() {
  return `demo-wallet-${Math.floor(Math.random() * 10000)}`;
}

// Test functions
function healthCheck() {
  const res = http.get(`${BASE_URL}/health`);
  check(res, {
    'health check status is 200': (r) => r.status === 200,
    'health check has status field': (r) => JSON.parse(r.body).status === 'ok',
  });
  errorRate.add(res.status !== 200);
}

function getMarkets() {
  const res = http.get(`${BASE_URL}/api/markets`);
  check(res, {
    'markets status is 200': (r) => r.status === 200,
    'markets response is array': (r) => Array.isArray(JSON.parse(r.body)),
  });
  errorRate.add(res.status !== 200);
  return res;
}

function getMarketDetail() {
  // First get markets
  const marketsRes = http.get(`${BASE_URL}/api/markets`);
  if (marketsRes.status === 200) {
    const markets = JSON.parse(marketsRes.body);
    if (markets.length > 0) {
      const marketId = markets[Math.floor(Math.random() * markets.length)].id;
      const res = http.get(`${BASE_URL}/api/markets/${marketId}`);
      check(res, {
        'market detail status is 200': (r) => r.status === 200,
        'market detail has id': (r) => JSON.parse(r.body).id !== undefined,
      });
      errorRate.add(res.status !== 200);
    }
  }
}

function placeTrade(wallet) {
  const payload = JSON.stringify({
    wallet: wallet,
    market_id: 1,
    outcome: 0,
    amount: 100000,
    leverage: 1,
  });
  
  const params = {
    headers: { 'Content-Type': 'application/json' },
  };
  
  const res = http.post(`${BASE_URL}/api/trade/place`, payload, params);
  check(res, {
    'trade status is 200': (r) => r.status === 200,
    'trade has signature': (r) => JSON.parse(r.body).signature !== undefined,
  });
  errorRate.add(res.status !== 200);
}

function getPositions(wallet) {
  const res = http.get(`${BASE_URL}/api/positions/${wallet}`);
  check(res, {
    'positions status is 200': (r) => r.status === 200,
    'positions response is array': (r) => Array.isArray(JSON.parse(r.body)),
  });
  errorRate.add(res.status !== 200);
}

function getPortfolio(wallet) {
  const res = http.get(`${BASE_URL}/api/portfolio/${wallet}`);
  check(res, {
    'portfolio status is 200': (r) => r.status === 200,
    'portfolio has balance': (r) => JSON.parse(r.body).balance !== undefined,
  });
  errorRate.add(res.status !== 200);
}

// Main test function
export default function() {
  const wallet = generateWallet();
  const scenario = selectScenario();
  
  switch (scenario) {
    case 'health_check':
      healthCheck();
      break;
    case 'get_markets':
      getMarkets();
      break;
    case 'get_market_detail':
      getMarketDetail();
      break;
    case 'place_trade':
      placeTrade(wallet);
      break;
    case 'get_positions':
      getPositions(wallet);
      break;
    case 'get_portfolio':
      getPortfolio(wallet);
      break;
  }
  
  // Random sleep between 1-3 seconds
  sleep(Math.random() * 2 + 1);
}