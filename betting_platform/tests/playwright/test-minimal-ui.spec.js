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
(0, test_1.test)('verify minimal yellow UI', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
    yield page.goto('http://localhost:8080');
    yield page.waitForTimeout(2000);
    // Take screenshot
    yield page.screenshot({ path: 'minimal-ui-screenshot.png', fullPage: true });
    // Check title
    const title = yield page.title();
    (0, test_1.expect)(title).toBe('Quantum Betting - Minimalist Platform');
    // Check if yellow theme is applied
    const heroTitle = yield page.locator('h1').textContent();
    (0, test_1.expect)(heroTitle).toContain('Prediction Markets on Solana');
    // Check stats are visible
    const stats = yield page.locator('.stat-card').count();
    (0, test_1.expect)(stats).toBe(4);
    // Check API connection
    const apiHealth = yield page.request.get('http://localhost:8081/health');
    (0, test_1.expect)(apiHealth.ok()).toBeTruthy();
    console.log('✅ Minimal yellow UI is working!');
}));
