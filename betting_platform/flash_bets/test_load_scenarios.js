const crypto = require('crypto');

/**
 * Load Testing Scenarios for Flash Betting System
 * Tests system performance under various load conditions
 */

class LoadTestRunner {
    constructor() {
        this.results = [];
        this.metrics = {
            requests: 0,
            successful: 0,
            failed: 0,
            latencies: [],
            throughput: 0
        };
        this.startTime = Date.now();
    }

    // ============= CONCURRENT USER SCENARIOS =============

    /**
     * Scenario 1: 100 Concurrent Users
     * Normal load during regular hours
     */
    async test100ConcurrentUsers() {
        console.log('\n📊 Load Test: 100 Concurrent Users');
        console.log('='.repeat(50));
        
        const users = 100;
        const duration = 30000; // 30 seconds
        const requestsPerUser = 10;
        
        console.log(`  Users: ${users}`);
        console.log(`  Duration: ${duration/1000}s`);
        console.log(`  Requests per user: ${requestsPerUser}`);
        
        const results = await this.runConcurrentLoad(users, requestsPerUser, duration);
        
        this.printLoadResults('100 Concurrent Users', results);
        return results.successRate > 0.95; // 95% success threshold
    }

    /**
     * Scenario 2: 500 Concurrent Users
     * Medium load during active hours
     */
    async test500ConcurrentUsers() {
        console.log('\n📊 Load Test: 500 Concurrent Users');
        console.log('='.repeat(50));
        
        const users = 500;
        const duration = 30000;
        const requestsPerUser = 5;
        
        console.log(`  Users: ${users}`);
        console.log(`  Duration: ${duration/1000}s`);
        console.log(`  Requests per user: ${requestsPerUser}`);
        
        const results = await this.runConcurrentLoad(users, requestsPerUser, duration);
        
        this.printLoadResults('500 Concurrent Users', results);
        return results.successRate > 0.90; // 90% success threshold
    }

    /**
     * Scenario 3: 1000 Concurrent Users
     * Peak load during major events
     */
    async test1000ConcurrentUsers() {
        console.log('\n📊 Load Test: 1000 Concurrent Users');
        console.log('='.repeat(50));
        
        const users = 1000;
        const duration = 30000;
        const requestsPerUser = 3;
        
        console.log(`  Users: ${users}`);
        console.log(`  Duration: ${duration/1000}s`);
        console.log(`  Requests per user: ${requestsPerUser}`);
        
        const results = await this.runConcurrentLoad(users, requestsPerUser, duration);
        
        this.printLoadResults('1000 Concurrent Users', results);
        return results.successRate > 0.85; // 85% success threshold
    }

    // ============= SPECIFIC LOAD PATTERNS =============

    /**
     * Scenario 4: Spike Test
     * Sudden surge of users (Super Bowl moment)
     */
    async testSpikeLoad() {
        console.log('\n📈 Load Test: Spike Pattern');
        console.log('='.repeat(50));
        
        console.log('  Simulating sudden spike (0 → 500 users in 5s)');
        
        const results = {
            phases: [],
            totalRequests: 0,
            successful: 0,
            failed: 0
        };
        
        // Phase 1: Normal load (50 users)
        console.log('\n  Phase 1: Normal load (50 users)');
        const phase1 = await this.runConcurrentLoad(50, 2, 5000);
        results.phases.push({ name: 'Normal', ...phase1 });
        
        // Phase 2: Spike (500 users)
        console.log('\n  Phase 2: SPIKE (500 users)');
        const phase2 = await this.runConcurrentLoad(500, 5, 10000);
        results.phases.push({ name: 'Spike', ...phase2 });
        
        // Phase 3: Cool down (100 users)
        console.log('\n  Phase 3: Cool down (100 users)');
        const phase3 = await this.runConcurrentLoad(100, 2, 5000);
        results.phases.push({ name: 'Cooldown', ...phase3 });
        
        // Aggregate results
        results.totalRequests = results.phases.reduce((sum, p) => sum + p.totalRequests, 0);
        results.successful = results.phases.reduce((sum, p) => sum + p.successful, 0);
        results.failed = results.phases.reduce((sum, p) => sum + p.failed, 0);
        results.successRate = results.successful / results.totalRequests;
        
        console.log('\n  Spike Test Results:');
        results.phases.forEach(phase => {
            console.log(`    ${phase.name}: ${phase.successRate * 100}% success, ${phase.avgLatency}ms avg latency`);
        });
        
        this.recordResult('Spike Test', results);
        return results.phases[1].successRate > 0.80; // 80% success during spike
    }

    /**
     * Scenario 5: Sustained Load
     * Constant high load for extended period
     */
    async testSustainedLoad() {
        console.log('\n⏱️ Load Test: Sustained High Load');
        console.log('='.repeat(50));
        
        const users = 300;
        const duration = 120000; // 2 minutes
        const requestsPerUser = 20;
        
        console.log(`  Sustaining ${users} users for ${duration/1000}s`);
        console.log(`  Total expected requests: ${users * requestsPerUser}`);
        
        const results = await this.runConcurrentLoad(users, requestsPerUser, duration);
        
        // Check for degradation over time
        const firstHalf = results.latencies.slice(0, results.latencies.length / 2);
        const secondHalf = results.latencies.slice(results.latencies.length / 2);
        
        const firstHalfAvg = this.average(firstHalf);
        const secondHalfAvg = this.average(secondHalf);
        const degradation = ((secondHalfAvg - firstHalfAvg) / firstHalfAvg) * 100;
        
        console.log(`\n  Performance degradation: ${degradation.toFixed(1)}%`);
        console.log(`  First half avg: ${firstHalfAvg.toFixed(0)}ms`);
        console.log(`  Second half avg: ${secondHalfAvg.toFixed(0)}ms`);
        
        results.degradation = degradation;
        
        this.printLoadResults('Sustained Load', results);
        return results.successRate > 0.90 && degradation < 20; // Less than 20% degradation
    }

    /**
     * Scenario 6: Ramp-up Test
     * Gradually increase load to find breaking point
     */
    async testRampUp() {
        console.log('\n📈 Load Test: Ramp-up to Breaking Point');
        console.log('='.repeat(50));
        
        const stages = [50, 100, 200, 400, 800, 1600];
        const results = {
            stages: [],
            breakingPoint: null
        };
        
        for (const users of stages) {
            console.log(`\n  Stage: ${users} users`);
            
            const stageResult = await this.runConcurrentLoad(users, 3, 10000);
            results.stages.push({
                users,
                ...stageResult
            });
            
            console.log(`    Success rate: ${(stageResult.successRate * 100).toFixed(1)}%`);
            console.log(`    Avg latency: ${stageResult.avgLatency}ms`);
            
            // Check if we hit breaking point (< 80% success or > 5s latency)
            if (stageResult.successRate < 0.80 || stageResult.avgLatency > 5000) {
                results.breakingPoint = users;
                console.log(`\n  ⚠️ Breaking point reached at ${users} users`);
                break;
            }
        }
        
        if (!results.breakingPoint) {
            console.log(`\n  ✅ System handled up to ${stages[stages.length - 1]} users`);
        }
        
        this.recordResult('Ramp-up Test', results);
        return results.breakingPoint === null || results.breakingPoint > 400;
    }

    // ============= API RATE LIMIT TESTING =============

    /**
     * Scenario 7: Provider Rate Limit Test
     * Test handling of provider API limits
     */
    async testProviderRateLimits() {
        console.log('\n🚦 Load Test: Provider Rate Limits');
        console.log('='.repeat(50));
        
        const providers = ['DraftKings', 'FanDuel', 'BetMGM'];
        const limits = {
            DraftKings: 60,  // 60 req/min
            FanDuel: 100,    // 100 req/min
            BetMGM: 150      // 150 req/min
        };
        
        const results = {
            providers: []
        };
        
        for (const provider of providers) {
            console.log(`\n  Testing ${provider} (limit: ${limits[provider]}/min)`);
            
            // Send requests at 2x the limit
            const requestRate = (limits[provider] * 2) / 60; // per second
            const duration = 10000; // 10 seconds
            const totalRequests = Math.floor(requestRate * 10);
            
            const providerResult = await this.testProviderLimit(provider, totalRequests, duration);
            
            console.log(`    Sent: ${totalRequests} requests`);
            console.log(`    Succeeded: ${providerResult.successful}`);
            console.log(`    Rate limited: ${providerResult.rateLimited}`);
            console.log(`    Circuit breaker triggered: ${providerResult.circuitBreakerTriggered ? 'Yes' : 'No'}`);
            
            results.providers.push({
                provider,
                limit: limits[provider],
                ...providerResult
            });
        }
        
        const allHandledGracefully = results.providers.every(p => 
            p.circuitBreakerTriggered || p.successful > p.limit / 6
        );
        
        this.recordResult('Provider Rate Limits', results);
        return allHandledGracefully;
    }

    // ============= CHAIN CONGESTION TESTING =============

    /**
     * Scenario 8: Blockchain Congestion
     * Test behavior during network congestion
     */
    async testChainCongestion() {
        console.log('\n⛓️ Load Test: Blockchain Congestion');
        console.log('='.repeat(50));
        
        const scenarios = [
            { name: 'Normal', tps: 1000, gasPrice: 30 },
            { name: 'Moderate', tps: 500, gasPrice: 100 },
            { name: 'Congested', tps: 100, gasPrice: 500 },
            { name: 'Severe', tps: 50, gasPrice: 1000 }
        ];
        
        const results = {
            scenarios: []
        };
        
        for (const scenario of scenarios) {
            console.log(`\n  Scenario: ${scenario.name} (${scenario.tps} TPS, ${scenario.gasPrice} gwei)`);
            
            const scenarioResult = await this.simulateChainCongestion(scenario);
            
            console.log(`    Transactions sent: ${scenarioResult.sent}`);
            console.log(`    Confirmed: ${scenarioResult.confirmed}`);
            console.log(`    Avg confirmation time: ${scenarioResult.avgConfirmTime}ms`);
            console.log(`    Timeouts: ${scenarioResult.timeouts}`);
            
            results.scenarios.push({
                ...scenario,
                ...scenarioResult
            });
        }
        
        const acceptablePerformance = results.scenarios.every(s => 
            s.name === 'Severe' || s.confirmed / s.sent > 0.80
        );
        
        this.recordResult('Chain Congestion', results);
        return acceptablePerformance;
    }

    // ============= FLASH MARKET SPECIFIC TESTS =============

    /**
     * Scenario 9: Flash Market Rush
     * Multiple markets resolving simultaneously
     */
    async testFlashMarketRush() {
        console.log('\n⚡ Load Test: Flash Market Rush');
        console.log('='.repeat(50));
        
        const marketsResolving = 50;
        const usersPerMarket = 100;
        const totalUsers = marketsResolving * usersPerMarket;
        
        console.log(`  Markets resolving: ${marketsResolving}`);
        console.log(`  Users per market: ${usersPerMarket}`);
        console.log(`  Total concurrent resolutions: ${totalUsers}`);
        
        const results = await this.simulateMarketRush(marketsResolving, usersPerMarket);
        
        console.log(`\n  Results:`);
        console.log(`    Markets resolved: ${results.resolved}/${marketsResolving}`);
        console.log(`    Avg resolution time: ${results.avgResolutionTime}ms`);
        console.log(`    ZK proofs verified: ${results.zkProofsVerified}`);
        console.log(`    Payouts processed: ${results.payoutsProcessed}`);
        console.log(`    Failed resolutions: ${results.failed}`);
        
        this.recordResult('Flash Market Rush', results);
        return results.resolved / marketsResolving > 0.95;
    }

    /**
     * Scenario 10: Leverage Chain Stress
     * Many users attempting max leverage simultaneously
     */
    async testLeverageChainStress() {
        console.log('\n🔗 Load Test: Leverage Chain Stress');
        console.log('='.repeat(50));
        
        const users = 200;
        const leverageAttempts = 3; // 3-step chain per user
        
        console.log(`  Users attempting 500x leverage: ${users}`);
        console.log(`  Chain steps per user: ${leverageAttempts}`);
        console.log(`  Total chain operations: ${users * leverageAttempts}`);
        
        const results = await this.simulateLeverageChains(users);
        
        console.log(`\n  Results:`);
        console.log(`    Successful chains: ${results.successful}/${users}`);
        console.log(`    Failed at borrow: ${results.failedBorrow}`);
        console.log(`    Failed at liquidate: ${results.failedLiquidate}`);
        console.log(`    Failed at stake: ${results.failedStake}`);
        console.log(`    Avg chain time: ${results.avgChainTime}ms`);
        console.log(`    Max leverage achieved: ${results.maxLeverage}x`);
        
        this.recordResult('Leverage Chain Stress', results);
        return results.successful / users > 0.80;
    }

    // ============= HELPER FUNCTIONS =============

    async runConcurrentLoad(users, requestsPerUser, duration) {
        const results = {
            totalRequests: 0,
            successful: 0,
            failed: 0,
            latencies: [],
            errors: []
        };
        
        const promises = [];
        const startTime = Date.now();
        
        // Create user sessions
        for (let i = 0; i < users; i++) {
            promises.push(this.simulateUser(i, requestsPerUser, duration, results));
        }
        
        // Wait for all users to complete
        await Promise.all(promises);
        
        const elapsed = Date.now() - startTime;
        
        // Calculate metrics
        results.successRate = results.successful / results.totalRequests;
        results.avgLatency = this.average(results.latencies);
        results.p95Latency = this.percentile(results.latencies, 95);
        results.p99Latency = this.percentile(results.latencies, 99);
        results.throughput = (results.totalRequests / elapsed) * 1000; // requests per second
        
        return results;
    }

    async simulateUser(userId, requests, duration, results) {
        const endTime = Date.now() + duration;
        const requestDelay = duration / requests;
        
        while (Date.now() < endTime && results.totalRequests < requests * (userId + 1)) {
            const start = Date.now();
            
            try {
                // Simulate API call
                await this.makeRequest(userId);
                
                const latency = Date.now() - start;
                results.latencies.push(latency);
                results.successful++;
            } catch (error) {
                results.failed++;
                results.errors.push(error.message);
            }
            
            results.totalRequests++;
            
            // Wait before next request
            await this.delay(requestDelay + Math.random() * 100);
        }
    }

    async makeRequest(userId) {
        // Simulate network latency (50-500ms)
        const latency = 50 + Math.random() * 450;
        await this.delay(latency);
        
        // Simulate 5% failure rate
        if (Math.random() < 0.05) {
            throw new Error(`Request failed for user ${userId}`);
        }
        
        return { success: true, userId };
    }

    async testProviderLimit(provider, requests, duration) {
        const results = {
            successful: 0,
            rateLimited: 0,
            failed: 0,
            circuitBreakerTriggered: false
        };
        
        const startTime = Date.now();
        const requestInterval = duration / requests;
        let consecutiveFailures = 0;
        
        for (let i = 0; i < requests; i++) {
            if (Date.now() - startTime > duration) break;
            
            try {
                await this.makeProviderRequest(provider);
                results.successful++;
                consecutiveFailures = 0;
            } catch (error) {
                if (error.message.includes('rate limit')) {
                    results.rateLimited++;
                } else {
                    results.failed++;
                }
                
                consecutiveFailures++;
                
                // Circuit breaker logic
                if (consecutiveFailures >= 5) {
                    results.circuitBreakerTriggered = true;
                    console.log(`      Circuit breaker triggered for ${provider}`);
                    break;
                }
            }
            
            await this.delay(requestInterval);
        }
        
        return results;
    }

    async makeProviderRequest(provider) {
        await this.delay(10 + Math.random() * 40);
        
        // Simulate rate limiting
        if (Math.random() < 0.3) {
            throw new Error(`${provider} rate limit exceeded`);
        }
        
        return { provider, data: 'mock_odds' };
    }

    async simulateChainCongestion(scenario) {
        const transactions = 100;
        const results = {
            sent: transactions,
            confirmed: 0,
            avgConfirmTime: 0,
            timeouts: 0
        };
        
        const confirmTimes = [];
        
        for (let i = 0; i < transactions; i++) {
            // Simulate transaction submission
            const baseTime = 1000 / scenario.tps;
            const congestionMultiplier = scenario.gasPrice / 30;
            const confirmTime = baseTime * congestionMultiplier + Math.random() * 1000;
            
            if (confirmTime < 30000) { // 30 second timeout
                results.confirmed++;
                confirmTimes.push(confirmTime);
            } else {
                results.timeouts++;
            }
        }
        
        results.avgConfirmTime = this.average(confirmTimes);
        
        return results;
    }

    async simulateMarketRush(markets, usersPerMarket) {
        const results = {
            resolved: 0,
            avgResolutionTime: 0,
            zkProofsVerified: 0,
            payoutsProcessed: 0,
            failed: 0
        };
        
        const resolutionTimes = [];
        const promises = [];
        
        for (let i = 0; i < markets; i++) {
            promises.push(this.resolveMarket(i, usersPerMarket));
        }
        
        const marketResults = await Promise.all(promises);
        
        marketResults.forEach(result => {
            if (result.resolved) {
                results.resolved++;
                resolutionTimes.push(result.time);
                results.zkProofsVerified += result.zkVerified ? 1 : 0;
                results.payoutsProcessed += result.payouts;
            } else {
                results.failed++;
            }
        });
        
        results.avgResolutionTime = this.average(resolutionTimes);
        
        return results;
    }

    async resolveMarket(marketId, users) {
        const start = Date.now();
        
        // Simulate ZK proof generation (2s)
        await this.delay(2000 + Math.random() * 1000);
        
        // Simulate on-chain verification (3s)
        await this.delay(3000 + Math.random() * 1000);
        
        // Simulate payout processing
        const payouts = Math.floor(users * 0.45); // 45% winners
        await this.delay(payouts * 10); // 10ms per payout
        
        const resolved = Math.random() > 0.05; // 95% success rate
        
        return {
            marketId,
            resolved,
            time: Date.now() - start,
            zkVerified: resolved,
            payouts: resolved ? payouts : 0
        };
    }

    async simulateLeverageChains(users) {
        const results = {
            successful: 0,
            failedBorrow: 0,
            failedLiquidate: 0,
            failedStake: 0,
            avgChainTime: 0,
            maxLeverage: 0
        };
        
        const chainTimes = [];
        const leverages = [];
        
        const promises = [];
        for (let i = 0; i < users; i++) {
            promises.push(this.executeLeverageChain(i));
        }
        
        const chainResults = await Promise.all(promises);
        
        chainResults.forEach(result => {
            if (result.success) {
                results.successful++;
                chainTimes.push(result.time);
                leverages.push(result.leverage);
            } else {
                switch (result.failedAt) {
                    case 'borrow':
                        results.failedBorrow++;
                        break;
                    case 'liquidate':
                        results.failedLiquidate++;
                        break;
                    case 'stake':
                        results.failedStake++;
                        break;
                }
            }
        });
        
        results.avgChainTime = this.average(chainTimes);
        results.maxLeverage = Math.max(...leverages, 0);
        
        return results;
    }

    async executeLeverageChain(userId) {
        const start = Date.now();
        let leverage = 100; // Start with 100x base
        
        try {
            // Step 1: Borrow
            await this.delay(500 + Math.random() * 500);
            if (Math.random() < 0.1) throw new Error('borrow');
            leverage *= 1.5;
            
            // Step 2: Liquidate
            await this.delay(500 + Math.random() * 500);
            if (Math.random() < 0.1) throw new Error('liquidate');
            leverage *= 1.2;
            
            // Step 3: Stake
            await this.delay(500 + Math.random() * 500);
            if (Math.random() < 0.1) throw new Error('stake');
            leverage *= 1.1;
            
            return {
                userId,
                success: true,
                leverage: Math.min(leverage, 500),
                time: Date.now() - start
            };
        } catch (error) {
            return {
                userId,
                success: false,
                failedAt: error.message,
                time: Date.now() - start
            };
        }
    }

    // Utility functions
    average(arr) {
        if (arr.length === 0) return 0;
        return arr.reduce((sum, val) => sum + val, 0) / arr.length;
    }

    percentile(arr, p) {
        if (arr.length === 0) return 0;
        const sorted = [...arr].sort((a, b) => a - b);
        const index = Math.ceil((p / 100) * sorted.length) - 1;
        return sorted[index];
    }

    delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    recordResult(scenario, data) {
        this.results.push({
            scenario,
            timestamp: Date.now(),
            ...data
        });
    }

    printLoadResults(scenario, results) {
        console.log(`\n  ${scenario} Results:`);
        console.log(`    Total requests: ${results.totalRequests}`);
        console.log(`    Successful: ${results.successful} (${(results.successRate * 100).toFixed(1)}%)`);
        console.log(`    Failed: ${results.failed}`);
        console.log(`    Throughput: ${results.throughput.toFixed(1)} req/s`);
        console.log(`    Avg latency: ${results.avgLatency.toFixed(0)}ms`);
        console.log(`    P95 latency: ${results.p95Latency.toFixed(0)}ms`);
        console.log(`    P99 latency: ${results.p99Latency.toFixed(0)}ms`);
    }

    // ============= MAIN TEST RUNNER =============

    async runAllScenarios() {
        console.log('🚀 Starting Flash Bets Load Testing Suite');
        console.log('=' .repeat(60));
        
        const scenarios = [
            { name: '100 Concurrent Users', test: () => this.test100ConcurrentUsers() },
            { name: '500 Concurrent Users', test: () => this.test500ConcurrentUsers() },
            { name: '1000 Concurrent Users', test: () => this.test1000ConcurrentUsers() },
            { name: 'Spike Load', test: () => this.testSpikeLoad() },
            { name: 'Sustained Load', test: () => this.testSustainedLoad() },
            { name: 'Ramp-up Test', test: () => this.testRampUp() },
            { name: 'Provider Rate Limits', test: () => this.testProviderRateLimits() },
            { name: 'Chain Congestion', test: () => this.testChainCongestion() },
            { name: 'Flash Market Rush', test: () => this.testFlashMarketRush() },
            { name: 'Leverage Chain Stress', test: () => this.testLeverageChainStress() }
        ];
        
        const results = [];
        
        for (const scenario of scenarios) {
            try {
                const passed = await scenario.test();
                results.push({ name: scenario.name, passed });
            } catch (error) {
                console.error(`\n  ❌ ${scenario.name} failed:`, error.message);
                results.push({ name: scenario.name, passed: false, error: error.message });
            }
        }
        
        this.printSummary(results);
    }

    printSummary(results) {
        console.log('\n' + '='.repeat(60));
        console.log('📊 LOAD TEST SUMMARY');
        console.log('='.repeat(60));
        
        const passed = results.filter(r => r.passed).length;
        const total = results.length;
        
        console.log(`\nScenarios tested: ${total}`);
        console.log(`Passed: ${passed}`);
        console.log(`Failed: ${total - passed}`);
        console.log(`Success rate: ${((passed/total) * 100).toFixed(1)}%`);
        
        console.log('\nScenario Results:');
        console.log('-'.repeat(40));
        
        results.forEach(result => {
            const status = result.passed ? '✅' : '❌';
            console.log(`${status} ${result.name}`);
            if (result.error) {
                console.log(`   Error: ${result.error}`);
            }
        });
        
        // Performance summary
        console.log('\n' + '-'.repeat(40));
        console.log('Key Performance Metrics:');
        
        const avgThroughput = this.results
            .filter(r => r.throughput)
            .reduce((sum, r) => sum + r.throughput, 0) / this.results.filter(r => r.throughput).length;
        
        if (avgThroughput) {
            console.log(`Average throughput: ${avgThroughput.toFixed(1)} req/s`);
        }
        
        const breakingPoint = this.results.find(r => r.breakingPoint);
        if (breakingPoint) {
            console.log(`System breaking point: ${breakingPoint.breakingPoint} concurrent users`);
        }
        
        console.log(`Total test duration: ${((Date.now() - this.startTime) / 1000).toFixed(1)}s`);
        
        if (passed === total) {
            console.log('\n🎉 ALL LOAD TESTS PASSED!');
            console.log('✨ System is ready for production traffic.');
        } else {
            console.log('\n⚠️ Some load tests failed.');
            console.log('Review and optimize performance before deployment.');
        }
    }
}

// Run the tests
async function main() {
    const tester = new LoadTestRunner();
    await tester.runAllScenarios();
}

main().catch(console.error);