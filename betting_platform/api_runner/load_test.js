#!/usr/bin/env node

const http = require('http');
const cluster = require('cluster');
const os = require('os');

const API_BASE = 'http://localhost:8081';
const TARGET_USERS = 10000;
const REQUESTS_PER_USER = 5;
const CONCURRENT_CONNECTIONS = 100;
const TEST_DURATION_MS = 60000; // 60 seconds

// Test scenarios to simulate
const TEST_SCENARIOS = [
    { path: '/health', method: 'GET', weight: 10 },
    { path: '/api/markets', method: 'GET', weight: 20 },
    { path: '/api/verses', method: 'GET', weight: 15 },
    { path: '/api/polymarket/markets?limit=10', method: 'GET', weight: 10 },
    { 
        path: '/api/orders/limit', 
        method: 'POST',
        weight: 25,
        body: {
            market_id: 1,
            wallet: 'test-wallet-{{ID}}',
            amount: 100000,
            outcome: 0,
            leverage: 2,
            price: 0.5,
            side: 'buy'
        }
    },
    {
        path: '/api/test/verse-match',
        method: 'POST', 
        weight: 10,
        body: {
            title: 'Test Market {{ID}}',
            category: 'politics',
            keywords: ['test', 'market', 'load']
        }
    },
    {
        path: '/api/quantum/create',
        method: 'POST',
        weight: 10,
        body: {
            states: [
                {
                    market_id: 1,
                    outcome: 0,
                    amount: 50000,
                    leverage: 2,
                    probability: 0.6
                },
                {
                    market_id: 1,
                    outcome: 1,
                    amount: 50000,
                    leverage: 2,
                    probability: 0.4
                }
            ]
        }
    }
];

// Metrics collection
const metrics = {
    totalRequests: 0,
    successfulRequests: 0,
    failedRequests: 0,
    totalLatency: 0,
    latencyBuckets: {
        '<10ms': 0,
        '10-50ms': 0,
        '50-100ms': 0,
        '100-500ms': 0,
        '500-1000ms': 0,
        '>1000ms': 0
    },
    errorCodes: {},
    startTime: 0,
    endTime: 0
};

function getRandomScenario() {
    const totalWeight = TEST_SCENARIOS.reduce((sum, s) => sum + s.weight, 0);
    let random = Math.random() * totalWeight;
    
    for (const scenario of TEST_SCENARIOS) {
        random -= scenario.weight;
        if (random <= 0) {
            return scenario;
        }
    }
    
    return TEST_SCENARIOS[0];
}

function makeRequest(scenario, userId) {
    return new Promise((resolve) => {
        const startTime = Date.now();
        
        // Replace placeholders in path and body
        let path = scenario.path.replace('{{ID}}', userId);
        let body = null;
        
        if (scenario.body) {
            body = JSON.stringify(scenario.body)
                .replace(/{{ID}}/g, userId);
        }
        
        const options = {
            hostname: 'localhost',
            port: 8081,
            path: path,
            method: scenario.method,
            headers: {}
        };
        
        if (body) {
            options.headers['Content-Type'] = 'application/json';
            options.headers['Content-Length'] = Buffer.byteLength(body);
        }
        
        const req = http.request(options, (res) => {
            let responseBody = '';
            res.on('data', chunk => responseBody += chunk);
            res.on('end', () => {
                const latency = Date.now() - startTime;
                const success = res.statusCode >= 200 && res.statusCode < 300;
                
                resolve({
                    success,
                    latency,
                    statusCode: res.statusCode,
                    path: path,
                    method: scenario.method
                });
            });
        });
        
        req.on('error', (err) => {
            const latency = Date.now() - startTime;
            resolve({
                success: false,
                latency,
                error: err.message,
                path: path,
                method: scenario.method
            });
        });
        
        req.setTimeout(5000, () => {
            req.destroy();
            resolve({
                success: false,
                latency: 5000,
                error: 'Timeout',
                path: path,
                method: scenario.method
            });
        });
        
        if (body) {
            req.write(body);
        }
        
        req.end();
    });
}

async function simulateUser(userId) {
    const results = [];
    
    for (let i = 0; i < REQUESTS_PER_USER; i++) {
        const scenario = getRandomScenario();
        const result = await makeRequest(scenario, userId);
        results.push(result);
        
        // Random delay between requests (10-100ms)
        await new Promise(resolve => setTimeout(resolve, Math.random() * 90 + 10));
    }
    
    return results;
}

async function runLoadTest() {
    console.log(`🚀 Starting Load Test`);
    console.log(`   Target Users: ${TARGET_USERS.toLocaleString()}`);
    console.log(`   Requests per User: ${REQUESTS_PER_USER}`);
    console.log(`   Total Requests: ${(TARGET_USERS * REQUESTS_PER_USER).toLocaleString()}`);
    console.log(`   Concurrent Connections: ${CONCURRENT_CONNECTIONS}`);
    console.log(`   Test Duration: ${TEST_DURATION_MS / 1000}s`);
    console.log('');
    
    metrics.startTime = Date.now();
    const endTime = metrics.startTime + TEST_DURATION_MS;
    
    let userCount = 0;
    const activePromises = [];
    
    // Progress tracking
    const progressInterval = setInterval(() => {
        const elapsed = Date.now() - metrics.startTime;
        const progress = (elapsed / TEST_DURATION_MS * 100).toFixed(1);
        const rps = (metrics.totalRequests / (elapsed / 1000)).toFixed(0);
        
        process.stdout.write(`\r⏱️  Progress: ${progress}% | RPS: ${rps} | Success: ${metrics.successfulRequests} | Failed: ${metrics.failedRequests}`);
    }, 1000);
    
    // Main load generation loop
    while (Date.now() < endTime && userCount < TARGET_USERS) {
        // Maintain concurrent connections
        while (activePromises.length < CONCURRENT_CONNECTIONS && userCount < TARGET_USERS && Date.now() < endTime) {
            const userId = userCount++;
            const promise = simulateUser(userId).then(results => {
                // Process results
                results.forEach(result => {
                    metrics.totalRequests++;
                    
                    if (result.success) {
                        metrics.successfulRequests++;
                    } else {
                        metrics.failedRequests++;
                        const errorKey = result.error || `HTTP ${result.statusCode}`;
                        metrics.errorCodes[errorKey] = (metrics.errorCodes[errorKey] || 0) + 1;
                    }
                    
                    metrics.totalLatency += result.latency;
                    
                    // Categorize latency
                    if (result.latency < 10) {
                        metrics.latencyBuckets['<10ms']++;
                    } else if (result.latency < 50) {
                        metrics.latencyBuckets['10-50ms']++;
                    } else if (result.latency < 100) {
                        metrics.latencyBuckets['50-100ms']++;
                    } else if (result.latency < 500) {
                        metrics.latencyBuckets['100-500ms']++;
                    } else if (result.latency < 1000) {
                        metrics.latencyBuckets['500-1000ms']++;
                    } else {
                        metrics.latencyBuckets['>1000ms']++;
                    }
                });
                
                // Remove from active promises
                const index = activePromises.indexOf(promise);
                if (index > -1) {
                    activePromises.splice(index, 1);
                }
            });
            
            activePromises.push(promise);
        }
        
        // Wait a bit before checking again
        await new Promise(resolve => setTimeout(resolve, 10));
    }
    
    // Wait for remaining requests to complete
    await Promise.all(activePromises);
    
    clearInterval(progressInterval);
    metrics.endTime = Date.now();
    
    console.log('\n\n📊 Load Test Results');
    console.log('====================');
    printResults();
}

function printResults() {
    const duration = (metrics.endTime - metrics.startTime) / 1000;
    const avgLatency = metrics.totalLatency / metrics.totalRequests;
    const successRate = (metrics.successfulRequests / metrics.totalRequests * 100).toFixed(2);
    const rps = (metrics.totalRequests / duration).toFixed(0);
    
    console.log(`\n📈 Performance Metrics:`);
    console.log(`   Duration: ${duration.toFixed(1)}s`);
    console.log(`   Total Requests: ${metrics.totalRequests.toLocaleString()}`);
    console.log(`   Successful: ${metrics.successfulRequests.toLocaleString()} (${successRate}%)`);
    console.log(`   Failed: ${metrics.failedRequests.toLocaleString()}`);
    console.log(`   Requests/Second: ${rps}`);
    console.log(`   Avg Latency: ${avgLatency.toFixed(2)}ms`);
    
    console.log(`\n📊 Latency Distribution:`);
    Object.entries(metrics.latencyBuckets).forEach(([bucket, count]) => {
        const percentage = (count / metrics.totalRequests * 100).toFixed(1);
        const bar = '█'.repeat(Math.floor(percentage / 2));
        console.log(`   ${bucket.padEnd(12)} ${count.toString().padStart(6)} (${percentage.padStart(5)}%) ${bar}`);
    });
    
    if (Object.keys(metrics.errorCodes).length > 0) {
        console.log(`\n❌ Error Summary:`);
        Object.entries(metrics.errorCodes).forEach(([error, count]) => {
            console.log(`   ${error}: ${count}`);
        });
    }
    
    // Performance rating
    console.log(`\n🎯 Performance Rating:`);
    if (successRate >= 99 && avgLatency < 100) {
        console.log(`   ⭐⭐⭐⭐⭐ EXCELLENT - Production ready!`);
    } else if (successRate >= 95 && avgLatency < 200) {
        console.log(`   ⭐⭐⭐⭐ GOOD - Minor optimizations needed`);
    } else if (successRate >= 90 && avgLatency < 500) {
        console.log(`   ⭐⭐⭐ FAIR - Performance tuning required`);
    } else {
        console.log(`   ⭐⭐ POOR - Significant improvements needed`);
    }
    
    // Recommendations
    console.log(`\n💡 Recommendations:`);
    if (avgLatency > 200) {
        console.log(`   • Consider caching frequently accessed data`);
        console.log(`   • Optimize database queries and indexes`);
    }
    if (successRate < 99) {
        console.log(`   • Investigate error patterns and add retries`);
        console.log(`   • Implement circuit breakers for failing services`);
    }
    if (metrics.latencyBuckets['>1000ms'] > metrics.totalRequests * 0.01) {
        console.log(`   • Address slow endpoints causing timeouts`);
        console.log(`   • Consider implementing request queuing`);
    }
}

// Cluster management for multi-core testing
if (cluster.isMaster) {
    console.log(`🖥️  Load Test Controller`);
    console.log(`   CPU Cores: ${os.cpus().length}`);
    console.log(`   Node Version: ${process.version}`);
    console.log('');
    
    // Fork workers
    for (let i = 0; i < Math.min(4, os.cpus().length); i++) {
        cluster.fork();
    }
    
    let workersComplete = 0;
    const workerMetrics = [];
    
    cluster.on('message', (worker, message) => {
        if (message.type === 'results') {
            workerMetrics.push(message.metrics);
            workersComplete++;
            
            if (workersComplete === Object.keys(cluster.workers).length) {
                // Aggregate results from all workers
                const aggregated = workerMetrics.reduce((acc, m) => {
                    acc.totalRequests += m.totalRequests;
                    acc.successfulRequests += m.successfulRequests;
                    acc.failedRequests += m.failedRequests;
                    acc.totalLatency += m.totalLatency;
                    
                    Object.keys(m.latencyBuckets).forEach(bucket => {
                        acc.latencyBuckets[bucket] += m.latencyBuckets[bucket];
                    });
                    
                    Object.keys(m.errorCodes).forEach(error => {
                        acc.errorCodes[error] = (acc.errorCodes[error] || 0) + m.errorCodes[error];
                    });
                    
                    return acc;
                }, {
                    totalRequests: 0,
                    successfulRequests: 0,
                    failedRequests: 0,
                    totalLatency: 0,
                    latencyBuckets: {
                        '<10ms': 0,
                        '10-50ms': 0,
                        '50-100ms': 0,
                        '100-500ms': 0,
                        '500-1000ms': 0,
                        '>1000ms': 0
                    },
                    errorCodes: {},
                    startTime: Math.min(...workerMetrics.map(m => m.startTime)),
                    endTime: Math.max(...workerMetrics.map(m => m.endTime))
                });
                
                // Print aggregated results
                Object.assign(metrics, aggregated);
                console.log('\n\n📊 Aggregated Load Test Results');
                console.log('================================');
                printResults();
                
                // Exit
                process.exit(0);
            }
        }
    });
    
    cluster.on('exit', (worker, code, signal) => {
        if (code !== 0) {
            console.error(`Worker ${worker.process.pid} died with code ${code}`);
        }
    });
} else {
    // Worker process
    runLoadTest().then(() => {
        process.send({ type: 'results', metrics });
        process.exit(0);
    }).catch(err => {
        console.error(`Worker error: ${err.message}`);
        process.exit(1);
    });
}