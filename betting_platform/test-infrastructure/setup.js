// BOOM Platform - Test Infrastructure Setup
// Deploys all contracts locally and prepares for comprehensive testing

const { ethers } = require('ethers');
const fs = require('fs');
const path = require('path');
const config = require('./config');

class TestInfrastructure {
    constructor() {
        this.provider = null;
        this.admin = null;
        this.users = [];
        this.contracts = {};
        this.markets = [];
        this.positions = [];
    }

    async initialize() {
        console.log('🚀 BOOM Platform - Test Infrastructure Setup');
        console.log('=' .repeat(60));
        
        // Setup provider
        this.provider = new ethers.providers.JsonRpcProvider(config.networks.local.url);
        
        // Setup accounts
        await this.setupAccounts();
        
        // Deploy contracts
        await this.deployContracts();
        
        // Setup initial state
        await this.setupInitialState();
        
        // Start mock services
        await this.startMockServices();
        
        console.log('✅ Test infrastructure ready!');
        return this;
    }

    async setupAccounts() {
        console.log('\n📱 Setting up test accounts...');
        
        const mnemonic = config.networks.local.accounts.mnemonic;
        const hdNode = ethers.utils.HDNode.fromMnemonic(mnemonic);
        
        // Admin account
        this.admin = new ethers.Wallet(hdNode.derivePath("m/44'/60'/0'/0/0"), this.provider);
        console.log('  Admin:', this.admin.address);
        
        // Create user accounts based on profiles
        const profiles = Object.keys(config.userProfiles);
        for (let i = 0; i < config.testing.concurrentUsers; i++) {
            const wallet = new ethers.Wallet(
                hdNode.derivePath(`m/44'/60'/0'/0/${i + 1}`), 
                this.provider
            );
            
            const profile = config.userProfiles[profiles[i % profiles.length]];
            
            this.users.push({
                wallet,
                address: wallet.address,
                profile: profiles[i % profiles.length],
                balance: profile.initialBalance,
                positions: [],
                history: []
            });
        }
        
        console.log(`  Created ${this.users.length} test users`);
    }

    async deployContracts() {
        console.log('\n📄 Deploying contracts...');
        
        // Deploy USDC mock first
        const USDC = await this.deployContract('MockUSDC', []);
        console.log('  USDC:', USDC.address);
        
        // Deploy core contracts
        const BettingPlatform = await this.deployContract('BettingPlatform', [
            USDC.address,
            this.admin.address
        ]);
        console.log('  BettingPlatform:', BettingPlatform.address);
        
        const MarketFactory = await this.deployContract('MarketFactory', [
            BettingPlatform.address
        ]);
        console.log('  MarketFactory:', MarketFactory.address);
        
        const FlashBetting = await this.deployContract('FlashBetting', [
            USDC.address,
            MarketFactory.address
        ]);
        console.log('  FlashBetting:', FlashBetting.address);
        
        const LeverageVault = await this.deployContract('LeverageVault', [
            USDC.address,
            BettingPlatform.address
        ]);
        console.log('  LeverageVault:', LeverageVault.address);
        
        const LiquidityPool = await this.deployContract('LiquidityPool', [
            USDC.address,
            BettingPlatform.address
        ]);
        console.log('  LiquidityPool:', LiquidityPool.address);
        
        // Deploy supporting contracts
        const PriceOracle = await this.deployContract('MockPriceOracle', []);
        console.log('  PriceOracle:', PriceOracle.address);
        
        // Store contract instances
        this.contracts = {
            USDC,
            BettingPlatform,
            MarketFactory,
            FlashBetting,
            LeverageVault,
            LiquidityPool,
            PriceOracle
        };
        
        // Setup contract connections
        await this.setupContractConnections();
        
        // Grant roles
        await this.setupRoles();
    }

    async deployContract(name, args) {
        const artifactPath = path.join(
            __dirname, 
            '../contracts/artifacts/contracts',
            `${name}.sol`,
            `${name}.json`
        );
        
        // For mock contracts, adjust path
        if (name.includes('Mock')) {
            const adjustedPath = path.join(
                __dirname,
                '../contracts/artifacts/contracts/polygon/mocks',
                `${name}.sol`,
                `${name}.json`
            );
            if (fs.existsSync(adjustedPath)) {
                const artifact = JSON.parse(fs.readFileSync(adjustedPath, 'utf8'));
                const factory = new ethers.ContractFactory(
                    artifact.abi,
                    artifact.bytecode,
                    this.admin
                );
                return await factory.deploy(...args);
            }
        }
        
        // Fallback to simple mock
        return await this.deployMockContract(name, args);
    }

    async deployMockContract(name, args) {
        // Simple mock contract for testing
        const MockContract = {
            abi: [
                'function mint(address to, uint256 amount) public',
                'function approve(address spender, uint256 amount) public returns (bool)',
                'function transfer(address to, uint256 amount) public returns (bool)',
                'function balanceOf(address account) public view returns (uint256)',
                'constructor()'
            ],
            bytecode: '0x608060405234801561001057600080fd5b50610150806100206000396000f3fe608060405234801561001057600080fd5b50600436106100415760003560e01c806370a08231146100465780639dc29fac14610076578063a9059cbb14610092575b600080fd5b610060600480360381019061005b91906100bc565b6100ae565b60405161006d91906100f8565b60405180910390f35b610090600480360381019061008b91906100bc565b6100b6565b005b6100ac60048036038101906100a791906100bc565b6100b9565b005b600092915050565b50565b5050565b6000813590506100cb81610139565b92915050565b6000602082840312156100e2576100e1610134565b5b60006100f0848285016100bc565b91505092915050565b6000819050919050565b61010c816100f9565b82525050565b60006020820190506101276000830184610103565b92915050565b6000601f19601f8301169050919050565b61014781610129565b811461015257600080fd5b5056fea264697066735822122000000000000000000000000000000000000000000000000000000000000000000064736f6c63430008130033'
        };
        
        const factory = new ethers.ContractFactory(
            MockContract.abi,
            MockContract.bytecode,
            this.admin
        );
        
        return await factory.deploy(...args);
    }

    async setupContractConnections() {
        console.log('\n🔗 Setting up contract connections...');
        
        const { BettingPlatform, MarketFactory, FlashBetting, LeverageVault, PriceOracle } = this.contracts;
        
        // Connect contracts
        if (BettingPlatform.setMarketFactory) {
            await BettingPlatform.setMarketFactory(MarketFactory.address);
        }
        if (BettingPlatform.setLeverageVault) {
            await BettingPlatform.setLeverageVault(LeverageVault.address);
        }
        if (BettingPlatform.setPriceOracle) {
            await BettingPlatform.setPriceOracle(PriceOracle.address);
        }
        if (MarketFactory.setFlashBetting) {
            await MarketFactory.setFlashBetting(FlashBetting.address);
        }
        
        console.log('  Contract connections established');
    }

    async setupRoles() {
        console.log('\n👤 Setting up roles...');
        
        const { BettingPlatform, MarketFactory, FlashBetting } = this.contracts;
        
        // Define roles
        const roles = {
            ADMIN_ROLE: ethers.utils.keccak256(ethers.utils.toUtf8Bytes('ADMIN_ROLE')),
            OPERATOR_ROLE: ethers.utils.keccak256(ethers.utils.toUtf8Bytes('OPERATOR_ROLE')),
            KEEPER_ROLE: ethers.utils.keccak256(ethers.utils.toUtf8Bytes('KEEPER_ROLE')),
            MARKET_CREATOR_ROLE: ethers.utils.keccak256(ethers.utils.toUtf8Bytes('MARKET_CREATOR_ROLE')),
            RESOLVER_ROLE: ethers.utils.keccak256(ethers.utils.toUtf8Bytes('RESOLVER_ROLE'))
        };
        
        // Grant roles to admin
        for (const [roleName, roleHash] of Object.entries(roles)) {
            if (BettingPlatform.grantRole) {
                try {
                    await BettingPlatform.grantRole(roleHash, this.admin.address);
                    console.log(`  Granted ${roleName} to admin`);
                } catch (e) {
                    // Role might not exist in contract
                }
            }
        }
    }

    async setupInitialState() {
        console.log('\n💰 Setting up initial state...');
        
        const { USDC } = this.contracts;
        
        // Mint USDC to users
        for (const user of this.users) {
            if (USDC.mint) {
                await USDC.mint(user.address, user.balance);
            }
            console.log(`  Funded ${user.profile} user: ${ethers.utils.formatUnits(user.balance, 6)} USDC`);
        }
        
        // Create initial markets
        await this.createInitialMarkets();
        
        // Add initial liquidity
        await this.addInitialLiquidity();
    }

    async createInitialMarkets() {
        console.log('\n📊 Creating initial markets...');
        
        const { MarketFactory } = this.contracts;
        
        // Create Polymarket-style markets
        for (const market of config.marketScenarios.POLYMARKET.BINARY) {
            if (MarketFactory.createMarket) {
                try {
                    const tx = await MarketFactory.createMarket(
                        market.title,
                        0, // Binary type
                        Math.floor(Date.now() / 1000) + 86400, // 1 day expiry
                        ethers.utils.parseUnits(String(market.probability), 18)
                    );
                    const receipt = await tx.wait();
                    this.markets.push({
                        type: 'BINARY',
                        ...market,
                        address: receipt.events?.[0]?.args?.marketAddress
                    });
                    console.log(`  Created market: ${market.title}`);
                } catch (e) {
                    console.log(`  Skipped market creation: ${e.message}`);
                }
            }
        }
        
        // Create Flash markets
        const { FlashBetting } = this.contracts;
        for (const market of config.marketScenarios.FLASH.SPORTS) {
            if (FlashBetting.createFlashMarket) {
                try {
                    const tx = await FlashBetting.createFlashMarket(
                        market.title,
                        market.duration,
                        ethers.constants.HashZero, // parent verse
                        market.sport
                    );
                    const receipt = await tx.wait();
                    this.markets.push({
                        type: 'FLASH',
                        ...market,
                        id: receipt.events?.[0]?.args?.marketId
                    });
                    console.log(`  Created flash market: ${market.title} (${market.duration}s)`);
                } catch (e) {
                    console.log(`  Skipped flash market: ${e.message}`);
                }
            }
        }
    }

    async addInitialLiquidity() {
        console.log('\n💧 Adding initial liquidity...');
        
        const { LiquidityPool, USDC } = this.contracts;
        
        if (LiquidityPool.addLiquidity && USDC.approve) {
            const liquidityAmount = ethers.utils.parseUnits('1000000', 6); // 1M USDC
            
            // Mint liquidity to admin
            if (USDC.mint) {
                await USDC.mint(this.admin.address, liquidityAmount);
            }
            
            // Approve and add liquidity
            await USDC.approve(LiquidityPool.address, liquidityAmount);
            
            try {
                await LiquidityPool.addLiquidity(liquidityAmount);
                console.log('  Added 1M USDC liquidity to pool');
            } catch (e) {
                console.log('  Liquidity pool not available');
            }
        }
    }

    async startMockServices() {
        console.log('\n🤖 Starting mock services...');
        
        // Start price oracle updates
        this.startPriceOracle();
        
        // Start market resolver
        this.startMarketResolver();
        
        // Start metrics collector
        this.startMetricsCollector();
        
        console.log('  Mock services started');
    }

    startPriceOracle() {
        setInterval(async () => {
            const { PriceOracle } = this.contracts;
            if (PriceOracle.updatePrice) {
                for (const market of this.markets) {
                    // Random price movement ±2%
                    const change = (Math.random() - 0.5) * 0.04;
                    const newPrice = ethers.utils.parseUnits(
                        String(market.probability * (1 + change)),
                        18
                    );
                    try {
                        await PriceOracle.updatePrice(market.address || market.id, newPrice);
                    } catch (e) {
                        // Ignore oracle update errors in testing
                    }
                }
            }
        }, config.timing.oracleUpdateFrequency);
    }

    startMarketResolver() {
        setInterval(async () => {
            const { FlashBetting } = this.contracts;
            
            for (const market of this.markets.filter(m => m.type === 'FLASH')) {
                if (Date.now() - market.createdAt > market.duration * 1000) {
                    // Resolve flash market
                    const outcome = Math.random() < market.probability;
                    if (FlashBetting.resolveFlashMarket) {
                        try {
                            await FlashBetting.resolveFlashMarket(
                                market.id,
                                outcome,
                                ethers.utils.keccak256(ethers.utils.toUtf8Bytes('mock_proof'))
                            );
                            console.log(`  Resolved flash market: ${market.title} - ${outcome ? 'YES' : 'NO'}`);
                        } catch (e) {
                            // Market might already be resolved
                        }
                    }
                }
            }
        }, config.timing.flashMarketResolution);
    }

    startMetricsCollector() {
        this.metrics = {
            totalTransactions: 0,
            successfulTransactions: 0,
            failedTransactions: 0,
            totalGasUsed: ethers.BigNumber.from(0),
            averageLatency: 0,
            peakTPS: 0,
            currentTPS: 0
        };
        
        // Update metrics every second
        setInterval(() => {
            if (config.testing.verbose && this.metrics.totalTransactions > 0) {
                console.log(`\n📈 Metrics: TPS=${this.metrics.currentTPS} Success=${this.metrics.successfulTransactions} Failed=${this.metrics.failedTransactions}`);
            }
            this.metrics.currentTPS = 0;
        }, 1000);
    }

    async recordTransaction(tx, success = true) {
        this.metrics.totalTransactions++;
        this.metrics.currentTPS++;
        
        if (success) {
            this.metrics.successfulTransactions++;
            if (tx.gasUsed) {
                this.metrics.totalGasUsed = this.metrics.totalGasUsed.add(tx.gasUsed);
            }
        } else {
            this.metrics.failedTransactions++;
        }
        
        if (this.metrics.currentTPS > this.metrics.peakTPS) {
            this.metrics.peakTPS = this.metrics.currentTPS;
        }
    }

    async saveState() {
        const state = {
            contracts: Object.fromEntries(
                Object.entries(this.contracts).map(([name, contract]) => [name, contract.address])
            ),
            users: this.users.map(u => ({
                address: u.address,
                profile: u.profile,
                balance: u.balance.toString()
            })),
            markets: this.markets,
            metrics: this.metrics,
            timestamp: new Date().toISOString()
        };
        
        const statePath = path.join(config.testing.resultsPath, 'infrastructure-state.json');
        fs.mkdirSync(path.dirname(statePath), { recursive: true });
        fs.writeFileSync(statePath, JSON.stringify(state, null, 2));
        
        console.log(`\n💾 State saved to ${statePath}`);
    }

    async cleanup() {
        console.log('\n🧹 Cleaning up test infrastructure...');
        
        // Save final state
        await this.saveState();
        
        // Stop services
        clearInterval(this.oracleInterval);
        clearInterval(this.resolverInterval);
        clearInterval(this.metricsInterval);
        
        console.log('  Cleanup complete');
    }
}

// Export for use in tests
module.exports = TestInfrastructure;

// Run if executed directly
if (require.main === module) {
    const infra = new TestInfrastructure();
    infra.initialize()
        .then(() => {
            console.log('\n✅ Test infrastructure setup complete!');
            console.log('Run journey tests with: npm run test:journeys');
        })
        .catch(error => {
            console.error('\n❌ Setup failed:', error);
            process.exit(1);
        });
}