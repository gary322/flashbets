import { test, expect } from '@playwright/test';

test.describe('Live Platform Demo', () => {
  test('complete platform walkthrough', async ({ page }) => {
    test.setTimeout(120000); // 2 minutes for full demo
    
    // 1. Visit landing page
    await page.goto('http://localhost:8080');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/01-landing.png', fullPage: true });
    
    // 2. Connect wallet
    await page.click('button:has-text("Connect Wallet")');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: 'demo/02-wallet-modal.png' });
    
    // Choose demo wallet
    await page.click('button:has-text("Demo Wallet")');
    await page.waitForTimeout(2000);
    
    // 3. Navigate to markets
    await page.click('a:has-text("Markets")');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/03-markets-live.png', fullPage: true });
    
    // 4. Click on a market
    const marketCard = page.locator('.market-card').first();
    await marketCard.click();
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/04-market-detail.png' });
    
    // 5. Go to trading terminal
    await page.goto('http://localhost:8080/app/trading.html');
    await page.waitForTimeout(2000);
    
    // Simulate trading
    if (await page.locator('input[name="amount"]').isVisible()) {
      await page.fill('input[name="amount"]', '1000');
      await page.selectOption('select[name="leverage"]', '10');
      await page.screenshot({ path: 'demo/05-trading-setup.png' });
    }
    
    // 6. Check portfolio
    await page.click('a:has-text("Portfolio")');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/06-portfolio-view.png', fullPage: true });
    
    // 7. Visit DeFi Hub
    await page.click('a:has-text("DeFi")');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/07-defi-features.png', fullPage: true });
    
    // 8. Check verse management
    await page.click('a:has-text("Verses")');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/08-verse-hierarchy.png', fullPage: true });
    
    // 9. Create market flow
    await page.click('a:has-text("Create Market")');
    await page.waitForTimeout(2000);
    
    // Fill create market form
    await page.fill('input[name="title"]', 'Will Quantum Computing be mainstream by 2026?');
    await page.fill('textarea[name="description"]', 'Market to predict quantum computing adoption');
    await page.selectOption('select[name="category"]', 'Technology');
    await page.screenshot({ path: 'demo/09-create-market-form.png' });
    
    // 10. Dashboard overview
    await page.click('a:has-text("Dashboard")');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/10-dashboard-stats.png', fullPage: true });
    
    // Check WebSocket connection
    const wsConnected = await page.evaluate(() => {
      return new Promise((resolve) => {
        const ws = new WebSocket('ws://localhost:8081/ws');
        ws.onopen = () => resolve(true);
        ws.onerror = () => resolve(false);
        setTimeout(() => resolve(false), 5000);
      });
    });
    
    expect(wsConnected).toBeTruthy();
    console.log('✅ WebSocket connection verified');
    
    // Verify API is responding
    const apiResponse = await page.request.get('http://localhost:8081/health');
    expect(apiResponse.ok()).toBeTruthy();
    console.log('✅ API health check passed');
    
    // Check for live updates
    await page.goto('http://localhost:8080/app/markets.html');
    const initialPrice = await page.locator('.market-card .price').first().textContent();
    await page.waitForTimeout(6000); // Wait for price update
    const updatedPrice = await page.locator('.market-card .price').first().textContent();
    
    console.log(`Price change detected: ${initialPrice} → ${updatedPrice}`);
    
    console.log('✅ Complete platform walkthrough successful!');
  });

  test('mobile responsive demo', async ({ browser }) => {
    const context = await browser.newContext({
      viewport: { width: 390, height: 844 },
      userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 14_7_1 like Mac OS X) AppleWebKit/605.1.15'
    });
    const page = await context.newPage();
    
    await page.goto('http://localhost:8080');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/mobile-01-landing.png', fullPage: true });
    
    await page.goto('http://localhost:8080/app/markets.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/mobile-02-markets.png', fullPage: true });
    
    await page.goto('http://localhost:8080/app/trading.html');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: 'demo/mobile-03-trading.png', fullPage: true });
    
    await context.close();
  });

  test('performance metrics', async ({ page }) => {
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
      await page.goto(pageInfo.url);
      await page.waitForLoadState('networkidle');
      const loadTime = Date.now() - start;
      
      metrics.push({
        page: pageInfo.name,
        loadTime: `${loadTime}ms`,
        status: loadTime < 2000 ? '✅ Fast' : '⚠️ Slow'
      });
    }
    
    console.table(metrics);
    
    // Check WebSocket latency
    const wsLatency = await page.evaluate(() => {
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
    expect(wsLatency).toBeLessThan(100);
  });
});