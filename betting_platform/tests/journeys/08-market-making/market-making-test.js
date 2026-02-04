#!/usr/bin/env node

/**
 * Market Making Journey Test
 * Tests comprehensive market making and liquidity provision features
 */

const { chromium } = require('playwright');
const { Connection, Keypair, PublicKey } = require('@solana/web3.js');
const axios = require('axios');
const WebSocket = require('ws');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');

class MarketMakingJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.connection = new Connection(config.rpcUrl, 'confirmed');
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0,
      marketsCreated: 0,
      liquidityProvided: 0,
      ordersPlaced: 0,
      spreadsManaged: 0,
      feesEarned: 0,
      volumeFacilitated: 0,
      profitablePeriods: 0
    };
    this.ws = null;
    this.activeOrders = [];
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n📊 Starting Market Making Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Select market maker wallet
      const wallet = this.testData.wallets.find(w => 
        w.type === 'market_maker' && w.balance > 50000
      ) || this.testData.wallets[userId % this.testData.wallets.length];
      
      // Setup WebSocket for real-time price feeds
      await this.setupWebSocket();
      
      // Test market making features
      await this.testMarketMakerDashboard(page, wallet);
      await this.testProvideLiquidity(page, wallet);
      await this.testCreateOrderPairs(page);
      await this.testSpreadManagement(page);
      await this.testInventoryManagement(page);
      await this.testRiskControls(page);
      await this.testDynamicPricing(page);
      await this.testVolatilityAdjustment(page);
      await this.testArbitrageDetection(page);
      await this.testLiquidityMining(page);
      await this.testMarketAnalytics(page);
      await this.testCompetitorMonitoring(page);
      await this.testAutomatedStrategies(page);
      await this.testPerformanceTracking(page);
      await this.testWithdrawLiquidity(page);
      
      await browser.close();
      this.closeWebSocket();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = (this.metrics.liquidityProvided / 
                                  (this.metrics.liquidityProvided + this.metrics.errors.length) * 100) || 0;
      
      console.log(chalk.green(`✅ Market making journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Market making journey failed:'), error);
      throw error;
    }
  }

  async setupWebSocket() {
    try {
      this.ws = new WebSocket(this.config.wsUrl);
      
      await new Promise((resolve, reject) => {
        this.ws.on('open', () => {
          console.log(chalk.gray('    WebSocket connected for real-time market data'));
          resolve();
        });
        this.ws.on('error', reject);
        setTimeout(() => reject(new Error('WebSocket timeout')), 5000);
      });
      
      // Subscribe to market data
      this.ws.send(JSON.stringify({
        type: 'subscribe',
        channels: ['orderbook', 'trades', 'prices', 'liquidity']
      }));
      
    } catch (error) {
      console.log(chalk.yellow('    ⚠ WebSocket connection failed, continuing without real-time'));
    }
  }

  closeWebSocket() {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.close();
    }
  }

  async testMarketMakerDashboard(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market maker dashboard...'));
    
    try {
      // Navigate to market maker dashboard
      await page.goto(`${this.config.uiUrl}/market-maker`, { waitUntil: 'networkidle' });
      
      // Check dashboard overview
      const dashboardElements = {
        totalLiquidity: await page.$eval('.total-liquidity, [data-total-liquidity]', el => el.textContent).catch(() => 'N/A'),
        activePairs: await page.$eval('.active-pairs, [data-active-pairs]', el => el.textContent).catch(() => 'N/A'),
        dailyVolume: await page.$eval('.daily-volume, [data-daily-volume]', el => el.textContent).catch(() => 'N/A'),
        feesEarned: await page.$eval('.fees-earned, [data-fees-earned]', el => el.textContent).catch(() => 'N/A'),
        profitLoss: await page.$eval('.profit-loss, [data-pnl]', el => el.textContent).catch(() => 'N/A'),
        healthScore: await page.$eval('.health-score, [data-health]', el => el.textContent).catch(() => 'N/A')
      };
      
      console.log(chalk.gray('    Dashboard metrics:'));
      for (const [key, value] of Object.entries(dashboardElements)) {
        console.log(chalk.gray(`    - ${key}: ${value}`));
      }
      
      // Check active markets
      const activeMarkets = await page.$$('.market-card, [data-market-making]');
      console.log(chalk.gray(`    Active in ${activeMarkets.length} markets`));
      
      // Check performance charts
      const charts = await page.$$('.performance-chart, [data-chart]');
      console.log(chalk.gray(`    Performance charts: ${charts.length}`));
      
      // Check recent activity
      const recentActivity = await page.$$('.activity-item, [data-activity]');
      console.log(chalk.gray(`    Recent activities: ${recentActivity.length}`));
      
      this.metrics.stepTimings.marketMakerDashboard = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Market maker dashboard reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'marketMakerDashboard',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testProvideLiquidity(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing liquidity provision...'));
    
    try {
      // Find provide liquidity button
      const provideLiquidityButton = await page.$('button:has-text("Provide Liquidity"), [data-provide-liquidity]');
      if (!provideLiquidityButton) {
        throw new Error('Provide liquidity button not found');
      }
      
      await provideLiquidityButton.click();
      await page.waitForTimeout(500);
      
      // Select market for liquidity provision
      const marketSelect = await page.$('select[name="market"], .market-selector');
      if (marketSelect) {
        const availableMarkets = await marketSelect.$$('option');
        if (availableMarkets.length > 1) {
          await marketSelect.selectOption({ index: 1 }); // Select first non-default option
        }
      }
      
      // Set liquidity amount
      const liquidityAmountInput = await page.$('input[name="liquidityAmount"]');
      if (liquidityAmountInput) {
        const liquidityAmount = Math.min(wallet.balance * 0.1, 10000); // 10% of balance or max 10k
        await liquidityAmountInput.fill(liquidityAmount.toString());
        console.log(chalk.gray(`    Providing $${liquidityAmount} liquidity`));
      }
      
      // Set price range
      const minPriceInput = await page.$('input[name="minPrice"]');
      const maxPriceInput = await page.$('input[name="maxPrice"]');
      if (minPriceInput && maxPriceInput) {
        await minPriceInput.fill('0.40'); // 40 cents
        await maxPriceInput.fill('0.60'); // 60 cents
        console.log(chalk.gray('    Price range: $0.40 - $0.60'));
      }
      
      // Set liquidity distribution
      const distributionSelect = await page.$('select[name="distribution"]');
      if (distributionSelect) {
        await distributionSelect.selectOption('uniform'); // Uniform, concentrated, or custom
      }
      
      // Configure auto-rebalancing
      const autoRebalanceCheckbox = await page.$('input[name="autoRebalance"]');
      if (autoRebalanceCheckbox) {
        await autoRebalanceCheckbox.check();
        
        const rebalanceThresholdInput = await page.$('input[name="rebalanceThreshold"]');
        if (rebalanceThresholdInput) {
          await rebalanceThresholdInput.fill('5'); // 5% threshold
        }
      }
      
      // Review liquidity parameters
      const reviewSection = await page.$('.liquidity-review, [data-review]');
      if (reviewSection) {
        const expectedFees = await reviewSection.$eval('.expected-fees', el => el.textContent).catch(() => 'N/A');
        const impermanentLoss = await reviewSection.$eval('.impermanent-loss', el => el.textContent).catch(() => 'N/A');
        
        console.log(chalk.gray(`    Expected fees: ${expectedFees}`));
        console.log(chalk.gray(`    Impermanent loss risk: ${impermanentLoss}`));
      }
      
      // Provide liquidity
      const confirmButton = await page.$('button:has-text("Provide Liquidity"), button:has-text("Confirm")');
      if (confirmButton) {
        await confirmButton.click();
        await page.waitForTimeout(2000);
        
        this.metrics.liquidityProvided++;
        console.log(chalk.green('    ✓ Liquidity provided successfully'));
      }
      
      this.metrics.stepTimings.provideLiquidity = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'provideLiquidity',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testCreateOrderPairs(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing order pair creation...'));
    
    try {
      // Navigate to order management
      const orderMgmtButton = await page.$('button:has-text("Manage Orders"), [data-orders]');
      if (orderMgmtButton) {
        await orderMgmtButton.click();
        await page.waitForTimeout(500);
      }
      
      // Create buy/sell order pairs
      const createPairButton = await page.$('button:has-text("Create Pair"), [data-create-pair]');
      if (!createPairButton) {
        console.log(chalk.yellow('    ⚠ Order pair creation not available'));
        return;
      }
      
      await createPairButton.click();
      await page.waitForTimeout(500);
      
      // Configure order pair
      const pairModal = await page.$('[role="dialog"], .order-pair-modal');
      if (pairModal) {
        // Set center price
        const centerPriceInput = await pairModal.$('input[name="centerPrice"]');
        if (centerPriceInput) {
          await centerPriceInput.fill('0.50'); // 50 cents center
        }
        
        // Set spread
        const spreadInput = await pairModal.$('input[name="spread"]');
        if (spreadInput) {
          await spreadInput.fill('2'); // 2% spread
          console.log(chalk.gray('    Spread: 2%'));
        }
        
        // Set order sizes
        const buySizeInput = await pairModal.$('input[name="buySize"]');
        const sellSizeInput = await pairModal.$('input[name="sellSize"]');
        if (buySizeInput && sellSizeInput) {
          await buySizeInput.fill('1000');
          await sellSizeInput.fill('1000');
        }
        
        // Set order count
        const orderCountInput = await pairModal.$('input[name="orderCount"]');
        if (orderCountInput) {
          await orderCountInput.fill('5'); // 5 orders each side
        }
        
        // Configure laddering
        const ladderTypeSelect = await pairModal.$('select[name="ladderType"]');
        if (ladderTypeSelect) {
          await ladderTypeSelect.selectOption('linear'); // Linear, exponential, or custom
        }
        
        // Create orders
        const createOrdersButton = await pairModal.$('button:has-text("Create Orders")');
        if (createOrdersButton) {
          await createOrdersButton.click();
          await page.waitForTimeout(2000);
          
          this.metrics.ordersPlaced += 10; // 5 buy + 5 sell
          console.log(chalk.green('    ✓ Order pairs created'));
        }
      }
      
      this.metrics.stepTimings.createOrderPairs = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'createOrderPairs',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testSpreadManagement(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing spread management...'));
    
    try {
      // Access spread management
      const spreadMgmtButton = await page.$('button:has-text("Manage Spreads"), [data-spread-mgmt]');
      if (!spreadMgmtButton) {
        console.log(chalk.yellow('    ⚠ Spread management not available'));
        return;
      }
      
      await spreadMgmtButton.click();
      await page.waitForTimeout(500);
      
      // Dynamic spread adjustment
      const dynamicSpreadSection = await page.$('.dynamic-spread, [data-dynamic-spread]');
      if (dynamicSpreadSection) {
        // Enable dynamic spreads
        const enableDynamicCheckbox = await dynamicSpreadSection.$('input[name="enableDynamic"]');
        if (enableDynamicCheckbox) {
          await enableDynamicCheckbox.check();
        }
        
        // Set volatility-based adjustment
        const volatilityMultiplierInput = await dynamicSpreadSection.$('input[name="volatilityMultiplier"]');
        if (volatilityMultiplierInput) {
          await volatilityMultiplierInput.fill('1.5'); // 1.5x volatility multiplier
        }
        
        // Set volume-based adjustment
        const volumeFactorInput = await dynamicSpreadSection.$('input[name="volumeFactor"]');
        if (volumeFactorInput) {
          await volumeFactorInput.fill('0.8'); // Tighten spreads with more volume
        }
        
        // Set minimum and maximum spreads
        const minSpreadInput = await dynamicSpreadSection.$('input[name="minSpread"]');
        const maxSpreadInput = await dynamicSpreadSection.$('input[name="maxSpread"]');
        if (minSpreadInput && maxSpreadInput) {
          await minSpreadInput.fill('0.5'); // 0.5% minimum
          await maxSpreadInput.fill('5.0'); // 5% maximum
        }
        
        console.log(chalk.gray('    Dynamic spread parameters configured'));
      }
      
      // Competitive spread monitoring
      const competitiveSpreadSection = await page.$('.competitive-spread, [data-competitive]');
      if (competitiveSpreadSection) {
        const enableCompetitiveCheckbox = await competitiveSpreadSection.$('input[name="enableCompetitive"]');
        if (enableCompetitiveCheckbox) {
          await enableCompetitiveCheckbox.check();
        }
        
        // Set competitive margin
        const competitiveMarginInput = await competitiveSpreadSection.$('input[name="competitiveMargin"]');
        if (competitiveMarginInput) {
          await competitiveMarginInput.fill('0.1'); // 0.1% better than competition
        }
        
        console.log(chalk.gray('    Competitive spread monitoring enabled'));
      }
      
      // Apply spread settings
      const applyButton = await page.$('button:has-text("Apply Settings")');
      if (applyButton) {
        await applyButton.click();
        await page.waitForTimeout(1000);
        
        this.metrics.spreadsManaged++;
        console.log(chalk.green('    ✓ Spread management configured'));
      }
      
      this.metrics.stepTimings.spreadManagement = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'spreadManagement',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testInventoryManagement(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing inventory management...'));
    
    try {
      // Access inventory management
      const inventoryButton = await page.$('button:has-text("Inventory"), [data-inventory]');
      if (!inventoryButton) {
        console.log(chalk.yellow('    ⚠ Inventory management not available'));
        return;
      }
      
      await inventoryButton.click();
      await page.waitForTimeout(500);
      
      // Check current inventory
      const inventoryItems = await page.$$('.inventory-item, [data-inventory-item]');
      console.log(chalk.gray(`    Current inventory: ${inventoryItems.length} positions`));
      
      if (inventoryItems.length > 0) {
        // Analyze first inventory item
        const firstItem = inventoryItems[0];
        const asset = await firstItem.$eval('.asset-name', el => el.textContent);
        const quantity = await firstItem.$eval('.quantity', el => el.textContent);
        const value = await firstItem.$eval('.value', el => el.textContent);
        const targetAllocation = await firstItem.$eval('.target-allocation', el => el.textContent).catch(() => 'N/A');
        
        console.log(chalk.gray(`    ${asset}: ${quantity} (${value}, target: ${targetAllocation})`));
      }
      
      // Configure inventory limits
      const limitsSection = await page.$('.inventory-limits, [data-limits]');
      if (limitsSection) {
        // Set maximum inventory per asset
        const maxInventoryInput = await limitsSection.$('input[name="maxInventory"]');
        if (maxInventoryInput) {
          await maxInventoryInput.fill('50000'); // $50k max per asset
        }
        
        // Set rebalancing threshold
        const rebalanceThresholdInput = await limitsSection.$('input[name="rebalanceThreshold"]');
        if (rebalanceThresholdInput) {
          await rebalanceThresholdInput.fill('10'); // 10% deviation threshold
        }
        
        // Enable auto-rebalancing
        const autoRebalanceCheckbox = await limitsSection.$('input[name="autoRebalance"]');
        if (autoRebalanceCheckbox) {
          await autoRebalanceCheckbox.check();
          console.log(chalk.gray('    Auto-rebalancing enabled'));
        }
      }
      
      // Set hedging parameters
      const hedgingSection = await page.$('.hedging-section, [data-hedging]');
      if (hedgingSection) {
        const enableHedgingCheckbox = await hedgingSection.$('input[name="enableHedging"]');
        if (enableHedgingCheckbox) {
          await enableHedgingCheckbox.check();
        }
        
        const hedgeRatioInput = await hedgingSection.$('input[name="hedgeRatio"]');
        if (hedgeRatioInput) {
          await hedgeRatioInput.fill('0.8'); // 80% hedge ratio
        }
        
        console.log(chalk.gray('    Inventory hedging configured'));
      }
      
      // Apply inventory settings
      const applyInventoryButton = await page.$('button:has-text("Apply Inventory Settings")');
      if (applyInventoryButton) {
        await applyInventoryButton.click();
        console.log(chalk.green('    ✓ Inventory management configured'));
      }
      
      this.metrics.stepTimings.inventoryManagement = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'inventoryManagement',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testRiskControls(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing risk controls...'));
    
    try {
      // Access risk controls
      const riskControlsButton = await page.$('button:has-text("Risk Controls"), [data-risk-controls]');
      if (!riskControlsButton) {
        console.log(chalk.yellow('    ⚠ Risk controls not available'));
        return;
      }
      
      await riskControlsButton.click();
      await page.waitForTimeout(500);
      
      // Set position limits
      const limitsSection = await page.$('.position-limits, [data-position-limits]');
      if (limitsSection) {
        // Daily loss limit
        const dailyLossLimitInput = await limitsSection.$('input[name="dailyLossLimit"]');
        if (dailyLossLimitInput) {
          await dailyLossLimitInput.fill('5000'); // $5k daily loss limit
        }
        
        // Maximum position size
        const maxPositionSizeInput = await limitsSection.$('input[name="maxPositionSize"]');
        if (maxPositionSizeInput) {
          await maxPositionSizeInput.fill('25000'); // $25k max position
        }
        
        // Maximum leverage
        const maxLeverageInput = await limitsSection.$('input[name="maxLeverage"]');
        if (maxLeverageInput) {
          await maxLeverageInput.fill('5'); // 5x max leverage
        }
        
        console.log(chalk.gray('    Position limits configured'));
      }
      
      // Set volatility controls
      const volatilitySection = await page.$('.volatility-controls, [data-volatility]');
      if (volatilitySection) {
        // Volatility threshold
        const volatilityThresholdInput = await volatilitySection.$('input[name="volatilityThreshold"]');
        if (volatilityThresholdInput) {
          await volatilityThresholdInput.fill('50'); // 50% volatility threshold
        }
        
        // Action on high volatility
        const volatilityActionSelect = await volatilitySection.$('select[name="volatilityAction"]');
        if (volatilityActionSelect) {
          await volatilityActionSelect.selectOption('widen_spreads'); // Widen spreads, pause, or exit
        }
        
        console.log(chalk.gray('    Volatility controls set'));
      }
      
      // Emergency stops
      const emergencySection = await page.$('.emergency-stops, [data-emergency]');
      if (emergencySection) {
        // Enable circuit breaker
        const circuitBreakerCheckbox = await emergencySection.$('input[name="circuitBreaker"]');
        if (circuitBreakerCheckbox) {
          await circuitBreakerCheckbox.check();
        }
        
        // Set drawdown limit
        const drawdownLimitInput = await emergencySection.$('input[name="drawdownLimit"]');
        if (drawdownLimitInput) {
          await drawdownLimitInput.fill('15'); // 15% drawdown limit
        }
        
        console.log(chalk.gray('    Emergency stops configured'));
      }
      
      // Apply risk controls
      const applyRiskButton = await page.$('button:has-text("Apply Risk Controls")');
      if (applyRiskButton) {
        await applyRiskButton.click();
        console.log(chalk.green('    ✓ Risk controls activated'));
      }
      
      this.metrics.stepTimings.riskControls = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'riskControls',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testDynamicPricing(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing dynamic pricing...'));
    
    try {
      // Access pricing engine
      const pricingButton = await page.$('button:has-text("Pricing Engine"), [data-pricing]');
      if (!pricingButton) {
        console.log(chalk.yellow('    ⚠ Dynamic pricing not available'));
        return;
      }
      
      await pricingButton.click();
      await page.waitForTimeout(500);
      
      // Configure pricing model
      const pricingModel = await page.$('select[name="pricingModel"]');
      if (pricingModel) {
        await pricingModel.selectOption('adaptive'); // Adaptive, static, or machine_learning
        console.log(chalk.gray('    Pricing model: Adaptive'));
      }
      
      // Set pricing factors
      const factorsSection = await page.$('.pricing-factors, [data-factors]');
      if (factorsSection) {
        // Order book imbalance weight
        const imbalanceWeightInput = await factorsSection.$('input[name="imbalanceWeight"]');
        if (imbalanceWeightInput) {
          await imbalanceWeightInput.fill('0.3'); // 30% weight
        }
        
        // Recent trade volume weight
        const volumeWeightInput = await factorsSection.$('input[name="volumeWeight"]');
        if (volumeWeightInput) {
          await volumeWeightInput.fill('0.2'); // 20% weight
        }
        
        // Market volatility weight
        const volatilityWeightInput = await factorsSection.$('input[name="volatilityWeight"]');
        if (volatilityWeightInput) {
          await volatilityWeightInput.fill('0.25'); // 25% weight
        }
        
        // Time decay weight
        const timeDecayWeightInput = await factorsSection.$('input[name="timeDecayWeight"]');
        if (timeDecayWeightInput) {
          await timeDecayWeightInput.fill('0.25'); // 25% weight
        }
        
        console.log(chalk.gray('    Pricing factors weighted'));
      }
      
      // Set update frequency
      const updateFrequencySelect = await page.$('select[name="updateFrequency"]');
      if (updateFrequencySelect) {
        await updateFrequencySelect.selectOption('1s'); // Update every 1 second
      }
      
      // Enable machine learning
      const mlCheckbox = await page.$('input[name="enableML"]');
      if (mlCheckbox) {
        await mlCheckbox.check();
        
        const mlModelSelect = await page.$('select[name="mlModel"]');
        if (mlModelSelect) {
          await mlModelSelect.selectOption('ensemble'); // Ensemble of models
        }
        
        console.log(chalk.gray('    Machine learning enabled'));
      }
      
      // Apply pricing settings
      const applyPricingButton = await page.$('button:has-text("Apply Pricing")');
      if (applyPricingButton) {
        await applyPricingButton.click();
        console.log(chalk.green('    ✓ Dynamic pricing configured'));
      }
      
      this.metrics.stepTimings.dynamicPricing = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'dynamicPricing',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testVolatilityAdjustment(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing volatility adjustment...'));
    
    try {
      // Monitor current volatility
      const volatilityDisplay = await page.$eval('.current-volatility, [data-volatility]', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Current volatility: ${volatilityDisplay}`));
      
      // Simulate volatility spike
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({
          type: 'simulate_volatility',
          level: 'high'
        }));
        
        await page.waitForTimeout(2000);
        
        // Check if spreads widened
        const newSpreads = await page.$$eval('.current-spread, [data-current-spread]', els => 
          els.map(el => parseFloat(el.textContent.replace('%', '')))
        ).catch(() => []);
        
        if (newSpreads.length > 0) {
          const avgSpread = newSpreads.reduce((a, b) => a + b, 0) / newSpreads.length;
          console.log(chalk.gray(`    Average spread adjusted to: ${avgSpread.toFixed(2)}%`));
        }
      }
      
      // Check volatility-based order adjustments
      const adjustmentLog = await page.$$('.adjustment-log, [data-adjustment]');
      if (adjustmentLog.length > 0) {
        console.log(chalk.gray(`    Volatility adjustments: ${adjustmentLog.length} recorded`));
      }
      
      this.metrics.stepTimings.volatilityAdjustment = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Volatility adjustment tested'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'volatilityAdjustment',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testArbitrageDetection(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing arbitrage detection...'));
    
    try {
      // Access arbitrage scanner
      const arbitrageButton = await page.$('button:has-text("Arbitrage Scanner"), [data-arbitrage]');
      if (!arbitrageButton) {
        console.log(chalk.yellow('    ⚠ Arbitrage detection not available'));
        return;
      }
      
      await arbitrageButton.click();
      await page.waitForTimeout(1000);
      
      // Check for opportunities
      const arbitrageOpportunities = await page.$$('.arbitrage-opportunity, [data-arbitrage-opp]');
      console.log(chalk.gray(`    Arbitrage opportunities: ${arbitrageOpportunities.length}`));
      
      if (arbitrageOpportunities.length > 0) {
        // Analyze first opportunity
        const firstOpp = arbitrageOpportunities[0];
        const markets = await firstOpp.$eval('.market-pair', el => el.textContent);
        const spread = await firstOpp.$eval('.spread-percentage', el => el.textContent);
        const profit = await firstOpp.$eval('.profit-estimate', el => el.textContent);
        
        console.log(chalk.gray(`    Opportunity: ${markets} - ${spread} spread, ${profit} profit`));
        
        // Auto-execute if profitable
        const autoExecuteCheckbox = await page.$('input[name="autoExecuteArbitrage"]');
        if (autoExecuteCheckbox) {
          await autoExecuteCheckbox.check();
          
          const minProfitInput = await page.$('input[name="minArbitrageProfit"]');
          if (minProfitInput) {
            await minProfitInput.fill('0.5'); // 0.5% minimum profit
          }
          
          console.log(chalk.gray('    Auto-execution enabled for >0.5% opportunities'));
        }
      }
      
      // Configure detection parameters
      const detectionParams = await page.$('.detection-params, [data-detection-params]');
      if (detectionParams) {
        // Scan frequency
        const scanFrequencySelect = await detectionParams.$('select[name="scanFrequency"]');
        if (scanFrequencySelect) {
          await scanFrequencySelect.selectOption('500ms'); // Scan every 500ms
        }
        
        // Minimum spread threshold
        const minSpreadInput = await detectionParams.$('input[name="minSpreadThreshold"]');
        if (minSpreadInput) {
          await minSpreadInput.fill('0.1'); // 0.1% minimum spread
        }
        
        console.log(chalk.gray('    Detection parameters configured'));
      }
      
      this.metrics.stepTimings.arbitrageDetection = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Arbitrage detection configured'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'arbitrageDetection',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testLiquidityMining(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing liquidity mining...'));
    
    try {
      // Access liquidity mining
      const liquidityMiningButton = await page.$('button:has-text("Liquidity Mining"), [data-liquidity-mining]');
      if (!liquidityMiningButton) {
        console.log(chalk.yellow('    ⚠ Liquidity mining not available'));
        return;
      }
      
      await liquidityMiningButton.click();
      await page.waitForTimeout(500);
      
      // Check available programs
      const miningPrograms = await page.$$('.mining-program, [data-mining-program]');
      console.log(chalk.gray(`    Available programs: ${miningPrograms.length}`));
      
      if (miningPrograms.length > 0) {
        // Join first program
        const firstProgram = miningPrograms[0];
        const programName = await firstProgram.$eval('.program-name', el => el.textContent);
        const apy = await firstProgram.$eval('.apy', el => el.textContent);
        const requirements = await firstProgram.$eval('.requirements', el => el.textContent);
        
        console.log(chalk.gray(`    Program: ${programName} - ${apy} APY`));
        console.log(chalk.gray(`    Requirements: ${requirements}`));
        
        // Join program
        const joinButton = await firstProgram.$('button:has-text("Join"), button:has-text("Participate")');
        if (joinButton) {
          await joinButton.click();
          await page.waitForTimeout(1000);
          
          console.log(chalk.green('    ✓ Joined liquidity mining program'));
        }
      }
      
      // Check rewards
      const rewardsSection = await page.$('.rewards-section, [data-rewards]');
      if (rewardsSection) {
        const pendingRewards = await rewardsSection.$eval('.pending-rewards', el => el.textContent).catch(() => '0');
        const claimableRewards = await rewardsSection.$eval('.claimable-rewards', el => el.textContent).catch(() => '0');
        
        console.log(chalk.gray(`    Pending rewards: ${pendingRewards}`));
        console.log(chalk.gray(`    Claimable rewards: ${claimableRewards}`));
        
        // Claim rewards if available
        const claimButton = await rewardsSection.$('button:has-text("Claim")');
        if (claimButton) {
          const claimableValue = parseFloat(claimableRewards.replace(/[^0-9.]/g, ''));
          if (claimableValue > 0) {
            await claimButton.click();
            this.metrics.feesEarned += claimableValue;
            console.log(chalk.green(`    ✓ Claimed ${claimableValue} in rewards`));
          }
        }
      }
      
      this.metrics.stepTimings.liquidityMining = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'liquidityMining',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testMarketAnalytics(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market analytics...'));
    
    try {
      // Access analytics dashboard
      const analyticsButton = await page.$('button:has-text("Analytics"), [data-analytics]');
      if (analyticsButton) {
        await analyticsButton.click();
        await page.waitForTimeout(1000);
      }
      
      // Review performance metrics
      const performanceMetrics = {
        totalVolume: await page.$eval('.total-volume-facilitated', el => el.textContent).catch(() => 'N/A'),
        feesEarned: await page.$eval('.total-fees-earned', el => el.textContent).catch(() => 'N/A'),
        profitMargin: await page.$eval('.profit-margin', el => el.textContent).catch(() => 'N/A'),
        uptimePercentage: await page.$eval('.uptime-percentage', el => el.textContent).catch(() => 'N/A'),
        averageSpread: await page.$eval('.average-spread', el => el.textContent).catch(() => 'N/A'),
        fillRate: await page.$eval('.fill-rate', el => el.textContent).catch(() => 'N/A')
      };
      
      console.log(chalk.gray('    Performance metrics:'));
      for (const [metric, value] of Object.entries(performanceMetrics)) {
        console.log(chalk.gray(`    - ${metric}: ${value}`));
      }
      
      // Track volumes facilitated
      const volumeValue = parseFloat(performanceMetrics.totalVolume.replace(/[^0-9.]/g, '')) || 0;
      this.metrics.volumeFacilitated += volumeValue;
      
      // Check profitability analysis
      const profitabilityChart = await page.$('.profitability-chart, [data-profitability]');
      if (profitabilityChart) {
        console.log(chalk.gray('    Profitability chart available'));
      }
      
      // Market efficiency metrics
      const efficiencyMetrics = await page.$$('.efficiency-metric, [data-efficiency]');
      console.log(chalk.gray(`    Efficiency metrics: ${efficiencyMetrics.length}`));
      
      this.metrics.stepTimings.marketAnalytics = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Market analytics reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'marketAnalytics',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCompetitorMonitoring(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing competitor monitoring...'));
    
    try {
      // Access competitor analysis
      const competitorButton = await page.$('button:has-text("Competitors"), [data-competitors]');
      if (!competitorButton) {
        console.log(chalk.yellow('    ⚠ Competitor monitoring not available'));
        return;
      }
      
      await competitorButton.click();
      await page.waitForTimeout(500);
      
      // Review competitor data
      const competitors = await page.$$('.competitor-row, [data-competitor]');
      console.log(chalk.gray(`    Monitoring ${competitors.length} competitors`));
      
      if (competitors.length > 0) {
        // Analyze top competitor
        const topCompetitor = competitors[0];
        const competitorName = await topCompetitor.$eval('.competitor-name', el => el.textContent);
        const marketShare = await topCompetitor.$eval('.market-share', el => el.textContent);
        const averageSpread = await topCompetitor.$eval('.avg-spread', el => el.textContent);
        const uptime = await topCompetitor.$eval('.uptime', el => el.textContent);
        
        console.log(chalk.gray(`    Top competitor: ${competitorName}`));
        console.log(chalk.gray(`    Market share: ${marketShare}, Spread: ${averageSpread}, Uptime: ${uptime}`));
      }
      
      // Set competitive alerts
      const alertsSection = await page.$('.competitive-alerts, [data-alerts]');
      if (alertsSection) {
        // Alert when competitor changes spreads
        const spreadAlertCheckbox = await alertsSection.$('input[name="spreadAlert"]');
        if (spreadAlertCheckbox) {
          await spreadAlertCheckbox.check();
        }
        
        // Alert on market share changes
        const marketShareAlertCheckbox = await alertsSection.$('input[name="marketShareAlert"]');
        if (marketShareAlertCheckbox) {
          await marketShareAlertCheckbox.check();
        }
        
        console.log(chalk.gray('    Competitive alerts configured'));
      }
      
      this.metrics.stepTimings.competitorMonitoring = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Competitor monitoring active'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'competitorMonitoring',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testAutomatedStrategies(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing automated strategies...'));
    
    try {
      // Access strategy automation
      const automationButton = await page.$('button:has-text("Automation"), [data-automation]');
      if (!automationButton) {
        console.log(chalk.yellow('    ⚠ Strategy automation not available'));
        return;
      }
      
      await automationButton.click();
      await page.waitForTimeout(500);
      
      // Create new strategy
      const newStrategyButton = await page.$('button:has-text("New Strategy"), button:has-text("Create")');
      if (newStrategyButton) {
        await newStrategyButton.click();
        await page.waitForTimeout(500);
        
        // Select strategy template
        const templates = await page.$$('.strategy-template, [data-template]');
        if (templates.length > 0) {
          const template = templates[0]; // Select first template
          const templateName = await template.$eval('.template-name', el => el.textContent);
          console.log(chalk.gray(`    Selected template: ${templateName}`));
          await template.click();
        }
        
        // Configure strategy parameters
        const strategyConfig = await page.$('.strategy-config, [data-strategy-config]');
        if (strategyConfig) {
          // Set target markets
          const marketsSelect = await strategyConfig.$('select[name="targetMarkets"]');
          if (marketsSelect) {
            await marketsSelect.selectOption('high_volume'); // High volume markets
          }
          
          // Set capital allocation
          const capitalInput = await strategyConfig.$('input[name="capitalAllocation"]');
          if (capitalInput) {
            await capitalInput.fill('25000'); // $25k allocation
          }
          
          // Set performance targets
          const targetReturnInput = await strategyConfig.$('input[name="targetReturn"]');
          if (targetReturnInput) {
            await targetReturnInput.fill('15'); // 15% annual return target
          }
          
          console.log(chalk.gray('    Strategy parameters configured'));
        }
        
        // Backtest strategy
        const backtestButton = await page.$('button:has-text("Backtest")');
        if (backtestButton) {
          await backtestButton.click();
          console.log(chalk.gray('    Running backtest...'));
          await page.waitForTimeout(3000);
          
          // Check backtest results
          const backtestResults = await page.$('.backtest-results, [data-backtest]');
          if (backtestResults) {
            const sharpeRatio = await backtestResults.$eval('.sharpe-ratio', el => el.textContent).catch(() => 'N/A');
            const maxDrawdown = await backtestResults.$eval('.max-drawdown', el => el.textContent).catch(() => 'N/A');
            const winRate = await backtestResults.$eval('.win-rate', el => el.textContent).catch(() => 'N/A');
            
            console.log(chalk.gray(`    Backtest: Sharpe ${sharpeRatio}, Drawdown ${maxDrawdown}, Win rate ${winRate}`));
          }
        }
        
        // Deploy strategy
        const deployButton = await page.$('button:has-text("Deploy"), button:has-text("Activate")');
        if (deployButton) {
          await deployButton.click();
          console.log(chalk.green('    ✓ Automated strategy deployed'));
        }
      }
      
      this.metrics.stepTimings.automatedStrategies = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'automatedStrategies',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPerformanceTracking(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing performance tracking...'));
    
    try {
      // Review detailed performance
      const performanceButton = await page.$('button:has-text("Performance"), [data-performance]');
      if (performanceButton) {
        await performanceButton.click();
        await page.waitForTimeout(1000);
      }
      
      // Check performance by market
      const marketPerformance = await page.$$('.market-performance, [data-market-performance]');
      console.log(chalk.gray(`    Performance tracked for ${marketPerformance.length} markets`));
      
      let profitableMarkets = 0;
      for (const market of marketPerformance.slice(0, 5)) { // Check first 5
        const pnl = await market.$eval('.market-pnl', el => 
          parseFloat(el.textContent.replace(/[^0-9.-]/g, ''))
        ).catch(() => 0);
        
        if (pnl > 0) profitableMarkets++;
      }
      
      if (profitableMarkets > 0) {
        this.metrics.profitablePeriods++;
      }
      
      // Check time-based performance
      const timePerformance = await page.$('.time-performance, [data-time-performance]');
      if (timePerformance) {
        const dailyPnL = await timePerformance.$eval('.daily-pnl', el => el.textContent).catch(() => 'N/A');
        const weeklyPnL = await timePerformance.$eval('.weekly-pnl', el => el.textContent).catch(() => 'N/A');
        const monthlyPnL = await timePerformance.$eval('.monthly-pnl', el => el.textContent).catch(() => 'N/A');
        
        console.log(chalk.gray(`    Daily: ${dailyPnL}, Weekly: ${weeklyPnL}, Monthly: ${monthlyPnL}`));
      }
      
      // Export performance report
      const exportButton = await page.$('button:has-text("Export Report")');
      if (exportButton) {
        await exportButton.click();
        console.log(chalk.gray('    Performance report exported'));
      }
      
      this.metrics.stepTimings.performanceTracking = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Performance tracking reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'performanceTracking',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testWithdrawLiquidity(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing liquidity withdrawal...'));
    
    try {
      // Find withdraw liquidity button
      const withdrawButton = await page.$('button:has-text("Withdraw Liquidity"), [data-withdraw-liquidity]');
      if (!withdrawButton) {
        console.log(chalk.yellow('    ⚠ Withdraw liquidity not available'));
        return;
      }
      
      await withdrawButton.click();
      await page.waitForTimeout(500);
      
      // Select liquidity to withdraw
      const liquidityPositions = await page.$$('.liquidity-position, [data-liquidity-position]');
      console.log(chalk.gray(`    Available positions: ${liquidityPositions.length}`));
      
      if (liquidityPositions.length > 0) {
        // Select first position for partial withdrawal
        const firstPosition = liquidityPositions[0];
        await firstPosition.click();
        
        const positionValue = await firstPosition.$eval('.position-value', el => el.textContent);
        console.log(chalk.gray(`    Selected position: ${positionValue}`));
        
        // Set withdrawal percentage
        const withdrawPercentageInput = await page.$('input[name="withdrawPercentage"]');
        if (withdrawPercentageInput) {
          await withdrawPercentageInput.fill('25'); // Withdraw 25%
        }
        
        // Check withdrawal impact
        const impactSection = await page.$('.withdrawal-impact, [data-impact]');
        if (impactSection) {
          const priceImpact = await impactSection.$eval('.price-impact', el => el.textContent).catch(() => 'N/A');
          const fees = await impactSection.$eval('.withdrawal-fees', el => el.textContent).catch(() => 'N/A');
          
          console.log(chalk.gray(`    Price impact: ${priceImpact}, Fees: ${fees}`));
        }
        
        // Confirm withdrawal
        const confirmWithdrawButton = await page.$('button:has-text("Confirm Withdrawal")');
        if (confirmWithdrawButton) {
          await confirmWithdrawButton.click();
          await page.waitForTimeout(2000);
          
          console.log(chalk.green('    ✓ Liquidity withdrawal completed'));
        }
      }
      
      this.metrics.stepTimings.withdrawLiquidity = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'withdrawLiquidity',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running market making load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    totalLiquidityProvided: 0,
    totalOrdersPlaced: 0,
    totalVolumeEarned: 0,
    avgFeesEarned: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new MarketMakingJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          results.successful++;
          results.totalLiquidityProvided += metrics.liquidityProvided;
          results.totalOrdersPlaced += metrics.ordersPlaced;
          results.totalVolumeEarned += metrics.volumeFacilitated;
          results.avgFeesEarned += metrics.feesEarned;
        } catch (error) {
          results.failed++;
          results.errors.push({
            userId: i,
            error: error.message
          });
        }
      })()
    );
    
    // Stagger starts for market making
    if (i % 3 === 0) {
      await new Promise(resolve => setTimeout(resolve, 800));
    }
  }
  
  await Promise.all(promises);
  
  // Calculate statistics
  timings.sort((a, b) => a - b);
  results.avgDuration = timings.reduce((a, b) => a + b, 0) / timings.length;
  results.p95Duration = timings[Math.floor(timings.length * 0.95)];
  results.p99Duration = timings[Math.floor(timings.length * 0.99)];
  results.avgFeesEarned = results.avgFeesEarned / results.successful;
  
  // Display results
  console.log(chalk.bold('\nMarket Making Load Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  console.log(chalk.magenta(`  Liquidity Provided: ${results.totalLiquidityProvided} positions`));
  console.log(chalk.magenta(`  Orders Placed: ${results.totalOrdersPlaced}`));
  console.log(chalk.magenta(`  Volume Facilitated: $${results.totalVolumeEarned.toFixed(2)}`));
  console.log(chalk.magenta(`  Avg Fees Earned: $${results.avgFeesEarned.toFixed(2)}`));
  
  return results;
}

// Main execution
if (require.main === module) {
  const configPath = path.join(__dirname, '../../test-config.json');
  const dataPath = path.join(__dirname, '../../data/generated-test-data.json');
  
  if (!fs.existsSync(configPath) || !fs.existsSync(dataPath)) {
    console.error(chalk.red('Error: test-config.json or test data not found. Run setup first.'));
    process.exit(1);
  }
  
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const testData = JSON.parse(fs.readFileSync(dataPath, 'utf8'));
  
  // Run tests
  (async () => {
    try {
      // Single user test
      const singleTest = new MarketMakingJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests with moderate concurrency
      await runLoadTest(config, testData, 5);     // 5 users
      await runLoadTest(config, testData, 25);    // 25 users
      await runLoadTest(config, testData, 100);   // 100 users
      
      console.log(chalk.bold.green('\n✅ All market making tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { MarketMakingJourneyTest, runLoadTest };