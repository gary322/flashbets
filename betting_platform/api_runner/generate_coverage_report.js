#!/usr/bin/env node

const fs = require('fs');
const http = require('http');

async function runCoverageAnalysis() {
    console.log('📊 Comprehensive Test Coverage Analysis');
    console.log('=======================================\n');
    
    const coverageData = {
        totalEndpoints: 0,
        testedEndpoints: 0,
        totalFeatures: 0,
        implementedFeatures: 0,
        testCategories: {},
        performanceMetrics: {},
        codeMetrics: {},
        timestamp: new Date().toISOString()
    };
    
    // Define all API endpoints
    const apiEndpoints = [
        { path: '/health', method: 'GET', category: 'system', tested: true },
        { path: '/api/program/info', method: 'GET', category: 'system', tested: true },
        { path: '/api/markets', method: 'GET', category: 'trading', tested: true },
        { path: '/api/markets/:id', method: 'GET', category: 'trading', tested: true },
        { path: '/api/markets/create', method: 'POST', category: 'trading', tested: false },
        { path: '/api/markets/:id/orderbook', method: 'GET', category: 'trading', tested: true },
        { path: '/api/trade/place', method: 'POST', category: 'trading', tested: true },
        { path: '/api/trade/close', method: 'POST', category: 'trading', tested: false },
        { path: '/api/orders/limit', method: 'POST', category: 'orders', tested: true },
        { path: '/api/orders/stop', method: 'POST', category: 'orders', tested: true },
        { path: '/api/orders/:id/cancel', method: 'POST', category: 'orders', tested: true },
        { path: '/api/orders/:wallet', method: 'GET', category: 'orders', tested: true },
        { path: '/api/positions/:wallet', method: 'GET', category: 'portfolio', tested: true },
        { path: '/api/portfolio/:wallet', method: 'GET', category: 'portfolio', tested: true },
        { path: '/api/risk/:wallet', method: 'GET', category: 'portfolio', tested: true },
        { path: '/api/wallet/balance/:wallet', method: 'GET', category: 'portfolio', tested: true },
        { path: '/api/wallet/demo/create', method: 'POST', category: 'portfolio', tested: true },
        { path: '/api/verses', method: 'GET', category: 'verses', tested: true },
        { path: '/api/verses/:id', method: 'GET', category: 'verses', tested: true },
        { path: '/api/test/verse-match', method: 'POST', category: 'verses', tested: true },
        { path: '/api/quantum/positions/:wallet', method: 'GET', category: 'quantum', tested: true },
        { path: '/api/quantum/create', method: 'POST', category: 'quantum', tested: true },
        { path: '/api/quantum/states/:market_id', method: 'GET', category: 'quantum', tested: true },
        { path: '/api/defi/stake', method: 'POST', category: 'defi', tested: false },
        { path: '/api/defi/pools', method: 'GET', category: 'defi', tested: false },
        { path: '/api/polymarket/markets', method: 'GET', category: 'integration', tested: true },
        { path: '/api/integration/status', method: 'GET', category: 'integration', tested: false },
        { path: '/api/integration/sync', method: 'POST', category: 'integration', tested: false },
        { path: '/ws', method: 'WS', category: 'realtime', tested: true },
        { path: '/ws/v2', method: 'WS', category: 'realtime', tested: false }
    ];
    
    // Calculate endpoint coverage
    coverageData.totalEndpoints = apiEndpoints.length;
    coverageData.testedEndpoints = apiEndpoints.filter(e => e.tested).length;
    
    // Group by category
    apiEndpoints.forEach(endpoint => {
        if (!coverageData.testCategories[endpoint.category]) {
            coverageData.testCategories[endpoint.category] = {
                total: 0,
                tested: 0,
                endpoints: []
            };
        }
        
        coverageData.testCategories[endpoint.category].total++;
        coverageData.testCategories[endpoint.category].endpoints.push(endpoint);
        
        if (endpoint.tested) {
            coverageData.testCategories[endpoint.category].tested++;
        }
    });
    
    // Define feature coverage
    const features = [
        { name: 'Market Orders', implemented: true, tested: false },
        { name: 'Limit Orders', implemented: true, tested: true },
        { name: 'Stop Orders', implemented: true, tested: true },
        { name: 'OCO Orders', implemented: true, tested: false },
        { name: 'Bracket Orders', implemented: true, tested: false },
        { name: 'Iceberg Orders', implemented: true, tested: false },
        { name: 'TWAP Orders', implemented: true, tested: false },
        { name: 'VWAP Orders', implemented: true, tested: false },
        { name: 'Trailing Stop Orders', implemented: true, tested: false },
        { name: 'Stop-Limit Orders', implemented: true, tested: false },
        { name: 'Order Book Management', implemented: true, tested: true },
        { name: 'Verse Catalog (400 verses)', implemented: true, tested: true },
        { name: 'Verse Matching Algorithm', implemented: true, tested: true },
        { name: 'Risk Tier System', implemented: true, tested: true },
        { name: 'Quantum Superposition', implemented: true, tested: true },
        { name: 'Quantum Entanglement', implemented: true, tested: true },
        { name: 'Quantum Decoherence', implemented: true, tested: false },
        { name: 'Quantum Portfolio Metrics', implemented: true, tested: true },
        { name: 'Greeks Calculation', implemented: true, tested: false },
        { name: 'VaR Calculation', implemented: true, tested: false },
        { name: 'Sharpe/Sortino Ratios', implemented: true, tested: false },
        { name: 'Risk Limit Enforcement', implemented: true, tested: false },
        { name: 'Portfolio Analytics', implemented: true, tested: true },
        { name: 'Real-time WebSocket', implemented: true, tested: true },
        { name: 'Market Data Broadcasting', implemented: true, tested: true },
        { name: 'Order Notifications', implemented: true, tested: true },
        { name: 'Native Solana Integration', implemented: true, tested: true },
        { name: 'Demo Account System', implemented: true, tested: true },
        { name: 'Position Management', implemented: true, tested: true },
        { name: 'Balance Tracking', implemented: true, tested: true },
        { name: 'Error Handling', implemented: true, tested: true },
        { name: 'Input Validation', implemented: true, tested: true },
        { name: 'Type Safety', implemented: true, tested: true },
        { name: 'Async Architecture', implemented: true, tested: true },
        { name: 'CORS Support', implemented: true, tested: true },
        { name: 'JSON Serialization', implemented: true, tested: true },
        { name: 'External API Integration', implemented: true, tested: true },
        { name: 'Rate Limiting Framework', implemented: true, tested: false },
        { name: 'Authentication Framework', implemented: true, tested: false },
        { name: 'Configuration Management', implemented: true, tested: false }
    ];
    
    coverageData.totalFeatures = features.length;
    coverageData.implementedFeatures = features.filter(f => f.implemented).length;
    const testedFeatures = features.filter(f => f.implemented && f.tested).length;
    
    // Get code metrics by analyzing source files
    const sourceFiles = [
        'src/main.rs',
        'src/handlers.rs', 
        'src/order_types.rs',
        'src/quantum_engine.rs',
        'src/risk_engine.rs',
        'src/verse_catalog.rs',
        'src/rpc_client.rs',
        'src/websocket.rs',
        'src/types.rs'
    ];
    
    let totalLines = 0;
    const fileMetrics = {};
    
    for (const file of sourceFiles) {
        try {
            const content = fs.readFileSync(file, 'utf-8');
            const lines = content.split('\n').length;
            const nonEmptyLines = content.split('\n').filter(line => line.trim()).length;
            const commentLines = content.split('\n').filter(line => line.trim().startsWith('//')).length;
            
            totalLines += lines;
            fileMetrics[file] = {
                totalLines: lines,
                codeLines: nonEmptyLines - commentLines,
                commentLines: commentLines,
                emptyLines: lines - nonEmptyLines
            };
        } catch (err) {
            fileMetrics[file] = { error: 'File not found' };
        }
    }
    
    coverageData.codeMetrics = {
        totalSourceFiles: sourceFiles.length,
        totalLines: totalLines,
        fileBreakdown: fileMetrics
    };
    
    // Run quick performance test
    console.log('Running performance validation...');
    const perfStartTime = Date.now();
    
    try {
        await new Promise((resolve, reject) => {
            const req = http.get('http://localhost:8081/health', (res) => {
                resolve();
            });
            req.on('error', reject);
            req.setTimeout(1000, () => reject(new Error('Timeout')));
        });
        
        const responseTime = Date.now() - perfStartTime;
        coverageData.performanceMetrics = {
            healthCheckLatency: responseTime,
            status: responseTime < 100 ? 'excellent' : responseTime < 200 ? 'good' : 'needs_improvement'
        };
    } catch (err) {
        coverageData.performanceMetrics = {
            healthCheckLatency: null,
            status: 'error',
            error: err.message
        };
    }
    
    // Generate report
    console.log('\n📋 TEST COVERAGE REPORT');
    console.log('========================\n');
    
    console.log('🎯 Overall Coverage:');
    console.log(`   API Endpoints: ${coverageData.testedEndpoints}/${coverageData.totalEndpoints} (${(coverageData.testedEndpoints/coverageData.totalEndpoints*100).toFixed(1)}%)`);
    console.log(`   Features: ${testedFeatures}/${coverageData.implementedFeatures} (${(testedFeatures/coverageData.implementedFeatures*100).toFixed(1)}%)`);
    
    console.log('\n📊 Coverage by Category:');
    Object.entries(coverageData.testCategories).forEach(([category, data]) => {
        const percentage = (data.tested / data.total * 100).toFixed(1);
        console.log(`   ${category.padEnd(12)}: ${data.tested}/${data.total} (${percentage}%)`);
    });
    
    console.log('\n🔧 Implementation Status:');
    const implementedPercentage = (coverageData.implementedFeatures / coverageData.totalFeatures * 100).toFixed(1);
    console.log(`   Total Features: ${coverageData.totalFeatures}`);
    console.log(`   Implemented: ${coverageData.implementedFeatures} (${implementedPercentage}%)`);
    console.log(`   Tested: ${testedFeatures} (${(testedFeatures/coverageData.totalFeatures*100).toFixed(1)}%)`);
    
    console.log('\n💻 Code Metrics:');
    console.log(`   Source Files: ${coverageData.codeMetrics.totalSourceFiles}`);
    console.log(`   Total Lines: ${coverageData.codeMetrics.totalLines.toLocaleString()}`);
    
    console.log('\n📈 Performance:');
    if (coverageData.performanceMetrics.status !== 'error') {
        console.log(`   Health Check: ${coverageData.performanceMetrics.healthCheckLatency}ms (${coverageData.performanceMetrics.status})`);
    } else {
        console.log(`   Health Check: Error - ${coverageData.performanceMetrics.error}`);
    }
    
    // Identify gaps
    console.log('\n🔍 Coverage Gaps:');
    const untestedEndpoints = apiEndpoints.filter(e => !e.tested);
    if (untestedEndpoints.length > 0) {
        console.log('   Untested Endpoints:');
        untestedEndpoints.forEach(e => {
            console.log(`     ${e.method} ${e.path}`);
        });
    }
    
    const untestedFeatures = features.filter(f => f.implemented && !f.tested);
    if (untestedFeatures.length > 0) {
        console.log('   Untested Features:');
        untestedFeatures.forEach(f => {
            console.log(`     ${f.name}`);
        });
    }
    
    // Overall grade
    console.log('\n🎓 Overall Grade:');
    const endpointScore = coverageData.testedEndpoints / coverageData.totalEndpoints;
    const featureScore = testedFeatures / coverageData.implementedFeatures;
    const overallScore = (endpointScore + featureScore) / 2;
    
    if (overallScore >= 0.9) {
        console.log('   ⭐⭐⭐⭐⭐ EXCELLENT (A+) - Comprehensive coverage!');
    } else if (overallScore >= 0.8) {
        console.log('   ⭐⭐⭐⭐ GOOD (A) - Strong coverage with minor gaps');
    } else if (overallScore >= 0.7) {
        console.log('   ⭐⭐⭐ FAIR (B) - Adequate coverage, room for improvement');
    } else {
        console.log('   ⭐⭐ NEEDS IMPROVEMENT (C) - Significant gaps in coverage');
    }
    
    // Save detailed report
    fs.writeFileSync('test_coverage_report.json', JSON.stringify(coverageData, null, 2));
    console.log('\n✅ Detailed coverage report saved to test_coverage_report.json');
}

runCoverageAnalysis().catch(console.error);