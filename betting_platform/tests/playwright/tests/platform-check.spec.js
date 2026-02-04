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
test_1.test.describe('Platform Health Check', () => {
    (0, test_1.test)('should verify UI is accessible', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('http://localhost:8080');
        yield (0, test_1.expect)(page).toHaveTitle(/Quantum Betting/);
    }));
    (0, test_1.test)('should verify API is accessible', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        const response = yield page.request.get('http://localhost:8081/health');
        (0, test_1.expect)(response.ok()).toBeTruthy();
        const data = yield response.json();
        (0, test_1.expect)(data.status).toBe('ok');
    }));
    (0, test_1.test)('should connect demo wallet', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('http://localhost:8080');
        // Click connect wallet
        yield page.click('button:has-text("Connect Wallet")');
        // Choose demo wallet
        yield page.click('button:has-text("Demo Wallet")');
        // Verify connected
        yield (0, test_1.expect)(page.locator('.wallet-connected')).toBeVisible({ timeout: 10000 });
    }));
    (0, test_1.test)('should load markets page', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('http://localhost:8080/app/markets.html');
        yield (0, test_1.expect)(page.locator('h1')).toContainText('Markets');
    }));
    (0, test_1.test)('should open trading terminal', (_a) => __awaiter(void 0, [_a], void 0, function* ({ page }) {
        yield page.goto('http://localhost:8080/app/trading.html');
        yield (0, test_1.expect)(page.locator('h1')).toContainText('Trading Terminal');
    }));
});
