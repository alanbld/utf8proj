// @ts-check
import { defineConfig } from '@playwright/test';

/**
 * Playwright configuration for demo video recording.
 * Uses slower motion and always records video.
 */
export default defineConfig({
  testDir: './tests',
  testMatch: /demo-.*\.spec\.js/,
  timeout: 300000, // 5 minutes for demo recording
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:8080',
    video: {
      mode: 'on',
      size: { width: 1280, height: 720 },
    },
    launchOptions: {
      slowMo: 100, // Slow down for visibility
    },
    viewport: { width: 1280, height: 720 },
  },
  projects: [
    {
      name: 'demo-recording',
      use: {
        browserName: 'chromium',
        headless: true,
      },
    },
  ],
  outputDir: 'demo-recordings/',
  webServer: {
    command: 'python3 -m http.server 8080',
    port: 8080,
    timeout: 30000,
    reuseExistingServer: true,
  },
});
