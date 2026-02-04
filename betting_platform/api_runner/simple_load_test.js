#!/usr/bin/env node

const http = require('http');

async function runSimpleLoadTest() {
    console.log('🚀 Simple Load Test - 100 Concurrent Requests');
    console.log('==============================================\n');
    
    const startTime = Date.now();
    const promises = [];
    
    // Test different endpoints
    const endpoints = [
        '/health',
        '/api/markets', 
        '/api/verses',
        '/api/quantum/states/1',
        '/api/risk/HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca'
    ];
    
    let successful = 0;
    let failed = 0;
    const latencies = [];
    
    // Generate 100 concurrent requests
    for (let i = 0; i < 100; i++) {
        const endpoint = endpoints[i % endpoints.length];
        
        const promise = new Promise((resolve) => {
            const reqStart = Date.now();
            
            const req = http.get(`http://localhost:8081${endpoint}`, (res) => {
                let body = '';
                res.on('data', chunk => body += chunk);
                res.on('end', () => {
                    const latency = Date.now() - reqStart;
                    latencies.push(latency);
                    
                    if (res.statusCode >= 200 && res.statusCode < 300) {
                        successful++;
                    } else {
                        failed++;
                    }
                    resolve();
                });
            });
            
            req.on('error', () => {
                failed++;
                resolve();
            });
            
            req.setTimeout(5000, () => {
                req.destroy();
                failed++;
                resolve();
            });
        });
        
        promises.push(promise);
    }
    
    await Promise.all(promises);
    
    const totalTime = Date.now() - startTime;
    const avgLatency = latencies.reduce((a, b) => a + b, 0) / latencies.length;
    const minLatency = Math.min(...latencies);
    const maxLatency = Math.max(...latencies);
    
    console.log('📊 Load Test Results:');
    console.log(`   Total Requests: 100`);
    console.log(`   Successful: ${successful}`);
    console.log(`   Failed: ${failed}`);
    console.log(`   Success Rate: ${(successful/100*100).toFixed(1)}%`);
    console.log(`   Total Time: ${totalTime}ms`);
    console.log(`   Requests/Second: ${(100/(totalTime/1000)).toFixed(0)}`);
    console.log(`   Avg Latency: ${avgLatency.toFixed(2)}ms`);
    console.log(`   Min Latency: ${minLatency}ms`);
    console.log(`   Max Latency: ${maxLatency}ms`);
    
    if (successful >= 95 && avgLatency < 100) {
        console.log('\n✅ EXCELLENT - System handles concurrent load well!');
    } else if (successful >= 90 && avgLatency < 200) {
        console.log('\n✅ GOOD - Minor performance tuning may be beneficial');
    } else {
        console.log('\n⚠️ FAIR - Consider performance optimizations');
    }
}

runSimpleLoadTest().catch(console.error);