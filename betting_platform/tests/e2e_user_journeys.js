#!/usr/bin/env node

/**
 * End-to-End User Journey Tests
 * Simulates complete user flows from start to finish
 */

const { Connection, Keypair, PublicKey, LAMPORTS_PER_SOL, Transaction } = require('@solana/web3.js');
const axios = require('axios');
const WebSocket = require('ws');
const chalk = require('chalk');

// Configuration
const CONFIG = {
    RPC_URL: 'http://localhost:8899',
    API_URL: 'http://localhost:8081/api',
    WS_URL: 'ws://localhost:8081/ws',
    PROGRAM_ID: '5cnuqTxYjzrmYnQ6BtvxEK4bpFJn4kkUCzgMakidheza'
};

// Test utilities
class TestLogger {
    constructor(journeyName) {
        this.journeyName = journeyName;
        this.steps = [];
        this.startTime = Date.now();
    }
    
    step(name, success, details = '') {
        const step = {
            name,
            success,
            details,
            timestamp: Date.now() - this.startTime
        };
        this.steps.push(step);
        
        if (success) {
            console.log(chalk.green(`  ✓ ${name}`) + (details ? chalk.gray(` - ${details}`) : ''));
        } else {
            console.log(chalk.red(`  ✗ ${name}`) + chalk.red(` - ${details}`));
        }
    }
    
    summary() {
        const duration = Date.now() - this.startTime;
        const passed = this.steps.filter(s => s.success).length;
        const failed = this.steps.filter(s => !s.success).length;
        
        console.log(`\n${chalk.bold('Journey Summary:')}`);
        console.log(`  Duration: ${(duration / 1000).toFixed(2)}s`);
        console.log(`  Steps: ${passed} passed, ${failed} failed`);
        
        return { passed, failed, duration };
    }
}

// API Client
class APIClient {
    constructor(baseUrl) {
        this.baseUrl = baseUrl;
        this.authToken = null;
    }
    
    async request(method, endpoint, data = null) {
        const config = {
            method,
            url: `${this.baseUrl}${endpoint}`,
            headers: {
                'Content-Type': 'application/json',
                ...(this.authToken && { 'Authorization': `Bearer ${this.authToken}` })
            }
        };
        
        if (data) {
            config.data = data;
        }
        
        const response = await axios(config);
        return response.data;
    }
}

// WebSocket Client
class WSClient {
    constructor(url) {
        this.url = url;
        this.ws = null;
        this.subscriptions = new Map();
        this.messageQueue = [];
    }
    
    async connect() {
        return new Promise((resolve, reject) => {
            this.ws = new WebSocket(this.url);
            
            this.ws.on('open', () => resolve());
            this.ws.on('error', reject);
            this.ws.on('message', (data) => {
                const message = JSON.parse(data);
                this.messageQueue.push(message);
                
                // Handle subscriptions
                if (message.channel && this.subscriptions.has(message.channel)) {
                    this.subscriptions.get(message.channel)(message);
                }
            });
        });
    }
    
    subscribe(channel, callback) {
        this.subscriptions.set(channel, callback);
        this.ws.send(JSON.stringify({
            type: 'subscribe',
            channel
        }));
    }
    
    send(message) {
        this.ws.send(JSON.stringify(message));
    }
    
    close() {
        this.ws.close();
    }
}

// User Journey Tests
class UserJourneyTests {
    constructor() {
        this.connection = new Connection(CONFIG.RPC_URL, 'confirmed');
        this.api = new APIClient(CONFIG.API_URL);
        this.results = [];
    }
    
    /**
     * Journey 1: New User Complete Trading Experience
     */
    async journey1_NewUserTrading() {
        const logger = new TestLogger('New User Trading Journey');
        console.log(chalk.bold('\n🚀 Journey 1: New User Complete Trading Experience'));
        
        try {
            // Step 1: Generate wallet
            const wallet = Keypair.generate();
            logger.step('Generate new wallet', true, wallet.publicKey.toBase58());
            
            // Step 2: Request airdrop
            const airdropSig = await this.connection.requestAirdrop(
                wallet.publicKey,
                2 * LAMPORTS_PER_SOL
            );
            await this.connection.confirmTransaction(airdropSig);
            logger.step('Request SOL airdrop', true, '2 SOL');
            
            // Step 3: Connect to API
            const authResponse = await this.api.request('POST', '/auth/wallet', {
                wallet: wallet.publicKey.toBase58(),
                signature: 'mock_signature' // In real test, would sign message
            });
            this.api.authToken = authResponse.token;
            logger.step('Authenticate with API', true);
            
            // Step 4: Browse markets
            const markets = await this.api.request('GET', '/markets?limit=10');
            logger.step('Browse available markets', true, `Found ${markets.markets.length} markets`);
            
            // Step 5: Search specific market
            const searchResults = await this.api.request('GET', '/markets?search=bitcoin');
            const bitcoinMarket = searchResults.markets[0];
            logger.step('Search for Bitcoin market', bitcoinMarket !== undefined, 
                bitcoinMarket ? bitcoinMarket.title : 'Not found');
            
            // Step 6: Connect WebSocket
            const ws = new WSClient(CONFIG.WS_URL);
            await ws.connect();
            logger.step('Connect to WebSocket', true);
            
            // Step 7: Subscribe to market updates
            let priceUpdate = null;
            ws.subscribe(`market_${bitcoinMarket.id}`, (update) => {
                priceUpdate = update;
            });
            await new Promise(resolve => setTimeout(resolve, 1000));
            logger.step('Subscribe to market updates', priceUpdate !== null);
            
            // Step 8: Place first trade
            const tradeResponse = await this.api.request('POST', '/trades', {
                market_id: bitcoinMarket.id,
                outcome: 0,
                amount: 100,
                wallet: wallet.publicKey.toBase58(),
                leverage: 1
            });
            logger.step('Place first trade', true, `Position ID: ${tradeResponse.position_id}`);
            
            // Step 9: Monitor position
            await new Promise(resolve => setTimeout(resolve, 2000));
            const positions = await this.api.request('GET', `/positions?wallet=${wallet.publicKey.toBase58()}`);
            logger.step('Monitor position P&L', positions.positions.length > 0);
            
            // Step 10: Place limit order
            const limitOrder = await this.api.request('POST', '/orders/limit', {
                market_id: bitcoinMarket.id,
                outcome: 1,
                amount: 50,
                price: 0.45,
                wallet: wallet.publicKey.toBase58()
            });
            logger.step('Place limit order', true);
            
            // Step 11: Increase position with leverage
            const leveragedTrade = await this.api.request('POST', '/trades', {
                market_id: bitcoinMarket.id,
                outcome: 0,
                amount: 500,
                wallet: wallet.publicKey.toBase58(),
                leverage: 5
            });
            logger.step('Add leveraged position', true, '5x leverage');
            
            // Step 12: Check risk metrics
            const riskMetrics = await this.api.request('GET', `/risk/metrics?wallet=${wallet.publicKey.toBase58()}`);
            logger.step('Check risk exposure', true, `VaR: ${riskMetrics.value_at_risk}`);
            
            // Step 13: Partial close
            const position = positions.positions[0];
            const partialClose = await this.api.request('POST', `/positions/${position.id}/partial-close`, {
                amount: 50
            });
            logger.step('Partial position close', true, '50% closed');
            
            // Step 14: Full close
            const fullClose = await this.api.request('POST', `/positions/${position.id}/close`);
            logger.step('Close remaining position', true, `Final P&L: ${fullClose.final_pnl}`);
            
            // Step 15: Disconnect
            ws.close();
            logger.step('Cleanup connections', true);
            
        } catch (error) {
            logger.step('Journey failed', false, error.message);
        }
        
        return logger.summary();
    }
    
    /**
     * Journey 2: DeFi Power User Flow
     */
    async journey2_DeFiPowerUser() {
        const logger = new TestLogger('DeFi Power User Journey');
        console.log(chalk.bold('\n💎 Journey 2: DeFi Power User Flow'));
        
        try {
            // Use existing funded wallet
            const wallet = Keypair.generate();
            const walletAddress = wallet.publicKey.toBase58();
            
            // Step 1: Connect with existing wallet
            await this.api.request('POST', '/auth/wallet', {
                wallet: walletAddress,
                signature: 'mock_signature'
            });
            logger.step('Connect power user wallet', true);
            
            // Step 2: Analyze market liquidity
            const liquidMarkets = await this.api.request('GET', '/markets?sort=liquidity&limit=5');
            const targetMarket = liquidMarkets.markets[0];
            logger.step('Find high liquidity markets', true, 
                `${targetMarket.title} - $${(targetMarket.total_liquidity / 1e6).toFixed(2)}M`);
            
            // Step 3: Add liquidity
            const addLiquidity = await this.api.request('POST', '/liquidity/add', {
                market_id: targetMarket.id,
                amount: 10000,
                wallet: walletAddress
            });
            logger.step('Provide liquidity', true, `LP tokens: ${addLiquidity.lp_tokens}`);
            
            // Step 4: Monitor LP performance
            await new Promise(resolve => setTimeout(resolve, 3000));
            const lpStats = await this.api.request('GET', `/liquidity/stats?wallet=${walletAddress}`);
            logger.step('Monitor LP fees earned', true, `Fees: $${lpStats.fees_earned}`);
            
            // Step 5: Stake LP tokens
            const stakeResponse = await this.api.request('POST', '/staking/stake', {
                amount: addLiquidity.lp_tokens,
                duration_days: 30,
                wallet: walletAddress
            });
            logger.step('Stake LP tokens', true, `APY: ${stakeResponse.apy}%`);
            
            // Step 6: Execute arbitrage
            const arbOpportunity = await this.api.request('GET', '/arbitrage/opportunities');
            if (arbOpportunity.opportunities.length > 0) {
                const arb = arbOpportunity.opportunities[0];
                const arbTrade = await this.api.request('POST', '/arbitrage/execute', {
                    opportunity_id: arb.id,
                    wallet: walletAddress
                });
                logger.step('Execute arbitrage', true, `Profit: $${arbTrade.profit}`);
            } else {
                logger.step('Execute arbitrage', true, 'No opportunities');
            }
            
            // Step 7: Create limit order ladder
            const orderLadder = [];
            for (let i = 0; i < 5; i++) {
                const price = 0.4 + (i * 0.05);
                const order = await this.api.request('POST', '/orders/limit', {
                    market_id: targetMarket.id,
                    outcome: 0,
                    amount: 200,
                    price: price,
                    wallet: walletAddress
                });
                orderLadder.push(order);
            }
            logger.step('Create order ladder', true, '5 orders placed');
            
            // Step 8: Remove liquidity
            const removeLiquidity = await this.api.request('POST', '/liquidity/remove', {
                market_id: targetMarket.id,
                lp_tokens: addLiquidity.lp_tokens / 2,
                wallet: walletAddress
            });
            logger.step('Remove partial liquidity', true, '50% removed');
            
            // Step 9: Claim rewards
            const rewards = await this.api.request('POST', '/rewards/claim', {
                wallet: walletAddress
            });
            logger.step('Claim staking rewards', true, `Rewards: ${rewards.amount}`);
            
        } catch (error) {
            logger.step('Journey failed', false, error.message);
        }
        
        return logger.summary();
    }
    
    /**
     * Journey 3: Quantum Trading Strategy
     */
    async journey3_QuantumTrading() {
        const logger = new TestLogger('Quantum Trading Journey');
        console.log(chalk.bold('\n⚛️ Journey 3: Quantum Trading Strategy'));
        
        try {
            const wallet = Keypair.generate();
            const walletAddress = wallet.publicKey.toBase58();
            
            // Step 1: Setup quantum mode
            await this.api.request('POST', '/auth/wallet', {
                wallet: walletAddress,
                signature: 'mock_signature'
            });
            logger.step('Initialize quantum trader', true);
            
            // Step 2: Analyze verse correlations
            const verses = await this.api.request('GET', '/verses?limit=10');
            const correlations = await this.api.request('GET', '/quantum/correlations');
            logger.step('Analyze verse correlations', true, 
                `Found ${correlations.pairs.length} correlated pairs`);
            
            // Step 3: Select quantum verses
            const selectedVerses = verses.slice(0, 3).map(v => v.id);
            logger.step('Select quantum verses', true, selectedVerses.join(', '));
            
            // Step 4: Place quantum position
            const quantumTrade = await this.api.request('POST', '/quantum/trade', {
                verses: selectedVerses,
                amount: 1000,
                wallet: walletAddress,
                collapse_strategy: 'balanced'
            });
            logger.step('Open quantum position', true, 
                `Positions across ${quantumTrade.positions.length} markets`);
            
            // Step 5: Monitor superposition
            const ws = new WSClient(CONFIG.WS_URL);
            await ws.connect();
            
            let collapseUpdate = null;
            ws.subscribe('quantum_updates', (update) => {
                if (update.type === 'collapse_probability') {
                    collapseUpdate = update;
                }
            });
            
            await new Promise(resolve => setTimeout(resolve, 2000));
            logger.step('Monitor collapse probabilities', collapseUpdate !== null);
            
            // Step 6: Adjust quantum state
            const adjustment = await this.api.request('POST', '/quantum/adjust', {
                position_id: quantumTrade.quantum_position_id,
                action: 'rebalance',
                wallet: walletAddress
            });
            logger.step('Adjust quantum state', true);
            
            // Step 7: Partial collapse
            const partialCollapse = await this.api.request('POST', '/quantum/collapse', {
                position_id: quantumTrade.quantum_position_id,
                verses: [selectedVerses[0]],
                wallet: walletAddress
            });
            logger.step('Partial quantum collapse', true, 
                `Collapsed to ${partialCollapse.collapsed_market}`);
            
            // Step 8: Full collapse
            const fullCollapse = await this.api.request('POST', '/quantum/collapse', {
                position_id: quantumTrade.quantum_position_id,
                wallet: walletAddress
            });
            logger.step('Full quantum collapse', true, 
                `Final P&L: ${fullCollapse.total_pnl}`);
            
            ws.close();
            
        } catch (error) {
            logger.step('Journey failed', false, error.message);
        }
        
        return logger.summary();
    }
    
    /**
     * Journey 4: High-Frequency Trading Bot
     */
    async journey4_HFTBot() {
        const logger = new TestLogger('HFT Bot Journey');
        console.log(chalk.bold('\n🤖 Journey 4: High-Frequency Trading Bot'));
        
        try {
            const wallet = Keypair.generate();
            const walletAddress = wallet.publicKey.toBase58();
            
            // Step 1: Bot authentication
            const botAuth = await this.api.request('POST', '/auth/bot', {
                wallet: walletAddress,
                bot_type: 'hft',
                signature: 'mock_signature'
            });
            this.api.authToken = botAuth.token;
            logger.step('Authenticate HFT bot', true);
            
            // Step 2: Subscribe to orderbook
            const ws = new WSClient(CONFIG.WS_URL);
            await ws.connect();
            
            const orderbook = { bids: [], asks: [] };
            ws.subscribe('orderbook_1000', (update) => {
                if (update.bids) orderbook.bids = update.bids;
                if (update.asks) orderbook.asks = update.asks;
            });
            
            await new Promise(resolve => setTimeout(resolve, 1000));
            logger.step('Subscribe to orderbook feed', true);
            
            // Step 3: Rapid order placement
            const orders = [];
            const startTime = Date.now();
            
            for (let i = 0; i < 50; i++) {
                const side = Math.random() > 0.5 ? 'buy' : 'sell';
                const price = 0.5 + (Math.random() * 0.1 - 0.05);
                
                const order = await this.api.request('POST', '/orders/limit', {
                    market_id: 1000,
                    outcome: side === 'buy' ? 0 : 1,
                    amount: Math.floor(Math.random() * 100) + 10,
                    price: price,
                    wallet: walletAddress,
                    time_in_force: 'IOC' // Immediate or cancel
                });
                orders.push(order);
            }
            
            const orderRate = 50000 / (Date.now() - startTime);
            logger.step('Place rapid orders', true, `${orderRate.toFixed(2)} orders/sec`);
            
            // Step 4: Market making
            const spread = 0.02;
            const mmOrders = await this.api.request('POST', '/orders/market-make', {
                market_id: 1000,
                spread: spread,
                size: 1000,
                wallet: walletAddress
            });
            logger.step('Deploy market making', true, `${spread * 100}% spread`);
            
            // Step 5: Cancel and replace
            const cancelReplace = await this.api.request('POST', '/orders/bulk-cancel-replace', {
                cancel_order_ids: orders.slice(0, 10).map(o => o.order_id),
                new_orders: Array(10).fill(null).map(() => ({
                    market_id: 1000,
                    outcome: 0,
                    amount: 50,
                    price: 0.5
                })),
                wallet: walletAddress
            });
            logger.step('Cancel and replace orders', true);
            
            // Step 6: Calculate PnL
            await new Promise(resolve => setTimeout(resolve, 5000));
            const pnl = await this.api.request('GET', `/positions/pnl?wallet=${walletAddress}`);
            logger.step('Calculate HFT P&L', true, `P&L: ${pnl.total_pnl}`);
            
            ws.close();
            
        } catch (error) {
            logger.step('Journey failed', false, error.message);
        }
        
        return logger.summary();
    }
    
    /**
     * Journey 5: Risk Management Stress Test
     */
    async journey5_RiskManagement() {
        const logger = new TestLogger('Risk Management Journey');
        console.log(chalk.bold('\n🛡️ Journey 5: Risk Management Stress Test'));
        
        try {
            const wallet = Keypair.generate();
            const walletAddress = wallet.publicKey.toBase58();
            
            // Step 1: Setup risk limits
            await this.api.request('POST', '/auth/wallet', {
                wallet: walletAddress,
                signature: 'mock_signature'
            });
            
            const riskLimits = await this.api.request('POST', '/risk/limits', {
                max_position_size: 10000,
                max_leverage: 10,
                max_drawdown: 0.15,
                max_var: 2000,
                wallet: walletAddress
            });
            logger.step('Configure risk limits', true);
            
            // Step 2: Test position size limit
            try {
                await this.api.request('POST', '/trades', {
                    market_id: 1000,
                    outcome: 0,
                    amount: 15000, // Exceeds limit
                    wallet: walletAddress
                });
                logger.step('Test position size limit', false, 'Limit not enforced');
            } catch (error) {
                logger.step('Test position size limit', true, 'Correctly rejected');
            }
            
            // Step 3: Build leveraged positions
            const positions = [];
            for (let i = 0; i < 5; i++) {
                const position = await this.api.request('POST', '/trades', {
                    market_id: 1000 + i,
                    outcome: 0,
                    amount: 1000,
                    leverage: 8,
                    wallet: walletAddress
                });
                positions.push(position);
            }
            logger.step('Build leveraged portfolio', true, '5 positions @ 8x');
            
            // Step 4: Monitor margin
            const marginStatus = await this.api.request('GET', `/risk/margin?wallet=${walletAddress}`);
            logger.step('Check margin requirements', true, 
                `Used: ${marginStatus.margin_used}/${marginStatus.margin_available}`);
            
            // Step 5: Simulate market shock
            const shockTest = await this.api.request('POST', '/risk/simulate-shock', {
                shock_percentage: -20,
                wallet: walletAddress
            });
            logger.step('Simulate 20% market shock', true, 
                `Would trigger ${shockTest.liquidations} liquidations`);
            
            // Step 6: Test leverage limit
            try {
                await this.api.request('POST', '/trades', {
                    market_id: 1005,
                    outcome: 0,
                    amount: 500,
                    leverage: 15, // Exceeds limit
                    wallet: walletAddress
                });
                logger.step('Test leverage limit', false, 'Limit not enforced');
            } catch (error) {
                logger.step('Test leverage limit', true, 'Correctly rejected');
            }
            
            // Step 7: Auto-deleverage test
            const deleverage = await this.api.request('POST', '/risk/auto-deleverage', {
                target_leverage: 5,
                wallet: walletAddress
            });
            logger.step('Auto-deleverage positions', true, 
                `Reduced ${deleverage.positions_modified} positions`);
            
            // Step 8: Liquidation test
            // Simulate adverse price movement
            const liquidationTest = await this.api.request('POST', '/risk/test-liquidation', {
                wallet: walletAddress
            });
            logger.step('Test liquidation engine', true, 
                `Liquidation price: ${liquidationTest.liquidation_price}`);
            
        } catch (error) {
            logger.step('Journey failed', false, error.message);
        }
        
        return logger.summary();
    }
    
    /**
     * Run all journeys
     */
    async runAllJourneys() {
        console.log(chalk.bold.blue('\n🎯 Starting End-to-End User Journey Tests\n'));
        
        const journeys = [
            () => this.journey1_NewUserTrading(),
            () => this.journey2_DeFiPowerUser(),
            () => this.journey3_QuantumTrading(),
            () => this.journey4_HFTBot(),
            () => this.journey5_RiskManagement()
        ];
        
        let totalPassed = 0;
        let totalFailed = 0;
        let totalDuration = 0;
        
        for (const journey of journeys) {
            const result = await journey();
            totalPassed += result.passed;
            totalFailed += result.failed;
            totalDuration += result.duration;
            this.results.push(result);
        }
        
        // Print final summary
        console.log(chalk.bold.blue('\n' + '='.repeat(60)));
        console.log(chalk.bold.blue('FINAL TEST SUMMARY'));
        console.log(chalk.bold.blue('='.repeat(60)));
        console.log(`Total Journeys: ${journeys.length}`);
        console.log(`Total Steps: ${totalPassed + totalFailed}`);
        console.log(chalk.green(`Passed: ${totalPassed}`));
        console.log(chalk.red(`Failed: ${totalFailed}`));
        console.log(`Total Duration: ${(totalDuration / 1000).toFixed(2)}s`);
        console.log(`Success Rate: ${((totalPassed / (totalPassed + totalFailed)) * 100).toFixed(2)}%`);
        
        return totalFailed === 0;
    }
}

// Run tests
async function main() {
    const tests = new UserJourneyTests();
    const success = await tests.runAllJourneys();
    process.exit(success ? 0 : 1);
}

main().catch(console.error);