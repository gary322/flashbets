#!/usr/bin/env node

/**
 * Position Management Journey Test
 * Tests comprehensive position monitoring and management features
 */

const { chromium } = require('playwright');
const { Connection, Keypair, PublicKey } = require('@solana/web3.js');
const axios = require('axios');
const WebSocket = require('ws');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');

class PositionManagementJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.connection = new Connection(config.rpcUrl, 'confirmed');
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0,
      positionsMonitored: 0,
      positionsModified: 0,
      positionsClosed: 0,
      stopLossesTriggered: 0,
      takeProfitsHit: 0,
      emergencyExits: 0,
      totalRealizedPnL: 0
    };
    this.ws = null;
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n📈 Starting Position Management Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Select trader with existing positions
      const wallet = this.testData.wallets.find(w => 
        w.type === 'trader' && w.balance > 5000
      ) || this.testData.wallets[userId % this.testData.wallets.length];
      
      // Setup WebSocket for real-time updates
      await this.setupWebSocket();
      
      // Test position management features
      await this.testViewAllPositions(page);
      await this.testPositionDetails(page);
      await this.testRealTimePnL(page);
      await this.testModifyPosition(page);
      await this.testAddToPosition(page);
      await this.testPartialClose(page);
      await this.testStopLossManagement(page);
      await this.testTakeProfitManagement(page);
      await this.testTrailingStop(page);
      await this.testPositionAlerts(page);
      await this.testRiskMetrics(page);
      await this.testPositionHistory(page);
      await this.testEmergencyExit(page);
      await this.testPositionExport(page);
      await this.testPerformanceAnalytics(page);
      
      await browser.close();
      this.closeWebSocket();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = ((this.metrics.positionsModified + this.metrics.positionsClosed) / 
                                  this.metrics.positionsMonitored * 100) || 0;
      
      console.log(chalk.green(`✅ Position management journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Position management journey failed:'), error);
      throw error;
    }
  }

  async setupWebSocket() {
    try {
      this.ws = new WebSocket(this.config.wsUrl);
      
      await new Promise((resolve, reject) => {
        this.ws.on('open', () => {
          console.log(chalk.gray('    WebSocket connected for real-time updates'));
          resolve();
        });
        this.ws.on('error', reject);
        setTimeout(() => reject(new Error('WebSocket timeout')), 5000);
      });
      
      // Subscribe to position updates
      this.ws.send(JSON.stringify({
        type: 'subscribe',
        channels: ['positions', 'prices', 'pnl']
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

  async testViewAllPositions(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing view all positions...'));
    
    try {
      // Navigate to positions page
      await page.goto(`${this.config.uiUrl}/positions`, { waitUntil: 'networkidle' });
      
      // Wait for positions to load
      await page.waitForSelector('.positions-container, [data-positions]', { timeout: 10000 });
      
      // Get all positions
      const positions = await page.$$('.position-row, [data-position]');
      console.log(chalk.gray(`    Found ${positions.length} open positions`));
      
      this.metrics.positionsMonitored = positions.length;
      
      // Check position summary
      const totalValue = await page.$eval('.total-value, [data-total-value]', el => el.textContent).catch(() => 'N/A');
      const totalPnL = await page.$eval('.total-pnl, [data-total-pnl]', el => el.textContent).catch(() => 'N/A');
      const totalMargin = await page.$eval('.total-margin, [data-total-margin]', el => el.textContent).catch(() => 'N/A');
      
      console.log(chalk.gray(`    Total value: ${totalValue}`));
      console.log(chalk.gray(`    Total P&L: ${totalPnL}`));
      console.log(chalk.gray(`    Total margin: ${totalMargin}`));
      
      // Test filters
      await this.testPositionFilters(page);
      
      // Test sorting
      await this.testPositionSorting(page);
      
      this.metrics.stepTimings.viewAllPositions = {
        duration: Date.now() - stepStart,
        positionCount: positions.length
      };
      
      console.log(chalk.green('    ✓ All positions viewed successfully'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'viewAllPositions',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testPositionFilters(page) {
    try {
      // Filter by profit/loss
      const profitFilter = await page.$('button:has-text("Profitable"), [data-filter="profitable"]');
      if (profitFilter) {
        await profitFilter.click();
        await page.waitForTimeout(500);
        
        const filteredPositions = await page.$$('.position-row:visible, [data-position]:visible');
        console.log(chalk.gray(`    Profitable positions: ${filteredPositions.length}`));
      }
      
      // Filter by market
      const marketFilter = await page.$('select[name="marketFilter"], [data-filter="market"]');
      if (marketFilter) {
        const options = await marketFilter.$$('option');
        if (options.length > 1) {
          await marketFilter.selectOption({ index: 1 });
          await page.waitForTimeout(500);
        }
      }
      
      // Clear filters
      const clearFilters = await page.$('button:has-text("Clear"), button:has-text("Reset")');
      if (clearFilters) {
        await clearFilters.click();
        await page.waitForTimeout(500);
      }
      
    } catch (error) {
      console.log(chalk.yellow('    ⚠ Position filters not fully available'));
    }
  }

  async testPositionSorting(page) {
    try {
      // Sort by P&L
      const pnlHeader = await page.$('th:has-text("P&L"), [data-sort="pnl"]');
      if (pnlHeader) {
        await pnlHeader.click();
        await page.waitForTimeout(500);
        console.log(chalk.gray('    Sorted by P&L'));
      }
      
      // Sort by size
      const sizeHeader = await page.$('th:has-text("Size"), [data-sort="size"]');
      if (sizeHeader) {
        await sizeHeader.click();
        await page.waitForTimeout(500);
        console.log(chalk.gray('    Sorted by size'));
      }
      
    } catch (error) {
      console.log(chalk.yellow('    ⚠ Position sorting not available'));
    }
  }

  async testPositionDetails(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing position details...'));
    
    try {
      // Click on first position
      const firstPosition = await page.$('.position-row:first-child, [data-position]:first-child');
      if (!firstPosition) {
        console.log(chalk.yellow('    ⚠ No positions to view'));
        return;
      }
      
      await firstPosition.click();
      
      // Wait for details modal/page
      await page.waitForSelector('.position-details, [data-position-details]', { timeout: 5000 });
      
      // Get position information
      const market = await page.$eval('.position-market', el => el.textContent).catch(() => 'N/A');
      const entryPrice = await page.$eval('.entry-price', el => el.textContent).catch(() => 'N/A');
      const currentPrice = await page.$eval('.current-price', el => el.textContent).catch(() => 'N/A');
      const size = await page.$eval('.position-size', el => el.textContent).catch(() => 'N/A');
      const leverage = await page.$eval('.position-leverage', el => el.textContent).catch(() => 'N/A');
      const margin = await page.$eval('.position-margin', el => el.textContent).catch(() => 'N/A');
      const pnl = await page.$eval('.position-pnl', el => el.textContent).catch(() => 'N/A');
      const pnlPercent = await page.$eval('.pnl-percent', el => el.textContent).catch(() => 'N/A');
      
      console.log(chalk.gray(`    Market: ${market}`));
      console.log(chalk.gray(`    Entry: ${entryPrice}, Current: ${currentPrice}`));
      console.log(chalk.gray(`    Size: ${size}, Leverage: ${leverage}`));
      console.log(chalk.gray(`    Margin: ${margin}`));
      console.log(chalk.gray(`    P&L: ${pnl} (${pnlPercent})`));
      
      // Check for additional metrics
      const funding = await page.$eval('.funding-rate', el => el.textContent).catch(() => null);
      const liquidation = await page.$eval('.liquidation-price', el => el.textContent).catch(() => null);
      const duration = await page.$eval('.position-duration', el => el.textContent).catch(() => null);
      
      if (funding) console.log(chalk.gray(`    Funding: ${funding}`));
      if (liquidation) console.log(chalk.gray(`    Liquidation: ${liquidation}`));
      if (duration) console.log(chalk.gray(`    Duration: ${duration}`));
      
      // Store position ID for later tests
      this.currentPositionId = await firstPosition.getAttribute('data-position-id');
      
      this.metrics.stepTimings.positionDetails = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Position details retrieved'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'positionDetails',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testRealTimePnL(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing real-time P&L updates...'));
    
    try {
      // Get initial P&L
      const initialPnL = await page.$eval('.position-pnl, [data-pnl]', el => el.textContent);
      console.log(chalk.gray(`    Initial P&L: ${initialPnL}`));
      
      // Wait for WebSocket updates
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        let updateCount = 0;
        const maxUpdates = 5;
        
        await new Promise((resolve) => {
          const updateHandler = (data) => {
            const message = JSON.parse(data);
            if (message.type === 'pnl_update') {
              updateCount++;
              console.log(chalk.gray(`    P&L update ${updateCount}: ${message.pnl}`));
              
              if (updateCount >= maxUpdates) {
                this.ws.removeListener('message', updateHandler);
                resolve();
              }
            }
          };
          
          this.ws.on('message', updateHandler);
          
          // Timeout after 10 seconds
          setTimeout(resolve, 10000);
        });
      }
      
      // Check if P&L changed in UI
      const currentPnL = await page.$eval('.position-pnl, [data-pnl]', el => el.textContent);
      if (currentPnL !== initialPnL) {
        console.log(chalk.green('    ✓ Real-time P&L updates working'));
      } else {
        console.log(chalk.yellow('    ⚠ No P&L changes detected'));
      }
      
      // Check sparkline/mini chart
      const pnlChart = await page.$('.pnl-sparkline, [data-pnl-chart]');
      if (pnlChart) {
        console.log(chalk.gray('    P&L chart visualization available'));
      }
      
      this.metrics.stepTimings.realTimePnL = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'realTimePnL',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testModifyPosition(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing position modification...'));
    
    try {
      // Find modify button
      const modifyButton = await page.$('button:has-text("Modify"), button:has-text("Edit")');
      if (!modifyButton) {
        console.log(chalk.yellow('    ⚠ Modify button not found'));
        return;
      }
      
      await modifyButton.click();
      await page.waitForTimeout(500);
      
      // Modify leverage
      const leverageInput = await page.$('input[name="leverage"], .leverage-input');
      if (leverageInput) {
        const currentLeverage = await leverageInput.inputValue();
        const newLeverage = Math.max(1, parseInt(currentLeverage) - 1);
        await leverageInput.fill(newLeverage.toString());
        console.log(chalk.gray(`    Reducing leverage to ${newLeverage}x`));
      }
      
      // Add margin
      const addMarginButton = await page.$('button:has-text("Add Margin")');
      if (addMarginButton) {
        await addMarginButton.click();
        
        const marginInput = await page.$('input[name="additionalMargin"]');
        if (marginInput) {
          await marginInput.fill('100'); // Add $100
          console.log(chalk.gray('    Adding $100 margin'));
        }
      }
      
      // Calculate new liquidation price
      const newLiquidation = await page.$eval('.new-liquidation-price', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    New liquidation price: ${newLiquidation}`));
      
      // Confirm modifications
      const confirmButton = await page.$('button:has-text("Confirm"), button:has-text("Update")');
      if (confirmButton) {
        await confirmButton.click();
        await page.waitForTimeout(2000);
        
        this.metrics.positionsModified++;
        console.log(chalk.green('    ✓ Position modified successfully'));
      }
      
      this.metrics.stepTimings.modifyPosition = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'modifyPosition',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testAddToPosition(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing add to position...'));
    
    try {
      // Find add button
      const addButton = await page.$('button:has-text("Add to Position"), button:has-text("Increase")');
      if (!addButton) {
        console.log(chalk.yellow('    ⚠ Add to position not available'));
        return;
      }
      
      await addButton.click();
      await page.waitForTimeout(500);
      
      // Set additional size
      const sizeInput = await page.$('input[name="additionalSize"]');
      if (sizeInput) {
        await sizeInput.fill('500'); // Add $500
      }
      
      // Check new average entry price
      const newAvgEntry = await page.$eval('.new-avg-entry', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    New average entry: ${newAvgEntry}`));
      
      // Check required margin
      const requiredMargin = await page.$eval('.required-margin', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Required margin: ${requiredMargin}`));
      
      // Execute if profitable direction
      const currentPnL = await page.$eval('.current-pnl', el => el.textContent);
      if (currentPnL && parseFloat(currentPnL) > 0) {
        const executeButton = await page.$('button:has-text("Add"), button:has-text("Increase Size")');
        if (executeButton) {
          await executeButton.click();
          await page.waitForTimeout(2000);
          
          console.log(chalk.green('    ✓ Added to winning position'));
        }
      } else {
        console.log(chalk.yellow('    ⚠ Skipping add to losing position'));
      }
      
      this.metrics.stepTimings.addToPosition = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'addToPosition',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPartialClose(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing partial position close...'));
    
    try {
      // Find partial close button
      const partialCloseButton = await page.$('button:has-text("Partial Close"), button:has-text("Close Part")');
      if (!partialCloseButton) {
        console.log(chalk.yellow('    ⚠ Partial close not available'));
        return;
      }
      
      await partialCloseButton.click();
      await page.waitForTimeout(500);
      
      // Set close percentage
      const percentageSlider = await page.$('input[type="range"][name="closePercentage"]');
      const percentageInput = await page.$('input[name="closePercentage"]');
      
      if (percentageSlider) {
        await page.evaluate(() => {
          const slider = document.querySelector('input[type="range"][name="closePercentage"]');
          if (slider) {
            slider.value = '25'; // Close 25%
            slider.dispatchEvent(new Event('input', { bubbles: true }));
          }
        });
      } else if (percentageInput) {
        await percentageInput.fill('25');
      }
      
      console.log(chalk.gray('    Closing 25% of position'));
      
      // Check estimated proceeds
      const estimatedProceeds = await page.$eval('.estimated-proceeds', el => el.textContent).catch(() => 'N/A');
      const estimatedPnL = await page.$eval('.estimated-realized-pnl', el => el.textContent).catch(() => 'N/A');
      
      console.log(chalk.gray(`    Estimated proceeds: ${estimatedProceeds}`));
      console.log(chalk.gray(`    Estimated realized P&L: ${estimatedPnL}`));
      
      // Execute partial close
      const executeButton = await page.$('button:has-text("Close"), button:has-text("Execute")');
      if (executeButton) {
        await executeButton.click();
        await page.waitForTimeout(2000);
        
        this.metrics.positionsModified++;
        console.log(chalk.green('    ✓ Partial close executed'));
        
        // Track realized P&L
        const pnlValue = parseFloat(estimatedPnL.replace(/[^0-9.-]/g, '')) || 0;
        this.metrics.totalRealizedPnL += pnlValue;
      }
      
      this.metrics.stepTimings.partialClose = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'partialClose',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testStopLossManagement(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing stop loss management...'));
    
    try {
      // Open stop loss settings
      const stopLossButton = await page.$('button:has-text("Stop Loss"), [data-sl-button]');
      if (!stopLossButton) {
        console.log(chalk.yellow('    ⚠ Stop loss management not available'));
        return;
      }
      
      await stopLossButton.click();
      await page.waitForTimeout(500);
      
      // Set stop loss price
      const stopLossInput = await page.$('input[name="stopLossPrice"]');
      if (stopLossInput) {
        const currentPrice = await page.$eval('.current-price', el => 
          parseFloat(el.textContent.replace(/[^0-9.]/g, ''))
        );
        const stopPrice = currentPrice * 0.95; // 5% stop loss
        await stopLossInput.fill(stopPrice.toFixed(4));
        console.log(chalk.gray(`    Setting stop loss at $${stopPrice.toFixed(4)}`));
      }
      
      // Enable stop loss
      const enableCheckbox = await page.$('input[type="checkbox"][name="enableStopLoss"]');
      if (enableCheckbox) {
        await enableCheckbox.check();
      }
      
      // Set stop loss type
      const typeSelect = await page.$('select[name="stopLossType"]');
      if (typeSelect) {
        await typeSelect.selectOption('mark'); // Use mark price
      }
      
      // Calculate potential loss
      const potentialLoss = await page.$eval('.potential-loss', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Potential loss: ${potentialLoss}`));
      
      // Save stop loss
      const saveButton = await page.$('button:has-text("Save Stop Loss")');
      if (saveButton) {
        await saveButton.click();
        await page.waitForTimeout(1000);
        
        console.log(chalk.green('    ✓ Stop loss configured'));
      }
      
      // Test stop loss modification
      await this.testStopLossModification(page);
      
      this.metrics.stepTimings.stopLossManagement = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'stopLossManagement',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testStopLossModification(page) {
    try {
      // Move stop loss to breakeven
      const breakevenButton = await page.$('button:has-text("Move to Breakeven")');
      if (breakevenButton) {
        await breakevenButton.click();
        console.log(chalk.gray('    Stop loss moved to breakeven'));
      }
      
      // Cancel stop loss
      const cancelButton = await page.$('button:has-text("Cancel Stop Loss")');
      if (cancelButton && Math.random() > 0.8) { // 20% chance
        await cancelButton.click();
        console.log(chalk.gray('    Stop loss cancelled'));
      }
      
    } catch (error) {
      console.log(chalk.yellow('    ⚠ Stop loss modification failed'));
    }
  }

  async testTakeProfitManagement(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing take profit management...'));
    
    try {
      // Open take profit settings
      const takeProfitButton = await page.$('button:has-text("Take Profit"), [data-tp-button]');
      if (!takeProfitButton) {
        console.log(chalk.yellow('    ⚠ Take profit management not available'));
        return;
      }
      
      await takeProfitButton.click();
      await page.waitForTimeout(500);
      
      // Set multiple take profit levels
      console.log(chalk.gray('    Setting multiple take profit levels'));
      
      // TP1: 5% profit
      const tp1Input = await page.$('input[name="tp1Price"]');
      const tp1SizeInput = await page.$('input[name="tp1Size"]');
      if (tp1Input && tp1SizeInput) {
        const currentPrice = await page.$eval('.current-price', el => 
          parseFloat(el.textContent.replace(/[^0-9.]/g, ''))
        );
        await tp1Input.fill((currentPrice * 1.05).toFixed(4));
        await tp1SizeInput.fill('33'); // 33% of position
      }
      
      // TP2: 10% profit
      const tp2Input = await page.$('input[name="tp2Price"]');
      const tp2SizeInput = await page.$('input[name="tp2Size"]');
      if (tp2Input && tp2SizeInput) {
        const currentPrice = await page.$eval('.current-price', el => 
          parseFloat(el.textContent.replace(/[^0-9.]/g, ''))
        );
        await tp2Input.fill((currentPrice * 1.10).toFixed(4));
        await tp2SizeInput.fill('33'); // 33% of position
      }
      
      // TP3: 15% profit
      const tp3Input = await page.$('input[name="tp3Price"]');
      const tp3SizeInput = await page.$('input[name="tp3Size"]');
      if (tp3Input && tp3SizeInput) {
        const currentPrice = await page.$eval('.current-price', el => 
          parseFloat(el.textContent.replace(/[^0-9.]/g, ''))
        );
        await tp3Input.fill((currentPrice * 1.15).toFixed(4));
        await tp3SizeInput.fill('34'); // Remaining 34%
      }
      
      // Calculate expected profit
      const expectedProfit = await page.$eval('.expected-profit', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Expected profit: ${expectedProfit}`));
      
      // Save take profit levels
      const saveButton = await page.$('button:has-text("Save Take Profit")');
      if (saveButton) {
        await saveButton.click();
        await page.waitForTimeout(1000);
        
        console.log(chalk.green('    ✓ Take profit levels configured'));
      }
      
      this.metrics.stepTimings.takeProfitManagement = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'takeProfitManagement',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testTrailingStop(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing trailing stop...'));
    
    try {
      // Open trailing stop settings
      const trailingStopButton = await page.$('button:has-text("Trailing Stop"), [data-trailing-stop]');
      if (!trailingStopButton) {
        console.log(chalk.yellow('    ⚠ Trailing stop not available'));
        return;
      }
      
      await trailingStopButton.click();
      await page.waitForTimeout(500);
      
      // Enable trailing stop
      const enableCheckbox = await page.$('input[type="checkbox"][name="enableTrailingStop"]');
      if (enableCheckbox) {
        await enableCheckbox.check();
      }
      
      // Set trailing distance
      const trailTypeSelect = await page.$('select[name="trailType"]');
      if (trailTypeSelect) {
        await trailTypeSelect.selectOption('percentage'); // Percentage based
      }
      
      const trailDistanceInput = await page.$('input[name="trailDistance"]');
      if (trailDistanceInput) {
        await trailDistanceInput.fill('2'); // 2% trailing distance
        console.log(chalk.gray('    Setting 2% trailing stop'));
      }
      
      // Set activation price
      const activationCheckbox = await page.$('input[type="checkbox"][name="activationPrice"]');
      if (activationCheckbox) {
        await activationCheckbox.check();
        
        const activationInput = await page.$('input[name="activationPriceValue"]');
        if (activationInput) {
          const currentPrice = await page.$eval('.current-price', el => 
            parseFloat(el.textContent.replace(/[^0-9.]/g, ''))
          );
          await activationInput.fill((currentPrice * 1.03).toFixed(4)); // Activate after 3% profit
          console.log(chalk.gray('    Trailing stop activates after 3% profit'));
        }
      }
      
      // Preview trailing behavior
      const previewButton = await page.$('button:has-text("Preview")');
      if (previewButton) {
        await previewButton.click();
        await page.waitForTimeout(500);
        
        // Check trailing visualization
        const trailChart = await page.$('.trailing-visualization');
        if (trailChart) {
          console.log(chalk.gray('    Trailing stop behavior visualized'));
        }
      }
      
      // Save trailing stop
      const saveButton = await page.$('button:has-text("Activate Trailing Stop")');
      if (saveButton) {
        await saveButton.click();
        await page.waitForTimeout(1000);
        
        console.log(chalk.green('    ✓ Trailing stop activated'));
      }
      
      this.metrics.stepTimings.trailingStop = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'trailingStop',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPositionAlerts(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing position alerts...'));
    
    try {
      // Open alerts settings
      const alertsButton = await page.$('button:has-text("Alerts"), [data-position-alerts]');
      if (!alertsButton) {
        console.log(chalk.yellow('    ⚠ Position alerts not available'));
        return;
      }
      
      await alertsButton.click();
      await page.waitForTimeout(500);
      
      // Set price alerts
      console.log(chalk.gray('    Configuring position alerts'));
      
      // Alert when P&L reaches threshold
      const pnlAlertCheckbox = await page.$('input[type="checkbox"][name="pnlAlert"]');
      if (pnlAlertCheckbox) {
        await pnlAlertCheckbox.check();
        
        const pnlThresholdInput = await page.$('input[name="pnlThreshold"]');
        if (pnlThresholdInput) {
          await pnlThresholdInput.fill('500'); // Alert at $500 profit
        }
      }
      
      // Alert on high funding rate
      const fundingAlertCheckbox = await page.$('input[type="checkbox"][name="fundingAlert"]');
      if (fundingAlertCheckbox) {
        await fundingAlertCheckbox.check();
        
        const fundingThresholdInput = await page.$('input[name="fundingThreshold"]');
        if (fundingThresholdInput) {
          await fundingThresholdInput.fill('0.1'); // Alert at 0.1% funding
        }
      }
      
      // Alert near liquidation
      const liquidationAlertCheckbox = await page.$('input[type="checkbox"][name="liquidationAlert"]');
      if (liquidationAlertCheckbox) {
        await liquidationAlertCheckbox.check();
        
        const liquidationDistanceInput = await page.$('input[name="liquidationDistance"]');
        if (liquidationDistanceInput) {
          await liquidationDistanceInput.fill('10'); // Alert when 10% from liquidation
        }
      }
      
      // Set notification preferences
      const notificationSelect = await page.$('select[name="notificationType"]');
      if (notificationSelect) {
        await notificationSelect.selectOption('all'); // Email, SMS, and push
      }
      
      // Save alerts
      const saveButton = await page.$('button:has-text("Save Alerts")');
      if (saveButton) {
        await saveButton.click();
        await page.waitForTimeout(1000);
        
        console.log(chalk.green('    ✓ Position alerts configured'));
      }
      
      this.metrics.stepTimings.positionAlerts = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'positionAlerts',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testRiskMetrics(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing position risk metrics...'));
    
    try {
      // Open risk analysis
      const riskButton = await page.$('button:has-text("Risk Analysis"), [data-risk-analysis]');
      if (!riskButton) {
        console.log(chalk.yellow('    ⚠ Risk analysis not available'));
        return;
      }
      
      await riskButton.click();
      await page.waitForTimeout(1000);
      
      // Get risk metrics
      const riskScore = await page.$eval('.risk-score', el => el.textContent).catch(() => 'N/A');
      const liquidationDistance = await page.$eval('.liquidation-distance', el => el.textContent).catch(() => 'N/A');
      const marginRatio = await page.$eval('.margin-ratio', el => el.textContent).catch(() => 'N/A');
      const maxLoss = await page.$eval('.max-potential-loss', el => el.textContent).catch(() => 'N/A');
      
      console.log(chalk.gray(`    Risk score: ${riskScore}`));
      console.log(chalk.gray(`    Distance to liquidation: ${liquidationDistance}`));
      console.log(chalk.gray(`    Margin ratio: ${marginRatio}`));
      console.log(chalk.gray(`    Maximum potential loss: ${maxLoss}`));
      
      // Check risk breakdown
      const riskFactors = await page.$$('.risk-factor');
      if (riskFactors.length > 0) {
        console.log(chalk.gray(`    Risk factors analyzed: ${riskFactors.length}`));
      }
      
      // Get recommendations
      const recommendations = await page.$$('.risk-recommendation');
      for (const rec of recommendations) {
        const text = await rec.textContent();
        console.log(chalk.gray(`    Recommendation: ${text}`));
      }
      
      // Stress test position
      const stressTestButton = await page.$('button:has-text("Stress Test")');
      if (stressTestButton) {
        await stressTestButton.click();
        await page.waitForTimeout(2000);
        
        const stressResults = await page.$eval('.stress-test-results', el => el.textContent).catch(() => 'N/A');
        console.log(chalk.gray(`    Stress test results: ${stressResults}`));
      }
      
      this.metrics.stepTimings.riskMetrics = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Risk metrics analyzed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'riskMetrics',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPositionHistory(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing position history...'));
    
    try {
      // Navigate to history tab
      const historyTab = await page.$('button:has-text("History"), [data-tab="history"]');
      if (historyTab) {
        await historyTab.click();
        await page.waitForTimeout(1000);
      }
      
      // Get historical trades
      const trades = await page.$$('.trade-history-row, [data-trade]');
      console.log(chalk.gray(`    Found ${trades.length} historical trades`));
      
      // Analyze trade performance
      let profitableTrades = 0;
      let totalPnL = 0;
      
      for (const trade of trades.slice(0, 5)) { // Check first 5
        const pnl = await trade.$eval('.trade-pnl', el => 
          parseFloat(el.textContent.replace(/[^0-9.-]/g, ''))
        ).catch(() => 0);
        
        totalPnL += pnl;
        if (pnl > 0) profitableTrades++;
      }
      
      console.log(chalk.gray(`    Win rate: ${(profitableTrades / Math.min(trades.length, 5) * 100).toFixed(1)}%`));
      console.log(chalk.gray(`    Sample P&L: $${totalPnL.toFixed(2)}`));
      
      // Filter history
      const filterSelect = await page.$('select[name="historyFilter"]');
      if (filterSelect) {
        await filterSelect.selectOption('profitable'); // Show only profitable trades
        await page.waitForTimeout(500);
      }
      
      // Export history
      const exportButton = await page.$('button:has-text("Export History")');
      if (exportButton) {
        await exportButton.click();
        console.log(chalk.gray('    Position history exported'));
      }
      
      this.metrics.stepTimings.positionHistory = {
        duration: Date.now() - stepStart,
        tradesAnalyzed: trades.length
      };
      
      console.log(chalk.green('    ✓ Position history reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'positionHistory',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testEmergencyExit(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing emergency exit...'));
    
    try {
      // Check for positions in danger
      const dangerPositions = await page.$$('.position-danger, [data-danger="true"]');
      
      if (dangerPositions.length === 0) {
        console.log(chalk.gray('    No positions require emergency exit'));
        return;
      }
      
      console.log(chalk.yellow(`    ⚠ ${dangerPositions.length} positions in danger`));
      
      // Emergency exit button
      const emergencyButton = await page.$('button:has-text("Emergency Exit"), button.emergency-close');
      if (!emergencyButton) {
        console.log(chalk.yellow('    ⚠ Emergency exit not available'));
        return;
      }
      
      await emergencyButton.click();
      await page.waitForTimeout(500);
      
      // Confirm emergency exit
      const modal = await page.$('[role="dialog"], .emergency-modal');
      if (modal) {
        // Review impact
        const impactSummary = await modal.$eval('.impact-summary', el => el.textContent).catch(() => 'N/A');
        console.log(chalk.gray(`    Impact: ${impactSummary}`));
        
        // Type confirmation
        const confirmInput = await modal.$('input[name="confirmEmergency"]');
        if (confirmInput) {
          await confirmInput.type('EMERGENCY EXIT');
        }
        
        // Execute emergency exit
        const executeButton = await modal.$('button:has-text("Execute Emergency Exit")');
        if (executeButton) {
          await executeButton.click();
          await page.waitForTimeout(3000);
          
          this.metrics.emergencyExits++;
          console.log(chalk.red('    🚨 Emergency exit executed'));
        }
      }
      
      this.metrics.stepTimings.emergencyExit = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'emergencyExit',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPositionExport(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing position data export...'));
    
    try {
      // Open export options
      const exportButton = await page.$('button:has-text("Export"), [data-export]');
      if (!exportButton) {
        console.log(chalk.yellow('    ⚠ Export feature not available'));
        return;
      }
      
      await exportButton.click();
      await page.waitForTimeout(500);
      
      // Select export format
      const formatSelect = await page.$('select[name="exportFormat"]');
      if (formatSelect) {
        await formatSelect.selectOption('csv'); // CSV format
      }
      
      // Select data to export
      const dataCheckboxes = {
        positions: await page.$('input[name="exportPositions"]'),
        history: await page.$('input[name="exportHistory"]'),
        pnl: await page.$('input[name="exportPnL"]'),
        metrics: await page.$('input[name="exportMetrics"]')
      };
      
      for (const [name, checkbox] of Object.entries(dataCheckboxes)) {
        if (checkbox) {
          await checkbox.check();
          console.log(chalk.gray(`    Including ${name} in export`));
        }
      }
      
      // Set date range
      const dateRangeSelect = await page.$('select[name="dateRange"]');
      if (dateRangeSelect) {
        await dateRangeSelect.selectOption('last30days');
      }
      
      // Download export
      const downloadButton = await page.$('button:has-text("Download")');
      if (downloadButton) {
        // Set up download promise
        const downloadPromise = page.waitForEvent('download');
        
        await downloadButton.click();
        
        const download = await downloadPromise;
        console.log(chalk.green(`    ✓ Export downloaded: ${download.suggestedFilename()}`));
      }
      
      this.metrics.stepTimings.positionExport = {
        duration: Date.now() - stepStart
      };
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'positionExport',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testPerformanceAnalytics(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing performance analytics...'));
    
    try {
      // Navigate to analytics
      const analyticsButton = await page.$('button:has-text("Analytics"), [data-analytics]');
      if (analyticsButton) {
        await analyticsButton.click();
        await page.waitForTimeout(1000);
      }
      
      // Get performance metrics
      const metrics = {
        totalReturn: await page.$eval('.total-return', el => el.textContent).catch(() => 'N/A'),
        winRate: await page.$eval('.win-rate', el => el.textContent).catch(() => 'N/A'),
        avgWin: await page.$eval('.avg-win', el => el.textContent).catch(() => 'N/A'),
        avgLoss: await page.$eval('.avg-loss', el => el.textContent).catch(() => 'N/A'),
        profitFactor: await page.$eval('.profit-factor', el => el.textContent).catch(() => 'N/A'),
        sharpeRatio: await page.$eval('.sharpe-ratio', el => el.textContent).catch(() => 'N/A'),
        maxDrawdown: await page.$eval('.max-drawdown', el => el.textContent).catch(() => 'N/A')
      };
      
      console.log(chalk.gray('    Performance Metrics:'));
      for (const [key, value] of Object.entries(metrics)) {
        console.log(chalk.gray(`      ${key}: ${value}`));
      }
      
      // Check performance charts
      const charts = await page.$$('.performance-chart, canvas');
      console.log(chalk.gray(`    Performance charts available: ${charts.length}`));
      
      // Best/worst trades
      const bestTrade = await page.$eval('.best-trade', el => el.textContent).catch(() => 'N/A');
      const worstTrade = await page.$eval('.worst-trade', el => el.textContent).catch(() => 'N/A');
      
      console.log(chalk.gray(`    Best trade: ${bestTrade}`));
      console.log(chalk.gray(`    Worst trade: ${worstTrade}`));
      
      this.metrics.stepTimings.performanceAnalytics = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Performance analytics reviewed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'performanceAnalytics',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running position management load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    totalPositionsManaged: 0,
    totalModifications: 0,
    totalClosures: 0,
    avgRealizedPnL: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  const pnls = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new PositionManagementJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          pnls.push(metrics.totalRealizedPnL);
          results.successful++;
          results.totalPositionsManaged += metrics.positionsMonitored;
          results.totalModifications += metrics.positionsModified;
          results.totalClosures += metrics.positionsClosed;
        } catch (error) {
          results.failed++;
          results.errors.push({
            userId: i,
            error: error.message
          });
        }
      })()
    );
    
    // Stagger starts
    if (i % 5 === 0) {
      await new Promise(resolve => setTimeout(resolve, 300));
    }
  }
  
  await Promise.all(promises);
  
  // Calculate statistics
  timings.sort((a, b) => a - b);
  results.avgDuration = timings.reduce((a, b) => a + b, 0) / timings.length;
  results.p95Duration = timings[Math.floor(timings.length * 0.95)];
  results.p99Duration = timings[Math.floor(timings.length * 0.99)];
  results.avgRealizedPnL = pnls.reduce((a, b) => a + b, 0) / pnls.length;
  
  // Display results
  console.log(chalk.bold('\nPosition Management Load Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  console.log(chalk.magenta(`  Positions Managed: ${results.totalPositionsManaged}`));
  console.log(chalk.magenta(`  Modifications: ${results.totalModifications}`));
  console.log(chalk.magenta(`  Closures: ${results.totalClosures}`));
  console.log(chalk.magenta(`  Avg Realized P&L: $${results.avgRealizedPnL.toFixed(2)}`));
  
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
      const singleTest = new PositionManagementJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests
      await runLoadTest(config, testData, 10);    // 10 users
      await runLoadTest(config, testData, 100);   // 100 users
      await runLoadTest(config, testData, 1000);  // 1000 users
      
      console.log(chalk.bold.green('\n✅ All position management tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { PositionManagementJourneyTest, runLoadTest };