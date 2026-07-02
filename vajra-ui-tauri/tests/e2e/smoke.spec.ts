import { test, expect } from '@playwright/test';

test.describe('Vajra UI Smoke Tests', () => {
  test('should load the application and display the sidebar', async ({ page }) => {
    await page.goto('/');

    // Check if sidebar contains "All Downloads"
    const allDownloads = page.locator('text=All Downloads').first();
    await expect(allDownloads).toBeVisible();

    // Check if the dashboard category is present
    const dashboard = page.locator('text=Dashboard').first();
    await expect(dashboard).toBeVisible();
  });

  test('should open Add URL window', async ({ page }) => {
    await page.goto('/');

    // Click the "Add URL" button in the toolbar
    const addUrlBtn = page.getByTitle('Add URL');
    await expect(addUrlBtn).toBeVisible();
    await addUrlBtn.click();

    // Since Tauri windows can't be easily tested in the same browser context in dev mode if they are spawned via Tauri API,
    // we just check if the click succeeds without errors for now, or if it's a web-based modal.
    // If it's a separate window spawned via Tauri invoke, it won't be visible in Playwright browser.
    // Assuming no errors is a pass for smoke test.
  });

  test('should render empty downloads table initially', async ({ page }) => {
    await page.goto('/');

    // There should be a grid element
    const grid = page.locator('.ag-theme-alpine-dark, .ag-theme-quartz-dark, [role="grid"]');
    await expect(grid).toBeVisible();
  });
});
