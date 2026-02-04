"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
const test_1 = require("@playwright/test");
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
function connectDemoWallet(page) {
    return __awaiter(this, void 0, void 0, function* () {
        yield page.click('button:has-text("Connect Wallet")');
        yield page.waitForSelector('.wallet-options');
        yield page.click('button:has-text("Demo Wallet")');
        yield page.waitForSelector('.wallet-address');
        yield (0, test_1.expect)(page.locator('.balance-amount')).toContainText('$');
    });
}
function navigateToMarkets(page) {
    return __awaiter(this, void 0, void 0, function* () {
        yield page.click('button:has-text("Markets")');
        yield page.waitForSelector('.markets-grid');
    });
}
function placeBasicTrade(page, marketTitle, side, amount) {
    return __awaiter(this, void 0, void 0, function* () {
        // Find and click on market
        yield page.click(`text=${marketTitle}`);
        yield page.waitForSelector('#marketDetailContent');
        // Click bet button
        yield page.click(`button:has-text("BET ${side.toUpperCase()}")`);
        yield page.waitForSelector('#tradeForm');
        // Enter trade details
        yield page.fill('#tradeAmount', amount.toString());
        yield page.click('button:has-text("Execute Trade")');
        // Wait for success
        yield page.waitForSelector('#successModal');
        yield page.click('button:has-text("View Position")');
    });
}
// Test Suite 1: New User Onboarding Journey
test_1.test.describe('New User Onboarding Journey', () => {
    (0, test_1.test)('should complete full onboarding flow', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        // Step 1: Land on homepage
        yield page.goto('/');
        yield (0, test_1.expect)(page).toHaveTitle(/Quantum Betting/);
        // Step 2: View features
        yield page.click('a[href="#features"]');
        yield (0, test_1.expect)(page.locator('#features')).toBeVisible();
        // Step 3: Connect demo wallet
        yield connectDemoWallet(page);
        // Step 4: Navigate to markets
        yield navigateToMarkets(page);
        // Step 5: Place first trade
        yield placeBasicTrade(page, TEST_MARKET_1.title, 'yes', 100);
        // Step 6: Check portfolio
        yield page.click('button:has-text("Portfolio")');
        yield (0, test_1.expect)(page.locator('.positions-table tbody tr')).toHaveCount(1);
    }));
});
// Test Suite 2: Complete Trading Lifecycle
test_1.test.describe('Complete Trading Lifecycle', () => {
    test_1.test.beforeEach((_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        yield connectDemoWallet(page);
    }));
    (0, test_1.test)('should execute full trading lifecycle', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        // Step 1: Analyze market
        yield navigateToMarkets(page);
        const marketCard = page.locator('.market-card').first();
        const volume = yield marketCard.locator('.volume-badge').textContent();
        (0, test_1.expect)(volume).toContain('$');
        // Step 2: Open position
        yield placeBasicTrade(page, TEST_MARKET_1.title, 'yes', 500);
        // Step 3: Monitor position
        yield page.waitForTimeout(3000); // Wait for price updates
        const pnlElement = page.locator('.positions-table tbody tr').first().locator('td:nth-child(6)');
        yield (0, test_1.expect)(pnlElement).toBeVisible();
        // Step 4: Add to position
        yield navigateToMarkets(page);
        yield placeBasicTrade(page, TEST_MARKET_1.title, 'yes', 200);
        // Step 5: Close position
        yield page.click('button:has-text("Portfolio")');
        yield page.click('button:has-text("Manage")');
        yield page.click('button:has-text("Close Position")');
        // Verify closed
        yield (0, test_1.expect)(page.locator('.positions-table tbody')).toContainText('No active positions');
    }));
});
// Test Suite 3: Leveraged Trading Journey
test_1.test.describe('Leveraged Trading Journey', () => {
    test_1.test.beforeEach((_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        yield connectDemoWallet(page);
    }));
    (0, test_1.test)('should manage leveraged positions', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield navigateToMarkets(page);
        // Open high leverage position
        yield page.click(`text=${TEST_MARKET_2.title}`);
        yield page.click('button:has-text("BET NO")');
        // Set leverage
        yield page.fill('#tradeAmount', '100');
        yield page.click('button[data-leverage="50"]');
        // Check preview
        const maxWin = yield page.locator('#previewMaxWin').textContent();
        (0, test_1.expect)(maxWin).toContain('+$');
        // Execute trade
        yield page.click('button:has-text("Execute Trade")');
        yield page.waitForSelector('#successModal');
        // Monitor liquidation price
        yield page.click('button:has-text("View Position")');
        const liquidationPrice = yield page.locator('td:contains("Liquidation")').textContent();
        (0, test_1.expect)(liquidationPrice).toBeTruthy();
        // Add collateral
        yield page.click('button:has-text("Add Collateral")');
        yield page.fill('#collateralAmount', '50');
        yield page.click('button:has-text("Add")');
        // Verify improved health
        yield (0, test_1.expect)(page.locator('.position-health')).toContainText('Healthy');
    }));
});
// Test Suite 4: Quantum Betting Journey
test_1.test.describe('Quantum Betting Journey', () => {
    test_1.test.beforeEach((_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        yield connectDemoWallet(page);
    }));
    (0, test_1.test)('should create and manage quantum positions', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        // Navigate to quantum tab
        yield page.click('button:has-text("Quantum")');
        yield page.waitForSelector('.quantum-features');
        // Create superposition
        yield page.click('button:has-text("Create Superposition")');
        yield page.selectOption('#marketSelect', TEST_MARKET_1.title);
        yield page.fill('#superpositionAmount', '1000');
        // Set amplitudes
        yield page.fill('#amplitudeYes', '0.7071'); // 50%
        yield page.fill('#amplitudeNo', '0.7071'); // 50%
        yield page.click('button:has-text("Create Quantum Position")');
        // Verify creation
        yield (0, test_1.expect)(page.locator('.quantum-positions')).toContainText('Superposition Active');
        // Create entanglement
        yield page.click('button:has-text("Entangle Positions")');
        yield page.selectOption('#entangleMarket', TEST_MARKET_2.title);
        yield page.click('button:has-text("Create Entanglement")');
        // Monitor coherence
        yield page.waitForTimeout(5000);
        const coherence = yield page.locator('.coherence-level').textContent();
        (0, test_1.expect)(coherence).toContain('%');
        // Collapse wavefunction
        yield page.click('button:has-text("Observe")');
        yield (0, test_1.expect)(page.locator('.quantum-state')).toContainText('Collapsed');
    }));
});
// Test Suite 5: Verse Navigation Journey
test_1.test.describe('Verse Navigation Journey', () => {
    test_1.test.beforeEach((_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        yield connectDemoWallet(page);
    }));
    (0, test_1.test)('should navigate through verse hierarchy', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        // Go to verses
        yield page.click('button:has-text("Verses")');
        yield page.waitForSelector('.verse-tree');
        // Start at root
        const rootVerse = page.locator('.verse-node[data-level="0"]');
        yield (0, test_1.expect)(rootVerse).toContainText('Root Universe');
        // Navigate to Sports verse
        yield page.click('.verse-node:has-text("Sports")');
        yield (0, test_1.expect)(page.locator('.verse-info')).toContainText('1.5x multiplier');
        // Open position in Sports verse
        yield page.click('button:has-text("Trade in Verse")');
        yield page.selectOption('#verseMarketSelect', 'NFL Game');
        yield page.fill('#tradeAmount', '100');
        yield page.click('button:has-text("Place Trade")');
        // Navigate deeper to NFL verse
        yield page.click('.verse-node:has-text("NFL")');
        yield (0, test_1.expect)(page.locator('.verse-info')).toContainText('3x cumulative');
        // Test auto-chain
        yield page.click('button:has-text("Auto-Chain")');
        yield page.fill('#chainDeposit', '200');
        yield page.click('button:has-text("Execute Chain")');
        // Verify chain execution
        yield (0, test_1.expect)(page.locator('.chain-results')).toContainText('Chain Executed');
    }));
});
// Test Suite 6: DeFi Integration Journey
test_1.test.describe('DeFi Integration Journey', () => {
    test_1.test.beforeEach((_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        yield connectDemoWallet(page);
    }));
    (0, test_1.test)('should interact with DeFi features', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        // Navigate to DeFi
        yield page.click('button:has-text("DeFi")');
        yield page.waitForSelector('.defi-grid');
        // Stake MMT
        yield page.click('button:has-text("Stake MMT")');
        yield page.fill('#stakeAmount', '5000');
        yield page.click('button:has-text("Stake")');
        // Verify staking
        yield (0, test_1.expect)(page.locator('.staking-info')).toContainText('5,000 MMT');
        yield (0, test_1.expect)(page.locator('.apy-display')).toContainText('18.7%');
        // Add liquidity
        yield page.click('button:has-text("Add Liquidity")');
        yield page.selectOption('#poolSelect', 'MMT-SOL LP');
        yield page.fill('#liquidityAmount', '1000');
        yield page.click('button:has-text("Add")');
        // Check rewards
        yield page.waitForTimeout(3000);
        yield (0, test_1.expect)(page.locator('.rewards-earned')).not.toContainText('0.00');
    }));
});
// Test Suite 7: Error Recovery Journey
test_1.test.describe('Error Recovery Journey', () => {
    (0, test_1.test)('should handle errors gracefully', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        // Test network error
        yield page.route('**/api/markets', route => route.abort());
        yield page.click('button:has-text("Markets")');
        yield (0, test_1.expect)(page.locator('.error-message')).toContainText('Failed to load markets');
        // Test recovery
        yield page.unroute('**/api/markets');
        yield page.click('button:has-text("Retry")');
        yield (0, test_1.expect)(page.locator('.markets-grid')).toBeVisible();
        // Test invalid trade
        yield connectDemoWallet(page);
        yield navigateToMarkets(page);
        yield page.click('.market-card').first();
        yield page.click('button:has-text("BET YES")');
        yield page.fill('#tradeAmount', '999999'); // Exceeds balance
        yield page.click('button:has-text("Execute Trade")');
        yield (0, test_1.expect)(page.locator('.error-notification')).toContainText('Insufficient balance');
    }));
});
// Test Suite 8: Mobile User Journey
test_1.test.describe('Mobile User Journey', () => {
    test_1.test.use({
        viewport: { width: 375, height: 667 },
        hasTouch: true,
    });
    (0, test_1.test)('should work on mobile devices', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        // Test mobile menu
        yield page.click('.mobile-menu-button');
        yield (0, test_1.expect)(page.locator('.mobile-menu')).toBeVisible();
        // Connect wallet on mobile
        yield page.click('button:has-text("Connect")');
        yield page.click('button:has-text("Demo Wallet")');
        // Navigate markets on mobile
        yield page.click('button:has-text("Markets")');
        yield (0, test_1.expect)(page.locator('.markets-grid')).toHaveCSS('grid-template-columns', '1fr');
        // Place trade on mobile
        yield page.click('.market-card').first();
        yield page.click('button:has-text("BET YES")');
        yield page.fill('#tradeAmount', '50');
        // Test swipe gestures
        yield page.locator('.leverage-selector').swipe('left');
        yield (0, test_1.expect)(page.locator('.leverage-btn[data-leverage="10"]')).toHaveClass(/active/);
    }));
});
// Test Suite 9: Performance Testing
test_1.test.describe('Performance Testing', () => {
    (0, test_1.test)('should handle high-frequency updates', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        yield connectDemoWallet(page);
        // Monitor WebSocket messages
        let messageCount = 0;
        page.on('websocket', ws => {
            ws.on('framereceived', () => messageCount++);
        });
        yield navigateToMarkets(page);
        // Wait for updates
        yield page.waitForTimeout(10000);
        // Verify updates received
        (0, test_1.expect)(messageCount).toBeGreaterThan(10);
        // Test UI responsiveness during updates
        const start = Date.now();
        yield page.click('.market-card').first();
        const loadTime = Date.now() - start;
        (0, test_1.expect)(loadTime).toBeLessThan(1000); // Should load in under 1 second
    }));
});
// Test Suite 10: Accessibility Testing
test_1.test.describe('Accessibility Testing', () => {
    (0, test_1.test)('should be keyboard navigable', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('/');
        // Tab navigation
        yield page.keyboard.press('Tab');
        yield (0, test_1.expect)(page.locator(':focus')).toBeVisible();
        // Enter to activate
        yield page.keyboard.press('Tab'); // Navigate to "Launch App"
        yield page.keyboard.press('Tab'); // Navigate to "Connect Wallet"
        yield page.keyboard.press('Enter');
        yield (0, test_1.expect)(page.locator('#walletModal')).toBeVisible();
        // Escape to close
        yield page.keyboard.press('Escape');
        yield (0, test_1.expect)(page.locator('#walletModal')).not.toBeVisible();
        // Screen reader labels
        const connectButton = page.locator('button:has-text("Connect Wallet")');
        yield (0, test_1.expect)(connectButton).toHaveAttribute('aria-label', /connect.*wallet/i);
    }));
});
