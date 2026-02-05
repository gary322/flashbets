import { test, expect } from '@playwright/test';

test('browse markets and submit a Polymarket order (demo)', async ({ page }) => {
  await page.goto('/markets');

  await expect(page.getByRole('heading', { name: 'Prediction Markets' })).toBeVisible();

  const marketCards = page.getByTestId('market-card');
  await expect(marketCards.first()).toBeVisible();
  await marketCards.first().click();

  await expect(page).toHaveURL(/\/trade\?market=/);

  const submitButton = page.getByTestId('trade-submit');
  await expect(submitButton).toBeVisible();

  // First click connects the (demo) wallet.
  await expect(submitButton).toHaveText(/Connect Wallet/i);
  await submitButton.click();
  await expect(submitButton).toHaveText(/Buy Position|Sell Position/);

  // Fill amount after connection to avoid early validation noise.
  const amountInput = page.getByTestId('trade-amount');
  await amountInput.fill('10');

  const confirmPromise = page.waitForEvent('dialog');
  const orderResponsePromise = page.waitForResponse((response) => {
    return response.url().includes('/api/orders/submit') && response.status() === 200;
  });

  await submitButton.click();

  const confirmDialog = await confirmPromise;
  expect(confirmDialog.type()).toBe('confirm');

  const alertPromise = page.waitForEvent('dialog');
  await confirmDialog.accept();

  await orderResponsePromise;

  const alertDialog = await alertPromise;
  expect(alertDialog.type()).toBe('alert');
  expect(alertDialog.message()).toContain('Order submitted successfully');
  await alertDialog.accept();
});

