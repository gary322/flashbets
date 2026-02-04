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
test_1.test.describe('Visual UI Demo', () => {
    (0, test_1.test)('capture all UI pages', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        // Set viewport
        yield page.setViewportSize({ width: 1920, height: 1080 });
        // 1. Landing Page
        yield page.goto('http://localhost:8080');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'screenshots/01-landing-page.png', fullPage: true });
        // 2. Markets Page
        yield page.goto('http://localhost:8080/app/markets.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'screenshots/02-markets-page.png', fullPage: true });
        // 3. Trading Terminal
        yield page.goto('http://localhost:8080/app/trading.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'screenshots/03-trading-terminal.png', fullPage: true });
        // 4. Create Market
        yield page.goto('http://localhost:8080/app/create-market.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'screenshots/04-create-market.png', fullPage: true });
        // 5. Verse Management
        yield page.goto('http://localhost:8080/app/verses.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'screenshots/05-verse-management.png', fullPage: true });
        // 6. Portfolio
        yield page.goto('http://localhost:8080/app/portfolio.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'screenshots/06-portfolio.png', fullPage: true });
        // 7. DeFi Hub
        yield page.goto('http://localhost:8080/app/defi.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'screenshots/07-defi-hub.png', fullPage: true });
        // 8. Dashboard
        yield page.goto('http://localhost:8080/app/dashboard.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'screenshots/08-dashboard.png', fullPage: true });
        // Test API connection
        const apiHealth = yield page.request.get('http://localhost:8081/health');
        (0, test_1.expect)(apiHealth.ok()).toBeTruthy();
        console.log('✅ All UI pages captured successfully!');
    }));
    (0, test_1.test)('demonstrate interactive features', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('http://localhost:8080');
        // Try to click connect wallet
        const connectButton = page.locator('button:has-text("Connect Wallet")');
        if (yield connectButton.isVisible()) {
            yield connectButton.click();
            yield page.waitForTimeout(1000);
            yield page.screenshot({ path: 'screenshots/wallet-modal.png' });
        }
        // Navigate through menu
        yield page.goto('http://localhost:8080/app/markets.html');
        // Check for market cards
        const marketCards = page.locator('.market-card');
        const count = yield marketCards.count();
        console.log(`Found ${count} market cards`);
        // Check WebSocket connection
        yield page.evaluate(() => {
            const ws = new WebSocket('ws://localhost:8081/ws');
            ws.onopen = () => console.log('WebSocket connected');
            ws.onmessage = (event) => console.log('WebSocket message:', event.data);
        });
        yield page.waitForTimeout(3000);
    }));
    (0, test_1.test)('mobile responsive test', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        // iPhone 12 Pro
        yield page.setViewportSize({ width: 390, height: 844 });
        yield page.goto('http://localhost:8080');
        yield page.screenshot({ path: 'screenshots/mobile-landing.png', fullPage: true });
        // iPad
        yield page.setViewportSize({ width: 820, height: 1180 });
        yield page.goto('http://localhost:8080/app/trading.html');
        yield page.screenshot({ path: 'screenshots/tablet-trading.png', fullPage: true });
    }));
});
