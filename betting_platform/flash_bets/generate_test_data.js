#!/usr/bin/env node

/**
 * TEST DATA GENERATOR FOR FLASH BETTING
 * 
 * Generates comprehensive test data for 100+ user journeys including:
 * - User profiles (10,000+ variations)
 * - Market data (1,000+ markets)
 * - Historical patterns
 * - Edge case scenarios
 * - Performance benchmarks
 */

const crypto = require('crypto');
const fs = require('fs');

class TestDataGenerator {
    constructor() {
        this.users = [];
        this.markets = [];
        this.historicalData = [];
        this.edgeCases = [];
    }

    // ==================== USER GENERATION ====================

    /**
     * Generate diverse user profiles
     */
    generateUsers(count = 10000) {
        console.log(`\n👥 Generating ${count.toLocaleString()} user profiles...`);
        
        const userTypes = [
            { type: 'novice', weight: 0.3, avgBalance: 100, avgBet: 10 },
            { type: 'regular', weight: 0.4, avgBalance: 1000, avgBet: 50 },
            { type: 'experienced', weight: 0.2, avgBalance: 5000, avgBet: 200 },
            { type: 'whale', weight: 0.05, avgBalance: 100000, avgBet: 5000 },
            { type: 'bot', weight: 0.03, avgBalance: 10000, avgBet: 100 },
            { type: 'micro', weight: 0.02, avgBalance: 50, avgBet: 1 }
        ];
        
        const regions = [
            'North America', 'Europe', 'Asia', 'South America', 
            'Africa', 'Oceania', 'Middle East'
        ];
        
        const devices = [
            'iPhone', 'Android', 'Desktop Chrome', 'Desktop Firefox',
            'iPad', 'Desktop Safari', 'Desktop Edge'
        ];
        
        for (let i = 0; i < count; i++) {
            const userType = this.weightedRandom(userTypes);
            
            const user = {
                id: `user_${crypto.randomBytes(8).toString('hex')}`,
                type: userType.type,
                wallet: `0x${crypto.randomBytes(20).toString('hex')}`,
                balance: this.randomInRange(userType.avgBalance * 0.5, userType.avgBalance * 2),
                region: regions[Math.floor(Math.random() * regions.length)],
                device: devices[Math.floor(Math.random() * devices.length)],
                created: new Date(Date.now() - Math.random() * 365 * 24 * 60 * 60 * 1000),
                stats: {
                    totalBets: Math.floor(Math.random() * 1000),
                    winRate: 0.4 + Math.random() * 0.3,
                    avgBet: userType.avgBet,
                    preferredLeverage: this.randomInRange(75, 500),
                    preferredTimeframe: this.randomChoice(['ultra', 'quick', 'match']),
                    riskProfile: this.randomChoice(['conservative', 'moderate', 'aggressive'])
                },
                kyc: {
                    level: userType.type === 'whale' ? 'full' : 
                           userType.type === 'bot' ? 'none' : 'basic',
                    country: this.randomCountry(),
                    verified: Math.random() > 0.2
                }
            };
            
            this.users.push(user);
            
            if ((i + 1) % 1000 === 0) {
                console.log(`  Generated ${i + 1} users...`);
            }
        }
        
        console.log(`✅ Generated ${this.users.length} unique user profiles`);
        return this.users;
    }

    // ==================== MARKET GENERATION ====================

    /**
     * Generate diverse flash markets
     */
    generateMarkets(count = 1000) {
        console.log(`\n📊 Generating ${count.toLocaleString()} flash markets...`);
        
        const sports = [
            { name: 'soccer', events: ['Next Goal', 'Corner', 'Card', 'Penalty'] },
            { name: 'basketball', events: ['Next Point', 'Quarter Winner', '3-Pointer', 'Free Throw'] },
            { name: 'tennis', events: ['Next Point', 'Game Winner', 'Ace', 'Break Point'] },
            { name: 'football', events: ['Next TD', 'Field Goal', 'Turnover', 'First Down'] },
            { name: 'baseball', events: ['Next Hit', 'Home Run', 'Strike Out', 'Inning Runs'] },
            { name: 'cricket', events: ['Next Wicket', 'Six', 'Boundary', 'Run Rate'] }
        ];
        
        const timeframes = [
            { name: 'ultra', min: 5, max: 60 },
            { name: 'quick', min: 60, max: 600 },
            { name: 'match', min: 600, max: 14400 }
        ];
        
        for (let i = 0; i < count; i++) {
            const sport = sports[Math.floor(Math.random() * sports.length)];
            const event = sport.events[Math.floor(Math.random() * sport.events.length)];
            const timeframe = timeframes[Math.floor(Math.random() * timeframes.length)];
            
            const market = {
                id: `market_${crypto.randomBytes(8).toString('hex')}`,
                title: `${sport.name.toUpperCase()} - ${event}`,
                sport: sport.name,
                event: event,
                timeLeft: this.randomInRange(timeframe.min, timeframe.max),
                timeframe: timeframe.name,
                outcomes: this.generateOutcomes(sport.name, event),
                liquidity: this.generateLiquidity(timeframe.name),
                volume: this.randomInRange(1000, 1000000),
                maxLeverage: this.getMaxLeverage(timeframe.name),
                created: new Date(),
                status: 'active',
                metadata: {
                    temperature: 0.5 + Math.random() * 0.5, // Market heat
                    volatility: Math.random(),
                    participants: Math.floor(10 + Math.random() * 990),
                    avgBet: this.randomInRange(10, 1000),
                    provider: this.randomChoice(['DraftKings', 'FanDuel', 'BetMGM', 'Caesars'])
                }
            };
            
            this.markets.push(market);
        }
        
        console.log(`✅ Generated ${this.markets.length} unique markets`);
        return this.markets;
    }

    /**
     * Generate market outcomes with odds
     */
    generateOutcomes(sport, event) {
        const outcomes = [];
        
        // Binary outcomes
        if (Math.random() > 0.3) {
            outcomes.push(
                { name: 'Yes', odds: 1.5 + Math.random() * 2 },
                { name: 'No', odds: 1.5 + Math.random() * 2 }
            );
        } else {
            // Multi-outcome
            const options = this.getOutcomeOptions(sport, event);
            for (const option of options) {
                outcomes.push({
                    name: option,
                    odds: 2 + Math.random() * 8
                });
            }
        }
        
        // Normalize odds
        const totalProb = outcomes.reduce((sum, o) => sum + (1 / o.odds), 0);
        outcomes.forEach(o => {
            o.probability = (1 / o.odds) / totalProb;
            o.impliedOdds = 1 / o.probability;
        });
        
        return outcomes;
    }

    /**
     * Get outcome options based on sport and event
     */
    getOutcomeOptions(sport, event) {
        const options = {
            'soccer': {
                'Next Goal': ['Team A', 'Team B', 'No Goal'],
                'Corner': ['0-2', '3-5', '6-8', '9+'],
                'Card': ['Yellow', 'Red', 'None'],
                'Penalty': ['Scored', 'Missed', 'None']
            },
            'basketball': {
                'Next Point': ['Team A', 'Team B'],
                'Quarter Winner': ['Home', 'Away', 'Tie'],
                '3-Pointer': ['Made', 'Missed'],
                'Free Throw': ['Made', 'Missed']
            },
            'tennis': {
                'Next Point': ['Player A', 'Player B'],
                'Game Winner': ['Player A', 'Player B'],
                'Ace': ['Yes', 'No'],
                'Break Point': ['Converted', 'Saved']
            }
        };
        
        return options[sport]?.[event] || ['Option A', 'Option B', 'Option C'];
    }

    /**
     * Generate liquidity based on timeframe
     */
    generateLiquidity(timeframe) {
        const baseLiquidity = {
            'ultra': this.randomInRange(10000, 100000),
            'quick': this.randomInRange(50000, 500000),
            'match': this.randomInRange(100000, 5000000)
        };
        
        return baseLiquidity[timeframe] || 50000;
    }

    /**
     * Get max leverage for timeframe
     */
    getMaxLeverage(timeframe) {
        const leverage = {
            'ultra': 500,
            'quick': 250,
            'match': 100
        };
        
        return leverage[timeframe] || 100;
    }

    // ==================== HISTORICAL DATA ====================

    /**
     * Generate historical betting patterns
     */
    generateHistoricalData(days = 30) {
        console.log(`\n📈 Generating ${days} days of historical data...`);
        
        const startDate = new Date(Date.now() - days * 24 * 60 * 60 * 1000);
        
        for (let day = 0; day < days; day++) {
            const date = new Date(startDate.getTime() + day * 24 * 60 * 60 * 1000);
            
            const dayData = {
                date: date.toISOString().split('T')[0],
                metrics: {
                    totalVolume: this.randomInRange(1000000, 10000000),
                    totalBets: this.randomInRange(10000, 100000),
                    uniqueUsers: this.randomInRange(1000, 10000),
                    avgBetSize: this.randomInRange(50, 500),
                    winRate: 0.48 + Math.random() * 0.04,
                    topMarket: this.randomChoice(['Soccer', 'Basketball', 'Tennis']),
                    peakHour: Math.floor(Math.random() * 24),
                    liquidations: Math.floor(Math.random() * 100),
                    newUsers: Math.floor(100 + Math.random() * 500)
                },
                events: this.generateDailyEvents(),
                patterns: this.generatePatterns(date)
            };
            
            this.historicalData.push(dayData);
        }
        
        console.log(`✅ Generated ${days} days of historical data`);
        return this.historicalData;
    }

    /**
     * Generate daily events
     */
    generateDailyEvents() {
        const events = [];
        const eventTypes = [
            'large_win', 'large_loss', 'new_whale', 'high_volume',
            'system_update', 'promotion', 'tournament', 'milestone'
        ];
        
        const numEvents = Math.floor(Math.random() * 5);
        for (let i = 0; i < numEvents; i++) {
            events.push({
                type: this.randomChoice(eventTypes),
                time: `${Math.floor(Math.random() * 24)}:${Math.floor(Math.random() * 60)}`,
                impact: this.randomChoice(['low', 'medium', 'high']),
                details: `Event ${i + 1} details`
            });
        }
        
        return events;
    }

    /**
     * Generate betting patterns
     */
    generatePatterns(date) {
        const dayOfWeek = date.getDay();
        const isWeekend = dayOfWeek === 0 || dayOfWeek === 6;
        
        return {
            dayType: isWeekend ? 'weekend' : 'weekday',
            peakPeriods: isWeekend ? ['afternoon', 'evening'] : ['lunch', 'evening'],
            dominantRegion: this.randomChoice(['NA', 'EU', 'ASIA']),
            trendDirection: this.randomChoice(['up', 'down', 'stable']),
            volatility: isWeekend ? 'high' : 'medium',
            seasonality: this.getSeason(date)
        };
    }

    /**
     * Get season for date
     */
    getSeason(date) {
        const month = date.getMonth();
        if (month >= 2 && month <= 4) return 'spring';
        if (month >= 5 && month <= 7) return 'summer';
        if (month >= 8 && month <= 10) return 'fall';
        return 'winter';
    }

    // ==================== EDGE CASES ====================

    /**
     * Generate edge case scenarios
     */
    generateEdgeCases() {
        console.log('\n⚠️ Generating edge case scenarios...');
        
        this.edgeCases = [
            // Extreme values
            { category: 'extreme', case: 'zero_balance', data: { balance: 0 } },
            { category: 'extreme', case: 'max_int_balance', data: { balance: Number.MAX_SAFE_INTEGER } },
            { category: 'extreme', case: 'negative_balance', data: { balance: -1000 } },
            { category: 'extreme', case: 'infinite_leverage', data: { leverage: Infinity } },
            
            // Time edge cases
            { category: 'time', case: 'expired_market', data: { timeLeft: -1 } },
            { category: 'time', case: 'instant_market', data: { timeLeft: 0 } },
            { category: 'time', case: 'year_long_market', data: { timeLeft: 31536000 } },
            
            // Invalid data
            { category: 'invalid', case: 'null_user', data: { user: null } },
            { category: 'invalid', case: 'undefined_market', data: { market: undefined } },
            { category: 'invalid', case: 'empty_outcomes', data: { outcomes: [] } },
            { category: 'invalid', case: 'nan_odds', data: { odds: NaN } },
            
            // Malicious inputs
            { category: 'malicious', case: 'sql_injection', data: { input: "'; DROP TABLE users; --" } },
            { category: 'malicious', case: 'xss_attack', data: { input: "<script>alert('XSS')</script>" } },
            { category: 'malicious', case: 'buffer_overflow', data: { input: 'A'.repeat(1000000) } },
            
            // Network conditions
            { category: 'network', case: 'timeout', data: { latency: 30000 } },
            { category: 'network', case: 'packet_loss', data: { loss: 0.5 } },
            { category: 'network', case: 'disconnection', data: { connected: false } },
            
            // Concurrent operations
            { category: 'concurrent', case: 'race_condition', data: { simultaneous: 1000 } },
            { category: 'concurrent', case: 'deadlock', data: { locks: ['A->B', 'B->A'] } },
            { category: 'concurrent', case: 'double_spend', data: { attempts: 2 } },
            
            // System limits
            { category: 'limits', case: 'max_positions', data: { positions: 10000 } },
            { category: 'limits', case: 'rate_limit', data: { requests: 10000 } },
            { category: 'limits', case: 'memory_exhaustion', data: { memory: '16GB' } }
        ];
        
        console.log(`✅ Generated ${this.edgeCases.length} edge case scenarios`);
        return this.edgeCases;
    }

    // ==================== PERFORMANCE BENCHMARKS ====================

    /**
     * Generate performance benchmark data
     */
    generateBenchmarks() {
        console.log('\n⚡ Generating performance benchmarks...');
        
        const benchmarks = {
            latency: {
                p50: 25,
                p95: 100,
                p99: 250,
                p999: 1000,
                max: 5000
            },
            throughput: {
                ordersPerSecond: 10000,
                matchesPerSecond: 5000,
                settlementsPerSecond: 1000,
                zkProofsPerSecond: 100
            },
            capacity: {
                maxConcurrentUsers: 100000,
                maxOpenPositions: 1000000,
                maxMarketsActive: 10000,
                maxOrderBookDepth: 1000
            },
            reliability: {
                uptime: 99.99,
                dataConsistency: 100,
                fundsSafety: 100,
                disasterRecovery: 99.9
            },
            scalability: {
                horizontalScaling: true,
                autoScaling: true,
                sharding: true,
                loadBalancing: true
            }
        };
        
        console.log('✅ Generated performance benchmarks');
        return benchmarks;
    }

    // ==================== UTILITY METHODS ====================

    randomInRange(min, max) {
        return Math.floor(min + Math.random() * (max - min));
    }

    randomChoice(array) {
        return array[Math.floor(Math.random() * array.length)];
    }

    weightedRandom(items) {
        const total = items.reduce((sum, item) => sum + item.weight, 0);
        let random = Math.random() * total;
        
        for (const item of items) {
            random -= item.weight;
            if (random <= 0) return item;
        }
        
        return items[items.length - 1];
    }

    randomCountry() {
        const countries = [
            'USA', 'UK', 'Germany', 'France', 'Japan', 'China',
            'Brazil', 'India', 'Australia', 'Canada', 'Mexico',
            'Spain', 'Italy', 'Netherlands', 'Sweden', 'Switzerland'
        ];
        return this.randomChoice(countries);
    }

    // ==================== EXPORT METHODS ====================

    /**
     * Save all generated data to files
     */
    saveToFiles() {
        console.log('\n💾 Saving test data to files...');
        
        // Save users
        fs.writeFileSync(
            'test_data_users.json',
            JSON.stringify(this.users, null, 2)
        );
        console.log(`  ✓ Saved ${this.users.length} users to test_data_users.json`);
        
        // Save markets
        fs.writeFileSync(
            'test_data_markets.json',
            JSON.stringify(this.markets, null, 2)
        );
        console.log(`  ✓ Saved ${this.markets.length} markets to test_data_markets.json`);
        
        // Save historical data
        fs.writeFileSync(
            'test_data_historical.json',
            JSON.stringify(this.historicalData, null, 2)
        );
        console.log(`  ✓ Saved ${this.historicalData.length} days to test_data_historical.json`);
        
        // Save edge cases
        fs.writeFileSync(
            'test_data_edge_cases.json',
            JSON.stringify(this.edgeCases, null, 2)
        );
        console.log(`  ✓ Saved ${this.edgeCases.length} edge cases to test_data_edge_cases.json`);
        
        // Save summary
        const summary = {
            generated: new Date().toISOString(),
            users: this.users.length,
            markets: this.markets.length,
            historicalDays: this.historicalData.length,
            edgeCases: this.edgeCases.length,
            totalSize: {
                users: `${(JSON.stringify(this.users).length / 1024 / 1024).toFixed(2)} MB`,
                markets: `${(JSON.stringify(this.markets).length / 1024 / 1024).toFixed(2)} MB`,
                historical: `${(JSON.stringify(this.historicalData).length / 1024 / 1024).toFixed(2)} MB`,
                edgeCases: `${(JSON.stringify(this.edgeCases).length / 1024).toFixed(2)} KB`
            }
        };
        
        fs.writeFileSync(
            'test_data_summary.json',
            JSON.stringify(summary, null, 2)
        );
        console.log('  ✓ Saved summary to test_data_summary.json');
        
        console.log('\n✅ All test data saved successfully!');
    }

    /**
     * Generate complete test dataset
     */
    generateAll() {
        console.log('='.repeat(60));
        console.log('🏗️ FLASH BETTING TEST DATA GENERATOR');
        console.log('='.repeat(60));
        
        this.generateUsers(10000);
        this.generateMarkets(1000);
        this.generateHistoricalData(30);
        this.generateEdgeCases();
        const benchmarks = this.generateBenchmarks();
        
        this.saveToFiles();
        
        console.log('\n' + '='.repeat(60));
        console.log('✅ TEST DATA GENERATION COMPLETE');
        console.log('='.repeat(60));
        
        return {
            users: this.users,
            markets: this.markets,
            historical: this.historicalData,
            edgeCases: this.edgeCases,
            benchmarks
        };
    }
}

// Execute if run directly
if (require.main === module) {
    const generator = new TestDataGenerator();
    generator.generateAll();
}

module.exports = TestDataGenerator;