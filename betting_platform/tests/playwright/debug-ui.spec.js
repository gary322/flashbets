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
(0, test_1.test)('debug UI loading', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
    // Capture console logs
    page.on('console', msg => {
        console.log(`Browser console [${msg.type()}]:`, msg.text());
    });
    // Capture page errors
    page.on('pageerror', error => {
        console.error('Page error:', error.message);
    });
    // Navigate to the page
    console.log('Navigating to http://localhost:8080...');
    yield page.goto('http://localhost:8080', { waitUntil: 'domcontentloaded' });
    // Wait a bit for JavaScript to execute
    yield page.waitForTimeout(3000);
    // Check page title
    const title = yield page.title();
    console.log('Page title:', title);
    // Check if loading screen is visible
    const loadingScreen = yield page.locator('#loadingScreen').isVisible();
    console.log('Loading screen visible:', loadingScreen);
    // Check if main content is visible
    const mainContent = yield page.locator('.hero-section').isVisible();
    console.log('Main content visible:', mainContent);
    // Check for any error messages
    const errorMessages = yield page.locator('.error').count();
    console.log('Error messages found:', errorMessages);
    // Take screenshot
    yield page.screenshot({ path: 'debug-screenshot.png', fullPage: true });
    // Try to interact with wallet button
    const walletButton = page.locator('button:has-text("Connect Wallet")');
    if (yield walletButton.isVisible()) {
        console.log('Wallet button found');
        yield walletButton.click();
        yield page.waitForTimeout(1000);
        yield page.screenshot({ path: 'debug-wallet-modal.png' });
    }
    // Check network requests
    const apiHealth = yield page.request.get('http://localhost:8081/health');
    console.log('API health check:', apiHealth.status(), yield apiHealth.text());
}));
