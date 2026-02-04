import { test, expect } from '@playwright/test';

test.describe('Platform Health Check', () => {
  test('should verify UI is accessible', async ({ page }) => {
    await page.goto('http://localhost:8080');
    await expect(page).toHaveTitle(/Quantum Betting/);
  });

  test('should verify API is accessible', async ({ page }) => {
    const response = await page.request.get('http://localhost:8081/health');
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.status).toBe('ok');
  });

  test('should connect demo wallet', async ({ page }) => {
    await page.goto('http://localhost:8080');
    
    // Click connect wallet
    await page.click('button:has-text("Connect Wallet")');
    
    // Choose demo wallet
    await page.click('button:has-text("Demo Wallet")');
    
    // Verify connected
    await expect(page.locator('.wallet-connected')).toBeVisible({ timeout: 10000 });
  });

  test('should load markets page', async ({ page }) => {
    await page.goto('http://localhost:8080/app/markets.html');
    await expect(page.locator('h1')).toContainText('Markets');
  });

  test('should open trading terminal', async ({ page }) => {
    await page.goto('http://localhost:8080/app/trading.html');
    await expect(page.locator('h1')).toContainText('Trading Terminal');
  });
});