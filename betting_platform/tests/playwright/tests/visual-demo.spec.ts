import { test, expect } from '@playwright/test';

test.describe('Visual UI Demo', () => {
  test('capture all UI pages', async ({ page }) => {
    // Set viewport
    await page.setViewportSize({ width: 1920, height: 1080 });
    
    // 1. Landing Page
    await page.goto('http://localhost:8080');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'screenshots/01-landing-page.png', fullPage: true });
    
    // 2. Markets Page
    await page.goto('http://localhost:8080/app/markets.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'screenshots/02-markets-page.png', fullPage: true });
    
    // 3. Trading Terminal
    await page.goto('http://localhost:8080/app/trading.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'screenshots/03-trading-terminal.png', fullPage: true });
    
    // 4. Create Market
    await page.goto('http://localhost:8080/app/create-market.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'screenshots/04-create-market.png', fullPage: true });
    
    // 5. Verse Management
    await page.goto('http://localhost:8080/app/verses.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'screenshots/05-verse-management.png', fullPage: true });
    
    // 6. Portfolio
    await page.goto('http://localhost:8080/app/portfolio.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'screenshots/06-portfolio.png', fullPage: true });
    
    // 7. DeFi Hub
    await page.goto('http://localhost:8080/app/defi.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'screenshots/07-defi-hub.png', fullPage: true });
    
    // 8. Dashboard
    await page.goto('http://localhost:8080/app/dashboard.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'screenshots/08-dashboard.png', fullPage: true });
    
    // Test API connection
    const apiHealth = await page.request.get('http://localhost:8081/health');
    expect(apiHealth.ok()).toBeTruthy();
    
    console.log('✅ All UI pages captured successfully!');
  });

  test('demonstrate interactive features', async ({ page }) => {
    await page.goto('http://localhost:8080');
    
    // Try to click connect wallet
    const connectButton = page.locator('button:has-text("Connect Wallet")');
    if (await connectButton.isVisible()) {
      await connectButton.click();
      await page.waitForTimeout(1000);
      await page.screenshot({ path: 'screenshots/wallet-modal.png' });
    }
    
    // Navigate through menu
    await page.goto('http://localhost:8080/app/markets.html');
    
    // Check for market cards
    const marketCards = page.locator('.market-card');
    const count = await marketCards.count();
    console.log(`Found ${count} market cards`);
    
    // Check WebSocket connection
    await page.evaluate(() => {
      const ws = new WebSocket('ws://localhost:8081/ws');
      ws.onopen = () => console.log('WebSocket connected');
      ws.onmessage = (event) => console.log('WebSocket message:', event.data);
    });
    
    await page.waitForTimeout(3000);
  });

  test('mobile responsive test', async ({ page }) => {
    // iPhone 12 Pro
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto('http://localhost:8080');
    await page.screenshot({ path: 'screenshots/mobile-landing.png', fullPage: true });
    
    // iPad
    await page.setViewportSize({ width: 820, height: 1180 });
    await page.goto('http://localhost:8080/app/trading.html');
    await page.screenshot({ path: 'screenshots/tablet-trading.png', fullPage: true });
  });
});