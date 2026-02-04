#!/usr/bin/env node

/**
 * Solana Smart Contract Stress Test
 * Tests the betting platform program under extreme load conditions
 */

const {
    Connection,
    Keypair,
    PublicKey,
    Transaction,
    TransactionInstruction,
    LAMPORTS_PER_SOL,
    SystemProgram,
    sendAndConfirmTransaction
} = require('@solana/web3.js');
const { Token, TOKEN_PROGRAM_ID } = require('@solana/spl-token');
const BN = require('bn.js');
const chalk = require('chalk');
const { performance } = require('perf_hooks');

// Configuration
const CONFIG = {
    RPC_URL: process.env.RPC_URL || 'http://localhost:8899',
    PROGRAM_ID: process.env.PROGRAM_ID || '5cnuqTxYjzrmYnQ6BtvxEK4bpFJn4kkUCzgMakidheza',
    COMMITMENT: 'confirmed'
};

// Test metrics
const metrics = {
    transactions: {
        sent: 0,
        confirmed: 0,
        failed: 0,
        latencies: []
    },
    instructions: {
        createMarket: { count: 0, latencies: [] },
        placeBet: { count: 0, latencies: [] },
        closePosition: { count: 0, latencies: [] },
        addLiquidity: { count: 0, latencies: [] },
        processQuantum: { count: 0, latencies: [] }
    },
    errors: [],
    startTime: Date.now()
};

// Utility functions
function logMetric(category, subcategory, value) {
    if (!metrics[category][subcategory]) {
        metrics[category][subcategory] = [];
    }
    metrics[category][subcategory].push(value);
}

function getStats(values) {
    if (values.length === 0) return { avg: 0, min: 0, max: 0, p95: 0 };
    
    const sorted = values.sort((a, b) => a - b);
    const avg = values.reduce((a, b) => a + b, 0) / values.length;
    const min = sorted[0];
    const max = sorted[sorted.length - 1];
    const p95 = sorted[Math.floor(values.length * 0.95)];
    
    return { avg, min, max, p95 };
}

// Program instruction builders
class InstructionBuilder {
    static createMarket(programId, admin, marketPubkey, question, outcomes, endTime) {
        const data = Buffer.concat([
            Buffer.from([0]), // Instruction index for create_market
            Buffer.from(question),
            Buffer.from([outcomes.length]),
            ...outcomes.map(o => Buffer.from(o)),
            new BN(endTime).toArrayLike(Buffer, 'le', 8)
        ]);
        
        return new TransactionInstruction({
            keys: [
                { pubkey: admin, isSigner: true, isWritable: true },
                { pubkey: marketPubkey, isSigner: false, isWritable: true },
                { pubkey: SystemProgram.programId, isSigner: false, isWritable: false }
            ],
            programId: new PublicKey(programId),
            data
        });
    }
    
    static placeBet(programId, trader, marketPubkey, outcome, amount, leverage = 1) {
        const data = Buffer.concat([
            Buffer.from([1]), // Instruction index for place_bet
            Buffer.from([outcome]),
            new BN(amount).toArrayLike(Buffer, 'le', 8),
            Buffer.from([leverage])
        ]);
        
        return new TransactionInstruction({
            keys: [
                { pubkey: trader, isSigner: true, isWritable: true },
                { pubkey: marketPubkey, isSigner: false, isWritable: true }
            ],
            programId: new PublicKey(programId),
            data
        });
    }
    
    static addLiquidity(programId, provider, marketPubkey, amount) {
        const data = Buffer.concat([
            Buffer.from([2]), // Instruction index for add_liquidity
            new BN(amount).toArrayLike(Buffer, 'le', 8)
        ]);
        
        return new TransactionInstruction({
            keys: [
                { pubkey: provider, isSigner: true, isWritable: true },
                { pubkey: marketPubkey, isSigner: false, isWritable: true }
            ],
            programId: new PublicKey(programId),
            data
        });
    }
    
    static processQuantumTrade(programId, trader, marketPubkeys, verses, amount) {
        const data = Buffer.concat([
            Buffer.from([3]), // Instruction index for quantum_trade
            Buffer.from([marketPubkeys.length]),
            ...verses.map(v => Buffer.from([v])),
            new BN(amount).toArrayLike(Buffer, 'le', 8)
        ]);
        
        const keys = [
            { pubkey: trader, isSigner: true, isWritable: true },
            ...marketPubkeys.map(pubkey => ({
                pubkey,
                isSigner: false,
                isWritable: true
            }))
        ];
        
        return new TransactionInstruction({
            keys,
            programId: new PublicKey(programId),
            data
        });
    }
}

// Stress test scenarios
class StressTestScenarios {
    constructor(connection, programId) {
        this.connection = connection;
        this.programId = programId;
        this.markets = [];
        this.traders = [];
    }
    
    async setup() {
        console.log(chalk.blue('🔧 Setting up test environment...'));
        
        // Generate test accounts
        for (let i = 0; i < 100; i++) {
            const trader = Keypair.generate();
            // Airdrop SOL to traders
            const sig = await this.connection.requestAirdrop(
                trader.publicKey,
                5 * LAMPORTS_PER_SOL
            );
            await this.connection.confirmTransaction(sig);
            this.traders.push(trader);
        }
        
        // Create test markets
        for (let i = 0; i < 10; i++) {
            const marketKeypair = Keypair.generate();
            this.markets.push(marketKeypair);
        }
        
        console.log(chalk.green(`✓ Created ${this.traders.length} traders and ${this.markets.length} markets`));
    }
    
    /**
     * Scenario 1: Concurrent Market Creation
     */
    async testConcurrentMarketCreation() {
        console.log(chalk.bold('\n📊 Test 1: Concurrent Market Creation'));
        
        const promises = [];
        const batchSize = 20;
        
        for (let i = 0; i < batchSize; i++) {
            const admin = this.traders[i];
            const market = Keypair.generate();
            
            const promise = (async () => {
                const start = performance.now();
                try {
                    const ix = InstructionBuilder.createMarket(
                        this.programId,
                        admin.publicKey,
                        market.publicKey,
                        `Test Market ${i}`,
                        ['Yes', 'No'],
                        Math.floor(Date.now() / 1000) + 86400
                    );
                    
                    const tx = new Transaction().add(ix);
                    await sendAndConfirmTransaction(this.connection, tx, [admin], {
                        commitment: CONFIG.COMMITMENT
                    });
                    
                    const latency = performance.now() - start;
                    metrics.instructions.createMarket.count++;
                    metrics.instructions.createMarket.latencies.push(latency);
                    metrics.transactions.confirmed++;
                    
                    return { success: true, latency };
                } catch (error) {
                    metrics.transactions.failed++;
                    metrics.errors.push({ type: 'createMarket', error: error.message });
                    return { success: false, error: error.message };
                }
            })();
            
            promises.push(promise);
            metrics.transactions.sent++;
        }
        
        const results = await Promise.allSettled(promises);
        const successful = results.filter(r => r.value?.success).length;
        
        console.log(`  Sent: ${batchSize}, Confirmed: ${successful}, Failed: ${batchSize - successful}`);
        const stats = getStats(metrics.instructions.createMarket.latencies);
        console.log(`  Latency - Avg: ${stats.avg.toFixed(0)}ms, P95: ${stats.p95.toFixed(0)}ms`);
    }
    
    /**
     * Scenario 2: High-Frequency Trading Simulation
     */
    async testHighFrequencyTrading() {
        console.log(chalk.bold('\n💹 Test 2: High-Frequency Trading'));
        
        const market = this.markets[0];
        const duration = 30000; // 30 seconds
        const startTime = Date.now();
        let tradesPlaced = 0;
        
        // Continuous trading loop
        const tradePromises = [];
        
        while (Date.now() - startTime < duration) {
            const trader = this.traders[Math.floor(Math.random() * this.traders.length)];
            const outcome = Math.random() > 0.5 ? 0 : 1;
            const amount = Math.floor(Math.random() * 1000) + 100;
            const leverage = Math.floor(Math.random() * 10) + 1;
            
            const promise = (async () => {
                const start = performance.now();
                try {
                    const ix = InstructionBuilder.placeBet(
                        this.programId,
                        trader.publicKey,
                        market.publicKey,
                        outcome,
                        amount,
                        leverage
                    );
                    
                    const tx = new Transaction().add(ix);
                    await sendAndConfirmTransaction(this.connection, tx, [trader], {
                        commitment: CONFIG.COMMITMENT
                    });
                    
                    const latency = performance.now() - start;
                    metrics.instructions.placeBet.count++;
                    metrics.instructions.placeBet.latencies.push(latency);
                    metrics.transactions.confirmed++;
                    
                    return { success: true, latency };
                } catch (error) {
                    metrics.transactions.failed++;
                    return { success: false };
                }
            })();
            
            tradePromises.push(promise);
            tradesPlaced++;
            metrics.transactions.sent++;
            
            // Don't wait for confirmation, send next trade immediately
            if (tradesPlaced % 10 === 0) {
                await new Promise(resolve => setTimeout(resolve, 10));
            }
        }
        
        // Wait for all trades to complete
        await Promise.allSettled(tradePromises);
        
        const tps = metrics.instructions.placeBet.count / (duration / 1000);
        console.log(`  Trades placed: ${tradesPlaced}`);
        console.log(`  TPS: ${tps.toFixed(2)}`);
        
        const stats = getStats(metrics.instructions.placeBet.latencies);
        console.log(`  Latency - Avg: ${stats.avg.toFixed(0)}ms, P95: ${stats.p95.toFixed(0)}ms`);
    }
    
    /**
     * Scenario 3: Liquidity Stress Test
     */
    async testLiquidityOperations() {
        console.log(chalk.bold('\n💧 Test 3: Liquidity Operations Under Load'));
        
        const market = this.markets[0];
        const providers = this.traders.slice(0, 50);
        const promises = [];
        
        // Concurrent liquidity additions
        for (const provider of providers) {
            const amount = Math.floor(Math.random() * 10000) + 1000;
            
            const promise = (async () => {
                const start = performance.now();
                try {
                    const ix = InstructionBuilder.addLiquidity(
                        this.programId,
                        provider.publicKey,
                        market.publicKey,
                        amount
                    );
                    
                    const tx = new Transaction().add(ix);
                    await sendAndConfirmTransaction(this.connection, tx, [provider], {
                        commitment: CONFIG.COMMITMENT
                    });
                    
                    const latency = performance.now() - start;
                    metrics.instructions.addLiquidity.count++;
                    metrics.instructions.addLiquidity.latencies.push(latency);
                    
                    return { success: true, latency };
                } catch (error) {
                    return { success: false };
                }
            })();
            
            promises.push(promise);
        }
        
        const results = await Promise.allSettled(promises);
        const successful = results.filter(r => r.value?.success).length;
        
        console.log(`  Liquidity providers: ${providers.length}`);
        console.log(`  Successful: ${successful}, Failed: ${providers.length - successful}`);
        
        const stats = getStats(metrics.instructions.addLiquidity.latencies);
        console.log(`  Latency - Avg: ${stats.avg.toFixed(0)}ms, P95: ${stats.p95.toFixed(0)}ms`);
    }
    
    /**
     * Scenario 4: Quantum Trading Complexity
     */
    async testQuantumTrading() {
        console.log(chalk.bold('\n⚛️ Test 4: Quantum Trading Complexity'));
        
        const quantumTraders = this.traders.slice(0, 20);
        const promises = [];
        
        for (const trader of quantumTraders) {
            // Select 3-5 markets for quantum position
            const numMarkets = Math.floor(Math.random() * 3) + 3;
            const selectedMarkets = this.markets
                .slice(0, numMarkets)
                .map(m => m.publicKey);
            const verses = Array(numMarkets).fill(0).map((_, i) => i + 1);
            const amount = Math.floor(Math.random() * 5000) + 1000;
            
            const promise = (async () => {
                const start = performance.now();
                try {
                    const ix = InstructionBuilder.processQuantumTrade(
                        this.programId,
                        trader.publicKey,
                        selectedMarkets,
                        verses,
                        amount
                    );
                    
                    const tx = new Transaction().add(ix);
                    await sendAndConfirmTransaction(this.connection, tx, [trader], {
                        commitment: CONFIG.COMMITMENT
                    });
                    
                    const latency = performance.now() - start;
                    metrics.instructions.processQuantum.count++;
                    metrics.instructions.processQuantum.latencies.push(latency);
                    
                    return { success: true, latency };
                } catch (error) {
                    return { success: false, error: error.message };
                }
            })();
            
            promises.push(promise);
        }
        
        const results = await Promise.allSettled(promises);
        const successful = results.filter(r => r.value?.success).length;
        
        console.log(`  Quantum positions: ${quantumTraders.length}`);
        console.log(`  Successful: ${successful}, Failed: ${quantumTraders.length - successful}`);
        
        const stats = getStats(metrics.instructions.processQuantum.latencies);
        console.log(`  Latency - Avg: ${stats.avg.toFixed(0)}ms, P95: ${stats.p95.toFixed(0)}ms`);
    }
    
    /**
     * Scenario 5: Burst Load Test
     */
    async testBurstLoad() {
        console.log(chalk.bold('\n🌊 Test 5: Burst Load Test'));
        
        const burstSize = 500;
        const instructions = [];
        
        // Build a mix of instructions
        for (let i = 0; i < burstSize; i++) {
            const trader = this.traders[i % this.traders.length];
            const market = this.markets[i % this.markets.length];
            const instructionType = i % 4;
            
            let ix;
            switch (instructionType) {
                case 0: // Place bet
                    ix = InstructionBuilder.placeBet(
                        this.programId,
                        trader.publicKey,
                        market.publicKey,
                        Math.random() > 0.5 ? 0 : 1,
                        Math.floor(Math.random() * 1000) + 100
                    );
                    break;
                case 1: // Add liquidity
                    ix = InstructionBuilder.addLiquidity(
                        this.programId,
                        trader.publicKey,
                        market.publicKey,
                        Math.floor(Math.random() * 5000) + 1000
                    );
                    break;
                case 2: // Create market
                    const newMarket = Keypair.generate();
                    ix = InstructionBuilder.createMarket(
                        this.programId,
                        trader.publicKey,
                        newMarket.publicKey,
                        `Burst Market ${i}`,
                        ['Option A', 'Option B'],
                        Math.floor(Date.now() / 1000) + 86400
                    );
                    break;
                case 3: // Another bet
                    ix = InstructionBuilder.placeBet(
                        this.programId,
                        trader.publicKey,
                        market.publicKey,
                        1,
                        Math.floor(Math.random() * 500) + 50,
                        Math.floor(Math.random() * 5) + 1
                    );
                    break;
            }
            
            instructions.push({ instruction: ix, signer: trader });
        }
        
        console.log(`  Sending ${burstSize} transactions in rapid succession...`);
        
        const startTime = performance.now();
        const promises = instructions.map(({ instruction, signer }) => {
            const tx = new Transaction().add(instruction);
            return sendAndConfirmTransaction(this.connection, tx, [signer], {
                commitment: CONFIG.COMMITMENT
            }).catch(e => ({ error: e.message }));
        });
        
        const results = await Promise.allSettled(promises);
        const duration = performance.now() - startTime;
        const successful = results.filter(r => r.status === 'fulfilled' && !r.value?.error).length;
        
        console.log(`  Burst completed in ${(duration / 1000).toFixed(2)}s`);
        console.log(`  Successful: ${successful}/${burstSize} (${((successful/burstSize) * 100).toFixed(2)}%)`);
        console.log(`  Burst TPS: ${(burstSize / (duration / 1000)).toFixed(2)}`);
    }
    
    /**
     * Print final report
     */
    printReport() {
        const duration = (Date.now() - metrics.startTime) / 1000;
        
        console.log(chalk.bold.blue('\n' + '='.repeat(60)));
        console.log(chalk.bold.blue('STRESS TEST REPORT'));
        console.log(chalk.bold.blue('='.repeat(60)));
        
        console.log(`\nTest Duration: ${duration.toFixed(2)}s`);
        console.log(`Total Transactions: ${metrics.transactions.sent}`);
        console.log(`Confirmed: ${metrics.transactions.confirmed}`);
        console.log(`Failed: ${metrics.transactions.failed}`);
        console.log(`Success Rate: ${((metrics.transactions.confirmed / metrics.transactions.sent) * 100).toFixed(2)}%`);
        console.log(`Average TPS: ${(metrics.transactions.confirmed / duration).toFixed(2)}`);
        
        console.log('\nInstruction Breakdown:');
        Object.entries(metrics.instructions).forEach(([name, data]) => {
            if (data.count > 0) {
                const stats = getStats(data.latencies);
                console.log(`  ${name}:`);
                console.log(`    Count: ${data.count}`);
                console.log(`    Avg Latency: ${stats.avg.toFixed(0)}ms`);
                console.log(`    P95 Latency: ${stats.p95.toFixed(0)}ms`);
            }
        });
        
        if (metrics.errors.length > 0) {
            console.log(`\nErrors (${metrics.errors.length} total):`);
            const errorCounts = {};
            metrics.errors.forEach(e => {
                errorCounts[e.type] = (errorCounts[e.type] || 0) + 1;
            });
            Object.entries(errorCounts).forEach(([type, count]) => {
                console.log(`  ${type}: ${count}`);
            });
        }
        
        // Performance grade
        const avgLatency = metrics.transactions.confirmed > 0 
            ? metrics.instructions.placeBet.latencies.reduce((a, b) => a + b, 0) / metrics.instructions.placeBet.latencies.length
            : 0;
        
        let grade = 'F';
        if (metrics.transactions.confirmed / metrics.transactions.sent > 0.95 && avgLatency < 500) {
            grade = 'A';
        } else if (metrics.transactions.confirmed / metrics.transactions.sent > 0.90 && avgLatency < 1000) {
            grade = 'B';
        } else if (metrics.transactions.confirmed / metrics.transactions.sent > 0.80 && avgLatency < 2000) {
            grade = 'C';
        } else if (metrics.transactions.confirmed / metrics.transactions.sent > 0.70) {
            grade = 'D';
        }
        
        console.log(`\nPerformance Grade: ${chalk.bold(grade)}`);
    }
}

// Main execution
async function main() {
    console.log(chalk.bold.cyan('🚀 Solana Smart Contract Stress Test'));
    console.log(chalk.cyan(`Program ID: ${CONFIG.PROGRAM_ID}`));
    console.log(chalk.cyan(`RPC URL: ${CONFIG.RPC_URL}\n`));
    
    const connection = new Connection(CONFIG.RPC_URL, CONFIG.COMMITMENT);
    const tester = new StressTestScenarios(connection, CONFIG.PROGRAM_ID);
    
    try {
        // Setup test environment
        await tester.setup();
        
        // Run all test scenarios
        await tester.testConcurrentMarketCreation();
        await tester.testHighFrequencyTrading();
        await tester.testLiquidityOperations();
        await tester.testQuantumTrading();
        await tester.testBurstLoad();
        
        // Print final report
        tester.printReport();
        
    } catch (error) {
        console.error(chalk.red('\nTest failed:'), error);
        process.exit(1);
    }
}

// Run stress test
main().catch(console.error);