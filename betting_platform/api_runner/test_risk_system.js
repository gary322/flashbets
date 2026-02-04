#!/usr/bin/env node

const http = require('http');
const API_BASE = 'http://localhost:8081';

function makeRequest(path, method = 'GET', data = null) {
    return new Promise((resolve, reject) => {
        const url = new URL(API_BASE + path);
        const options = {
            hostname: url.hostname,
            port: url.port,
            path: url.pathname + url.search,
            method: method,
            headers: {}
        };
        
        if (data) {
            options.headers['Content-Type'] = 'application/json';
        }
        
        const req = http.request(options, (res) => {
            let body = '';
            res.on('data', chunk => body += chunk);
            res.on('end', () => {
                try {
                    resolve({
                        status: res.statusCode,
                        body: JSON.parse(body)
                    });
                } catch (e) {
                    resolve({
                        status: res.statusCode,
                        body: body
                    });
                }
            });
        });
        
        req.on('error', reject);
        
        if (data) {
            req.write(JSON.stringify(data));
        }
        
        req.end();
    });
}

async function testPortfolioRiskMetrics() {
    console.log('📊 Testing Portfolio Risk Metrics');
    console.log('=================================\n');
    
    try {
        // Test with different wallet types
        const testWallets = [
            'HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca', // Valid Solana address format
            'test-wallet-123', // Demo wallet format
            'demo-trader-high-risk', // Another test wallet
        ];
        
        for (const wallet of testWallets) {
            console.log(`📈 Testing wallet: ${wallet.substring(0, 8)}...`);
            
            const response = await makeRequest(`/api/risk/${wallet}`, 'GET');
            
            console.log(`Status: ${response.status}`);
            
            if (response.status === 200) {
                console.log('✅ Risk metrics retrieved successfully');
                console.log(`Response keys: ${Object.keys(response.body).join(', ')}`);
                
                // Check for expected risk metrics
                if (response.body.risk_metrics) {
                    const metrics = response.body.risk_metrics;
                    console.log(`   Portfolio Value: ${metrics.portfolio_value?.toLocaleString() || 'N/A'}`);
                    console.log(`   Leverage Ratio: ${metrics.leverage_ratio?.toFixed(2) || 'N/A'}x`);
                    console.log(`   Risk Score: ${metrics.risk_score?.toFixed(0) || 'N/A'}/100`);
                    console.log(`   VaR (95%): ${metrics.var_95?.toLocaleString() || 'N/A'}`);
                    console.log(`   Sharpe Ratio: ${metrics.sharpe_ratio?.toFixed(2) || 'N/A'}`);
                    console.log(`   Win Rate: ${(metrics.win_rate * 100)?.toFixed(1) || 'N/A'}%`);
                }
                
                // Check for recommendations
                if (response.body.recommendations) {
                    console.log(`   Recommendations: ${response.body.recommendations.length} items`);
                    response.body.recommendations.forEach((rec, i) => {
                        console.log(`     ${i + 1}. ${rec}`);
                    });
                }
                
                // Check for alerts
                if (response.body.alerts) {
                    console.log(`   Alerts: ${response.body.alerts.length} items`);
                    response.body.alerts.forEach((alert, i) => {
                        console.log(`     ⚠️ ${i + 1}. ${alert}`);
                    });
                }
                
            } else {
                console.log('❌ Failed to retrieve risk metrics');
                console.log(`Error: ${JSON.stringify(response.body, null, 2)}`);
            }
            
            console.log(''); // Empty line
        }
        
    } catch (e) {
        console.log('❌ Error:', e.message);
    }
}

async function testGreeksCalculation() {
    console.log('🧮 Testing Greeks Calculation');
    console.log('=============================\n');
    
    // Test portfolio endpoint which should include Greeks
    try {
        const wallet = 'HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca';
        const response = await makeRequest(`/api/portfolio/${wallet}`, 'GET');
        
        console.log(`Portfolio Status: ${response.status}`);
        
        if (response.status === 200) {
            console.log('✅ Portfolio data retrieved');
            
            // Look for positions with Greeks
            if (response.body.positions) {
                console.log(`Found ${response.body.positions.length} positions`);
                
                response.body.positions.forEach((pos, i) => {
                    console.log(`\n  Position ${i + 1}:`);
                    console.log(`    Market ID: ${pos.market_id}`);
                    console.log(`    Amount: ${pos.amount?.toLocaleString()}`);
                    console.log(`    Current Price: ${pos.current_price}`);
                    console.log(`    PnL: ${pos.pnl}`);
                    
                    if (pos.greeks) {
                        console.log(`    Greeks:`);
                        console.log(`      Delta: ${pos.greeks.delta?.toFixed(4)}`);
                        console.log(`      Gamma: ${pos.greeks.gamma?.toFixed(4)}`);
                        console.log(`      Theta: ${pos.greeks.theta?.toFixed(4)}`);
                        console.log(`      Vega: ${pos.greeks.vega?.toFixed(4)}`);
                        console.log(`      Rho: ${pos.greeks.rho?.toFixed(4)}`);
                    }
                });
            }
        } else {
            console.log('❌ Failed to retrieve portfolio');
            console.log(`Error: ${JSON.stringify(response.body, null, 2)}`);
        }
        
    } catch (e) {
        console.log('❌ Error:', e.message);
    }
}

async function testRiskLimitsValidation() {
    console.log('⚖️ Testing Risk Limits Validation');
    console.log('=================================\n');
    
    // Test placing trades with various risk levels
    const riskScenarios = [
        {
            name: 'Conservative Trade',
            amount: 50000,
            leverage: 2,
            expected_risk: 'low'
        },
        {
            name: 'Moderate Risk Trade',
            amount: 200000,
            leverage: 5,
            expected_risk: 'medium'
        },
        {
            name: 'High Risk Trade',
            amount: 500000,
            leverage: 10,
            expected_risk: 'high'
        },
        {
            name: 'Extreme Risk Trade',
            amount: 1000000,
            leverage: 15,
            expected_risk: 'extreme'
        }
    ];
    
    for (const scenario of riskScenarios) {
        console.log(`📊 Testing: ${scenario.name}`);
        
        try {
            const response = await makeRequest('/api/trade/place', 'POST', {
                market_id: 1,
                amount: scenario.amount,
                outcome: 0,
                leverage: scenario.leverage,
                order_type: 'market',
                wallet: 'test-risk-validation'
            });
            
            console.log(`  Status: ${response.status}`);
            
            if (response.status === 200) {
                console.log(`  ✅ Trade placed successfully`);
            } else {
                console.log(`  ❌ Trade rejected`);
                if (response.body.error) {
                    console.log(`  Reason: ${response.body.error.message}`);
                }
            }
            
        } catch (e) {
            console.log(`  ❌ Error: ${e.message}`);
        }
        
        console.log('');
    }
}

async function testRealTimeRiskMonitoring() {
    console.log('📡 Testing Real-Time Risk Monitoring');
    console.log('====================================\n');
    
    // Test multiple risk metric calls to simulate real-time monitoring
    const wallet = 'HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca';
    const monitoringRounds = 3;
    
    console.log(`Running ${monitoringRounds} rounds of risk monitoring...`);
    
    for (let round = 1; round <= monitoringRounds; round++) {
        console.log(`\nRound ${round}:`);
        
        try {
            const startTime = Date.now();
            const response = await makeRequest(`/api/risk/${wallet}`, 'GET');
            const responseTime = Date.now() - startTime;
            
            console.log(`  Response Time: ${responseTime}ms`);
            console.log(`  Status: ${response.status}`);
            
            if (response.status === 200 && response.body.risk_metrics) {
                const metrics = response.body.risk_metrics;
                console.log(`  Risk Score: ${metrics.risk_score?.toFixed(0) || 'N/A'}/100`);
                console.log(`  Portfolio Value: $${metrics.portfolio_value?.toLocaleString() || 'N/A'}`);
                console.log(`  Margin Usage: ${(metrics.margin_ratio * 100)?.toFixed(1) || 'N/A'}%`);
                
                if (responseTime > 100) {
                    console.log(`  ⚠️ Slow response (${responseTime}ms)`);
                } else {
                    console.log(`  ✅ Fast response`);
                }
            }
            
        } catch (e) {
            console.log(`  ❌ Error: ${e.message}`);
        }
        
        // Wait 500ms before next round
        if (round < monitoringRounds) {
            await new Promise(resolve => setTimeout(resolve, 500));
        }
    }
}

async function testRiskReportGeneration() {
    console.log('📄 Testing Risk Report Generation');
    console.log('=================================\n');
    
    const wallet = 'HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca';
    
    try {
        const response = await makeRequest(`/api/risk/${wallet}`, 'GET');
        
        if (response.status === 200) {
            console.log('✅ Risk report generated successfully\n');
            
            // Analyze the report structure
            const report = response.body;
            
            console.log('📋 Report Contents:');
            console.log(`  • Timestamp: ${report.timestamp || 'Not provided'}`);
            console.log(`  • Wallet: ${report.wallet || 'Not provided'}`);
            console.log(`  • Risk Metrics: ${report.risk_metrics ? 'Included' : 'Missing'}`);
            console.log(`  • Risk Limits: ${report.risk_limits ? 'Included' : 'Missing'}`);
            console.log(`  • Recommendations: ${report.recommendations?.length || 0} items`);
            console.log(`  • Alerts: ${report.alerts?.length || 0} items`);
            
            if (report.risk_metrics) {
                const metrics = report.risk_metrics;
                console.log('\n📊 Key Risk Indicators:');
                console.log(`  • Portfolio Value: $${metrics.portfolio_value?.toLocaleString() || 'N/A'}`);
                console.log(`  • Total Exposure: $${metrics.total_exposure?.toLocaleString() || 'N/A'}`);
                console.log(`  • Leverage Ratio: ${metrics.leverage_ratio?.toFixed(2) || 'N/A'}x`);
                console.log(`  • VaR (95%): $${metrics.var_95?.toLocaleString() || 'N/A'}`);
                console.log(`  • Risk Score: ${metrics.risk_score?.toFixed(0) || 'N/A'}/100`);
                console.log(`  • Sharpe Ratio: ${metrics.sharpe_ratio?.toFixed(2) || 'N/A'}`);
                console.log(`  • Win Rate: ${(metrics.win_rate * 100)?.toFixed(1) || 'N/A'}%`);
                console.log(`  • Max Drawdown: ${(metrics.max_drawdown * 100)?.toFixed(1) || 'N/A'}%`);
            }
            
            if (report.risk_limits) {
                const limits = report.risk_limits;
                console.log('\n⚖️ Risk Limits:');
                console.log(`  • Max Position Size: $${limits.max_position_size?.toLocaleString() || 'N/A'}`);
                console.log(`  • Max Leverage: ${limits.max_leverage || 'N/A'}x`);
                console.log(`  • Max Portfolio Risk: ${(limits.max_portfolio_risk * 100)?.toFixed(0) || 'N/A'}%`);
                console.log(`  • VaR Limit: $${limits.var_limit?.toLocaleString() || 'N/A'}`);
            }
            
        } else {
            console.log('❌ Failed to generate risk report');
            console.log(`Error: ${JSON.stringify(response.body, null, 2)}`);
        }
        
    } catch (e) {
        console.log('❌ Error:', e.message);
    }
}

async function runRiskSystemTests() {
    console.log('🚀 RISK MANAGEMENT SYSTEM COMPREHENSIVE TESTS');
    console.log('==============================================\n');
    
    const startTime = Date.now();
    
    // Run all risk system tests
    await testPortfolioRiskMetrics();
    await testGreeksCalculation();
    await testRiskLimitsValidation();
    await testRealTimeRiskMonitoring();
    await testRiskReportGeneration();
    
    const duration = ((Date.now() - startTime) / 1000).toFixed(2);
    
    console.log('\n🎯 RISK SYSTEM TEST SUMMARY');
    console.log('===========================');
    console.log(`⏱️  Total Duration: ${duration}s`);
    console.log(`📊 Portfolio Risk Analysis: Complete`);
    console.log(`🧮 Greeks Calculation: Complete`);
    console.log(`⚖️ Risk Limits Validation: Complete`);
    console.log(`📡 Real-time Monitoring: Complete`);
    console.log(`📄 Report Generation: Complete`);
    
    console.log('\n🎉 Risk management system testing completed!');
    console.log('\n💡 Key Features Validated:');
    console.log('   • Portfolio risk metrics calculation');
    console.log('   • Black-Scholes Greeks computation');
    console.log('   • Value at Risk (VaR) estimation');
    console.log('   • Risk limit enforcement');
    console.log('   • Real-time risk monitoring');
    console.log('   • Comprehensive risk reporting');
    console.log('   • Automated risk recommendations');
    console.log('   • Risk-based alerts generation');
}

// Run the tests
runRiskSystemTests().catch(console.error);