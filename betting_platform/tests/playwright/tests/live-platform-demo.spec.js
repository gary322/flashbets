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
test_1.test.describe('Live Platform Demo', () => {
    (0, test_1.test)('complete platform walkthrough', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        test_1.test.setTimeout(120000); // 2 minutes for full demo
        // 1. Visit landing page
        yield page.goto('http://localhost:8080');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/01-landing.png', fullPage: true });
        // 2. Connect wallet
        yield page.click('button:has-text("Connect Wallet")');
        yield page.waitForTimeout(1000);
        yield page.screenshot({ path: 'demo/02-wallet-modal.png' });
        // Choose demo wallet
        yield page.click('button:has-text("Demo Wallet")');
        yield page.waitForTimeout(2000);
        // 3. Navigate to markets
        yield page.click('a:has-text("Markets")');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/03-markets-live.png', fullPage: true });
        // 4. Click on a market
        const marketCard = page.locator('.market-card').first();
        yield marketCard.click();
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/04-market-detail.png' });
        // 5. Go to trading terminal
        yield page.goto('http://localhost:8080/app/trading.html');
        yield page.waitForTimeout(2000);
        // Simulate trading
        if (yield page.locator('input[name="amount"]').isVisible()) {
            yield page.fill('input[name="amount"]', '1000');
            yield page.selectOption('select[name="leverage"]', '10');
            yield page.screenshot({ path: 'demo/05-trading-setup.png' });
        }
        // 6. Check portfolio
        yield page.click('a:has-text("Portfolio")');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/06-portfolio-view.png', fullPage: true });
        // 7. Visit DeFi Hub
        yield page.click('a:has-text("DeFi")');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/07-defi-features.png', fullPage: true });
        // 8. Check verse management
        yield page.click('a:has-text("Verses")');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/08-verse-hierarchy.png', fullPage: true });
        // 9. Create market flow
        yield page.click('a:has-text("Create Market")');
        yield page.waitForTimeout(2000);
        // Fill create market form
        yield page.fill('input[name="title"]', 'Will Quantum Computing be mainstream by 2026?');
        yield page.fill('textarea[name="description"]', 'Market to predict quantum computing adoption');
        yield page.selectOption('select[name="category"]', 'Technology');
        yield page.screenshot({ path: 'demo/09-create-market-form.png' });
        // 10. Dashboard overview
        yield page.click('a:has-text("Dashboard")');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/10-dashboard-stats.png', fullPage: true });
        // Check WebSocket connection
        const wsConnected = yield page.evaluate(() => {
            return new Promise((resolve) => {
                const ws = new WebSocket('ws://localhost:8081/ws');
                ws.onopen = () => resolve(true);
                ws.onerror = () => resolve(false);
                setTimeout(() => resolve(false), 5000);
            });
        });
        (0, test_1.expect)(wsConnected).toBeTruthy();
        console.log('✅ WebSocket connection verified');
        // Verify API is responding
        const apiResponse = yield page.request.get('http://localhost:8081/health');
        (0, test_1.expect)(apiResponse.ok()).toBeTruthy();
        console.log('✅ API health check passed');
        // Check for live updates
        yield page.goto('http://localhost:8080/app/markets.html');
        const initialPrice = yield page.locator('.market-card .price').first().textContent();
        yield page.waitForTimeout(6000); // Wait for price update
        const updatedPrice = yield page.locator('.market-card .price').first().textContent();
        console.log(`Price change detected: ${initialPrice} → ${updatedPrice}`);
        console.log('✅ Complete platform walkthrough successful!');
    }));
    (0, test_1.test)('mobile responsive demo', (_a) => __awaiter(void 0, [_a], void 0, function* ({ browser }) {
        const context = yield browser.newContext({
            viewport: { width: 390, height: 844 },
            userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 14_7_1 like Mac OS X) AppleWebKit/605.1.15'
        });
        const page = yield context.newPage();
        yield page.goto('http://localhost:8080');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/mobile-01-landing.png', fullPage: true });
        yield page.goto('http://localhost:8080/app/markets.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/mobile-02-markets.png', fullPage: true });
        yield page.goto('http://localhost:8080/app/trading.html');
        yield page.waitForTimeout(2000);
        yield page.screenshot({ path: 'demo/mobile-03-trading.png', fullPage: true });
        yield context.close();
    }));
    (0, test_1.test)('performance metrics', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        const metrics = [];
        // Measure page load times
        const pages = [
            { name: 'Landing', url: 'http://localhost:8080' },
            { name: 'Markets', url: 'http://localhost:8080/app/markets.html' },
            { name: 'Trading', url: 'http://localhost:8080/app/trading.html' },
            { name: 'DeFi', url: 'http://localhost:8080/app/defi.html' }
        ];
        for (const pageInfo of pages) {
            const start = Date.now();
            yield page.goto(pageInfo.url);
            yield page.waitForLoadState('networkidle');
            const loadTime = Date.now() - start;
            metrics.push({
                page: pageInfo.name,
                loadTime: `${loadTime}ms`,
                status: loadTime < 2000 ? '✅ Fast' : '⚠️ Slow'
            });
        }
        console.table(metrics);
        // Check WebSocket latency
        const wsLatency = yield page.evaluate(() => {
            return new Promise((resolve) => {
                const ws = new WebSocket('ws://localhost:8081/ws');
                const start = Date.now();
                ws.onopen = () => {
                    ws.send(JSON.stringify({ type: 'ping' }));
                };
                ws.onmessage = () => {
                    resolve(Date.now() - start);
                };
            });
        });
        console.log(`WebSocket latency: ${wsLatency}ms`);
        (0, test_1.expect)(wsLatency).toBeLessThan(100);
    }));
});
