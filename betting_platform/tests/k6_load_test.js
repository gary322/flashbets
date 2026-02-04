/**
 * K6 Load Test Script for Betting Platform API
 * Tests API endpoints under various load conditions
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { randomIntBetween, randomItem } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

// Custom metrics
const errorRate = new Rate('errors');
const marketSearchDuration = new Trend('market_search_duration');
const tradePlacementDuration = new Trend('trade_placement_duration');
const positionFetchDuration = new Trend('position_fetch_duration');

// Test configuration
export let options = {
    scenarios: {
        // Scenario 1: Gradual ramp-up
        gradual_load: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '2m', target: 100 },   // Ramp up to 100 users
                { duration: '5m', target: 100 },   // Stay at 100 users
                { duration: '2m', target: 200 },   // Ramp up to 200 users
                { duration: '5m', target: 200 },   // Stay at 200 users
                { duration: '2m', target: 0 },     // Ramp down to 0
            ],
            gracefulRampDown: '30s',
            exec: 'normalUserBehavior',
        },
        
        // Scenario 2: Spike test
        spike_test: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '30s', target: 50 },    // Warm up
                { duration: '1m', target: 50 },     // Stay at 50
                { duration: '30s', target: 1000 },  // Spike to 1000 users
                { duration: '3m', target: 1000 },   // Stay at 1000
                { duration: '30s', target: 50 },    // Back to normal
                { duration: '2m', target: 50 },     // Stay at normal
                { duration: '30s', target: 0 },     // Ramp down
            ],
            gracefulRampDown: '30s',
            exec: 'spikeUserBehavior',
            startTime: '16m', // Start after gradual load test
        },
        
        // Scenario 3: Stress test
        stress_test: {
            executor: 'ramping-arrival-rate',
            startRate: 50,
            timeUnit: '1s',
            preAllocatedVUs: 500,
            maxVUs: 2000,
            stages: [
                { duration: '2m', target: 300 },    // Ramp up to 300 req/s
                { duration: '5m', target: 300 },    // Stay at 300 req/s
                { duration: '2m', target: 600 },    // Ramp up to 600 req/s
                { duration: '5m', target: 600 },    // Stay at 600 req/s
                { duration: '2m', target: 1000 },   // Push to 1000 req/s
                { duration: '3m', target: 1000 },   // Stay at 1000 req/s
                { duration: '2m', target: 0 },      // Ramp down
            ],
            exec: 'stressTestBehavior',
            startTime: '32m', // Start after spike test
        },
    },
    
    thresholds: {
        http_req_duration: ['p(95)<500', 'p(99)<1000'], // 95% under 500ms, 99% under 1s
        http_req_failed: ['rate<0.1'],                   // Error rate under 10%
        errors: ['rate<0.1'],                            // Custom error rate under 10%
        market_search_duration: ['p(95)<400'],           // Market search 95% under 400ms
        trade_placement_duration: ['p(95)<800'],         // Trade placement 95% under 800ms
        position_fetch_duration: ['p(95)<300'],          // Position fetch 95% under 300ms
    },
};

const BASE_URL = 'http://localhost:8081/api';

// Test data
const searchTerms = ['bitcoin', 'ethereum', 'trump', 'election', 'crypto', 'sports', 'finance'];
const marketIds = [1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, 1008, 1009];
const wallets = Array.from({ length: 100 }, (_, i) => `demo_wallet_load_test_${i}`);

// Helper functions
function generateTradePayload() {
    return {
        market_id: randomItem(marketIds),
        outcome: randomIntBetween(0, 1),
        amount: randomIntBetween(10, 1000),
        wallet: randomItem(wallets),
        leverage: randomIntBetween(1, 20),
        order_type: randomItem(['market', 'limit']),
        price: Math.random() * 0.8 + 0.1, // Random price between 0.1 and 0.9
    };
}

// Normal user behavior scenario
export function normalUserBehavior() {
    // 1. Search for markets
    const searchTerm = randomItem(searchTerms);
    const searchStart = Date.now();
    const searchRes = http.get(`${BASE_URL}/markets?search=${searchTerm}&limit=10`);
    marketSearchDuration.add(Date.now() - searchStart);
    
    check(searchRes, {
        'market search status is 200': (r) => r.status === 200,
        'market search returns results': (r) => {
            const body = JSON.parse(r.body);
            return body.markets && body.markets.length > 0;
        },
    }) || errorRate.add(1);
    
    sleep(randomIntBetween(1, 3)); // User thinks
    
    // 2. Get market details
    const marketId = randomItem(marketIds);
    const detailsRes = http.get(`${BASE_URL}/markets/${marketId}`);
    
    check(detailsRes, {
        'market details status is 200': (r) => r.status === 200,
    }) || errorRate.add(1);
    
    sleep(randomIntBetween(2, 5)); // User reads market
    
    // 3. Place a trade
    const tradePayload = generateTradePayload();
    const tradeStart = Date.now();
    const tradeRes = http.post(`${BASE_URL}/trades`, JSON.stringify(tradePayload), {
        headers: { 'Content-Type': 'application/json' },
    });
    tradePlacementDuration.add(Date.now() - tradeStart);
    
    check(tradeRes, {
        'trade placement status is 200': (r) => r.status === 200,
        'trade returns signature': (r) => {
            const body = JSON.parse(r.body);
            return body.signature !== undefined;
        },
    }) || errorRate.add(1);
    
    sleep(randomIntBetween(1, 2));
    
    // 4. Check positions
    const positionStart = Date.now();
    const positionsRes = http.get(`${BASE_URL}/positions?wallet=${tradePayload.wallet}`);
    positionFetchDuration.add(Date.now() - positionStart);
    
    check(positionsRes, {
        'positions fetch status is 200': (r) => r.status === 200,
    }) || errorRate.add(1);
    
    sleep(randomIntBetween(3, 8)); // User monitors position
}

// Spike user behavior (aggressive trading)
export function spikeUserBehavior() {
    const wallet = randomItem(wallets);
    
    // Rapid fire trades
    for (let i = 0; i < 5; i++) {
        const tradePayload = generateTradePayload();
        tradePayload.wallet = wallet;
        
        const tradeRes = http.post(`${BASE_URL}/trades`, JSON.stringify(tradePayload), {
            headers: { 'Content-Type': 'application/json' },
        });
        
        check(tradeRes, {
            'spike trade status is 200': (r) => r.status === 200,
        }) || errorRate.add(1);
        
        sleep(0.1); // Very short delay
    }
    
    // Check all positions
    const positionsRes = http.get(`${BASE_URL}/positions?wallet=${wallet}`);
    check(positionsRes, {
        'spike positions status is 200': (r) => r.status === 200,
    }) || errorRate.add(1);
}

// Stress test behavior (maximum load)
export function stressTestBehavior() {
    const requests = [
        // Market operations
        () => http.get(`${BASE_URL}/markets?limit=50`),
        () => http.get(`${BASE_URL}/markets?search=${randomItem(searchTerms)}`),
        () => http.get(`${BASE_URL}/markets/${randomItem(marketIds)}`),
        
        // Trading operations
        () => http.post(`${BASE_URL}/trades`, JSON.stringify(generateTradePayload()), {
            headers: { 'Content-Type': 'application/json' },
        }),
        
        // Position operations
        () => http.get(`${BASE_URL}/positions?wallet=${randomItem(wallets)}`),
        
        // Risk operations
        () => http.get(`${BASE_URL}/risk/metrics?wallet=${randomItem(wallets)}`),
        
        // Verse operations
        () => http.get(`${BASE_URL}/verses?limit=20`),
    ];
    
    // Execute random request
    const request = randomItem(requests);
    const res = request();
    
    check(res, {
        'stress test status < 500': (r) => r.status < 500,
    }) || errorRate.add(1);
}

// Handle summary
export function handleSummary(data) {
    return {
        'stdout': textSummary(data, { indent: ' ', enableColors: true }),
        'summary.json': JSON.stringify(data),
        'summary.html': htmlReport(data),
    };
}

// Text summary helper
function textSummary(data, options) {
    const { indent = '', enableColors = false } = options;
    const color = enableColors ? {
        green: '\x1b[32m',
        red: '\x1b[31m',
        yellow: '\x1b[33m',
        reset: '\x1b[0m',
    } : {
        green: '',
        red: '',
        yellow: '',
        reset: '',
    };
    
    let summary = `${indent}${color.yellow}=== Load Test Summary ===${color.reset}\n\n`;
    
    // Scenarios
    summary += `${indent}Scenarios:\n`;
    Object.entries(data.scenarios || {}).forEach(([name, scenario]) => {
        const status = scenario.passes > 0 ? `${color.green}✓${color.reset}` : `${color.red}✗${color.reset}`;
        summary += `${indent}  ${status} ${name}\n`;
    });
    
    // Key metrics
    summary += `\n${indent}Key Metrics:\n`;
    if (data.metrics) {
        // Request duration
        const duration = data.metrics.http_req_duration;
        if (duration) {
            summary += `${indent}  Request Duration:\n`;
            summary += `${indent}    p(50): ${duration.values['p(50)'].toFixed(2)}ms\n`;
            summary += `${indent}    p(95): ${duration.values['p(95)'].toFixed(2)}ms\n`;
            summary += `${indent}    p(99): ${duration.values['p(99)'].toFixed(2)}ms\n`;
        }
        
        // Error rate
        const errorRate = data.metrics.http_req_failed;
        if (errorRate) {
            const rate = (errorRate.values.rate * 100).toFixed(2);
            const rateColor = rate < 5 ? color.green : rate < 10 ? color.yellow : color.red;
            summary += `${indent}  Error Rate: ${rateColor}${rate}%${color.reset}\n`;
        }
        
        // Throughput
        const reqs = data.metrics.http_reqs;
        if (reqs) {
            summary += `${indent}  Throughput: ${reqs.values.rate.toFixed(2)} req/s\n`;
        }
    }
    
    return summary;
}

// HTML report helper
function htmlReport(data) {
    return `
<!DOCTYPE html>
<html>
<head>
    <title>Load Test Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .metric { margin: 10px 0; padding: 10px; background: #f0f0f0; }
        .pass { color: green; }
        .fail { color: red; }
        .chart { margin: 20px 0; }
    </style>
</head>
<body>
    <h1>Betting Platform Load Test Report</h1>
    <div class="metric">
        <h2>Test Summary</h2>
        <p>Duration: ${(data.duration / 1000 / 60).toFixed(2)} minutes</p>
        <p>Total Requests: ${data.metrics?.http_reqs?.values?.count || 0}</p>
        <p>Error Rate: ${((data.metrics?.http_req_failed?.values?.rate || 0) * 100).toFixed(2)}%</p>
    </div>
    
    <div class="metric">
        <h2>Response Times</h2>
        <p>Median: ${data.metrics?.http_req_duration?.values['p(50)']?.toFixed(2)}ms</p>
        <p>95th Percentile: ${data.metrics?.http_req_duration?.values['p(95)']?.toFixed(2)}ms</p>
        <p>99th Percentile: ${data.metrics?.http_req_duration?.values['p(99)']?.toFixed(2)}ms</p>
    </div>
    
    <div class="metric">
        <h2>Throughput</h2>
        <p>Average: ${data.metrics?.http_reqs?.values?.rate?.toFixed(2)} req/s</p>
    </div>
</body>
</html>
    `;
}