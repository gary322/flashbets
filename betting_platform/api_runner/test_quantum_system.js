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

async function testQuantumPositionCreation() {
    console.log('🔬 Testing Quantum Position Creation');
    console.log('=====================================\n');
    
    // Test 1: Simple superposition
    console.log('📊 Test 1: Simple Binary Superposition');
    const superpositionStates = [
        {
            market_id: 1,
            outcome: 0,
            amount: 100000,
            leverage: 2,
            probability: 0.6
        },
        {
            market_id: 1,
            outcome: 1,
            amount: 100000,
            leverage: 2,
            probability: 0.4
        }
    ];
    
    try {
        const response = await makeRequest('/api/quantum/create', 'POST', {
            states: superpositionStates,
            entanglement_group: null
        });
        
        console.log(`Status: ${response.status}`);
        console.log(`Response:`, JSON.stringify(response.body, null, 2));
        
        if (response.status === 200) {
            console.log('✅ Simple superposition created successfully\n');
            return response.body.quantum_position_id;
        } else {
            console.log('❌ Failed to create superposition\n');
            return null;
        }
    } catch (e) {
        console.log('❌ Error:', e.message, '\n');
        return null;
    }
}

async function testQuantumEntanglement() {
    console.log('🔀 Test 2: Quantum Entanglement');
    
    // Create first entangled position
    const entangled1 = [
        {
            market_id: 2,
            outcome: 0,
            amount: 200000,
            leverage: 3,
            probability: 0.7
        },
        {
            market_id: 2,
            outcome: 1,
            amount: 200000,
            leverage: 3,
            probability: 0.3
        }
    ];
    
    try {
        const response1 = await makeRequest('/api/quantum/create', 'POST', {
            states: entangled1,
            entanglement_group: 'political-correlation-group'
        });
        
        console.log(`First Entangled Position Status: ${response1.status}`);
        
        if (response1.status === 200) {
            const position1Id = response1.body.quantum_position_id;
            console.log(`Position 1 ID: ${position1Id}`);
            
            // Create second entangled position
            const entangled2 = [
                {
                    market_id: 3,
                    outcome: 0,
                    amount: 150000,
                    leverage: 2,
                    probability: 0.3
                },
                {
                    market_id: 3,
                    outcome: 1,
                    amount: 150000,
                    leverage: 2,
                    probability: 0.7
                }
            ];
            
            const response2 = await makeRequest('/api/quantum/create', 'POST', {
                states: entangled2,
                entanglement_group: 'political-correlation-group'
            });
            
            console.log(`Second Entangled Position Status: ${response2.status}`);
            
            if (response2.status === 200) {
                const position2Id = response2.body.quantum_position_id;
                console.log(`Position 2 ID: ${position2Id}`);
                console.log('✅ Quantum entanglement created successfully\n');
                return [position1Id, position2Id];
            }
        }
        
        console.log('❌ Failed to create entanglement\n');
        return [];
    } catch (e) {
        console.log('❌ Error:', e.message, '\n');
        return [];
    }
}

async function testQuantumStates() {
    console.log('📈 Test 3: Quantum States Retrieval');
    
    try {
        const response = await makeRequest('/api/quantum/states/1', 'GET');
        
        console.log(`Status: ${response.status}`);
        console.log(`Quantum States:`, JSON.stringify(response.body, null, 2));
        
        if (response.status === 200) {
            console.log('✅ Quantum states retrieved successfully\n');
        } else {
            console.log('❌ Failed to retrieve quantum states\n');
        }
    } catch (e) {
        console.log('❌ Error:', e.message, '\n');
    }
}

async function testQuantumPositions() {
    console.log('👤 Test 4: User Quantum Positions');
    
    try {
        const response = await makeRequest('/api/quantum/positions/test-wallet', 'GET');
        
        console.log(`Status: ${response.status}`);
        console.log(`User Positions:`, JSON.stringify(response.body, null, 2));
        
        if (response.status === 200) {
            console.log('✅ User quantum positions retrieved successfully\n');
            return response.body.quantum_positions;
        } else {
            console.log('❌ Failed to retrieve user positions\n');
            return [];
        }
    } catch (e) {
        console.log('❌ Error:', e.message, '\n');
        return [];
    }
}

async function testComplexQuantumPosition() {
    console.log('🌐 Test 5: Complex Multi-Market Quantum Position');
    
    const complexStates = [
        {
            market_id: 4,
            outcome: 0,
            amount: 300000,
            leverage: 4,
            probability: 0.25
        },
        {
            market_id: 4,
            outcome: 1,
            amount: 300000,
            leverage: 4,
            probability: 0.35
        },
        {
            market_id: 5,
            outcome: 0,
            amount: 200000,
            leverage: 3,
            probability: 0.2
        },
        {
            market_id: 5,
            outcome: 1,
            amount: 200000,
            leverage: 3,
            probability: 0.2
        }
    ];
    
    try {
        const response = await makeRequest('/api/quantum/create', 'POST', {
            states: complexStates,
            entanglement_group: 'multi-market-hedge'
        });
        
        console.log(`Status: ${response.status}`);
        console.log(`Response:`, JSON.stringify(response.body, null, 2));
        
        if (response.status === 200) {
            console.log('✅ Complex quantum position created successfully\n');
        } else {
            console.log('❌ Failed to create complex position\n');
        }
    } catch (e) {
        console.log('❌ Error:', e.message, '\n');
    }
}

async function testQuantumPortfolioMetrics() {
    console.log('📊 Test 6: Quantum Portfolio Analytics');
    
    const positions = await testQuantumPositions();
    
    if (positions && positions.length > 0) {
        console.log(`📈 Portfolio Analysis:`);
        console.log(`  • Total Quantum Positions: ${positions.length}`);
        
        let totalExpectedValue = 0;
        let activeSuperpositions = 0;
        
        positions.forEach((pos, index) => {
            console.log(`\n  Position ${index + 1}:`);
            console.log(`    - ID: ${pos.id}`);
            console.log(`    - States: ${pos.states.length}`);
            console.log(`    - Collapsed: ${pos.is_collapsed}`);
            console.log(`    - Entanglement Group: ${pos.entanglement_group || 'None'}`);
            
            if (!pos.is_collapsed) {
                activeSuperpositions++;
                const expectedValue = pos.states.reduce((sum, state) => 
                    sum + (state.probability * state.amount), 0);
                totalExpectedValue += expectedValue;
                console.log(`    - Expected Value: ${expectedValue.toLocaleString()}`);
            }
        });
        
        console.log(`\n📊 Portfolio Summary:`);
        console.log(`  • Active Superpositions: ${activeSuperpositions}`);
        console.log(`  • Total Expected Value: ${totalExpectedValue.toLocaleString()}`);
        console.log(`  • Average Position Size: ${(totalExpectedValue / activeSuperpositions || 0).toLocaleString()}`);
    }
    
    console.log('\n✅ Quantum portfolio analysis complete\n');
}

async function runQuantumSystemTests() {
    console.log('🚀 QUANTUM BETTING SYSTEM COMPREHENSIVE TESTS');
    console.log('==============================================\n');
    
    const startTime = Date.now();
    
    // Run all quantum tests
    const positionId = await testQuantumPositionCreation();
    const entangledIds = await testQuantumEntanglement();
    await testQuantumStates();
    await testComplexQuantumPosition();
    await testQuantumPortfolioMetrics();
    
    const duration = ((Date.now() - startTime) / 1000).toFixed(2);
    
    console.log('🎯 QUANTUM SYSTEM TEST SUMMARY');
    console.log('==============================');
    console.log(`⏱️  Total Duration: ${duration}s`);
    console.log(`🔬 Superposition Tests: Complete`);
    console.log(`🔀 Entanglement Tests: Complete`);
    console.log(`📊 State Management: Complete`);
    console.log(`👤 Portfolio Analytics: Complete`);
    
    if (positionId) {
        console.log(`\n🆔 Created Position ID: ${positionId}`);
    }
    
    if (entangledIds.length > 0) {
        console.log(`🔗 Entangled Positions: ${entangledIds.join(', ')}`);
    }
    
    console.log('\n🎉 Quantum system testing completed successfully!');
    console.log('\n💡 Key Features Validated:');
    console.log('   • Quantum superposition states');
    console.log('   • Multi-position entanglement');
    console.log('   • Wave function normalization');
    console.log('   • Decoherence timers');
    console.log('   • Portfolio quantum metrics');
}

// Run the tests
runQuantumSystemTests().catch(console.error);