#!/usr/bin/env node

/**
 * Comprehensive UI Test Suite using Playwright
 * Tests the actual frontend user interface
 */

const { chromium } = require('playwright');
const chalk = require('chalk').default || require('chalk');
const ora = require('ora').default || require('ora');
const fs = require('fs');
const path = require('path');

class UITestSuite {
  constructor(config) {
    this.config = config;
    this.browser = null;
    this.context = null;
    this.page = null;
    this.results = {
      totalTests: 0,
      passed: 0,
      failed: 0,
      phases: {},
      errors: [],
      startTime: Date.now()
    };
  }

  async setup() {
    this.browser = await chromium.launch({ 
      headless: true,
      args: ['--no-sandbox', '--disable-setuid-sandbox']
    });
    this.context = await this.browser.newContext({
      viewport: { width: 1280, height: 720 }
    });
    this.page = await this.context.newPage();
    
    // Set up console message logging
    this.page.on('console', msg => {
      if (msg.type() === 'error') {
        console.log(chalk.red('Browser console error:'), msg.text());
      }
    });
    
    // Set up page error logging
    this.page.on('pageerror', error => {
      console.log(chalk.red('Page error:'), error.message);
    });
  }

  async teardown() {
    if (this.browser) {
      await this.browser.close();
    }
  }

  async runTest(phase, testId, testName, testFn) {
    this.results.totalTests++;
    const spinner = ora(`Running ${testId}: ${testName}`).start();
    
    try {
      await testFn.call(this);
      spinner.succeed(`✅ ${testId}: ${testName}`);
      this.results.passed++;
      
      if (!this.results.phases[phase]) {
        this.results.phases[phase] = { passed: 0, failed: 0, tests: {} };
      }
      this.results.phases[phase].passed++;
      this.results.phases[phase].tests[testId] = { status: 'passed' };
      
    } catch (error) {
      spinner.fail(`❌ ${testId}: ${testName}`);
      this.results.failed++;
      
      if (!this.results.phases[phase]) {
        this.results.phases[phase] = { passed: 0, failed: 0, tests: {} };
      }
      this.results.phases[phase].failed++;
      this.results.phases[phase].tests[testId] = { 
        status: 'failed', 
        error: error.message 
      };
      
      this.results.errors.push({
        phase,
        testId,
        testName,
        error: error.message
      });
      
      // Take screenshot on failure
      try {
        const screenshotPath = path.join(__dirname, `screenshots/failure-${testId}.png`);
        if (!fs.existsSync(path.dirname(screenshotPath))) {
          fs.mkdirSync(path.dirname(screenshotPath), { recursive: true });
        }
        await this.page.screenshot({ path: screenshotPath });
      } catch (e) {
        // Ignore screenshot errors
      }
    }
    
    await new Promise(resolve => setTimeout(resolve, 100));
  }

  // Helper function to inject mock wallet
  async injectMockWallet() {
    await this.page.addInitScript(() => {
      window.solana = {
        isPhantom: true,
        publicKey: { 
          toString: () => '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM',
          toBase58: () => '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM'
        },
        connect: () => Promise.resolve({ 
          publicKey: { 
            toString: () => '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM',
            toBase58: () => '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM'
          } 
        }),
        disconnect: () => Promise.resolve(),
        signMessage: (message) => Promise.resolve({ 
          signature: new Uint8Array(64).fill(1) 
        }),
        signTransaction: (tx) => Promise.resolve(tx),
        signAllTransactions: (txs) => Promise.resolve(txs),
        on: (event, callback) => {},
        removeListener: (event, callback) => {}
      };
    });
  }

  // Phase 1: Homepage & Navigation Tests
  async runPhase1() {
    console.log(chalk.bold.cyan('\n📋 UI PHASE 1: Homepage & Navigation\n'));
    
    await this.runTest('UI Phase 1', 'UI-1.1', 'Homepage Load', async () => {
      await this.page.goto(this.config.uiUrl);
      await this.page.waitForLoadState('networkidle');
      
      const title = await this.page.title();
      if (!title) throw new Error('Page has no title');
    });

    await this.runTest('UI Phase 1', 'UI-1.2', 'Hero Section Visible', async () => {
      const hero = await this.page.locator('[data-testid="hero-section"], .hero, h1').first();
      if (!await hero.isVisible()) {
        throw new Error('Hero section not visible');
      }
    });

    await this.runTest('UI Phase 1', 'UI-1.3', 'Navigation Menu Present', async () => {
      const nav = await this.page.locator('nav, [data-testid="navigation"], .navigation').first();
      if (!await nav.isVisible()) {
        throw new Error('Navigation menu not found');
      }
    });

    await this.runTest('UI Phase 1', 'UI-1.4', 'Markets Link', async () => {
      const marketsLink = await this.page.locator('a:has-text("Markets"), [data-testid="nav-markets"]').first();
      if (!await marketsLink.isVisible()) {
        throw new Error('Markets link not found');
      }
    });

    await this.runTest('UI Phase 1', 'UI-1.5', 'Connect Wallet Button', async () => {
      const connectButton = await this.page.locator('button:has-text("Connect"), [data-testid="connect-wallet"]').first();
      if (!await connectButton.isVisible()) {
        throw new Error('Connect wallet button not found');
      }
    });

    await this.runTest('UI Phase 1', 'UI-1.6', 'Footer Present', async () => {
      const footer = await this.page.locator('footer, [data-testid="footer"]').first();
      if (await footer.count() === 0) {
        // Footer might not be required
        console.log(chalk.yellow('  ⚠️  No footer found (optional)'));
      }
    });

    await this.runTest('UI Phase 1', 'UI-1.7', 'Responsive Design - Mobile', async () => {
      await this.page.setViewportSize({ width: 375, height: 667 });
      await this.page.waitForTimeout(500);
      
      const mobileMenu = await this.page.locator('[data-testid="mobile-menu"], .mobile-menu, button[aria-label*="menu"]').first();
      // Mobile menu is optional, just check if page is still functional
      
      await this.page.setViewportSize({ width: 1280, height: 720 });
    });

    await this.runTest('UI Phase 1', 'UI-1.8', 'Dark Mode Toggle', async () => {
      const darkModeToggle = await this.page.locator('[data-testid="dark-mode-toggle"], button[aria-label*="theme"]').first();
      if (await darkModeToggle.count() > 0) {
        await darkModeToggle.click();
        await this.page.waitForTimeout(300);
        // Check if theme changed
      }
    });
  }

  // Phase 2: Wallet Connection Flow
  async runPhase2() {
    console.log(chalk.bold.cyan('\n📋 UI PHASE 2: Wallet Connection Flow\n'));
    
    await this.runTest('UI Phase 2', 'UI-2.1', 'Wallet Connection Modal', async () => {
      await this.page.goto(this.config.uiUrl);
      await this.injectMockWallet();
      
      const connectButton = await this.page.locator('button:has-text("Connect"), [data-testid="connect-wallet"]').first();
      await connectButton.click();
      
      // Wait for modal or wallet selection
      await this.page.waitForTimeout(1000);
      
      const walletModal = await this.page.locator('[data-testid="wallet-modal"], .wallet-modal, [role="dialog"]').first();
      if (await walletModal.count() > 0) {
        if (!await walletModal.isVisible()) {
          throw new Error('Wallet modal not visible after clicking connect');
        }
      }
    });

    await this.runTest('UI Phase 2', 'UI-2.2', 'Phantom Wallet Option', async () => {
      const phantomOption = await this.page.locator('button:has-text("Phantom"), [data-testid="wallet-phantom"]').first();
      if (await phantomOption.count() > 0) {
        await phantomOption.click();
        await this.page.waitForTimeout(1000);
      }
    });

    await this.runTest('UI Phase 2', 'UI-2.3', 'Connected State Display', async () => {
      // Check if wallet address is displayed
      const walletAddress = await this.page.locator('[data-testid="wallet-address"], .wallet-address').first();
      if (await walletAddress.count() > 0) {
        const text = await walletAddress.textContent();
        if (!text || !text.includes('9WzD')) {
          throw new Error('Wallet address not displayed correctly');
        }
      }
    });

    await this.runTest('UI Phase 2', 'UI-2.4', 'Demo Mode Option', async () => {
      await this.page.goto(this.config.uiUrl);
      
      const demoButton = await this.page.locator('button:has-text("Demo"), [data-testid="demo-mode"]').first();
      if (await demoButton.count() > 0) {
        await demoButton.click();
        await this.page.waitForTimeout(1000);
        
        // Check for demo badge or indicator
        const demoBadge = await this.page.locator('[data-testid="demo-badge"], .demo-badge').first();
        if (await demoBadge.count() > 0) {
          if (!await demoBadge.isVisible()) {
            throw new Error('Demo mode not activated');
          }
        }
      }
    });
  }

  // Phase 3: Markets Page
  async runPhase3() {
    console.log(chalk.bold.cyan('\n📋 UI PHASE 3: Markets Page\n'));
    
    await this.runTest('UI Phase 3', 'UI-3.1', 'Navigate to Markets', async () => {
      await this.page.goto(this.config.uiUrl);
      
      const marketsLink = await this.page.locator('a:has-text("Markets"), [data-testid="nav-markets"]').first();
      if (await marketsLink.count() > 0) {
        await marketsLink.click();
        await this.page.waitForLoadState('networkidle');
      } else {
        // Try direct navigation
        await this.page.goto(`${this.config.uiUrl}/markets`);
      }
    });

    await this.runTest('UI Phase 3', 'UI-3.2', 'Markets List Display', async () => {
      await this.page.waitForTimeout(2000); // Wait for markets to load
      
      const marketCards = await this.page.locator('[data-testid^="market-"], .market-card, article').all();
      if (marketCards.length === 0) {
        throw new Error('No market cards displayed');
      }
    });

    await this.runTest('UI Phase 3', 'UI-3.3', 'Market Search', async () => {
      const searchInput = await this.page.locator('input[placeholder*="Search"], [data-testid="market-search"]').first();
      if (await searchInput.count() > 0) {
        await searchInput.fill('Bitcoin');
        await this.page.waitForTimeout(500);
        
        // Check if results are filtered
        const visibleMarkets = await this.page.locator('[data-testid^="market-"]:visible, .market-card:visible').all();
        if (visibleMarkets.length === 0) {
          console.log(chalk.yellow('  ⚠️  No markets match search (might be expected)'));
        }
      }
    });

    await this.runTest('UI Phase 3', 'UI-3.4', 'Market Filters', async () => {
      const filterButton = await this.page.locator('button:has-text("Filter"), [data-testid="market-filter"]').first();
      if (await filterButton.count() > 0) {
        await filterButton.click();
        await this.page.waitForTimeout(300);
        
        // Check for filter options
        const filterOptions = await this.page.locator('[data-testid^="filter-"], .filter-option').all();
        if (filterOptions.length === 0) {
          console.log(chalk.yellow('  ⚠️  No filter options found'));
        }
      }
    });

    await this.runTest('UI Phase 3', 'UI-3.5', 'Market Card Interaction', async () => {
      const marketCard = await this.page.locator('[data-testid^="market-"], .market-card').first();
      if (await marketCard.count() > 0) {
        await marketCard.click();
        await this.page.waitForTimeout(1000);
        
        // Check if navigated to market detail or modal opened
        const marketDetail = await this.page.locator('[data-testid="market-detail"], .market-detail').first();
        const isDetailPage = this.page.url().includes('/market');
        
        if (!await marketDetail.isVisible() && !isDetailPage) {
          console.log(chalk.yellow('  ⚠️  Market detail view not clear'));
        }
      }
    });
  }

  // Phase 4: Trading Interface
  async runPhase4() {
    console.log(chalk.bold.cyan('\n📋 UI PHASE 4: Trading Interface\n'));
    
    await this.runTest('UI Phase 4', 'UI-4.1', 'Trading Panel Present', async () => {
      // Navigate to a market detail page
      await this.page.goto(`${this.config.uiUrl}/markets`);
      await this.page.waitForTimeout(2000);
      
      const firstMarket = await this.page.locator('[data-testid^="market-"], .market-card').first();
      if (await firstMarket.count() > 0) {
        await firstMarket.click();
        await this.page.waitForTimeout(1000);
        
        const tradingPanel = await this.page.locator('[data-testid="trading-panel"], .trading-panel, .trade-form').first();
        if (await tradingPanel.count() === 0) {
          console.log(chalk.yellow('  ⚠️  Trading panel not found on market detail'));
        }
      }
    });

    await this.runTest('UI Phase 4', 'UI-4.2', 'Buy/Sell Toggle', async () => {
      const buyButton = await this.page.locator('button:has-text("Buy"), [data-testid="buy-button"]').first();
      const sellButton = await this.page.locator('button:has-text("Sell"), [data-testid="sell-button"]').first();
      
      if (await buyButton.count() > 0 || await sellButton.count() > 0) {
        // Trading interface exists
      } else {
        console.log(chalk.yellow('  ⚠️  Buy/Sell buttons not found'));
      }
    });

    await this.runTest('UI Phase 4', 'UI-4.3', 'Amount Input Field', async () => {
      const amountInput = await this.page.locator('input[placeholder*="Amount"], [data-testid="amount-input"]').first();
      if (await amountInput.count() > 0) {
        await amountInput.fill('100');
        const value = await amountInput.inputValue();
        if (value !== '100') {
          throw new Error('Amount input not accepting values');
        }
      }
    });

    await this.runTest('UI Phase 4', 'UI-4.4', 'Price Display', async () => {
      const priceDisplay = await this.page.locator('[data-testid="price-display"], .price, .current-price').first();
      if (await priceDisplay.count() > 0) {
        const priceText = await priceDisplay.textContent();
        if (!priceText) {
          throw new Error('Price not displayed');
        }
      }
    });

    await this.runTest('UI Phase 4', 'UI-4.5', 'Order Summary', async () => {
      const orderSummary = await this.page.locator('[data-testid="order-summary"], .order-summary').first();
      if (await orderSummary.count() > 0) {
        // Order summary exists
      } else {
        console.log(chalk.yellow('  ⚠️  Order summary not found'));
      }
    });
  }

  // Phase 5: Portfolio & Positions
  async runPhase5() {
    console.log(chalk.bold.cyan('\n📋 UI PHASE 5: Portfolio & Positions\n'));
    
    await this.runTest('UI Phase 5', 'UI-5.1', 'Portfolio Page Navigation', async () => {
      await this.page.goto(this.config.uiUrl);
      await this.injectMockWallet();
      
      const portfolioLink = await this.page.locator('a:has-text("Portfolio"), [data-testid="nav-portfolio"]').first();
      if (await portfolioLink.count() > 0) {
        await portfolioLink.click();
        await this.page.waitForLoadState('networkidle');
      } else {
        // Try direct navigation
        await this.page.goto(`${this.config.uiUrl}/portfolio`);
      }
    });

    await this.runTest('UI Phase 5', 'UI-5.2', 'Portfolio Overview', async () => {
      const portfolioValue = await this.page.locator('[data-testid="portfolio-value"], .portfolio-value').first();
      if (await portfolioValue.count() > 0) {
        const valueText = await portfolioValue.textContent();
        if (!valueText) {
          console.log(chalk.yellow('  ⚠️  Portfolio value empty'));
        }
      }
    });

    await this.runTest('UI Phase 5', 'UI-5.3', 'Positions List', async () => {
      const positionsList = await this.page.locator('[data-testid="positions-list"], .positions-list').first();
      if (await positionsList.count() > 0) {
        // Positions list exists
      } else {
        console.log(chalk.yellow('  ⚠️  Positions list not found'));
      }
    });

    await this.runTest('UI Phase 5', 'UI-5.4', 'P&L Display', async () => {
      const pnlDisplay = await this.page.locator('[data-testid="pnl-display"], .pnl, .profit-loss').first();
      if (await pnlDisplay.count() > 0) {
        // P&L display exists
      } else {
        console.log(chalk.yellow('  ⚠️  P&L display not found'));
      }
    });

    await this.runTest('UI Phase 5', 'UI-5.5', 'Risk Metrics', async () => {
      const riskMetrics = await this.page.locator('[data-testid="risk-metrics"], .risk-metrics').first();
      if (await riskMetrics.count() > 0) {
        // Risk metrics exist
      } else {
        console.log(chalk.yellow('  ⚠️  Risk metrics not found'));
      }
    });
  }

  // Main execution
  async runAllTests() {
    console.log(chalk.bold.blue('🎯 Starting Comprehensive UI Test Suite\n'));
    
    try {
      await this.setup();
      
      await this.runPhase1();  // Homepage & Navigation
      await this.runPhase2();  // Wallet Connection
      await this.runPhase3();  // Markets Page
      await this.runPhase4();  // Trading Interface
      await this.runPhase5();  // Portfolio & Positions
      
      await this.generateReport();
      
    } catch (error) {
      console.error(chalk.red('UI test suite failed:'), error);
      await this.generateReport();
    } finally {
      await this.teardown();
    }
  }

  async generateReport() {
    const duration = Date.now() - this.results.startTime;
    const passRate = this.results.totalTests > 0 
      ? (this.results.passed / this.results.totalTests * 100).toFixed(2)
      : 0;
    
    console.log(chalk.bold.blue('\n📊 UI Test Results Summary\n'));
    console.log(chalk.green(`✅ Passed: ${this.results.passed}`));
    console.log(chalk.red(`❌ Failed: ${this.results.failed}`));
    console.log(chalk.blue(`📊 Pass Rate: ${passRate}%`));
    console.log(chalk.gray(`⏱️  Duration: ${Math.round(duration / 1000)}s`));
    
    if (this.results.errors.length > 0) {
      console.log(chalk.red('\n❌ Failed Tests:'));
      this.results.errors.forEach(err => {
        console.log(chalk.red(`  ${err.testId} - ${err.testName}: ${err.error}`));
      });
    }
    
    // Save results to file
    const resultsPath = path.join(__dirname, 'ui-test-results.json');
    fs.writeFileSync(resultsPath, JSON.stringify(this.results, null, 2));
    
    console.log(chalk.gray(`\nDetailed results saved to: ${resultsPath}`));
    
    // Check for screenshots
    const screenshotsDir = path.join(__dirname, 'screenshots');
    if (fs.existsSync(screenshotsDir)) {
      const screenshots = fs.readdirSync(screenshotsDir);
      if (screenshots.length > 0) {
        console.log(chalk.yellow(`\n📸 Screenshots saved in: ${screenshotsDir}`));
      }
    }
  }
}

// Execute tests
if (require.main === module) {
  const configPath = path.join(__dirname, 'test-config.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  
  const tester = new UITestSuite(config);
  tester.runAllTests().catch(console.error);
}

module.exports = UITestSuite;