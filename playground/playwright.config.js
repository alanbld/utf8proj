// @ts-check
import { defineConfig, devices } from '@playwright/test';

/**
 * Standard Playwright configuration for E2E tests.
 * Runs tests in headless mode with video/screenshot on failure.
 */
export default defineConfig({
  testDir: './tests',
  timeout: 30000,
  expect: {
    timeout: 5000,
  },
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: [
    ['list'],
    ['html', { open: 'never' }],
  ],
  use: {
    baseURL: 'http://localhost:8080',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  outputDir: 'test-results/',
  webServer: {
    command: 'python3 -m http.server 8080',
    port: 8080,
    timeout: 30000,
    reuseExistingServer: !process.env.CI,
  },
});
