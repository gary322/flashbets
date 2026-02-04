#!/usr/bin/env node

const axios = require('axios');

async function testAPI() {
    console.log('Testing API endpoints...\n');
    
    const tests = [
        {
            name: 'Health Check',
            method: 'GET',
            url: 'http://localhost:8081/health',
            expectedStatus: 200
        },
        {
            name: 'Markets Endpoint',
            method: 'GET', 
            url: 'http://localhost:8081/api/markets',
            expectedStatus: 200
        },
        {
            name: 'Verses Endpoint',
            method: 'GET',
            url: 'http://localhost:8081/api/verses',
            expectedStatus: 200
        }
    ];
    
    let passed = 0;
    let failed = 0;
    
    for (const test of tests) {
        try {
            const response = await axios({
                method: test.method,
                url: test.url,
                timeout: 5000
            });
            
            if (response.status === test.expectedStatus) {
                console.log(`✅ ${test.name}: PASSED (${response.status})`);
                passed++;
            } else {
                console.log(`❌ ${test.name}: FAILED (Expected ${test.expectedStatus}, got ${response.status})`);
                failed++;
            }
        } catch (error) {
            console.log(`❌ ${test.name}: FAILED (${error.message})`);
            failed++;
        }
    }
    
    console.log(`\nTotal: ${passed + failed}, Passed: ${passed}, Failed: ${failed}`);
    process.exit(failed > 0 ? 1 : 0);
}

testAPI();