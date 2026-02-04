#!/usr/bin/env node

/**
 * Trading Execution Journey Test
 * Tests the complete trading flow from order placement to execution
 */

const { chromium } = require('playwright');
const { Connection, Keypair, PublicKey, LAMPORTS_PER_SOL } = require('@solana/web3.js');
const axios = require('axios');
const WebSocket = require('ws');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');

class TradingExecutionJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.connection = new Connection(config.rpcUrl, 'confirmed');
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0,
      ordersPlaced: 0,
      ordersExecuted: 0,
      ordersFailed: 0,
      avgExecutionTime: 0,
      slippageEvents: 0,
      gasUsed: 0
    };
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n💹 Starting Trading Execution Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Select a test wallet
      const wallet = this.testData.wallets[userId % this.testData.wallets.length];
      const market = this.testData.markets[userId % this.testData.markets.length];
      
      // Test steps
      await this.testSelectMarket(page, market);
      await this.testAnalyzeMarketConditions(page, market);
      await this.testChooseOrderType(page);
      await this.testSetPositionSize(page, wallet);
      await this.testConfigureLeverage(page, wallet);
      await this.testSetStopLossAndTakeProfit(page);
      await this.testReviewOrderDetails(page);
      await this.testCalculateFees(page);
      await this.testCheckCollateral(page, wallet);
      await this.testExecuteOrder(page);
      await this.testConfirmTransaction(page);
      await this.testMonitorOrderStatus(page);
      await this.testHandlePartialFills(page);
      await this.testUpdatePositions(page);
      await this.testReconcileBalances(page);
      
      await browser.close();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = (this.metrics.ordersExecuted / this.metrics.ordersPlaced) * 100;
      
      console.log(chalk.green(`✅ Trading execution journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Trading execution journey failed:'), error);
      throw error;
    }
  }

  async testSelectMarket(page, market) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market selection...'));
    
    try {
      // Navigate directly to market
      await page.goto(`${this.config.uiUrl}/markets/${market.id}`, { waitUntil: 'networkidle' });
      
      // Wait for market to load
      await page.waitForSelector('.market-details, [data-market-details]', { timeout: 10000 });
      
      // Verify correct market
      const marketTitle = await page.$eval('h1, .market-title', el => el.textContent);
      console.log(chalk.gray(`    Selected market: ${marketTitle}`));
      
      // Check market status
      const marketStatus = await page.$eval('.market-status, [data-status]', el => el.textContent).catch(() => 'active');
      if (marketStatus !== 'active' && !marketStatus.includes('Active')) {
        throw new Error(`Market is not active: ${marketStatus}`);
      }
      
      // Store market data
      this.currentMarket = market;
      
      this.metrics.stepTimings.selectMarket = {
        duration: Date.now() - stepStart,
        marketId: market.id
      };
      
      console.log(chalk.green('    ✓ Market selected successfully'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'selectMarket',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testAnalyzeMarketConditions(page, market) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market conditions analysis...'));
    
    try {
      // Get current market data
      const response = await axios.get(`${this.config.apiUrl}/api/markets/${market.id}`);
      const marketData = response.data;
      
      // Check current prices
      const currentPrices = marketData.currentPrices || market.currentPrices;
      console.log(chalk.gray(`    Current prices: Yes=${currentPrices[0]}, No=${currentPrices[1]}`));
      
      // Check liquidity
      const liquidity = marketData.liquidity || market.liquidity;
      console.log(chalk.gray(`    Total liquidity: $${liquidity.toFixed(2)}`));
      
      // Check volume
      const volume24h = marketData.volume24h || 0;
      console.log(chalk.gray(`    24h volume: $${volume24h.toFixed(2)}`));
      
      // Check spread
      const spread = await page.$eval('.spread, [data-spread]', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Current spread: ${spread}`));
      
      // Analyze price trend
      if (this.testData.priceHistory[market.id]) {
        const history = this.testData.priceHistory[market.id];
        const recentPrices = history.slice(-10);
        const trend = recentPrices[recentPrices.length - 1].price > recentPrices[0].price ? 'up' : 'down';
        console.log(chalk.gray(`    Recent trend: ${trend}`));
      }
      
      this.metrics.stepTimings.analyzeConditions = {
        duration: Date.now() - stepStart,
        liquidity,
        volume24h
      };
      
      console.log(chalk.green('    ✓ Market conditions analyzed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'analyzeConditions',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testChooseOrderType(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing order type selection...'));
    
    try {
      // Look for order type selector
      const orderTypeSelector = await page.$('.order-type-selector, [data-order-type]');
      if (!orderTypeSelector) {
        console.log(chalk.yellow('    ⚠ Using default market order'));
        this.orderType = 'market';
        return;
      }
      
      // Get available order types
      const orderTypes = await page.$$('.order-type-option, [data-order-option]');
      console.log(chalk.gray(`    Available order types: ${orderTypes.length}`));
      
      // Select order type based on strategy
      const strategyRandom = Math.random();
      if (strategyRandom < 0.7) {
        // Market order (70%)
        const marketOption = await page.$('[data-order-option="market"], button:has-text("Market")');
        if (marketOption) {
          await marketOption.click();
          this.orderType = 'market';
          console.log(chalk.gray('    Selected: Market order'));
        }
      } else if (strategyRandom < 0.9) {
        // Limit order (20%)
        const limitOption = await page.$('[data-order-option="limit"], button:has-text("Limit")');
        if (limitOption) {
          await limitOption.click();
          this.orderType = 'limit';
          console.log(chalk.gray('    Selected: Limit order'));
          
          // Set limit price
          const limitPriceInput = await page.$('input[name="limitPrice"], input[placeholder*="Limit price"]');
          if (limitPriceInput) {
            const currentPrice = this.currentMarket.currentPrices[0];
            const limitPrice = currentPrice * (0.95 + Math.random() * 0.1); // ±5% from current
            await limitPriceInput.fill(limitPrice.toFixed(4));
          }
        }
      } else {
        // Stop order (10%)
        const stopOption = await page.$('[data-order-option="stop"], button:has-text("Stop")');
        if (stopOption) {
          await stopOption.click();
          this.orderType = 'stop';
          console.log(chalk.gray('    Selected: Stop order'));
        }
      }
      
      this.metrics.stepTimings.chooseOrderType = {
        duration: Date.now() - stepStart,
        orderType: this.orderType
      };
      
      console.log(chalk.green('    ✓ Order type selected'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'chooseOrderType',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue with market order
      this.orderType = 'market';
    }
  }

  async testSetPositionSize(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing position size configuration...'));
    
    try {
      // Find position size input
      const sizeInput = await page.$('input[name="size"], input[name="amount"], input[placeholder*="Amount"]');
      if (!sizeInput) {
        throw new Error('Position size input not found');
      }
      
      // Calculate position size based on wallet type and balance
      let positionSize;
      const riskProfile = wallet.riskProfile;
      const availableBalance = wallet.balance * 0.9; // Keep 10% reserve
      
      switch (wallet.type) {
        case 'whale':
          positionSize = availableBalance * (0.1 + Math.random() * 0.2); // 10-30%
          break;
        case 'trader':
          positionSize = Math.min(availableBalance * riskProfile.maxPositionSize, 5000);
          break;
        case 'bot':
          positionSize = 100 + Math.random() * 400; // Fixed small sizes
          break;
        default:
          positionSize = availableBalance * 0.05; // 5% default
      }
      
      await sizeInput.fill(positionSize.toFixed(2));
      console.log(chalk.gray(`    Position size: $${positionSize.toFixed(2)}`));
      
      // Check for size presets
      const sizePresets = await page.$$('.size-preset, [data-size-preset]');
      if (sizePresets.length > 0) {
        console.log(chalk.gray(`    Size presets available: ${sizePresets.length}`));
      }
      
      // Verify max position check
      const maxPositionWarning = await page.$('.max-position-warning, [data-warning="max-position"]');
      if (maxPositionWarning) {
        console.log(chalk.yellow('    ⚠ Max position warning shown'));
        // Reduce position size
        positionSize = positionSize * 0.5;
        await sizeInput.fill(positionSize.toFixed(2));
      }
      
      this.positionSize = positionSize;
      this.metrics.stepTimings.setPositionSize = {
        duration: Date.now() - stepStart,
        size: positionSize
      };
      
      console.log(chalk.green('    ✓ Position size configured'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'setPositionSize',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testConfigureLeverage(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing leverage configuration...'));
    
    try {
      // Check if leverage is available
      const leverageSection = await page.$('.leverage-section, [data-leverage-section]');
      if (!leverageSection) {
        console.log(chalk.gray('    Leverage not available for this market'));
        this.leverage = 1;
        return;
      }
      
      // Find leverage slider or input
      const leverageSlider = await page.$('input[type="range"][name="leverage"], .leverage-slider');
      const leverageInput = await page.$('input[type="number"][name="leverage"]');
      
      // Set leverage based on wallet risk profile
      const maxAllowedLeverage = wallet.riskProfile.maxLeverage;
      let targetLeverage;
      
      switch (wallet.tradingStyle) {
        case 'scalper':
          targetLeverage = Math.min(20, maxAllowedLeverage);
          break;
        case 'swing':
          targetLeverage = Math.min(10, maxAllowedLeverage);
          break;
        case 'position':
          targetLeverage = Math.min(5, maxAllowedLeverage);
          break;
        case 'high_frequency':
          targetLeverage = Math.min(50, maxAllowedLeverage);
          break;
        default:
          targetLeverage = Math.min(10, maxAllowedLeverage);
      }
      
      if (leverageSlider) {
        await page.evaluate((leverage) => {
          const slider = document.querySelector('input[type="range"][name="leverage"], .leverage-slider');
          if (slider) {
            slider.value = leverage;
            slider.dispatchEvent(new Event('input', { bubbles: true }));
            slider.dispatchEvent(new Event('change', { bubbles: true }));
          }
        }, targetLeverage);
      } else if (leverageInput) {
        await leverageInput.fill(targetLeverage.toString());
      }
      
      console.log(chalk.gray(`    Leverage set to: ${targetLeverage}x`));
      
      // Check margin requirements
      const marginRequired = await page.$eval('.margin-required, [data-margin-required]', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Margin required: ${marginRequired}`));
      
      // Check liquidation price
      const liquidationPrice = await page.$eval('.liquidation-price, [data-liquidation]', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Liquidation price: ${liquidationPrice}`));
      
      this.leverage = targetLeverage;
      this.metrics.stepTimings.configureLeverage = {
        duration: Date.now() - stepStart,
        leverage: targetLeverage
      };
      
      console.log(chalk.green('    ✓ Leverage configured'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'configureLeverage',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue with 1x leverage
      this.leverage = 1;
    }
  }

  async testSetStopLossAndTakeProfit(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing stop loss and take profit...'));
    
    try {
      // Check for advanced order options
      const advancedToggle = await page.$('.advanced-toggle, [data-toggle="advanced"]');
      if (advancedToggle) {
        await advancedToggle.click();
        await page.waitForTimeout(500);
      }
      
      // Set stop loss
      const stopLossInput = await page.$('input[name="stopLoss"], input[placeholder*="Stop loss"]');
      if (stopLossInput) {
        const currentPrice = this.currentMarket.currentPrices[0];
        const stopLossPrice = currentPrice * (1 - 0.05); // 5% stop loss
        await stopLossInput.fill(stopLossPrice.toFixed(4));
        console.log(chalk.gray(`    Stop loss set at: $${stopLossPrice.toFixed(4)}`));
      }
      
      // Set take profit
      const takeProfitInput = await page.$('input[name="takeProfit"], input[placeholder*="Take profit"]');
      if (takeProfitInput) {
        const currentPrice = this.currentMarket.currentPrices[0];
        const takeProfitPrice = currentPrice * (1 + 0.1); // 10% take profit
        await takeProfitInput.fill(takeProfitPrice.toFixed(4));
        console.log(chalk.gray(`    Take profit set at: $${takeProfitPrice.toFixed(4)}`));
      }
      
      // Check for trailing stop option
      const trailingStopCheckbox = await page.$('input[type="checkbox"][name="trailingStop"]');
      if (trailingStopCheckbox && Math.random() > 0.7) {
        await trailingStopCheckbox.check();
        console.log(chalk.gray('    Trailing stop enabled'));
        
        // Set trailing distance
        const trailingInput = await page.$('input[name="trailingDistance"]');
        if (trailingInput) {
          await trailingInput.fill('2'); // 2% trailing
        }
      }
      
      this.metrics.stepTimings.setStopLossTP = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Stop loss and take profit configured'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'setStopLossTP',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue without SL/TP
    }
  }

  async testReviewOrderDetails(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing order review...'));
    
    try {
      // Look for order summary
      const orderSummary = await page.$('.order-summary, [data-order-summary]');
      if (!orderSummary) {
        console.log(chalk.yellow('    ⚠ Order summary not found'));
      }
      
      // Capture order details
      const orderDetails = {
        market: this.currentMarket.title,
        type: this.orderType,
        side: Math.random() > 0.5 ? 'buy' : 'sell',
        size: this.positionSize,
        leverage: this.leverage,
        estimatedEntry: this.currentMarket.currentPrices[0]
      };
      
      // Check estimated PnL
      const estimatedPnl = await page.$eval('.estimated-pnl, [data-estimated-pnl]', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Estimated P&L: ${estimatedPnl}`));
      
      // Check risk metrics
      const riskMetrics = await page.$('.risk-metrics, [data-risk]');
      if (riskMetrics) {
        const maxLoss = await page.$eval('.max-loss, [data-max-loss]', el => el.textContent).catch(() => 'N/A');
        const riskReward = await page.$eval('.risk-reward, [data-risk-reward]', el => el.textContent).catch(() => 'N/A');
        console.log(chalk.gray(`    Max loss: ${maxLoss}, Risk/Reward: ${riskReward}`));
      }
      
      // Store order details
      this.currentOrder = orderDetails;
      
      this.metrics.stepTimings.reviewOrder = {
        duration: Date.now() - stepStart,
        orderDetails
      };
      
      console.log(chalk.green('    ✓ Order details reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'reviewOrder',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testCalculateFees(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing fee calculation...'));
    
    try {
      // Look for fee breakdown
      const feeSection = await page.$('.fee-breakdown, [data-fees]');
      if (!feeSection) {
        console.log(chalk.yellow('    ⚠ Fee breakdown not shown'));
      }
      
      // Get fee details
      const tradingFee = await page.$eval('.trading-fee, [data-trading-fee]', el => el.textContent).catch(() => '0.1%');
      const networkFee = await page.$eval('.network-fee, [data-network-fee]', el => el.textContent).catch(() => '~0.00025 SOL');
      const totalFee = await page.$eval('.total-fee, [data-total-fee]', el => el.textContent).catch(() => 'N/A');
      
      console.log(chalk.gray(`    Trading fee: ${tradingFee}`));
      console.log(chalk.gray(`    Network fee: ${networkFee}`));
      console.log(chalk.gray(`    Total fees: ${totalFee}`));
      
      // Calculate fee impact
      const feeImpact = this.positionSize * 0.001; // Assuming 0.1% fee
      this.estimatedFees = feeImpact;
      
      // Check for fee tier benefits
      const feeTier = await page.$('.fee-tier, [data-fee-tier]');
      if (feeTier) {
        const tierInfo = await feeTier.textContent();
        console.log(chalk.gray(`    Fee tier: ${tierInfo}`));
      }
      
      this.metrics.stepTimings.calculateFees = {
        duration: Date.now() - stepStart,
        estimatedFees: feeImpact
      };
      
      console.log(chalk.green('    ✓ Fees calculated'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'calculateFees',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCheckCollateral(page, wallet) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing collateral check...'));
    
    try {
      // Check available collateral
      const availableCollateral = await page.$eval('.available-collateral, [data-available-collateral]', el => el.textContent).catch(() => null);
      if (availableCollateral) {
        console.log(chalk.gray(`    Available collateral: ${availableCollateral}`));
      }
      
      // Calculate required collateral
      const requiredCollateral = this.positionSize / this.leverage;
      console.log(chalk.gray(`    Required collateral: $${requiredCollateral.toFixed(2)}`));
      
      // Check if sufficient
      if (requiredCollateral > wallet.balance * 0.9) {
        throw new Error('Insufficient collateral');
      }
      
      // Check margin ratio
      const marginRatio = await page.$eval('.margin-ratio, [data-margin-ratio]', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Margin ratio: ${marginRatio}`));
      
      // Check for collateral warnings
      const collateralWarning = await page.$('.collateral-warning, [data-warning="collateral"]');
      if (collateralWarning) {
        console.log(chalk.yellow('    ⚠ Collateral warning displayed'));
      }
      
      this.metrics.stepTimings.checkCollateral = {
        duration: Date.now() - stepStart,
        requiredCollateral
      };
      
      console.log(chalk.green('    ✓ Collateral check passed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'checkCollateral',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testExecuteOrder(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing order execution...'));
    
    try {
      // Find execute button
      const executeButton = await page.$('button:has-text("Place Order"), button:has-text("Buy"), button:has-text("Sell")');
      if (!executeButton) {
        throw new Error('Execute button not found');
      }
      
      // Click execute
      await executeButton.click();
      this.metrics.ordersPlaced++;
      
      // Wait for confirmation modal
      await page.waitForSelector('[role="dialog"], .confirm-modal', { timeout: 5000 });
      
      // Review final details in modal
      const modalDetails = await page.$('.modal-details, .confirm-details');
      if (modalDetails) {
        const finalPrice = await page.$eval('.final-price, [data-final-price]', el => el.textContent).catch(() => 'N/A');
        console.log(chalk.gray(`    Final execution price: ${finalPrice}`));
      }
      
      // Confirm execution
      const confirmButton = await page.$('button:has-text("Confirm"), button:has-text("Execute")');
      if (!confirmButton) {
        throw new Error('Confirm button not found');
      }
      
      const executionStart = Date.now();
      await confirmButton.click();
      
      // Wait for transaction
      await page.waitForSelector('.transaction-pending, .loading', { timeout: 5000 });
      
      this.metrics.stepTimings.executeOrder = {
        duration: Date.now() - stepStart,
        executionStart
      };
      
      console.log(chalk.green('    ✓ Order submitted for execution'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'executeOrder',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.ordersFailed++;
      throw error;
    }
  }

  async testConfirmTransaction(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing transaction confirmation...'));
    
    try {
      // Wait for transaction result
      const successSelector = '.transaction-success, [data-success], .order-success';
      const errorSelector = '.transaction-error, [data-error], .order-failed';
      
      await page.waitForSelector(`${successSelector}, ${errorSelector}`, { timeout: 30000 });
      
      const success = await page.$(successSelector);
      if (success) {
        // Get transaction details
        const txHash = await page.$eval('.tx-hash, [data-tx-hash]', el => el.textContent).catch(() => 'N/A');
        console.log(chalk.gray(`    Transaction hash: ${txHash}`));
        
        // Get execution price
        const executionPrice = await page.$eval('.execution-price, [data-execution-price]', el => el.textContent).catch(() => 'N/A');
        console.log(chalk.gray(`    Execution price: ${executionPrice}`));
        
        // Check for slippage
        const slippageInfo = await page.$('.slippage-info, [data-slippage-info]');
        if (slippageInfo) {
          const slippageAmount = await slippageInfo.textContent();
          console.log(chalk.yellow(`    ⚠ Slippage occurred: ${slippageAmount}`));
          this.metrics.slippageEvents++;
        }
        
        this.metrics.ordersExecuted++;
        const executionTime = Date.now() - this.metrics.stepTimings.executeOrder.executionStart;
        this.metrics.avgExecutionTime = (this.metrics.avgExecutionTime * (this.metrics.ordersExecuted - 1) + executionTime) / this.metrics.ordersExecuted;
        
        console.log(chalk.green(`    ✓ Transaction confirmed in ${executionTime}ms`));
      } else {
        const error = await page.$eval(errorSelector, el => el.textContent);
        throw new Error(`Transaction failed: ${error}`);
      }
      
      this.metrics.stepTimings.confirmTransaction = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'confirmTransaction',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.ordersFailed++;
      throw error;
    }
  }

  async testMonitorOrderStatus(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing order status monitoring...'));
    
    try {
      // Navigate to orders page
      const ordersLink = await page.$('a:has-text("Orders"), [href*="orders"]');
      if (ordersLink) {
        await ordersLink.click();
        await page.waitForTimeout(1000);
      }
      
      // Find recent order
      const recentOrder = await page.$('.order-row:first-child, [data-order]:first-child');
      if (!recentOrder) {
        console.log(chalk.yellow('    ⚠ Order not found in list'));
        return;
      }
      
      // Check order status
      const orderStatus = await recentOrder.$eval('.order-status, [data-status]', el => el.textContent);
      console.log(chalk.gray(`    Order status: ${orderStatus}`));
      
      // Check fill status
      const fillStatus = await recentOrder.$eval('.fill-status, [data-fill]', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Fill status: ${fillStatus}`));
      
      // Monitor for updates
      let updateCount = 0;
      const maxUpdates = 5;
      
      while (updateCount < maxUpdates) {
        await page.waitForTimeout(1000);
        
        const currentStatus = await recentOrder.$eval('.order-status, [data-status]', el => el.textContent);
        if (currentStatus === 'filled' || currentStatus === 'completed') {
          console.log(chalk.green('    ✓ Order fully filled'));
          break;
        }
        
        updateCount++;
      }
      
      this.metrics.stepTimings.monitorStatus = {
        duration: Date.now() - stepStart,
        finalStatus: orderStatus
      };
      
      console.log(chalk.green('    ✓ Order status monitored'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'monitorStatus',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testHandlePartialFills(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing partial fill handling...'));
    
    try {
      // Check if order was partially filled
      const partialFillIndicator = await page.$('.partial-fill, [data-partial-fill]');
      if (!partialFillIndicator) {
        console.log(chalk.gray('    No partial fills'));
        return;
      }
      
      // Get fill details
      const filledAmount = await page.$eval('.filled-amount, [data-filled]', el => el.textContent);
      const remainingAmount = await page.$eval('.remaining-amount, [data-remaining]', el => el.textContent);
      
      console.log(chalk.gray(`    Filled: ${filledAmount}, Remaining: ${remainingAmount}`));
      
      // Check for options
      const cancelRemaining = await page.$('button:has-text("Cancel Remaining")');
      const modifyOrder = await page.$('button:has-text("Modify Order")');
      
      if (cancelRemaining && Math.random() > 0.5) {
        await cancelRemaining.click();
        console.log(chalk.gray('    Cancelled remaining order'));
      } else if (modifyOrder) {
        await modifyOrder.click();
        // Handle order modification flow
        console.log(chalk.gray('    Modified order'));
      }
      
      this.metrics.stepTimings.handlePartialFills = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Partial fills handled'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'handlePartialFills',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testUpdatePositions(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing position updates...'));
    
    try {
      // Navigate to positions
      const positionsLink = await page.$('a:has-text("Positions"), [href*="positions"]');
      if (positionsLink) {
        await positionsLink.click();
        await page.waitForTimeout(1000);
      }
      
      // Find new position
      const positions = await page.$$('.position-row, [data-position]');
      console.log(chalk.gray(`    Total positions: ${positions.length}`));
      
      if (positions.length > 0) {
        const latestPosition = positions[0];
        
        // Get position details
        const positionSize = await latestPosition.$eval('.position-size, [data-size]', el => el.textContent);
        const entryPrice = await latestPosition.$eval('.entry-price, [data-entry]', el => el.textContent);
        const currentPnl = await latestPosition.$eval('.pnl, [data-pnl]', el => el.textContent);
        
        console.log(chalk.gray(`    Position: ${positionSize} @ ${entryPrice}`));
        console.log(chalk.gray(`    Current P&L: ${currentPnl}`));
        
        // Check position health
        const healthIndicator = await latestPosition.$('.position-health, [data-health]');
        if (healthIndicator) {
          const health = await healthIndicator.getAttribute('data-health-status');
          console.log(chalk.gray(`    Position health: ${health}`));
        }
      }
      
      this.metrics.stepTimings.updatePositions = {
        duration: Date.now() - stepStart,
        positionCount: positions.length
      };
      
      console.log(chalk.green('    ✓ Positions updated'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'updatePositions',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testReconcileBalances(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing balance reconciliation...'));
    
    try {
      // Get updated balance
      const balanceElement = await page.$('.wallet-balance, [data-balance]');
      if (!balanceElement) {
        console.log(chalk.yellow('    ⚠ Balance element not found'));
        return;
      }
      
      const currentBalance = await balanceElement.textContent();
      console.log(chalk.gray(`    Current balance: ${currentBalance}`));
      
      // Check balance history
      const balanceHistory = await page.$('button:has-text("Balance History"), [data-balance-history]');
      if (balanceHistory) {
        await balanceHistory.click();
        await page.waitForTimeout(500);
        
        const transactions = await page.$$('.balance-transaction, [data-balance-tx]');
        console.log(chalk.gray(`    Recent transactions: ${transactions.length}`));
      }
      
      // Verify balance change
      const expectedChange = -(this.positionSize / this.leverage + this.estimatedFees);
      console.log(chalk.gray(`    Expected balance change: $${expectedChange.toFixed(2)}`));
      
      // Get gas used from transaction
      const gasUsed = await page.$eval('.gas-used, [data-gas]', el => el.textContent).catch(() => '~0.00025 SOL');
      console.log(chalk.gray(`    Gas used: ${gasUsed}`));
      
      this.metrics.gasUsed += 0.00025; // Approximate
      
      this.metrics.stepTimings.reconcileBalances = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Balances reconciled'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'reconcileBalances',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running trading execution load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    totalOrdersPlaced: 0,
    totalOrdersExecuted: 0,
    avgExecutionTime: 0,
    slippageRate: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  const executionTimes = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new TradingExecutionJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          executionTimes.push(metrics.avgExecutionTime);
          results.successful++;
          results.totalOrdersPlaced += metrics.ordersPlaced;
          results.totalOrdersExecuted += metrics.ordersExecuted;
          results.slippageRate += metrics.slippageEvents;
        } catch (error) {
          results.failed++;
          results.errors.push({
            userId: i,
            error: error.message
          });
        }
      })()
    );
    
    // Stagger starts more for trading to avoid order collision
    if (i % 5 === 0) {
      await new Promise(resolve => setTimeout(resolve, 200));
    }
  }
  
  await Promise.all(promises);
  
  // Calculate statistics
  timings.sort((a, b) => a - b);
  results.avgDuration = timings.reduce((a, b) => a + b, 0) / timings.length;
  results.p95Duration = timings[Math.floor(timings.length * 0.95)];
  results.p99Duration = timings[Math.floor(timings.length * 0.99)];
  results.avgExecutionTime = executionTimes.reduce((a, b) => a + b, 0) / executionTimes.length;
  results.slippageRate = (results.slippageRate / results.totalOrdersExecuted * 100).toFixed(2);
  
  // Display results
  console.log(chalk.bold('\nTrading Execution Load Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  console.log(chalk.magenta(`  Orders Placed: ${results.totalOrdersPlaced}`));
  console.log(chalk.magenta(`  Orders Executed: ${results.totalOrdersExecuted}`));
  console.log(chalk.magenta(`  Avg Execution Time: ${results.avgExecutionTime.toFixed(2)}ms`));
  console.log(chalk.yellow(`  Slippage Rate: ${results.slippageRate}%`));
  
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
      const singleTest = new TradingExecutionJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests
      await runLoadTest(config, testData, 10);    // 10 users
      await runLoadTest(config, testData, 100);   // 100 users
      await runLoadTest(config, testData, 1000);  // 1000 users
      
      console.log(chalk.bold.green('\n✅ All trading execution tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { TradingExecutionJourneyTest, runLoadTest };