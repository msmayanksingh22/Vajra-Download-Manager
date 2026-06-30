import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30000,
  expect: {
    timeout: 5000,
  },
  fullyParallel: false,
  retries: 1,
  workers: 1,
  reporter: 'html',
  use: {
    actionTimeout: 0,
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  // Note: For real E2E we would start the daemon and vite server here using webServer.
  // We assume the dev server and daemon are already running for local execution, or we use a custom script.
  webServer: {
    command: 'npm run dev',
    port: 1420,
    reuseExistingServer: true,
  },
});
