import { test, expect, Page } from '@playwright/test';

// Test data
const TEST_MARKET_1 = {
  title: "Will global temperature rise >1.5°C by 2030?",
  yesPrice: 73,
  noPrice: 27,
};

const TEST_MARKET_2 = {
  title: "Will SpaceX land on Mars before 2030?",
  yesPrice: 41,
  noPrice: 59,
};

// Helper functions
async function connectDemoWallet(page: Page) {
  await page.click('button:has-text("Connect Wallet")');
  await page.waitForSelector('.wallet-options');
  await page.click('button:has-text("Demo Wallet")');
  await page.waitForSelector('.wallet-address');
  await expect(page.locator('.balance-amount')).toContainText('$');
}

async function navigateToMarkets(page: Page) {
  await page.click('button:has-text("Markets")');
  await page.waitForSelector('.markets-grid');
}

async function placeBasicTrade(page: Page, marketTitle: string, side: 'yes' | 'no', amount: number) {
  // Find and click on market
  await page.click(`text=${marketTitle}`);
  await page.waitForSelector('#marketDetailContent');
  
  // Click bet button
  await page.click(`button:has-text("BET ${side.toUpperCase()}")`);
  await page.waitForSelector('#tradeForm');
  
  // Enter trade details
  await page.fill('#tradeAmount', amount.toString());
  await page.click('button:has-text("Execute Trade")');
  
  // Wait for success
  await page.waitForSelector('#successModal');
  await page.click('button:has-text("View Position")');
}

// Test Suite 1: New User Onboarding Journey
test.describe('New User Onboarding Journey', () => {
  test('should complete full onboarding flow', async ({ page }) => {
    // Step 1: Land on homepage
    await page.goto('/');
    await expect(page).toHaveTitle(/Quantum Betting/);
    
    // Step 2: View features
    await page.click('a[href="#features"]');
    await expect(page.locator('#features')).toBeVisible();
    
    // Step 3: Connect demo wallet
    await connectDemoWallet(page);
    
    // Step 4: Navigate to markets
    await navigateToMarkets(page);
    
    // Step 5: Place first trade
    await placeBasicTrade(page, TEST_MARKET_1.title, 'yes', 100);
    
    // Step 6: Check portfolio
    await page.click('button:has-text("Portfolio")');
    await expect(page.locator('.positions-table tbody tr')).toHaveCount(1);
  });
});

// Test Suite 2: Complete Trading Lifecycle
test.describe('Complete Trading Lifecycle', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await connectDemoWallet(page);
  });

  test('should execute full trading lifecycle', async ({ page }) => {
    // Step 1: Analyze market
    await navigateToMarkets(page);
    const marketCard = page.locator('.market-card').first();
    const volume = await marketCard.locator('.volume-badge').textContent();
    expect(volume).toContain('$');
    
    // Step 2: Open position
    await placeBasicTrade(page, TEST_MARKET_1.title, 'yes', 500);
    
    // Step 3: Monitor position
    await page.waitForTimeout(3000); // Wait for price updates
    const pnlElement = page.locator('.positions-table tbody tr').first().locator('td:nth-child(6)');
    await expect(pnlElement).toBeVisible();
    
    // Step 4: Add to position
    await navigateToMarkets(page);
    await placeBasicTrade(page, TEST_MARKET_1.title, 'yes', 200);
    
    // Step 5: Close position
    await page.click('button:has-text("Portfolio")');
    await page.click('button:has-text("Manage")');
    await page.click('button:has-text("Close Position")');
    
    // Verify closed
    await expect(page.locator('.positions-table tbody')).toContainText('No active positions');
  });
});

// Test Suite 3: Leveraged Trading Journey
test.describe('Leveraged Trading Journey', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await connectDemoWallet(page);
  });

  test('should manage leveraged positions', async ({ page }) => {
    await navigateToMarkets(page);
    
    // Open high leverage position
    await page.click(`text=${TEST_MARKET_2.title}`);
    await page.click('button:has-text("BET NO")');
    
    // Set leverage
    await page.fill('#tradeAmount', '100');
    await page.click('button[data-leverage="50"]');
    
    // Check preview
    const maxWin = await page.locator('#previewMaxWin').textContent();
    expect(maxWin).toContain('+$');
    
    // Execute trade
    await page.click('button:has-text("Execute Trade")');
    await page.waitForSelector('#successModal');
    
    // Monitor liquidation price
    await page.click('button:has-text("View Position")');
    const liquidationPrice = await page.locator('td:contains("Liquidation")').textContent();
    expect(liquidationPrice).toBeTruthy();
    
    // Add collateral
    await page.click('button:has-text("Add Collateral")');
    await page.fill('#collateralAmount', '50');
    await page.click('button:has-text("Add")');
    
    // Verify improved health
    await expect(page.locator('.position-health')).toContainText('Healthy');
  });
});

// Test Suite 4: Quantum Betting Journey
test.describe('Quantum Betting Journey', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await connectDemoWallet(page);
  });

  test('should create and manage quantum positions', async ({ page }) => {
    // Navigate to quantum tab
    await page.click('button:has-text("Quantum")');
    await page.waitForSelector('.quantum-features');
    
    // Create superposition
    await page.click('button:has-text("Create Superposition")');
    await page.selectOption('#marketSelect', TEST_MARKET_1.title);
    await page.fill('#superpositionAmount', '1000');
    
    // Set amplitudes
    await page.fill('#amplitudeYes', '0.7071'); // 50%
    await page.fill('#amplitudeNo', '0.7071');   // 50%
    
    await page.click('button:has-text("Create Quantum Position")');
    
    // Verify creation
    await expect(page.locator('.quantum-positions')).toContainText('Superposition Active');
    
    // Create entanglement
    await page.click('button:has-text("Entangle Positions")');
    await page.selectOption('#entangleMarket', TEST_MARKET_2.title);
    await page.click('button:has-text("Create Entanglement")');
    
    // Monitor coherence
    await page.waitForTimeout(5000);
    const coherence = await page.locator('.coherence-level').textContent();
    expect(coherence).toContain('%');
    
    // Collapse wavefunction
    await page.click('button:has-text("Observe")');
    await expect(page.locator('.quantum-state')).toContainText('Collapsed');
  });
});

// Test Suite 5: Verse Navigation Journey
test.describe('Verse Navigation Journey', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await connectDemoWallet(page);
  });

  test('should navigate through verse hierarchy', async ({ page }) => {
    // Go to verses
    await page.click('button:has-text("Verses")');
    await page.waitForSelector('.verse-tree');
    
    // Start at root
    const rootVerse = page.locator('.verse-node[data-level="0"]');
    await expect(rootVerse).toContainText('Root Universe');
    
    // Navigate to Sports verse
    await page.click('.verse-node:has-text("Sports")');
    await expect(page.locator('.verse-info')).toContainText('1.5x multiplier');
    
    // Open position in Sports verse
    await page.click('button:has-text("Trade in Verse")');
    await page.selectOption('#verseMarketSelect', 'NFL Game');
    await page.fill('#tradeAmount', '100');
    await page.click('button:has-text("Place Trade")');
    
    // Navigate deeper to NFL verse
    await page.click('.verse-node:has-text("NFL")');
    await expect(page.locator('.verse-info')).toContainText('3x cumulative');
    
    // Test auto-chain
    await page.click('button:has-text("Auto-Chain")');
    await page.fill('#chainDeposit', '200');
    await page.click('button:has-text("Execute Chain")');
    
    // Verify chain execution
    await expect(page.locator('.chain-results')).toContainText('Chain Executed');
  });
});

// Test Suite 6: DeFi Integration Journey
test.describe('DeFi Integration Journey', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await connectDemoWallet(page);
  });

  test('should interact with DeFi features', async ({ page }) => {
    // Navigate to DeFi
    await page.click('button:has-text("DeFi")');
    await page.waitForSelector('.defi-grid');
    
    // Stake MMT
    await page.click('button:has-text("Stake MMT")');
    await page.fill('#stakeAmount', '5000');
    await page.click('button:has-text("Stake")');
    
    // Verify staking
    await expect(page.locator('.staking-info')).toContainText('5,000 MMT');
    await expect(page.locator('.apy-display')).toContainText('18.7%');
    
    // Add liquidity
    await page.click('button:has-text("Add Liquidity")');
    await page.selectOption('#poolSelect', 'MMT-SOL LP');
    await page.fill('#liquidityAmount', '1000');
    await page.click('button:has-text("Add")');
    
    // Check rewards
    await page.waitForTimeout(3000);
    await expect(page.locator('.rewards-earned')).not.toContainText('0.00');
  });
});

// Test Suite 7: Error Recovery Journey
test.describe('Error Recovery Journey', () => {
  test('should handle errors gracefully', async ({ page }) => {
    await page.goto('/');
    
    // Test network error
    await page.route('**/api/markets', route => route.abort());
    await page.click('button:has-text("Markets")');
    await expect(page.locator('.error-message')).toContainText('Failed to load markets');
    
    // Test recovery
    await page.unroute('**/api/markets');
    await page.click('button:has-text("Retry")');
    await expect(page.locator('.markets-grid')).toBeVisible();
    
    // Test invalid trade
    await connectDemoWallet(page);
    await navigateToMarkets(page);
    await page.click('.market-card').first();
    await page.click('button:has-text("BET YES")');
    await page.fill('#tradeAmount', '999999'); // Exceeds balance
    await page.click('button:has-text("Execute Trade")');
    
    await expect(page.locator('.error-notification')).toContainText('Insufficient balance');
  });
});

// Test Suite 8: Mobile User Journey
test.describe('Mobile User Journey', () => {
  test.use({ 
    viewport: { width: 375, height: 667 },
    hasTouch: true,
  });

  test('should work on mobile devices', async ({ page }) => {
    await page.goto('/');
    
    // Test mobile menu
    await page.click('.mobile-menu-button');
    await expect(page.locator('.mobile-menu')).toBeVisible();
    
    // Connect wallet on mobile
    await page.click('button:has-text("Connect")');
    await page.click('button:has-text("Demo Wallet")');
    
    // Navigate markets on mobile
    await page.click('button:has-text("Markets")');
    await expect(page.locator('.markets-grid')).toHaveCSS('grid-template-columns', '1fr');
    
    // Place trade on mobile
    await page.click('.market-card').first();
    await page.click('button:has-text("BET YES")');
    await page.fill('#tradeAmount', '50');
    
    // Test swipe gestures
    await page.locator('.leverage-selector').swipe('left');
    await expect(page.locator('.leverage-btn[data-leverage="10"]')).toHaveClass(/active/);
  });
});

// Test Suite 9: Performance Testing
test.describe('Performance Testing', () => {
  test('should handle high-frequency updates', async ({ page }) => {
    await page.goto('/');
    await connectDemoWallet(page);
    
    // Monitor WebSocket messages
    let messageCount = 0;
    page.on('websocket', ws => {
      ws.on('framereceived', () => messageCount++);
    });
    
    await navigateToMarkets(page);
    
    // Wait for updates
    await page.waitForTimeout(10000);
    
    // Verify updates received
    expect(messageCount).toBeGreaterThan(10);
    
    // Test UI responsiveness during updates
    const start = Date.now();
    await page.click('.market-card').first();
    const loadTime = Date.now() - start;
    
    expect(loadTime).toBeLessThan(1000); // Should load in under 1 second
  });
});

// Test Suite 10: Accessibility Testing
test.describe('Accessibility Testing', () => {
  test('should be keyboard navigable', async ({ page }) => {
    await page.goto('/');
    
    // Tab navigation
    await page.keyboard.press('Tab');
    await expect(page.locator(':focus')).toBeVisible();
    
    // Enter to activate
    await page.keyboard.press('Tab'); // Navigate to "Launch App"
    await page.keyboard.press('Tab'); // Navigate to "Connect Wallet"
    await page.keyboard.press('Enter');
    
    await expect(page.locator('#walletModal')).toBeVisible();
    
    // Escape to close
    await page.keyboard.press('Escape');
    await expect(page.locator('#walletModal')).not.toBeVisible();
    
    // Screen reader labels
    const connectButton = page.locator('button:has-text("Connect Wallet")');
    await expect(connectButton).toHaveAttribute('aria-label', /connect.*wallet/i);
  });
});