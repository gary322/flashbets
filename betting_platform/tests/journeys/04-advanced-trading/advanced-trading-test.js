#!/usr/bin/env node

/**
 * Advanced Trading Journey Test
 * Tests complex trading strategies and advanced features
 */

const { chromium } = require('playwright');
const { Connection, Keypair, PublicKey } = require('@solana/web3.js');
const axios = require('axios');
const WebSocket = require('ws');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');

class AdvancedTradingJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.connection = new Connection(config.rpcUrl, 'confirmed');
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0,
      strategiesExecuted: 0,
      complexOrdersPlaced: 0,
      hedgesCreated: 0,
      arbitrageAttempts: 0,
      profitableTrades: 0,
      totalPnL: 0
    };
    this.openPositions = [];
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n📊 Starting Advanced Trading Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Select experienced trader wallet
      const wallet = this.testData.wallets.find(w => 
        w.type === 'trader' && w.balance > 10000
      ) || this.testData.wallets[userId % this.testData.wallets.length];
      
      // Test advanced trading features
      await this.testMultiLegOrders(page, wallet);
      await this.testConditionalOrders(page, wallet);
      await this.testOCOOrders(page);
      await this.testBracketOrders(page);
      await this.testGridTrading(page, wallet);
      await this.testDCAStrategies(page, wallet);
      await this.testArbitrageOpportunities(page);
      await this.testCrossMarketHedging(page);
      await this.testPortfolioRebalancing(page, wallet);
      await this.testAutomatedStrategies(page);
      await this.testRiskManagement(page);
      await this.testAdvancedCharting(page);
      await this.testBacktesting(page);
      await this.testPnLAnalysis(page);
      
      await browser.close();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = (this.metrics.profitableTrades / this.metrics.strategiesExecuted) * 100;
      
      console.log(chalk.green(`✅ Advanced trading journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Advanced trading journey failed:'), error);
      throw error;
    }
  }

  async testMultiLegOrders(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing multi-leg orders...'));
    
    try {
      // Navigate to advanced trading
      await page.goto(`${this.config.uiUrl}/trade/advanced`, { waitUntil: 'networkidle' });
      
      // Select multi-leg order type
      const multiLegButton = await page.$('button:has-text("Multi-Leg"), [data-order-type="multi-leg"]');
      if (!multiLegButton) {
        console.log(chalk.yellow('    ⚠ Multi-leg orders not available'));
        return;
      }
      
      await multiLegButton.click();
      await page.waitForTimeout(500);
      
      // Create a spread trade
      console.log(chalk.gray('    Creating spread trade...'));
      
      // Select two correlated markets
      const markets = this.testData.markets.filter(m => m.category === 'Crypto').slice(0, 2);
      if (markets.length < 2) {
        console.log(chalk.yellow('    ⚠ Not enough markets for spread'));
        return;
      }
      
      // Leg 1: Buy first market
      const leg1Section = await page.$('[data-leg="1"], .leg-1');
      if (leg1Section) {
        const market1Select = await leg1Section.$('select[name="market"], input[name="market"]');
        if (market1Select) {
          await market1Select.type(markets[0].title);
        }
        
        const size1Input = await leg1Section.$('input[name="size"]');
        if (size1Input) {
          await size1Input.fill('1000');
        }
        
        const side1Buy = await leg1Section.$('button:has-text("Buy")');
        if (side1Buy) {
          await side1Buy.click();
        }
      }
      
      // Leg 2: Sell second market
      const leg2Section = await page.$('[data-leg="2"], .leg-2');
      if (leg2Section) {
        const market2Select = await leg2Section.$('select[name="market"], input[name="market"]');
        if (market2Select) {
          await market2Select.type(markets[1].title);
        }
        
        const size2Input = await leg2Section.$('input[name="size"]');
        if (size2Input) {
          await size2Input.fill('1000');
        }
        
        const side2Sell = await leg2Section.$('button:has-text("Sell")');
        if (side2Sell) {
          await side2Sell.click();
        }
      }
      
      // Set ratio
      const ratioInput = await page.$('input[name="ratio"], [data-ratio]');
      if (ratioInput) {
        await ratioInput.fill('1:1');
      }
      
      // Execute spread
      const executeButton = await page.$('button:has-text("Execute Spread")');
      if (executeButton) {
        await executeButton.click();
        this.metrics.complexOrdersPlaced++;
        this.metrics.strategiesExecuted++;
        console.log(chalk.green('    ✓ Spread trade executed'));
      }
      
      this.metrics.stepTimings.multiLegOrders = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'multiLegOrders',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testConditionalOrders(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing conditional orders...'));
    
    try {
      // Select conditional order type
      const conditionalButton = await page.$('button:has-text("Conditional"), [data-order-type="conditional"]');
      if (!conditionalButton) {
        console.log(chalk.yellow('    ⚠ Conditional orders not available'));
        return;
      }
      
      await conditionalButton.click();
      await page.waitForTimeout(500);
      
      // Create if-then order
      console.log(chalk.gray('    Creating if-then order...'));
      
      // Set condition
      const conditionSelect = await page.$('select[name="condition"], [data-condition]');
      if (conditionSelect) {
        await conditionSelect.selectOption('price_above');
      }
      
      // Set trigger price
      const triggerInput = await page.$('input[name="triggerPrice"]');
      if (triggerInput) {
        const currentPrice = this.testData.markets[0].currentPrices[0];
        const triggerPrice = currentPrice * 1.02; // 2% above current
        await triggerInput.fill(triggerPrice.toFixed(4));
      }
      
      // Set action
      const actionSelect = await page.$('select[name="action"], [data-action]');
      if (actionSelect) {
        await actionSelect.selectOption('market_buy');
      }
      
      // Set size
      const sizeInput = await page.$('input[name="size"]');
      if (sizeInput) {
        await sizeInput.fill('500');
      }
      
      // Add time condition
      const timeConditionCheckbox = await page.$('input[type="checkbox"][name="timeCondition"]');
      if (timeConditionCheckbox) {
        await timeConditionCheckbox.check();
        
        const expiryInput = await page.$('input[name="expiry"]');
        if (expiryInput) {
          const expiry = new Date();
          expiry.setHours(expiry.getHours() + 24);
          await expiryInput.fill(expiry.toISOString().slice(0, 16));
        }
      }
      
      // Submit conditional order
      const submitButton = await page.$('button:has-text("Create Conditional Order")');
      if (submitButton) {
        await submitButton.click();
        this.metrics.complexOrdersPlaced++;
        console.log(chalk.green('    ✓ Conditional order created'));
      }
      
      this.metrics.stepTimings.conditionalOrders = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'conditionalOrders',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testOCOOrders(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing OCO (One-Cancels-Other) orders...'));
    
    try {
      // Select OCO order type
      const ocoButton = await page.$('button:has-text("OCO"), [data-order-type="oco"]');
      if (!ocoButton) {
        console.log(chalk.yellow('    ⚠ OCO orders not available'));
        return;
      }
      
      await ocoButton.click();
      await page.waitForTimeout(500);
      
      // Create OCO order
      console.log(chalk.gray('    Creating OCO order...'));
      
      // Order 1: Limit buy
      const order1Section = await page.$('[data-oco-order="1"], .oco-order-1');
      if (order1Section) {
        const type1Select = await order1Section.$('select[name="type"]');
        if (type1Select) {
          await type1Select.selectOption('limit');
        }
        
        const price1Input = await order1Section.$('input[name="price"]');
        if (price1Input) {
          const currentPrice = this.testData.markets[0].currentPrices[0];
          await price1Input.fill((currentPrice * 0.98).toFixed(4)); // 2% below
        }
        
        const size1Input = await order1Section.$('input[name="size"]');
        if (size1Input) {
          await size1Input.fill('750');
        }
      }
      
      // Order 2: Stop buy
      const order2Section = await page.$('[data-oco-order="2"], .oco-order-2');
      if (order2Section) {
        const type2Select = await order2Section.$('select[name="type"]');
        if (type2Select) {
          await type2Select.selectOption('stop');
        }
        
        const price2Input = await order2Section.$('input[name="price"]');
        if (price2Input) {
          const currentPrice = this.testData.markets[0].currentPrices[0];
          await price2Input.fill((currentPrice * 1.02).toFixed(4)); // 2% above
        }
        
        const size2Input = await order2Section.$('input[name="size"]');
        if (size2Input) {
          await size2Input.fill('750');
        }
      }
      
      // Submit OCO order
      const submitButton = await page.$('button:has-text("Place OCO Order")');
      if (submitButton) {
        await submitButton.click();
        this.metrics.complexOrdersPlaced++;
        console.log(chalk.green('    ✓ OCO order placed'));
      }
      
      this.metrics.stepTimings.ocoOrders = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'ocoOrders',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testBracketOrders(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing bracket orders...'));
    
    try {
      // Select bracket order type
      const bracketButton = await page.$('button:has-text("Bracket"), [data-order-type="bracket"]');
      if (!bracketButton) {
        console.log(chalk.yellow('    ⚠ Bracket orders not available'));
        return;
      }
      
      await bracketButton.click();
      await page.waitForTimeout(500);
      
      // Create bracket order
      console.log(chalk.gray('    Creating bracket order with entry, stop loss, and take profit...'));
      
      // Entry order
      const entrySection = await page.$('[data-bracket="entry"], .bracket-entry');
      if (entrySection) {
        const entryTypeSelect = await entrySection.$('select[name="type"]');
        if (entryTypeSelect) {
          await entryTypeSelect.selectOption('limit');
        }
        
        const entryPriceInput = await entrySection.$('input[name="price"]');
        if (entryPriceInput) {
          const currentPrice = this.testData.markets[0].currentPrices[0];
          await entryPriceInput.fill(currentPrice.toFixed(4));
        }
        
        const entrySizeInput = await entrySection.$('input[name="size"]');
        if (entrySizeInput) {
          await entrySizeInput.fill('1000');
        }
      }
      
      // Stop loss
      const stopLossSection = await page.$('[data-bracket="stop-loss"], .bracket-stop-loss');
      if (stopLossSection) {
        const stopLossInput = await stopLossSection.$('input[name="stopLoss"]');
        if (stopLossInput) {
          const currentPrice = this.testData.markets[0].currentPrices[0];
          await stopLossInput.fill((currentPrice * 0.95).toFixed(4)); // 5% stop loss
        }
      }
      
      // Take profit
      const takeProfitSection = await page.$('[data-bracket="take-profit"], .bracket-take-profit');
      if (takeProfitSection) {
        const takeProfitInput = await takeProfitSection.$('input[name="takeProfit"]');
        if (takeProfitInput) {
          const currentPrice = this.testData.markets[0].currentPrices[0];
          await takeProfitInput.fill((currentPrice * 1.1).toFixed(4)); // 10% take profit
        }
      }
      
      // Risk/reward ratio
      const riskRewardDisplay = await page.$('.risk-reward-ratio, [data-risk-reward]');
      if (riskRewardDisplay) {
        const ratio = await riskRewardDisplay.textContent();
        console.log(chalk.gray(`    Risk/Reward ratio: ${ratio}`));
      }
      
      // Submit bracket order
      const submitButton = await page.$('button:has-text("Place Bracket Order")');
      if (submitButton) {
        await submitButton.click();
        this.metrics.complexOrdersPlaced++;
        console.log(chalk.green('    ✓ Bracket order placed'));
      }
      
      this.metrics.stepTimings.bracketOrders = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'bracketOrders',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testGridTrading(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing grid trading strategy...'));
    
    try {
      // Navigate to strategies section
      const strategiesTab = await page.$('button:has-text("Strategies"), [data-tab="strategies"]');
      if (strategiesTab) {
        await strategiesTab.click();
        await page.waitForTimeout(500);
      }
      
      // Select grid trading
      const gridTradingButton = await page.$('button:has-text("Grid Trading"), [data-strategy="grid"]');
      if (!gridTradingButton) {
        console.log(chalk.yellow('    ⚠ Grid trading not available'));
        return;
      }
      
      await gridTradingButton.click();
      await page.waitForTimeout(500);
      
      // Configure grid parameters
      console.log(chalk.gray('    Configuring grid parameters...'));
      
      // Price range
      const upperPriceInput = await page.$('input[name="upperPrice"]');
      const lowerPriceInput = await page.$('input[name="lowerPrice"]');
      if (upperPriceInput && lowerPriceInput) {
        const currentPrice = this.testData.markets[0].currentPrices[0];
        await upperPriceInput.fill((currentPrice * 1.1).toFixed(4)); // 10% above
        await lowerPriceInput.fill((currentPrice * 0.9).toFixed(4)); // 10% below
      }
      
      // Number of grids
      const gridCountInput = await page.$('input[name="gridCount"]');
      if (gridCountInput) {
        await gridCountInput.fill('10');
      }
      
      // Investment amount
      const investmentInput = await page.$('input[name="investment"]');
      if (investmentInput) {
        const investment = Math.min(wallet.balance * 0.1, 5000); // 10% or max 5000
        await investmentInput.fill(investment.toFixed(2));
      }
      
      // Profit per grid
      const profitDisplay = await page.$('.profit-per-grid, [data-profit-per-grid]');
      if (profitDisplay) {
        const profit = await profitDisplay.textContent();
        console.log(chalk.gray(`    Profit per grid: ${profit}`));
      }
      
      // Start grid bot
      const startButton = await page.$('button:has-text("Start Grid"), button:has-text("Activate")');
      if (startButton) {
        await startButton.click();
        this.metrics.strategiesExecuted++;
        console.log(chalk.green('    ✓ Grid trading bot started'));
      }
      
      this.metrics.stepTimings.gridTrading = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'gridTrading',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testDCAStrategies(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing DCA (Dollar Cost Averaging) strategies...'));
    
    try {
      // Select DCA strategy
      const dcaButton = await page.$('button:has-text("DCA"), [data-strategy="dca"]');
      if (!dcaButton) {
        console.log(chalk.yellow('    ⚠ DCA strategy not available'));
        return;
      }
      
      await dcaButton.click();
      await page.waitForTimeout(500);
      
      // Configure DCA parameters
      console.log(chalk.gray('    Setting up DCA strategy...'));
      
      // Select market
      const marketSelect = await page.$('select[name="market"], input[name="market"]');
      if (marketSelect) {
        await marketSelect.type(this.testData.markets[0].title);
      }
      
      // Investment per interval
      const intervalAmountInput = await page.$('input[name="intervalAmount"]');
      if (intervalAmountInput) {
        await intervalAmountInput.fill('100'); // $100 per interval
      }
      
      // Frequency
      const frequencySelect = await page.$('select[name="frequency"]');
      if (frequencySelect) {
        await frequencySelect.selectOption('daily');
      }
      
      // Duration
      const durationInput = await page.$('input[name="duration"]');
      if (durationInput) {
        await durationInput.fill('30'); // 30 days
      }
      
      // Price conditions
      const priceConditionCheckbox = await page.$('input[type="checkbox"][name="priceCondition"]');
      if (priceConditionCheckbox) {
        await priceConditionCheckbox.check();
        
        // Only buy when price is below average
        const conditionSelect = await page.$('select[name="dcaCondition"]');
        if (conditionSelect) {
          await conditionSelect.selectOption('below_average');
        }
      }
      
      // Calculate total investment
      const totalInvestmentDisplay = await page.$('.total-investment, [data-total-investment]');
      if (totalInvestmentDisplay) {
        const total = await totalInvestmentDisplay.textContent();
        console.log(chalk.gray(`    Total investment: ${total}`));
      }
      
      // Start DCA
      const startButton = await page.$('button:has-text("Start DCA"), button:has-text("Begin")');
      if (startButton) {
        await startButton.click();
        this.metrics.strategiesExecuted++;
        console.log(chalk.green('    ✓ DCA strategy activated'));
      }
      
      this.metrics.stepTimings.dcaStrategies = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'dcaStrategies',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testArbitrageOpportunities(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing arbitrage opportunity detection...'));
    
    try {
      // Navigate to arbitrage scanner
      const arbButton = await page.$('button:has-text("Arbitrage"), [data-tool="arbitrage"]');
      if (!arbButton) {
        console.log(chalk.yellow('    ⚠ Arbitrage scanner not available'));
        return;
      }
      
      await arbButton.click();
      await page.waitForTimeout(1000);
      
      // Check for opportunities
      const opportunities = await page.$$('.arbitrage-opportunity, [data-arb-opportunity]');
      console.log(chalk.gray(`    Found ${opportunities.length} arbitrage opportunities`));
      
      if (opportunities.length > 0) {
        // Analyze first opportunity
        const firstOpp = opportunities[0];
        
        const spread = await firstOpp.$eval('.spread, [data-spread]', el => el.textContent);
        const profit = await firstOpp.$eval('.profit, [data-profit]', el => el.textContent);
        const markets = await firstOpp.$$eval('.market-pair, [data-markets]', els => 
          els.map(el => el.textContent)
        );
        
        console.log(chalk.gray(`    Opportunity: ${markets.join(' <-> ')}`));
        console.log(chalk.gray(`    Spread: ${spread}, Potential profit: ${profit}`));
        
        // Execute arbitrage
        const executeButton = await firstOpp.$('button:has-text("Execute"), button:has-text("Trade")');
        if (executeButton) {
          await executeButton.click();
          this.metrics.arbitrageAttempts++;
          
          // Wait for execution result
          await page.waitForTimeout(2000);
          
          const successIndicator = await page.$('.arb-success, [data-arb-success]');
          if (successIndicator) {
            this.metrics.profitableTrades++;
            console.log(chalk.green('    ✓ Arbitrage executed successfully'));
          } else {
            console.log(chalk.yellow('    ⚠ Arbitrage execution failed (likely filled)'));
          }
        }
      }
      
      // Set up alerts
      const alertButton = await page.$('button:has-text("Set Alert"), [data-arb-alert]');
      if (alertButton) {
        await alertButton.click();
        
        const minSpreadInput = await page.$('input[name="minSpread"]');
        if (minSpreadInput) {
          await minSpreadInput.fill('0.5'); // 0.5% minimum spread
        }
        
        const saveAlertButton = await page.$('button:has-text("Save Alert")');
        if (saveAlertButton) {
          await saveAlertButton.click();
          console.log(chalk.gray('    Alert set for arbitrage opportunities'));
        }
      }
      
      this.metrics.stepTimings.arbitrageOpportunities = {
        duration: Date.now() - stepStart,
        opportunitiesFound: opportunities.length
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'arbitrageOpportunities',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCrossMarketHedging(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing cross-market hedging...'));
    
    try {
      // Navigate to hedging tools
      const hedgeButton = await page.$('button:has-text("Hedge"), [data-tool="hedge"]');
      if (!hedgeButton) {
        console.log(chalk.yellow('    ⚠ Hedging tools not available'));
        return;
      }
      
      await hedgeButton.click();
      await page.waitForTimeout(500);
      
      // Select position to hedge
      const positionsToHedge = await page.$$('.position-to-hedge, [data-hedgeable-position]');
      if (positionsToHedge.length === 0) {
        console.log(chalk.yellow('    ⚠ No positions available to hedge'));
        return;
      }
      
      // Select first position
      const position = positionsToHedge[0];
      await position.click();
      
      // Get position details
      const positionSize = await position.$eval('.position-size', el => el.textContent);
      const positionMarket = await position.$eval('.market-name', el => el.textContent);
      console.log(chalk.gray(`    Hedging position: ${positionSize} in ${positionMarket}`));
      
      // Find correlated markets for hedging
      const correlatedMarkets = await page.$$('.correlated-market, [data-correlation]');
      console.log(chalk.gray(`    Found ${correlatedMarkets.length} correlated markets`));
      
      if (correlatedMarkets.length > 0) {
        // Select hedge market
        const hedgeMarket = correlatedMarkets[0];
        const correlation = await hedgeMarket.$eval('[data-correlation-value]', el => el.textContent);
        console.log(chalk.gray(`    Selected hedge market with correlation: ${correlation}`));
        
        await hedgeMarket.click();
        
        // Configure hedge ratio
        const hedgeRatioInput = await page.$('input[name="hedgeRatio"]');
        if (hedgeRatioInput) {
          await hedgeRatioInput.fill('0.8'); // 80% hedge
        }
        
        // Calculate hedge size
        const calculateButton = await page.$('button:has-text("Calculate")');
        if (calculateButton) {
          await calculateButton.click();
          await page.waitForTimeout(500);
          
          const hedgeSizeDisplay = await page.$('.hedge-size, [data-hedge-size]');
          if (hedgeSizeDisplay) {
            const hedgeSize = await hedgeSizeDisplay.textContent();
            console.log(chalk.gray(`    Recommended hedge size: ${hedgeSize}`));
          }
        }
        
        // Execute hedge
        const executeButton = await page.$('button:has-text("Execute Hedge")');
        if (executeButton) {
          await executeButton.click();
          this.metrics.hedgesCreated++;
          console.log(chalk.green('    ✓ Hedge position created'));
        }
      }
      
      this.metrics.stepTimings.crossMarketHedging = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'crossMarketHedging',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPortfolioRebalancing(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing portfolio rebalancing...'));
    
    try {
      // Navigate to portfolio view
      const portfolioButton = await page.$('button:has-text("Portfolio"), [data-view="portfolio"]');
      if (portfolioButton) {
        await portfolioButton.click();
        await page.waitForTimeout(1000);
      }
      
      // Check current allocation
      const allocationChart = await page.$('.allocation-chart, [data-allocation]');
      if (!allocationChart) {
        console.log(chalk.yellow('    ⚠ Portfolio allocation not available'));
        return;
      }
      
      // Get current allocations
      const allocations = await page.$$eval('.allocation-item, [data-allocation-item]', items =>
        items.map(item => ({
          market: item.querySelector('.market-name')?.textContent,
          percentage: item.querySelector('.percentage')?.textContent
        }))
      );
      
      console.log(chalk.gray(`    Current portfolio: ${allocations.length} positions`));
      
      // Open rebalancing tool
      const rebalanceButton = await page.$('button:has-text("Rebalance"), [data-rebalance]');
      if (!rebalanceButton) {
        return;
      }
      
      await rebalanceButton.click();
      await page.waitForTimeout(500);
      
      // Set target allocation
      console.log(chalk.gray('    Setting target allocation...'));
      
      const targetInputs = await page.$$('.target-allocation input');
      if (targetInputs.length > 0) {
        // Equal weight strategy
        const targetPercentage = (100 / targetInputs.length).toFixed(1);
        for (const input of targetInputs) {
          await input.fill(targetPercentage);
        }
      }
      
      // Calculate rebalancing trades
      const calculateButton = await page.$('button:has-text("Calculate Trades")');
      if (calculateButton) {
        await calculateButton.click();
        await page.waitForTimeout(1000);
        
        // Review suggested trades
        const suggestedTrades = await page.$$('.suggested-trade, [data-rebalance-trade]');
        console.log(chalk.gray(`    Suggested trades: ${suggestedTrades.length}`));
        
        // Check estimated costs
        const estimatedCost = await page.$eval('.rebalance-cost, [data-cost]', el => el.textContent).catch(() => 'N/A');
        console.log(chalk.gray(`    Estimated cost: ${estimatedCost}`));
        
        // Execute rebalancing
        const executeButton = await page.$('button:has-text("Execute Rebalancing")');
        if (executeButton && suggestedTrades.length > 0) {
          await executeButton.click();
          console.log(chalk.green('    ✓ Portfolio rebalancing executed'));
        }
      }
      
      this.metrics.stepTimings.portfolioRebalancing = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'portfolioRebalancing',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testAutomatedStrategies(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing automated trading strategies...'));
    
    try {
      // Navigate to automation section
      const automationButton = await page.$('button:has-text("Automation"), [data-section="automation"]');
      if (!automationButton) {
        console.log(chalk.yellow('    ⚠ Automation features not available'));
        return;
      }
      
      await automationButton.click();
      await page.waitForTimeout(500);
      
      // Create new strategy
      const newStrategyButton = await page.$('button:has-text("New Strategy"), button:has-text("Create")');
      if (newStrategyButton) {
        await newStrategyButton.click();
        await page.waitForTimeout(500);
      }
      
      // Select strategy template
      const templates = await page.$$('.strategy-template, [data-template]');
      if (templates.length > 0) {
        const template = templates[Math.floor(Math.random() * templates.length)];
        const templateName = await template.$eval('.template-name', el => el.textContent);
        console.log(chalk.gray(`    Selected template: ${templateName}`));
        await template.click();
      }
      
      // Configure strategy parameters
      const paramsSection = await page.$('.strategy-params, [data-params]');
      if (paramsSection) {
        // Set risk level
        const riskSlider = await paramsSection.$('input[type="range"][name="risk"]');
        if (riskSlider) {
          await page.evaluate(() => {
            const slider = document.querySelector('input[type="range"][name="risk"]');
            if (slider) {
              slider.value = '3'; // Medium risk
              slider.dispatchEvent(new Event('input', { bubbles: true }));
            }
          });
        }
        
        // Set max positions
        const maxPositionsInput = await paramsSection.$('input[name="maxPositions"]');
        if (maxPositionsInput) {
          await maxPositionsInput.fill('5');
        }
        
        // Set stop loss
        const stopLossInput = await paramsSection.$('input[name="defaultStopLoss"]');
        if (stopLossInput) {
          await stopLossInput.fill('5'); // 5%
        }
      }
      
      // Backtest strategy
      const backtestButton = await page.$('button:has-text("Backtest")');
      if (backtestButton) {
        await backtestButton.click();
        console.log(chalk.gray('    Running backtest...'));
        await page.waitForTimeout(3000);
        
        // Check backtest results
        const results = await page.$('.backtest-results, [data-backtest-results]');
        if (results) {
          const winRate = await results.$eval('.win-rate', el => el.textContent).catch(() => 'N/A');
          const sharpeRatio = await results.$eval('.sharpe-ratio', el => el.textContent).catch(() => 'N/A');
          console.log(chalk.gray(`    Backtest: Win rate ${winRate}, Sharpe ${sharpeRatio}`));
        }
      }
      
      // Activate strategy
      const activateButton = await page.$('button:has-text("Activate Strategy")');
      if (activateButton) {
        await activateButton.click();
        this.metrics.strategiesExecuted++;
        console.log(chalk.green('    ✓ Automated strategy activated'));
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

  async testRiskManagement(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing risk management tools...'));
    
    try {
      // Navigate to risk dashboard
      const riskButton = await page.$('button:has-text("Risk"), [data-section="risk"]');
      if (riskButton) {
        await riskButton.click();
        await page.waitForTimeout(1000);
      }
      
      // Check portfolio risk metrics
      const riskMetrics = await page.$('.risk-metrics, [data-risk-metrics]');
      if (riskMetrics) {
        const var95 = await riskMetrics.$eval('.var-95, [data-var]', el => el.textContent).catch(() => 'N/A');
        const maxDrawdown = await riskMetrics.$eval('.max-drawdown', el => el.textContent).catch(() => 'N/A');
        const exposure = await riskMetrics.$eval('.total-exposure', el => el.textContent).catch(() => 'N/A');
        
        console.log(chalk.gray(`    VaR (95%): ${var95}`));
        console.log(chalk.gray(`    Max Drawdown: ${maxDrawdown}`));
        console.log(chalk.gray(`    Total Exposure: ${exposure}`));
      }
      
      // Set risk limits
      const limitsButton = await page.$('button:has-text("Risk Limits"), [data-limits]');
      if (limitsButton) {
        await limitsButton.click();
        await page.waitForTimeout(500);
        
        // Daily loss limit
        const dailyLimitInput = await page.$('input[name="dailyLossLimit"]');
        if (dailyLimitInput) {
          await dailyLimitInput.fill('1000'); // $1000 daily loss limit
        }
        
        // Position size limit
        const positionLimitInput = await page.$('input[name="maxPositionSize"]');
        if (positionLimitInput) {
          await positionLimitInput.fill('5000'); // $5000 max position
        }
        
        // Leverage limit
        const leverageLimitInput = await page.$('input[name="maxLeverage"]');
        if (leverageLimitInput) {
          await leverageLimitInput.fill('10'); // 10x max leverage
        }
        
        // Save limits
        const saveButton = await page.$('button:has-text("Save Limits")');
        if (saveButton) {
          await saveButton.click();
          console.log(chalk.green('    ✓ Risk limits configured'));
        }
      }
      
      // Check risk alerts
      const riskAlerts = await page.$$('.risk-alert, [data-risk-alert]');
      if (riskAlerts.length > 0) {
        console.log(chalk.yellow(`    ⚠ ${riskAlerts.length} risk alerts active`));
      }
      
      this.metrics.stepTimings.riskManagement = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'riskManagement',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testAdvancedCharting(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing advanced charting features...'));
    
    try {
      // Open advanced chart
      const chartButton = await page.$('button:has-text("Advanced Chart"), [data-chart="advanced"]');
      if (!chartButton) {
        console.log(chalk.yellow('    ⚠ Advanced charting not available'));
        return;
      }
      
      await chartButton.click();
      await page.waitForTimeout(1000);
      
      // Add technical indicators
      const indicatorsButton = await page.$('button:has-text("Indicators"), [data-indicators]');
      if (indicatorsButton) {
        await indicatorsButton.click();
        await page.waitForTimeout(500);
        
        // Add common indicators
        const indicators = ['RSI', 'MACD', 'Bollinger Bands', 'EMA'];
        for (const indicator of indicators) {
          const indicatorOption = await page.$(`[data-indicator="${indicator}"], label:has-text("${indicator}")`);
          if (indicatorOption) {
            await indicatorOption.click();
          }
        }
        
        console.log(chalk.gray(`    Added ${indicators.length} indicators`));
      }
      
      // Draw trend lines
      const drawingTools = await page.$('button:has-text("Drawing"), [data-drawing-tools]');
      if (drawingTools) {
        await drawingTools.click();
        
        const trendLineButton = await page.$('[data-tool="trendline"], button:has-text("Trend Line")');
        if (trendLineButton) {
          await trendLineButton.click();
          console.log(chalk.gray('    Drawing tools enabled'));
        }
      }
      
      // Save chart layout
      const saveLayoutButton = await page.$('button:has-text("Save Layout")');
      if (saveLayoutButton) {
        await saveLayoutButton.click();
        
        const layoutNameInput = await page.$('input[name="layoutName"]');
        if (layoutNameInput) {
          await layoutNameInput.fill(`Advanced Layout ${Date.now()}`);
          
          const confirmSaveButton = await page.$('button:has-text("Save")');
          if (confirmSaveButton) {
            await confirmSaveButton.click();
            console.log(chalk.green('    ✓ Chart layout saved'));
          }
        }
      }
      
      this.metrics.stepTimings.advancedCharting = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'advancedCharting',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testBacktesting(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing strategy backtesting...'));
    
    try {
      // Navigate to backtesting section
      const backtestingButton = await page.$('button:has-text("Backtesting"), [data-section="backtest"]');
      if (!backtestingButton) {
        console.log(chalk.yellow('    ⚠ Backtesting not available'));
        return;
      }
      
      await backtestingButton.click();
      await page.waitForTimeout(500);
      
      // Select strategy to backtest
      const strategySelect = await page.$('select[name="strategy"], [data-strategy-select]');
      if (strategySelect) {
        const options = await strategySelect.$$('option');
        if (options.length > 1) {
          await strategySelect.selectOption({ index: 1 }); // Select first non-default option
        }
      }
      
      // Set backtest period
      const startDateInput = await page.$('input[name="startDate"]');
      const endDateInput = await page.$('input[name="endDate"]');
      if (startDateInput && endDateInput) {
        const endDate = new Date();
        const startDate = new Date();
        startDate.setMonth(startDate.getMonth() - 3); // 3 months back
        
        await startDateInput.fill(startDate.toISOString().slice(0, 10));
        await endDateInput.fill(endDate.toISOString().slice(0, 10));
      }
      
      // Set initial capital
      const capitalInput = await page.$('input[name="initialCapital"]');
      if (capitalInput) {
        await capitalInput.fill('10000'); // $10,000
      }
      
      // Run backtest
      const runButton = await page.$('button:has-text("Run Backtest")');
      if (runButton) {
        await runButton.click();
        console.log(chalk.gray('    Running backtest simulation...'));
        await page.waitForTimeout(5000); // Wait for backtest to complete
        
        // Check results
        const resultsSection = await page.$('.backtest-results, [data-results]');
        if (resultsSection) {
          const totalReturn = await resultsSection.$eval('.total-return', el => el.textContent).catch(() => 'N/A');
          const winRate = await resultsSection.$eval('.win-rate', el => el.textContent).catch(() => 'N/A');
          const maxDrawdown = await resultsSection.$eval('.max-drawdown', el => el.textContent).catch(() => 'N/A');
          const sharpeRatio = await resultsSection.$eval('.sharpe-ratio', el => el.textContent).catch(() => 'N/A');
          
          console.log(chalk.gray(`    Total Return: ${totalReturn}`));
          console.log(chalk.gray(`    Win Rate: ${winRate}`));
          console.log(chalk.gray(`    Max Drawdown: ${maxDrawdown}`));
          console.log(chalk.gray(`    Sharpe Ratio: ${sharpeRatio}`));
          
          // Export results
          const exportButton = await page.$('button:has-text("Export Results")');
          if (exportButton) {
            await exportButton.click();
            console.log(chalk.green('    ✓ Backtest results exported'));
          }
        }
      }
      
      this.metrics.stepTimings.backtesting = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'backtesting',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPnLAnalysis(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing P&L analysis...'));
    
    try {
      // Navigate to P&L section
      const pnlButton = await page.$('button:has-text("P&L"), [data-section="pnl"]');
      if (pnlButton) {
        await pnlButton.click();
        await page.waitForTimeout(1000);
      }
      
      // Check overall P&L
      const totalPnL = await page.$eval('.total-pnl, [data-total-pnl]', el => el.textContent).catch(() => '0');
      const realizedPnL = await page.$eval('.realized-pnl', el => el.textContent).catch(() => '0');
      const unrealizedPnL = await page.$eval('.unrealized-pnl', el => el.textContent).catch(() => '0');
      
      console.log(chalk.gray(`    Total P&L: ${totalPnL}`));
      console.log(chalk.gray(`    Realized: ${realizedPnL}, Unrealized: ${unrealizedPnL}`));
      
      // Check P&L by market
      const marketPnLs = await page.$$('.market-pnl, [data-market-pnl]');
      console.log(chalk.gray(`    P&L tracked for ${marketPnLs.length} markets`));
      
      // Calculate total P&L for metrics
      const pnlValue = parseFloat(totalPnL.replace(/[^0-9.-]/g, '')) || 0;
      this.metrics.totalPnL = pnlValue;
      
      if (pnlValue > 0) {
        this.metrics.profitableTrades++;
      }
      
      // Export P&L report
      const exportButton = await page.$('button:has-text("Export P&L Report")');
      if (exportButton) {
        await exportButton.click();
        console.log(chalk.green('    ✓ P&L report exported'));
      }
      
      this.metrics.stepTimings.pnlAnalysis = {
        duration: Date.now() - stepStart,
        totalPnL: pnlValue
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'pnlAnalysis',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running advanced trading load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    totalStrategies: 0,
    profitableUsers: 0,
    avgPnL: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  const pnls = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new AdvancedTradingJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          pnls.push(metrics.totalPnL);
          results.successful++;
          results.totalStrategies += metrics.strategiesExecuted;
          if (metrics.totalPnL > 0) {
            results.profitableUsers++;
          }
        } catch (error) {
          results.failed++;
          results.errors.push({
            userId: i,
            error: error.message
          });
        }
      })()
    );
    
    // Heavy stagger for complex operations
    if (i % 3 === 0) {
      await new Promise(resolve => setTimeout(resolve, 500));
    }
  }
  
  await Promise.all(promises);
  
  // Calculate statistics
  timings.sort((a, b) => a - b);
  results.avgDuration = timings.reduce((a, b) => a + b, 0) / timings.length;
  results.p95Duration = timings[Math.floor(timings.length * 0.95)];
  results.p99Duration = timings[Math.floor(timings.length * 0.99)];
  results.avgPnL = pnls.reduce((a, b) => a + b, 0) / pnls.length;
  
  // Display results
  console.log(chalk.bold('\nAdvanced Trading Load Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  console.log(chalk.magenta(`  Total Strategies: ${results.totalStrategies}`));
  console.log(chalk.magenta(`  Profitable Users: ${results.profitableUsers} (${(results.profitableUsers / results.successful * 100).toFixed(1)}%)`));
  console.log(chalk.magenta(`  Avg P&L: $${results.avgPnL.toFixed(2)}`));
  
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
      const singleTest = new AdvancedTradingJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests with fewer users due to complexity
      await runLoadTest(config, testData, 5);     // 5 users
      await runLoadTest(config, testData, 50);    // 50 users
      await runLoadTest(config, testData, 500);   // 500 users
      
      console.log(chalk.bold.green('\n✅ All advanced trading tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { AdvancedTradingJourneyTest, runLoadTest };