#!/usr/bin/env node

/**
 * API-based test suite for comprehensive testing
 */

const fetch = require('node-fetch');
const { Keypair } = require('@solana/web3.js');
const nacl = require('tweetnacl');
const bs58 = require('bs58').default || require('bs58');
const chalk = require('chalk').default || require('chalk');
const ora = require('ora').default || require('ora');

class APITestSuite {
  constructor(config) {
    this.config = config;
    this.results = {
      passed: 0,
      failed: 0,
      tests: {},
      errors: []
    };
  }

  async runTest(testName, testFn) {
    const spinner = ora(`Running ${testName}`).start();
    
    try {
      await testFn.call(this);
      spinner.succeed(chalk.green(`✅ ${testName}`));
      this.results.passed++;
      this.results.tests[testName] = { status: 'passed' };
    } catch (error) {
      spinner.fail(chalk.red(`❌ ${testName}: ${error.message}`));
      this.results.failed++;
      this.results.tests[testName] = { 
        status: 'failed', 
        error: error.message 
      };
      this.results.errors.push({
        test: testName,
        error: error.message
      });
    }
  }

  // Phase 1: Authentication & Onboarding Tests
  async testWalletChallenge() {
    const keypair = Keypair.generate();
    const publicKey = keypair.publicKey.toBase58();
    
    const response = await fetch(`${this.config.apiUrl}/api/wallet/challenge/${publicKey}`);
    if (!response.ok) throw new Error(`Challenge failed: ${response.status}`);
    
    const { challenge } = await response.json();
    if (!challenge) throw new Error('No challenge received');
  }

  async testWalletVerification() {
    const keypair = Keypair.generate();
    const publicKey = keypair.publicKey.toBase58();
    
    // Get challenge
    const challengeResponse = await fetch(`${this.config.apiUrl}/api/wallet/challenge/${publicKey}`);
    const { challenge } = await challengeResponse.json();
    
    // Sign challenge
    const messageBytes = new TextEncoder().encode(challenge);
    const signature = nacl.sign.detached(messageBytes, keypair.secretKey);
    const signatureBase58 = bs58.encode(signature);
    
    // Verify signature
    const verifyResponse = await fetch(`${this.config.apiUrl}/api/wallet/verify`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        wallet: publicKey,
        signature: signatureBase58,
        challenge
      })
    });
    
    if (!verifyResponse.ok) throw new Error(`Verification failed: ${verifyResponse.status}`);
    
    const { token } = await verifyResponse.json();
    if (!token) throw new Error('No auth token received');
  }

  async testDemoAccountCreation() {
    const response = await fetch(`${this.config.apiUrl}/api/wallet/demo`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ initial_balance: 10000 })
    });
    
    if (!response.ok) throw new Error(`Demo creation failed: ${response.status}`);
    
    const data = await response.json();
    if (!data.wallet || !data.balance) throw new Error('Invalid demo account response');
  }

  // Phase 2: Market Tests
  async testMarketsList() {
    const response = await fetch(`${this.config.apiUrl}/api/markets`);
    if (!response.ok) throw new Error(`Markets fetch failed: ${response.status}`);
    
    const markets = await response.json();
    if (!Array.isArray(markets)) throw new Error('Markets response not an array');
  }

  async testMarketDetails() {
    // First get markets
    const marketsResponse = await fetch(`${this.config.apiUrl}/api/markets`);
    const markets = await marketsResponse.json();
    
    if (markets.length === 0) throw new Error('No markets available');
    
    // Test first market detail
    const marketId = markets[0].market_pubkey || markets[0].id;
    const response = await fetch(`${this.config.apiUrl}/api/markets/${marketId}`);
    
    if (!response.ok) throw new Error(`Market detail failed: ${response.status}`);
    
    const market = await response.json();
    if (!market.market_pubkey && !market.id) throw new Error('Invalid market detail response');
  }

  async testMarketSearch() {
    const response = await fetch(`${this.config.apiUrl}/api/markets?search=BTC`);
    if (!response.ok) throw new Error(`Market search failed: ${response.status}`);
    
    const markets = await response.json();
    if (!Array.isArray(markets)) throw new Error('Search response not an array');
  }

  async testVersesList() {
    const response = await fetch(`${this.config.apiUrl}/api/verses`);
    if (!response.ok) throw new Error(`Verses fetch failed: ${response.status}`);
    
    const verses = await response.json();
    if (!Array.isArray(verses)) throw new Error('Verses response not an array');
  }

  // Phase 3: Trading Tests (simplified for API testing)
  async testOrderPlacement() {
    // This would require authenticated wallet - simplified for now
    const orderData = {
      market_id: 'test-market',
      side: 'buy',
      outcome: 0,
      amount: 100,
      price: 0.5
    };
    
    // We expect this to fail without auth, but we're testing the endpoint exists
    const response = await fetch(`${this.config.apiUrl}/api/orders`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(orderData)
    });
    
    // For now, we just check if endpoint exists (401 is expected without auth)
    if (response.status !== 401 && response.status !== 400) {
      throw new Error(`Unexpected response: ${response.status}`);
    }
  }

  async testPositionsList() {
    // Test with demo wallet
    const demoResponse = await fetch(`${this.config.apiUrl}/api/wallet/demo`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ initial_balance: 10000 })
    });
    
    const { wallet } = await demoResponse.json();
    
    const response = await fetch(`${this.config.apiUrl}/api/positions/${wallet}`);
    if (!response.ok && response.status !== 404) {
      throw new Error(`Positions fetch failed: ${response.status}`);
    }
  }

  // Run comprehensive API tests
  async runAllTests() {
    console.log(chalk.bold.blue('\n🧪 Running Comprehensive API Test Suite\n'));
    
    // Phase 1: Authentication
    console.log(chalk.cyan('\n📋 Phase 1: Authentication & Onboarding\n'));
    await this.runTest('Wallet Challenge Generation', this.testWalletChallenge);
    await this.runTest('Wallet Signature Verification', this.testWalletVerification);
    await this.runTest('Demo Account Creation', this.testDemoAccountCreation);
    
    // Phase 2: Markets
    console.log(chalk.cyan('\n📋 Phase 2: Market Discovery\n'));
    await this.runTest('Markets List', this.testMarketsList);
    await this.runTest('Market Details', this.testMarketDetails);
    await this.runTest('Market Search', this.testMarketSearch);
    await this.runTest('Verses List', this.testVersesList);
    
    // Phase 3: Trading
    console.log(chalk.cyan('\n📋 Phase 3: Trading Operations\n'));
    await this.runTest('Order Placement Endpoint', this.testOrderPlacement);
    await this.runTest('Positions List', this.testPositionsList);
    
    // Summary
    console.log(chalk.bold.blue('\n📊 Test Summary\n'));
    console.log(chalk.green(`✅ Passed: ${this.results.passed}`));
    console.log(chalk.red(`❌ Failed: ${this.results.failed}`));
    
    if (this.results.errors.length > 0) {
      console.log(chalk.red('\n❌ Failed Tests:'));
      this.results.errors.forEach(err => {
        console.log(chalk.red(`  - ${err.test}: ${err.error}`));
      });
    }
    
    return this.results;
  }
}

// Run tests
if (require.main === module) {
  const fs = require('fs');
  const path = require('path');
  
  const configPath = path.join(__dirname, 'test-config.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  
  const tester = new APITestSuite(config);
  tester.runAllTests()
    .then(results => {
      fs.writeFileSync(
        path.join(__dirname, 'api-test-results.json'),
        JSON.stringify(results, null, 2)
      );
      process.exit(results.failed > 0 ? 1 : 0);
    })
    .catch(console.error);
}

module.exports = APITestSuite;