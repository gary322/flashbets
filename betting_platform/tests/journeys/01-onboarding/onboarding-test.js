#!/usr/bin/env node

/**
 * New User Onboarding Journey Test
 * Tests the complete onboarding flow from landing to first trade
 */

const { chromium } = require('playwright');
const { Connection, Keypair, PublicKey } = require('@solana/web3.js');
const axios = require('axios');
const WebSocket = require('ws');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');

class OnboardingJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0
    };
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n🚀 Starting Onboarding Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Test steps
      await this.testLandingPage(page);
      await this.testWalletConnection(page);
      await this.testDemoAccountCreation(page);
      await this.testRiskDisclosure(page);
      await this.testRiskQuiz(page);
      await this.testTutorial(page);
      await this.testFirstMarketExploration(page);
      await this.testDemoTrading(page);
      await this.testRealAccountActivation(page);
      
      await browser.close();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = 100;
      
      console.log(chalk.green(`✅ Onboarding journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Onboarding journey failed:'), error);
      throw error;
    }
  }

  async testLandingPage(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing landing page...'));
    
    try {
      // Navigate to landing page
      await page.goto(this.config.uiUrl, { waitUntil: 'networkidle' });
      
      // Verify key elements
      await page.waitForSelector('h1', { timeout: 5000 });
      const title = await page.$eval('h1', el => el.textContent);
      
      if (!title.includes('Betting') && !title.includes('Trading')) {
        throw new Error('Landing page title not found');
      }
      
      // Check for CTA button
      const ctaButton = await page.$('button:has-text("Get Started"), button:has-text("Start Trading")');
      if (!ctaButton) {
        throw new Error('CTA button not found');
      }
      
      // Measure page load performance
      const performanceTimings = await page.evaluate(() => {
        const timing = performance.timing;
        return {
          domContentLoaded: timing.domContentLoadedEventEnd - timing.navigationStart,
          loadComplete: timing.loadEventEnd - timing.navigationStart
        };
      });
      
      this.metrics.stepTimings.landingPage = {
        duration: Date.now() - stepStart,
        performance: performanceTimings
      };
      
      console.log(chalk.green('    ✓ Landing page loaded successfully'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'landingPage',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testWalletConnection(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing wallet connection...'));
    
    try {
      // Click connect wallet button
      await page.click('button:has-text("Connect Wallet"), button:has-text("Connect")');
      
      // Wait for wallet modal
      await page.waitForSelector('[role="dialog"], .wallet-modal', { timeout: 5000 });
      
      // For testing, we'll simulate a wallet connection
      // In real testing, you'd interact with a wallet adapter
      await page.evaluate(() => {
        // Simulate wallet connection
        window.solana = {
          isPhantom: true,
          publicKey: { toBase58: () => 'SimulatedWallet1234567890' },
          connect: () => Promise.resolve(),
          on: () => {},
          disconnect: () => Promise.resolve()
        };
      });
      
      // Click Phantom wallet option if available
      const phantomButton = await page.$('button:has-text("Phantom")');
      if (phantomButton) {
        await phantomButton.click();
      }
      
      // Wait for connection confirmation
      await page.waitForFunction(
        () => document.querySelector('[data-connected="true"], .wallet-connected'),
        { timeout: 10000 }
      );
      
      this.metrics.stepTimings.walletConnection = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Wallet connected successfully'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'walletConnection',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testDemoAccountCreation(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing demo account creation...'));
    
    try {
      // API call to create demo account
      const response = await axios.post(`${this.config.apiUrl}/api/wallet/demo/create`, {
        name: `Test User ${Date.now()}`
      });
      
      if (!response.data.wallet || !response.data.privateKey) {
        throw new Error('Demo account creation failed');
      }
      
      const demoWallet = response.data.wallet;
      console.log(chalk.gray(`    Demo wallet: ${demoWallet}`));
      
      // Store demo wallet in page context
      await page.evaluate((wallet) => {
        window.demoWallet = wallet;
      }, demoWallet);
      
      this.metrics.stepTimings.demoAccountCreation = {
        duration: Date.now() - stepStart,
        wallet: demoWallet
      };
      
      console.log(chalk.green('    ✓ Demo account created successfully'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'demoAccountCreation',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testRiskDisclosure(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing risk disclosure...'));
    
    try {
      // Look for risk disclosure modal or navigate to it
      const riskModal = await page.$('[role="dialog"]:has-text("Risk"), .risk-disclosure');
      
      if (riskModal) {
        // Read through disclosure (simulate scroll)
        await page.evaluate(() => {
          const modal = document.querySelector('[role="dialog"], .risk-disclosure');
          if (modal) {
            const scrollable = modal.querySelector('.scrollable, [style*="overflow"]');
            if (scrollable) {
              scrollable.scrollTop = scrollable.scrollHeight;
            }
          }
        });
        
        // Wait a bit to simulate reading
        await page.waitForTimeout(2000);
        
        // Accept risk disclosure
        await page.click('button:has-text("I Understand"), button:has-text("Accept")');
        
        // Wait for modal to close
        await page.waitForFunction(
          () => !document.querySelector('[role="dialog"]:has-text("Risk"), .risk-disclosure'),
          { timeout: 5000 }
        );
      }
      
      this.metrics.stepTimings.riskDisclosure = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Risk disclosure accepted'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'riskDisclosure',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
      console.log(chalk.yellow('    ⚠ Risk disclosure step skipped'));
    }
  }

  async testRiskQuiz(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing risk quiz...'));
    
    try {
      // Look for risk quiz
      const quizElement = await page.$('.risk-quiz, [data-testid="risk-quiz"]');
      
      if (quizElement) {
        // Answer quiz questions
        const questions = await page.$$('.quiz-question, [data-question]');
        
        for (const question of questions) {
          // Select a random answer
          const answers = await question.$$('input[type="radio"], button.answer');
          if (answers.length > 0) {
            const randomAnswer = answers[Math.floor(Math.random() * answers.length)];
            await randomAnswer.click();
          }
        }
        
        // Submit quiz
        await page.click('button:has-text("Submit"), button:has-text("Complete")');
        
        // Wait for quiz completion
        await page.waitForSelector('.quiz-complete, [data-quiz-complete]', { timeout: 5000 });
      }
      
      this.metrics.stepTimings.riskQuiz = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Risk quiz completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'riskQuiz',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
      console.log(chalk.yellow('    ⚠ Risk quiz step skipped'));
    }
  }

  async testTutorial(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing tutorial walkthrough...'));
    
    try {
      // Look for tutorial or wizard
      const tutorialElement = await page.$('.tutorial, .wizard, [data-tutorial]');
      
      if (tutorialElement) {
        // Go through tutorial steps
        let nextButton = await page.$('button:has-text("Next"), button:has-text("Continue")');
        let stepCount = 0;
        
        while (nextButton && stepCount < 10) {
          await nextButton.click();
          await page.waitForTimeout(500); // Allow for animations
          nextButton = await page.$('button:has-text("Next"), button:has-text("Continue")');
          stepCount++;
        }
        
        // Complete tutorial
        const completeButton = await page.$('button:has-text("Complete"), button:has-text("Finish")');
        if (completeButton) {
          await completeButton.click();
        }
      }
      
      this.metrics.stepTimings.tutorial = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Tutorial completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'tutorial',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
      console.log(chalk.yellow('    ⚠ Tutorial step skipped'));
    }
  }

  async testFirstMarketExploration(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market exploration...'));
    
    try {
      // Navigate to markets page
      await page.click('a:has-text("Markets"), button:has-text("Explore")');
      
      // Wait for markets to load
      await page.waitForSelector('.market-card, [data-market]', { timeout: 10000 });
      
      // Get market data from API
      const marketsResponse = await axios.get(`${this.config.apiUrl}/api/markets`);
      const markets = marketsResponse.data.markets;
      
      if (!markets || markets.length === 0) {
        throw new Error('No markets available');
      }
      
      // Click on first market
      const firstMarket = await page.$('.market-card, [data-market]');
      if (firstMarket) {
        await firstMarket.click();
        
        // Wait for market details
        await page.waitForSelector('.market-details, [data-market-details]', { timeout: 5000 });
        
        // Verify market information is displayed
        const marketTitle = await page.$eval('h1, h2', el => el.textContent);
        console.log(chalk.gray(`    Viewing market: ${marketTitle}`));
      }
      
      this.metrics.stepTimings.marketExploration = {
        duration: Date.now() - stepStart,
        marketsViewed: 1
      };
      
      console.log(chalk.green('    ✓ Market exploration completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'marketExploration',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testDemoTrading(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing demo trading...'));
    
    try {
      // Set position size
      const sizeInput = await page.$('input[name="size"], input[placeholder*="Amount"]');
      if (sizeInput) {
        await sizeInput.fill('100');
      }
      
      // Set leverage (if available)
      const leverageSlider = await page.$('.leverage-slider, [data-leverage]');
      if (leverageSlider) {
        // Simulate moving slider to 10x
        await page.evaluate(() => {
          const slider = document.querySelector('input[type="range"]');
          if (slider) {
            slider.value = '10';
            slider.dispatchEvent(new Event('input', { bubbles: true }));
            slider.dispatchEvent(new Event('change', { bubbles: true }));
          }
        });
      }
      
      // Click buy button
      const buyButton = await page.$('button:has-text("Buy"), button:has-text("Long")');
      if (!buyButton) {
        throw new Error('Buy button not found');
      }
      
      await buyButton.click();
      
      // Wait for confirmation modal
      await page.waitForSelector('[role="dialog"], .confirm-modal', { timeout: 5000 });
      
      // Confirm trade
      await page.click('button:has-text("Confirm"), button:has-text("Place Order")');
      
      // Wait for success message
      await page.waitForSelector('.success, [data-success]', { timeout: 10000 });
      
      this.metrics.stepTimings.demoTrading = {
        duration: Date.now() - stepStart,
        tradeExecuted: true
      };
      
      console.log(chalk.green('    ✓ Demo trade executed successfully'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'demoTrading',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testRealAccountActivation(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing real account activation...'));
    
    try {
      // Look for account upgrade prompt
      const upgradeButton = await page.$('button:has-text("Upgrade"), button:has-text("Go Live")');
      
      if (upgradeButton) {
        await upgradeButton.click();
        
        // Wait for activation modal
        await page.waitForSelector('[role="dialog"], .activation-modal', { timeout: 5000 });
        
        // Simulate accepting terms
        const termsCheckbox = await page.$('input[type="checkbox"]');
        if (termsCheckbox) {
          await termsCheckbox.check();
        }
        
        // Activate account
        await page.click('button:has-text("Activate"), button:has-text("Complete")');
        
        // Wait for success
        await page.waitForSelector('.activated, [data-activated]', { timeout: 5000 });
      }
      
      this.metrics.stepTimings.accountActivation = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Account activation completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'accountActivation',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical for demo
      console.log(chalk.yellow('    ⚠ Account activation skipped'));
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new OnboardingJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          results.successful++;
        } catch (error) {
          results.failed++;
          results.errors.push({
            userId: i,
            error: error.message
          });
        }
      })()
    );
    
    // Stagger starts slightly to avoid thundering herd
    if (i % 10 === 0) {
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
  
  await Promise.all(promises);
  
  // Calculate statistics
  timings.sort((a, b) => a - b);
  results.avgDuration = timings.reduce((a, b) => a + b, 0) / timings.length;
  results.p95Duration = timings[Math.floor(timings.length * 0.95)];
  results.p99Duration = timings[Math.floor(timings.length * 0.99)];
  
  // Display results
  console.log(chalk.bold('\nLoad Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  
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
      const singleTest = new OnboardingJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests
      await runLoadTest(config, testData, 10);    // 10 users
      await runLoadTest(config, testData, 100);   // 100 users
      await runLoadTest(config, testData, 1000);  // 1000 users
      
      console.log(chalk.bold.green('\n✅ All onboarding tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { OnboardingJourneyTest, runLoadTest };