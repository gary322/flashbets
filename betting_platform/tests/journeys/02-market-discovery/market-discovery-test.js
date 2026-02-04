#!/usr/bin/env node

/**
 * Market Discovery and Analysis Journey Test
 * Tests the complete market exploration flow
 */

const { chromium } = require('playwright');
const axios = require('axios');
const WebSocket = require('ws');
const chalk = require('chalk');
const fs = require('fs');
const path = require('path');

class MarketDiscoveryJourneyTest {
  constructor(config, testData) {
    this.config = config;
    this.testData = testData;
    this.metrics = {
      stepTimings: {},
      errors: [],
      successRate: 0,
      totalTime: 0,
      marketsAnalyzed: 0,
      filtersApplied: 0,
      searchesPerformed: 0
    };
  }

  async runTest(userId = 0) {
    console.log(chalk.blue(`\n🔍 Starting Market Discovery Journey Test for User ${userId}`));
    const startTime = Date.now();
    
    try {
      const browser = await chromium.launch({ headless: true });
      const context = await browser.newContext();
      const page = await context.newPage();
      
      // Select a test wallet
      const wallet = this.testData.wallets[userId % this.testData.wallets.length];
      
      // Test steps
      await this.testBrowseAllMarkets(page);
      await this.testFilterByCategory(page);
      await this.testFilterByVerse(page);
      await this.testSearchSpecificMarkets(page);
      await this.testViewMarketDetails(page);
      await this.testAnalyzePriceHistory(page);
      await this.testCheckLiquidityDepth(page);
      await this.testReviewMarketOutcomes(page);
      await this.testStudyOrderBooks(page);
      await this.testCheckMarketExpiry(page);
      await this.testCompareMultipleMarkets(page);
      await this.testWatchlistManagement(page);
      await this.testMarketAlerts(page);
      
      await browser.close();
      
      this.metrics.totalTime = Date.now() - startTime;
      this.metrics.successRate = 100;
      
      console.log(chalk.green(`✅ Market discovery journey completed in ${this.metrics.totalTime}ms`));
      return this.metrics;
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'overall',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      this.metrics.successRate = 0;
      console.error(chalk.red('❌ Market discovery journey failed:'), error);
      throw error;
    }
  }

  async testBrowseAllMarkets(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing browse all markets...'));
    
    try {
      // Navigate to markets page
      await page.goto(`${this.config.uiUrl}/markets`, { waitUntil: 'networkidle' });
      
      // Wait for markets to load
      await page.waitForSelector('.market-grid, .market-list', { timeout: 10000 });
      
      // Get all markets from API
      const response = await axios.get(`${this.config.apiUrl}/api/markets`);
      const markets = response.data.markets;
      
      console.log(chalk.gray(`    Found ${markets.length} markets`));
      
      // Verify pagination if needed
      const marketCards = await page.$$('.market-card, [data-market]');
      if (markets.length > 20 && marketCards.length <= 20) {
        // Test pagination
        const nextButton = await page.$('button:has-text("Next"), [aria-label="Next page"]');
        if (nextButton) {
          await nextButton.click();
          await page.waitForTimeout(1000);
        }
      }
      
      // Test infinite scroll if implemented
      const hasInfiniteScroll = await page.evaluate(() => {
        return window.innerHeight + window.scrollY < document.body.offsetHeight;
      });
      
      if (hasInfiniteScroll) {
        await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
        await page.waitForTimeout(1000);
      }
      
      this.metrics.stepTimings.browseAllMarkets = {
        duration: Date.now() - stepStart,
        marketsLoaded: markets.length
      };
      
      console.log(chalk.green('    ✓ Browse all markets completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'browseAllMarkets',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testFilterByCategory(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing filter by category...'));
    
    try {
      // Look for category filter
      const categoryFilter = await page.$('.category-filter, [data-filter="category"]');
      if (!categoryFilter) {
        throw new Error('Category filter not found');
      }
      
      // Get available categories
      const categories = ['Politics', 'Sports', 'Crypto', 'Finance', 'Technology'];
      
      for (const category of categories.slice(0, 3)) { // Test first 3
        // Apply category filter
        const categoryButton = await page.$(`button:has-text("${category}"), [data-category="${category}"]`);
        if (categoryButton) {
          await categoryButton.click();
          await page.waitForTimeout(500);
          
          // Verify filtered results
          const marketCards = await page.$$('.market-card, [data-market]');
          console.log(chalk.gray(`    ${category}: ${marketCards.length} markets`));
          
          this.metrics.filtersApplied++;
        }
      }
      
      // Clear filters
      const clearButton = await page.$('button:has-text("Clear"), button:has-text("Reset")');
      if (clearButton) {
        await clearButton.click();
        await page.waitForTimeout(500);
      }
      
      this.metrics.stepTimings.filterByCategory = {
        duration: Date.now() - stepStart,
        categoriesTested: 3
      };
      
      console.log(chalk.green('    ✓ Filter by category completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'filterByCategory',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testFilterByVerse(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing filter by verse...'));
    
    try {
      // Look for verse filter
      const verseFilter = await page.$('.verse-filter, [data-filter="verse"]');
      if (!verseFilter) {
        console.log(chalk.yellow('    ⚠ Verse filter not found, skipping'));
        return;
      }
      
      // Click verse filter dropdown
      await verseFilter.click();
      await page.waitForTimeout(500);
      
      // Select first verse
      const firstVerse = await page.$('.verse-option:first-child, [data-verse-option]:first-child');
      if (firstVerse) {
        const verseName = await firstVerse.textContent();
        await firstVerse.click();
        await page.waitForTimeout(1000);
        
        // Verify filtered results
        const marketCards = await page.$$('.market-card, [data-market]');
        console.log(chalk.gray(`    Verse "${verseName}": ${marketCards.length} markets`));
        
        this.metrics.filtersApplied++;
      }
      
      this.metrics.stepTimings.filterByVerse = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Filter by verse completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'filterByVerse',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testSearchSpecificMarkets(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market search...'));
    
    try {
      // Find search input
      const searchInput = await page.$('input[type="search"], input[placeholder*="Search"]');
      if (!searchInput) {
        throw new Error('Search input not found');
      }
      
      // Test various search queries
      const searchQueries = [
        'election',
        'BTC',
        'Will',
        '2024',
        'Super Bowl'
      ];
      
      for (const query of searchQueries.slice(0, 3)) {
        // Clear and type search query
        await searchInput.fill('');
        await searchInput.type(query, { delay: 50 });
        
        // Wait for search results
        await page.waitForTimeout(1000);
        
        // Check results
        const resultCount = await page.$$eval('.market-card, [data-market]', cards => cards.length);
        console.log(chalk.gray(`    Search "${query}": ${resultCount} results`));
        
        this.metrics.searchesPerformed++;
      }
      
      // Clear search
      await searchInput.fill('');
      
      this.metrics.stepTimings.searchMarkets = {
        duration: Date.now() - stepStart,
        searchesPerformed: this.metrics.searchesPerformed
      };
      
      console.log(chalk.green('    ✓ Market search completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'searchMarkets',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testViewMarketDetails(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing view market details...'));
    
    try {
      // Click on first market
      const firstMarket = await page.$('.market-card:first-child, [data-market]:first-child');
      if (!firstMarket) {
        throw new Error('No markets found');
      }
      
      await firstMarket.click();
      
      // Wait for market details page
      await page.waitForSelector('.market-details, [data-market-details]', { timeout: 5000 });
      
      // Verify key elements are present
      const elements = {
        title: await page.$('h1, .market-title'),
        description: await page.$('.market-description, [data-description]'),
        outcomes: await page.$('.outcomes, .market-outcomes'),
        price: await page.$('.current-price, [data-price]'),
        volume: await page.$('.volume, [data-volume]'),
        liquidity: await page.$('.liquidity, [data-liquidity]')
      };
      
      for (const [name, element] of Object.entries(elements)) {
        if (!element) {
          console.log(chalk.yellow(`    ⚠ ${name} element not found`));
        }
      }
      
      // Get market ID for further tests
      this.currentMarketId = await page.evaluate(() => {
        return window.location.pathname.split('/').pop();
      });
      
      this.metrics.stepTimings.viewMarketDetails = {
        duration: Date.now() - stepStart,
        elementsFound: Object.values(elements).filter(e => e).length
      };
      
      this.metrics.marketsAnalyzed++;
      console.log(chalk.green('    ✓ View market details completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'viewMarketDetails',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      throw error;
    }
  }

  async testAnalyzePriceHistory(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing price history analysis...'));
    
    try {
      // Look for price chart
      const priceChart = await page.$('.price-chart, canvas, [data-chart]');
      if (!priceChart) {
        console.log(chalk.yellow('    ⚠ Price chart not found'));
        return;
      }
      
      // Test time range selectors
      const timeRanges = ['1H', '1D', '1W', '1M', 'ALL'];
      for (const range of timeRanges) {
        const rangeButton = await page.$(`button:has-text("${range}"), [data-range="${range}"]`);
        if (rangeButton) {
          await rangeButton.click();
          await page.waitForTimeout(500);
        }
      }
      
      // Get price history data from API
      if (this.currentMarketId) {
        const response = await axios.get(`${this.config.apiUrl}/api/markets/${this.currentMarketId}/history`);
        const history = response.data.history || [];
        console.log(chalk.gray(`    Price history: ${history.length} data points`));
      }
      
      // Test chart interactions
      await page.hover('.price-chart, canvas');
      await page.waitForTimeout(500);
      
      this.metrics.stepTimings.analyzePriceHistory = {
        duration: Date.now() - stepStart,
        timeRangesTested: timeRanges.length
      };
      
      console.log(chalk.green('    ✓ Price history analysis completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'analyzePriceHistory',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCheckLiquidityDepth(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing liquidity depth...'));
    
    try {
      // Look for liquidity section
      const liquiditySection = await page.$('.liquidity-depth, [data-liquidity-depth]');
      if (!liquiditySection) {
        console.log(chalk.yellow('    ⚠ Liquidity depth section not found'));
        return;
      }
      
      // Check for depth chart
      const depthChart = await page.$('.depth-chart, [data-depth-chart]');
      if (depthChart) {
        // Hover over depth chart to see tooltips
        await page.hover('.depth-chart, [data-depth-chart]');
        await page.waitForTimeout(500);
      }
      
      // Check liquidity values
      const liquidityValue = await page.$eval('.liquidity-value, [data-liquidity-value]', el => el.textContent);
      console.log(chalk.gray(`    Total liquidity: ${liquidityValue}`));
      
      // Check slippage calculator if available
      const slippageCalc = await page.$('.slippage-calculator, [data-slippage]');
      if (slippageCalc) {
        const amountInput = await page.$('input[name="amount"], input[placeholder*="Amount"]');
        if (amountInput) {
          await amountInput.fill('1000');
          await page.waitForTimeout(500);
          
          const slippage = await page.$eval('.slippage-result, [data-slippage-result]', el => el.textContent);
          console.log(chalk.gray(`    Slippage for 1000: ${slippage}`));
        }
      }
      
      this.metrics.stepTimings.checkLiquidityDepth = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Liquidity depth check completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'checkLiquidityDepth',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testReviewMarketOutcomes(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market outcomes review...'));
    
    try {
      // Find outcomes section
      const outcomesSection = await page.$('.outcomes-section, [data-outcomes]');
      if (!outcomesSection) {
        console.log(chalk.yellow('    ⚠ Outcomes section not found'));
        return;
      }
      
      // Get all outcome options
      const outcomeElements = await page.$$('.outcome, [data-outcome]');
      console.log(chalk.gray(`    Found ${outcomeElements.length} outcomes`));
      
      for (let i = 0; i < Math.min(outcomeElements.length, 3); i++) {
        const outcome = outcomeElements[i];
        const outcomeName = await outcome.$eval('.outcome-name, [data-outcome-name]', el => el.textContent);
        const outcomePrice = await outcome.$eval('.outcome-price, [data-outcome-price]', el => el.textContent);
        
        console.log(chalk.gray(`    Outcome ${i + 1}: ${outcomeName} - ${outcomePrice}`));
        
        // Click outcome for more details
        await outcome.click();
        await page.waitForTimeout(500);
      }
      
      // Check outcome probabilities
      const probabilities = await page.$$eval('.outcome-probability, [data-probability]', els => 
        els.map(el => el.textContent)
      );
      
      if (probabilities.length > 0) {
        console.log(chalk.gray(`    Probabilities: ${probabilities.join(', ')}`));
      }
      
      this.metrics.stepTimings.reviewOutcomes = {
        duration: Date.now() - stepStart,
        outcomesReviewed: outcomeElements.length
      };
      
      console.log(chalk.green('    ✓ Market outcomes review completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'reviewOutcomes',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testStudyOrderBooks(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing order book analysis...'));
    
    try {
      // Navigate to order book tab
      const orderBookTab = await page.$('button:has-text("Order Book"), [data-tab="orderbook"]');
      if (orderBookTab) {
        await orderBookTab.click();
        await page.waitForTimeout(1000);
      }
      
      // Look for order book
      const orderBook = await page.$('.order-book, [data-orderbook]');
      if (!orderBook) {
        console.log(chalk.yellow('    ⚠ Order book not found'));
        return;
      }
      
      // Analyze buy orders
      const buyOrders = await page.$$('.buy-order, [data-buy-order]');
      console.log(chalk.gray(`    Buy orders: ${buyOrders.length}`));
      
      // Analyze sell orders
      const sellOrders = await page.$$('.sell-order, [data-sell-order]');
      console.log(chalk.gray(`    Sell orders: ${sellOrders.length}`));
      
      // Check spread
      const spread = await page.$eval('.spread, [data-spread]', el => el.textContent).catch(() => 'N/A');
      console.log(chalk.gray(`    Spread: ${spread}`));
      
      // Test order book depth visualization
      const depthViz = await page.$('.order-book-depth, [data-orderbook-depth]');
      if (depthViz) {
        await page.hover('.order-book-depth, [data-orderbook-depth]');
        await page.waitForTimeout(500);
      }
      
      this.metrics.stepTimings.studyOrderBooks = {
        duration: Date.now() - stepStart,
        ordersAnalyzed: buyOrders.length + sellOrders.length
      };
      
      console.log(chalk.green('    ✓ Order book analysis completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'studyOrderBooks',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCheckMarketExpiry(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market expiry check...'));
    
    try {
      // Find expiry information
      const expiryElement = await page.$('.market-expiry, [data-expiry]');
      if (!expiryElement) {
        console.log(chalk.yellow('    ⚠ Expiry information not found'));
        return;
      }
      
      const expiryText = await expiryElement.textContent();
      console.log(chalk.gray(`    Market expiry: ${expiryText}`));
      
      // Check for countdown timer
      const countdown = await page.$('.countdown, [data-countdown]');
      if (countdown) {
        const initialTime = await countdown.textContent();
        await page.waitForTimeout(2000);
        const updatedTime = await countdown.textContent();
        
        if (initialTime !== updatedTime) {
          console.log(chalk.gray('    ✓ Countdown timer is active'));
        }
      }
      
      // Check for settlement rules
      const settlementRules = await page.$('.settlement-rules, [data-settlement]');
      if (settlementRules) {
        const rulesText = await settlementRules.textContent();
        console.log(chalk.gray(`    Settlement rules found: ${rulesText.substring(0, 50)}...`));
      }
      
      this.metrics.stepTimings.checkMarketExpiry = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Market expiry check completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'checkMarketExpiry',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testCompareMultipleMarkets(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market comparison...'));
    
    try {
      // Go back to markets list
      await page.goto(`${this.config.uiUrl}/markets`, { waitUntil: 'networkidle' });
      
      // Look for compare feature
      const compareButtons = await page.$$('.compare-button, [data-compare]');
      if (compareButtons.length === 0) {
        console.log(chalk.yellow('    ⚠ Compare feature not found'));
        return;
      }
      
      // Select 2-3 markets to compare
      const marketsToCompare = Math.min(3, compareButtons.length);
      for (let i = 0; i < marketsToCompare; i++) {
        await compareButtons[i].click();
        await page.waitForTimeout(500);
      }
      
      // Look for compare view button
      const compareViewButton = await page.$('button:has-text("Compare"), [data-compare-view]');
      if (compareViewButton) {
        await compareViewButton.click();
        await page.waitForTimeout(1000);
        
        // Verify comparison view
        const comparisonTable = await page.$('.comparison-table, [data-comparison]');
        if (comparisonTable) {
          console.log(chalk.gray(`    Comparing ${marketsToCompare} markets`));
          
          // Check comparison metrics
          const metrics = await page.$$('.comparison-metric, [data-metric]');
          console.log(chalk.gray(`    Comparison metrics: ${metrics.length}`));
        }
      }
      
      this.metrics.stepTimings.compareMarkets = {
        duration: Date.now() - stepStart,
        marketsCompared: marketsToCompare
      };
      
      console.log(chalk.green('    ✓ Market comparison completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'compareMarkets',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testWatchlistManagement(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing watchlist management...'));
    
    try {
      // Go to a specific market
      const marketCard = await page.$('.market-card:first-child, [data-market]:first-child');
      if (marketCard) {
        await marketCard.click();
        await page.waitForTimeout(1000);
      }
      
      // Look for watchlist button
      const watchlistButton = await page.$('button:has-text("Watch"), button:has-text("Add to Watchlist"), [data-watchlist]');
      if (!watchlistButton) {
        console.log(chalk.yellow('    ⚠ Watchlist feature not found'));
        return;
      }
      
      // Add to watchlist
      await watchlistButton.click();
      await page.waitForTimeout(500);
      
      // Verify added
      const buttonText = await watchlistButton.textContent();
      if (buttonText.includes('Watching') || buttonText.includes('Remove')) {
        console.log(chalk.gray('    ✓ Market added to watchlist'));
      }
      
      // Navigate to watchlist
      const watchlistLink = await page.$('a:has-text("Watchlist"), [href*="watchlist"]');
      if (watchlistLink) {
        await watchlistLink.click();
        await page.waitForTimeout(1000);
        
        const watchedMarkets = await page.$$('.watched-market, [data-watched]');
        console.log(chalk.gray(`    Watchlist contains ${watchedMarkets.length} markets`));
      }
      
      this.metrics.stepTimings.watchlistManagement = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Watchlist management completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'watchlistManagement',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }

  async testMarketAlerts(page) {
    const stepStart = Date.now();
    console.log(chalk.cyan('  Testing market alerts...'));
    
    try {
      // Look for alerts button
      const alertsButton = await page.$('button:has-text("Set Alert"), button:has-text("Alerts"), [data-alerts]');
      if (!alertsButton) {
        console.log(chalk.yellow('    ⚠ Alerts feature not found'));
        return;
      }
      
      await alertsButton.click();
      await page.waitForTimeout(500);
      
      // Look for alert modal
      const alertModal = await page.$('[role="dialog"], .alert-modal');
      if (alertModal) {
        // Set price alert
        const priceAlertInput = await page.$('input[name="price"], input[placeholder*="Price"]');
        if (priceAlertInput) {
          await priceAlertInput.fill('0.75');
        }
        
        // Select alert type
        const alertTypeSelect = await page.$('select[name="alertType"], [data-alert-type]');
        if (alertTypeSelect) {
          await alertTypeSelect.selectOption('price_above');
        }
        
        // Save alert
        const saveButton = await page.$('button:has-text("Save"), button:has-text("Create Alert")');
        if (saveButton) {
          await saveButton.click();
          await page.waitForTimeout(1000);
          
          console.log(chalk.gray('    ✓ Price alert created'));
        }
      }
      
      this.metrics.stepTimings.marketAlerts = {
        duration: Date.now() - stepStart
      };
      
      console.log(chalk.green('    ✓ Market alerts completed'));
      
    } catch (error) {
      this.metrics.errors.push({
        step: 'marketAlerts',
        error: error.message,
        timestamp: new Date().toISOString()
      });
      // Non-critical, continue
    }
  }
}

// Load testing function
async function runLoadTest(config, testData, concurrentUsers) {
  console.log(chalk.bold.yellow(`\n🔥 Running market discovery load test with ${concurrentUsers} concurrent users`));
  
  const results = {
    totalUsers: concurrentUsers,
    successful: 0,
    failed: 0,
    avgDuration: 0,
    p95Duration: 0,
    p99Duration: 0,
    totalMarketsAnalyzed: 0,
    totalSearches: 0,
    errors: []
  };
  
  const promises = [];
  const timings = [];
  
  for (let i = 0; i < concurrentUsers; i++) {
    promises.push(
      (async () => {
        try {
          const test = new MarketDiscoveryJourneyTest(config, testData);
          const metrics = await test.runTest(i);
          timings.push(metrics.totalTime);
          results.successful++;
          results.totalMarketsAnalyzed += metrics.marketsAnalyzed;
          results.totalSearches += metrics.searchesPerformed;
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
  console.log(chalk.bold('\nMarket Discovery Load Test Results:'));
  console.log(chalk.green(`  Successful: ${results.successful}`));
  console.log(chalk.red(`  Failed: ${results.failed}`));
  console.log(chalk.blue(`  Success Rate: ${(results.successful / results.totalUsers * 100).toFixed(2)}%`));
  console.log(chalk.cyan(`  Avg Duration: ${results.avgDuration.toFixed(2)}ms`));
  console.log(chalk.cyan(`  P95 Duration: ${results.p95Duration}ms`));
  console.log(chalk.cyan(`  P99 Duration: ${results.p99Duration}ms`));
  console.log(chalk.magenta(`  Total Markets Analyzed: ${results.totalMarketsAnalyzed}`));
  console.log(chalk.magenta(`  Total Searches: ${results.totalSearches}`));
  
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
      const singleTest = new MarketDiscoveryJourneyTest(config, testData);
      await singleTest.runTest();
      
      // Load tests
      await runLoadTest(config, testData, 10);    // 10 users
      await runLoadTest(config, testData, 100);   // 100 users
      await runLoadTest(config, testData, 1000);  // 1000 users
      
      console.log(chalk.bold.green('\n✅ All market discovery tests completed!'));
      
    } catch (error) {
      console.error(chalk.red('Test failed:'), error);
      process.exit(1);
    }
  })();
}

module.exports = { MarketDiscoveryJourneyTest, runLoadTest };