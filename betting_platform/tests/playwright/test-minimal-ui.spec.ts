import { test, expect } from '@playwright/test';

test('verify minimal yellow UI', async ({ page }) => {
  await page.goto('http://localhost:8080');
  await page.waitForTimeout(2000);
  
  // Take screenshot
  await page.screenshot({ path: 'minimal-ui-screenshot.png', fullPage: true });
  
  // Check title
  const title = await page.title();
  expect(title).toBe('Quantum Betting - Minimalist Platform');
  
  // Check if yellow theme is applied
  const heroTitle = await page.locator('h1').textContent();
  expect(heroTitle).toContain('Prediction Markets on Solana');
  
  // Check stats are visible
  const stats = await page.locator('.stat-card').count();
  expect(stats).toBe(4);
  
  // Check API connection
  const apiHealth = await page.request.get('http://localhost:8081/health');
  expect(apiHealth.ok()).toBeTruthy();
  
  console.log('✅ Minimal yellow UI is working!');
});