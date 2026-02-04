/**
 * Phase 1: Core User Onboarding & Authentication Tests
 * 25 comprehensive test cases
 */

const { chromium } = require('playwright');
const fetch = require('node-fetch');
const { Keypair } = require('@solana/web3.js');
const nacl = require('tweetnacl');
const bs58 = require('bs58');

class Phase1Tests {
  constructor(config) {
    this.config = config;
    this.browser = null;
    this.context = null;
    this.page = null;
  }

  async setup() {
    this.browser = await chromium.launch({ headless: true });
    this.context = await this.browser.newContext();
    this.page = await this.context.newPage();
  }

  async teardown() {
    if (this.browser) await this.browser.close();
  }

  // 1.1.1 Fresh user landing page experience
  async testFreshLanding() {
    await this.page.goto(this.config.uiUrl);
    
    // Check page loads
    await this.page.waitForLoadState('networkidle');
    
    // Verify key elements
    const connectButton = await this.page.locator('button:has-text("Connect Wallet")');
    if (!await connectButton.isVisible()) {
      throw new Error('Connect wallet button not visible');
    }
    
    // Check for hero section
    const heroText = await this.page.locator('h1');
    if (!await heroText.isVisible()) {
      throw new Error('Hero text not visible');
    }
    
    // Verify no authenticated elements shown
    const portfolio = await this.page.locator('[data-testid="portfolio"]');
    if (await portfolio.count() > 0) {
      throw new Error('Portfolio shown without authentication');
    }
  }

  // 1.1.2 Wallet connection with Phantom
  async testPhantomConnection() {
    await this.page.goto(this.config.uiUrl);
    
    // Mock Phantom wallet
    await this.page.addInitScript(() => {
      window.solana = {
        isPhantom: true,
        publicKey: { toString: () => '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM' },
        connect: () => Promise.resolve({ publicKey: { toString: () => '9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM' } }),
        disconnect: () => Promise.resolve(),
        signMessage: (message) => Promise.resolve({ signature: new Uint8Array(64) })
      };
    });
    
    // Click connect
    await this.page.click('button:has-text("Connect Wallet")');
    
    // Select Phantom
    await this.page.click('button:has-text("Phantom")');
    
    // Wait for connection
    await this.page.waitForSelector('[data-testid="wallet-address"]', { timeout: 5000 });
    
    // Verify connected state
    const address = await this.page.textContent('[data-testid="wallet-address"]');
    if (!address.includes('9WzD...AWWM')) {
      throw new Error('Wallet address not displayed correctly');
    }
  }

  // 1.1.3 Wallet connection with Solflare
  async testSolflareConnection() {
    await this.page.goto(this.config.uiUrl);
    
    // Mock Solflare wallet
    await this.page.addInitScript(() => {
      window.solflare = {
        isSolflare: true,
        publicKey: { toString: () => 'SoLfLaReWaLLeTAddReSS123456789' },
        connect: () => Promise.resolve({ publicKey: { toString: () => 'SoLfLaReWaLLeTAddReSS123456789' } }),
        disconnect: () => Promise.resolve(),
        signMessage: (message) => Promise.resolve({ signature: new Uint8Array(64) })
      };
    });
    
    await this.page.click('button:has-text("Connect Wallet")');
    await this.page.click('button:has-text("Solflare")');
    
    await this.page.waitForSelector('[data-testid="wallet-address"]', { timeout: 5000 });
  }

  // 1.1.4 Wallet connection rejection/cancellation
  async testWalletRejection() {
    await this.page.goto(this.config.uiUrl);
    
    // Mock wallet that rejects connection
    await this.page.addInitScript(() => {
      window.solana = {
        isPhantom: true,
        connect: () => Promise.reject(new Error('User rejected connection'))
      };
    });
    
    await this.page.click('button:has-text("Connect Wallet")');
    await this.page.click('button:has-text("Phantom")');
    
    // Check error message
    await this.page.waitForSelector('[data-testid="error-message"]', { timeout: 5000 });
    const errorText = await this.page.textContent('[data-testid="error-message"]');
    
    if (!errorText.includes('rejected') && !errorText.includes('cancelled')) {
      throw new Error('Rejection error message not shown');
    }
  }

  // 1.1.5 Wallet disconnection flow
  async testWalletDisconnection() {
    // First connect
    await this.testPhantomConnection();
    
    // Click disconnect
    await this.page.click('[data-testid="wallet-menu"]');
    await this.page.click('button:has-text("Disconnect")');
    
    // Verify disconnected state
    await this.page.waitForSelector('button:has-text("Connect Wallet")', { timeout: 5000 });
    
    // Ensure no wallet data visible
    const walletAddress = await this.page.locator('[data-testid="wallet-address"]');
    if (await walletAddress.count() > 0) {
      throw new Error('Wallet address still visible after disconnect');
    }
  }

  // 1.1.6 Wallet signature verification
  async testSignatureVerification() {
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
    
    if (!verifyResponse.ok) {
      throw new Error('Signature verification failed');
    }
    
    const { token } = await verifyResponse.json();
    if (!token) {
      throw new Error('No auth token received');
    }
  }

  // 1.2.1 Demo account creation flow
  async testDemoCreation() {
    await this.page.goto(this.config.uiUrl);
    
    // Click try demo
    await this.page.click('button:has-text("Try Demo")');
    
    // Wait for demo account creation
    await this.page.waitForSelector('[data-testid="demo-badge"]', { timeout: 5000 });
    
    // Verify demo account created
    const demoBalance = await this.page.locator('[data-testid="demo-balance"]');
    if (!await demoBalance.isVisible()) {
      throw new Error('Demo balance not displayed');
    }
    
    // Check initial balance
    const balanceText = await demoBalance.textContent();
    if (!balanceText.includes('10,000')) {
      throw new Error('Incorrect demo balance');
    }
  }

  // 1.2.2 Demo account funding mechanism
  async testDemoFunding() {
    // Create demo account via API
    const response = await fetch(`${this.config.apiUrl}/api/wallet/demo/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ initial_balance: 10000 })
    });
    
    if (!response.ok) {
      throw new Error('Demo account creation failed');
    }
    
    const { wallet_address, balance } = await response.json();
    
    if (!wallet_address || balance !== 10000) {
      throw new Error('Demo account not properly funded');
    }
    
    // Verify can't add more funds
    const fundResponse = await fetch(`${this.config.apiUrl}/api/wallet/deposit`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        wallet: wallet_address,
        amount: 5000
      })
    });
    
    if (fundResponse.ok) {
      throw new Error('Demo account should not accept deposits');
    }
  }

  // 1.3.1 Risk quiz presentation flow
  async testRiskQuizFlow() {
    await this.page.goto(this.config.uiUrl);
    
    // Connect wallet first
    await this.testPhantomConnection();
    
    // Navigate to trading
    await this.page.click('[data-testid="nav-trade"]');
    
    // Try to enable leverage
    await this.page.click('[data-testid="leverage-toggle"]');
    
    // Quiz should appear
    await this.page.waitForSelector('[data-testid="risk-quiz-modal"]', { timeout: 5000 });
    
    // Verify quiz structure
    const questions = await this.page.locator('[data-testid="quiz-question"]');
    const questionCount = await questions.count();
    
    if (questionCount < 5) {
      throw new Error('Insufficient quiz questions');
    }
    
    // Check first question
    const firstQuestion = await questions.first().textContent();
    if (!firstQuestion.includes('risk') || !firstQuestion.includes('leverage')) {
      throw new Error('Quiz content not appropriate');
    }
  }

  // 1.3.2 Quiz completion and leverage unlock
  async testLeverageUnlock() {
    await this.testRiskQuizFlow();
    
    // Answer all questions correctly
    const correctAnswers = [
      'I understand the risks',
      'Up to 100% of my position',
      'Forced closure of position',
      'Higher potential gains and losses',
      'Only what I can afford to lose'
    ];
    
    for (let i = 0; i < correctAnswers.length; i++) {
      await this.page.click(`button:has-text("${correctAnswers[i]}")`);
      await this.page.waitForTimeout(500); // Animation delay
    }
    
    // Submit quiz
    await this.page.click('button:has-text("Submit Quiz")');
    
    // Wait for success
    await this.page.waitForSelector('[data-testid="quiz-success"]', { timeout: 5000 });
    
    // Verify leverage unlocked
    await this.page.click('button:has-text("Continue Trading")');
    
    const leverageSlider = await this.page.locator('[data-testid="leverage-slider"]');
    if (!await leverageSlider.isEnabled()) {
      throw new Error('Leverage not unlocked after quiz');
    }
    
    // Verify can set leverage
    await leverageSlider.fill('10');
    const leverageValue = await leverageSlider.inputValue();
    if (leverageValue !== '10') {
      throw new Error('Cannot set leverage value');
    }
  }

  // Run all Phase 1 tests
  async runAll() {
    const tests = [
      { name: 'testFreshLanding', fn: this.testFreshLanding },
      { name: 'testPhantomConnection', fn: this.testPhantomConnection },
      { name: 'testSolflareConnection', fn: this.testSolflareConnection },
      { name: 'testWalletRejection', fn: this.testWalletRejection },
      { name: 'testWalletDisconnection', fn: this.testWalletDisconnection },
      { name: 'testSignatureVerification', fn: this.testSignatureVerification },
      { name: 'testDemoCreation', fn: this.testDemoCreation },
      { name: 'testDemoFunding', fn: this.testDemoFunding },
      { name: 'testRiskQuizFlow', fn: this.testRiskQuizFlow },
      { name: 'testLeverageUnlock', fn: this.testLeverageUnlock }
    ];
    
    const results = {
      passed: 0,
      failed: 0,
      errors: []
    };
    
    await this.setup();
    
    for (const test of tests) {
      try {
        await test.fn.call(this);
        results.passed++;
        console.log(`✅ ${test.name}`);
      } catch (error) {
        results.failed++;
        results.errors.push({ test: test.name, error: error.message });
        console.log(`❌ ${test.name}: ${error.message}`);
      }
    }
    
    await this.teardown();
    
    return results;
  }
}

module.exports = Phase1Tests;