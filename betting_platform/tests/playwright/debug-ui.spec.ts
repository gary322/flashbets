import { test, expect } from '@playwright/test';

test('debug UI loading', async ({ page }) => {
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
  await page.goto('http://localhost:8080', { waitUntil: 'domcontentloaded' });
  
  // Wait a bit for JavaScript to execute
  await page.waitForTimeout(3000);
  
  // Check page title
  const title = await page.title();
  console.log('Page title:', title);
  
  // Check if loading screen is visible
  const loadingScreen = await page.locator('#loadingScreen').isVisible();
  console.log('Loading screen visible:', loadingScreen);
  
  // Check if main content is visible
  const mainContent = await page.locator('.hero-section').isVisible();
  console.log('Main content visible:', mainContent);
  
  // Check for any error messages
  const errorMessages = await page.locator('.error').count();
  console.log('Error messages found:', errorMessages);
  
  // Take screenshot
  await page.screenshot({ path: 'debug-screenshot.png', fullPage: true });
  
  // Try to interact with wallet button
  const walletButton = page.locator('button:has-text("Connect Wallet")');
  if (await walletButton.isVisible()) {
    console.log('Wallet button found');
    await walletButton.click();
    await page.waitForTimeout(1000);
    await page.screenshot({ path: 'debug-wallet-modal.png' });
  }
  
  // Check network requests
  const apiHealth = await page.request.get('http://localhost:8081/health');
  console.log('API health check:', apiHealth.status(), await apiHealth.text());
});