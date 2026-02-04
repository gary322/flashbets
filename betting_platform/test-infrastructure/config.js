// BOOM Platform - Test Infrastructure Configuration
// Production-grade testing setup for mainnet readiness

const { ethers } = require('ethers');

module.exports = {
    // Network Configuration
    networks: {
        local: {
            url: 'http://127.0.0.1:8545',
            chainId: 31337,
            accounts: {
                mnemonic: 'test test test test test test test test test test test junk',
                count: 100 // 100 test accounts for concurrent testing
            },
            gasPrice: 20000000000,
            gasLimit: 30000000,
            blockTime: 1 // 1 second blocks for faster testing
        },
        polygon: {
            url: process.env.POLYGON_RPC || 'http://127.0.0.1:8545',
            chainId: 137,
            gasPrice: 30000000000
        },
        solana: {
            url: process.env.SOLANA_RPC || 'http://127.0.0.1:8899',
            commitment: 'confirmed'
        }
    },

    // Contract Addresses (will be populated after deployment)
    contracts: {
        BettingPlatform: null,
        MarketFactory: null,
        FlashBetting: null,
        LeverageVault: null,
        QuantumPositions: null,
        VerseHierarchy: null,
        OrderBook: null,
        LiquidityPool: null,
        PriceOracle: null,
        USDC: null,
        WormholeBridge: null
    },

    // Test User Profiles
    userProfiles: {
        WHALE: {
            initialBalance: ethers.utils.parseUnits('100000', 6), // 100k USDC
            riskProfile: 'aggressive',
            leveragePreference: [50, 100, 500],
            betFrequency: 'high'
        },
        DEGEN: {
            initialBalance: ethers.utils.parseUnits('1000', 6), // 1k USDC
            riskProfile: 'extreme',
            leveragePreference: [100, 300, 500],
            betFrequency: 'very_high'
        },
        RETAIL: {
            initialBalance: ethers.utils.parseUnits('500', 6), // 500 USDC
            riskProfile: 'moderate',
            leveragePreference: [1, 10, 20],
            betFrequency: 'medium'
        },
        CONSERVATIVE: {
            initialBalance: ethers.utils.parseUnits('10000', 6), // 10k USDC
            riskProfile: 'low',
            leveragePreference: [1, 2, 5],
            betFrequency: 'low'
        },
        BOT: {
            initialBalance: ethers.utils.parseUnits('5000', 6), // 5k USDC
            riskProfile: 'algorithmic',
            leveragePreference: [10, 20, 30],
            betFrequency: 'ultra_high'
        }
    },

    // Market Scenarios
    marketScenarios: {
        POLYMARKET: {
            BINARY: [
                { title: 'US Recession Q4 2025', probability: 0.42, volume: 127000000 },
                { title: 'Bitcoin >$150k by 2025', probability: 0.65, volume: 89000000 },
                { title: 'SpaceX Mars Landing 2026', probability: 0.31, volume: 43000000 }
            ],
            CATEGORICAL: [
                { title: '2025 NBA Champions', outcomes: 30, favorite: 'Celtics', volume: 56000000 },
                { title: 'Next Fed Chair', outcomes: 8, favorite: 'Powell', volume: 23000000 }
            ],
            SCALAR: [
                { title: 'S&P 500 EOY 2025', range: [4000, 6000], current: 4850, volume: 178000000 },
                { title: 'US Unemployment Rate', range: [3.0, 7.0], current: 4.2, volume: 34000000 }
            ]
        },
        FLASH: {
            SPORTS: [
                { sport: 'basketball', duration: 24, title: 'Next shot made?', probability: 0.45 },
                { sport: 'soccer', duration: 30, title: 'Corner goal?', probability: 0.28 },
                { sport: 'tennis', duration: 60, title: 'Ace on serve?', probability: 0.22 },
                { sport: 'football', duration: 15, title: 'First down?', probability: 0.67 },
                { sport: 'baseball', duration: 8, title: 'Strike?', probability: 0.65 }
            ]
        }
    },

    // Test Timing Configuration
    timing: {
        flashMarketResolution: 1000, // 1 second for test
        normalMarketResolution: 10000, // 10 seconds for test
        blockConfirmations: 1,
        oracleUpdateFrequency: 500, // 500ms
        heartbeatInterval: 1000,
        maxTestDuration: 300000 // 5 minutes per test max
    },

    // Gas Settings
    gas: {
        maxGasPrice: ethers.utils.parseUnits('100', 'gwei'),
        maxGasLimit: 10000000,
        bufferMultiplier: 1.2 // 20% buffer for gas estimates
    },

    // Risk Parameters
    risk: {
        maxLeverage: 500,
        liquidationThreshold: 0.8,
        maintenanceMargin: 0.05,
        maxPositionSize: ethers.utils.parseUnits('1000000', 6), // 1M USDC
        maxDailyVolume: ethers.utils.parseUnits('10000000', 6) // 10M USDC
    },

    // Testing Parameters
    testing: {
        concurrentUsers: 100,
        transactionsPerSecond: 1000,
        testIterations: 10,
        stressTestDuration: 60000, // 1 minute
        randomSeed: 12345,
        verbose: true,
        saveResults: true,
        resultsPath: './test-results/'
    },

    // Oracle Configuration
    oracles: {
        primary: 'chainlink',
        fallback: 'band',
        timeout: 5000,
        maxPriceDeviation: 0.02, // 2%
        updateThreshold: 0.001 // 0.1%
    },

    // Security Settings
    security: {
        maxSlippage: 0.01, // 1%
        frontRunningProtection: true,
        sandwichProtection: true,
        flashLoanProtection: true,
        reentrancyGuard: true,
        pauseEnabled: true
    },

    // Monitoring Configuration
    monitoring: {
        enableMetrics: true,
        metricsPort: 9090,
        enableLogs: true,
        logLevel: 'debug',
        enableAlerts: true,
        alertThresholds: {
            gasPrice: ethers.utils.parseUnits('200', 'gwei'),
            failureRate: 0.01, // 1%
            latency: 1000 // 1 second
        }
    }
};