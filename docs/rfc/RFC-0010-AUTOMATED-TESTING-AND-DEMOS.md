# RFC-0010: Automated Testing and Demo Recording

**Status:** Partial (Demo Videos Only)
**Created:** 2026-01-16
**Implemented:** 2026-01-16 (videos only, E2E tests deferred)
**Author:** Claude + Human collaboration

## Summary

Add Playwright-based E2E testing for the WASM playground and automated demo video recording with subtitles for user onboarding.

## Motivation

1. **Quality Assurance**: The WASM playground lacks automated testing - bugs can slip through
2. **User Onboarding**: New users struggle to understand capabilities without visual tutorials
3. **Documentation**: Video demos are more engaging than text-only documentation
4. **Regression Prevention**: Automated tests catch breaking changes in the playground

### Inspiration

The [multi-brand-dealer-network-vehicle-configurator](https://production.eng.it/gitlab/gcatalan/multi-brand-dealer-network-vehicle-configurator) project demonstrates production-grade patterns:
- 42 organized E2E tests with Playwright
- Demo video recording with injected subtitles
- Multiple configurations (standard, demo, innovative)
- Environment-gated demo recording (`RECORD_DEMO=true`)

## Design

### Phase 1: Test Infrastructure

```
playground/
├── tests/
│   ├── e2e.spec.js           # Core E2E test suite
│   ├── demo-quickstart.spec.js    # Quick start video
│   └── demo-features.spec.js      # Feature showcase video
├── playwright.config.js       # Standard test config
├── playwright.demo.config.js  # Demo recording config
└── package.json              # Test scripts
```

### Phase 2: E2E Test Suite

**Test Categories:**

| Category | Tests | Purpose |
|----------|-------|---------|
| Editor | 5 | Monaco loads, syntax highlighting, error markers |
| Parser | 6 | Valid projects parse, errors shown inline |
| Scheduler | 8 | Schedule computes, critical path displays |
| Gantt | 10 | SVG renders, tooltips work, zoom controls |
| Excel | 5 | Export produces file, formulas work |
| Share | 3 | URL encoding, decoding, clipboard |
| Themes | 2 | Light/dark switching |

**Total: ~39 tests**

### Phase 3: Demo Videos

| Demo | Duration | Scenes |
|------|----------|--------|
| **Quick Start** | ~2 min | Load → Type project → Schedule → View Gantt |
| **Excel Export** | ~3 min | Project → Configure → Export → Show spreadsheet |
| **Resource Leveling** | ~3 min | Overallocation → Enable `-l` → See L001 diagnostics |
| **Progress Tracking** | ~2 min | Add `complete: 50%` → Forecasts → Variance |
| **Full Tour** | ~10 min | All features combined |

### Implementation Details

#### 1. Test Helper Functions

```javascript
// Wait for WASM scheduler to complete
async function waitForSchedule(page) {
    await page.waitForFunction(() => {
        const output = document.getElementById('output');
        return output && output.textContent.includes('scheduled successfully');
    }, { timeout: 15000 });
    await page.waitForTimeout(500);
}

// Wait for Gantt SVG to render
async function waitForGantt(page) {
    await page.waitForFunction(() => {
        const gantt = document.querySelector('.gantt-container svg');
        return gantt && gantt.querySelectorAll('rect').length > 0;
    }, { timeout: 10000 });
}

// Type project code with realistic timing
async function typeProject(page, code) {
    const editor = page.locator('.monaco-editor textarea');
    await editor.focus();
    await page.keyboard.press('Control+a');
    await page.keyboard.type(code, { delay: 30 });
}
```

#### 2. Subtitle Injection

```javascript
async function injectSubtitleOverlay(page) {
    await page.evaluate(() => {
        const overlay = document.createElement('div');
        overlay.id = 'demo-subtitle';
        overlay.style.cssText = `
            position: fixed;
            bottom: 30px;
            left: 50%;
            transform: translateX(-50%);
            background: rgba(0, 0, 0, 0.85);
            color: white;
            padding: 16px 32px;
            border-radius: 8px;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
            font-size: 20px;
            font-weight: 500;
            z-index: 999999;
            max-width: 80%;
            text-align: center;
            box-shadow: 0 4px 20px rgba(0,0,0,0.3);
            transition: opacity 0.3s ease;
            opacity: 0;
        `;
        document.body.appendChild(overlay);
    });
}

async function showSubtitle(page, text, duration = 2000) {
    await page.evaluate((text) => {
        const el = document.getElementById('demo-subtitle');
        if (el) {
            el.style.opacity = '1';
            el.textContent = text;
        }
    }, text);
    await page.waitForTimeout(duration);
}

async function hideSubtitle(page) {
    await page.evaluate(() => {
        const el = document.getElementById('demo-subtitle');
        if (el) el.style.opacity = '0';
    });
    await page.waitForTimeout(300);
}
```

#### 3. Playwright Configuration

**Standard Tests (`playwright.config.js`):**
```javascript
module.exports = {
    testDir: './tests',
    timeout: 30000,
    expect: { timeout: 5000 },
    fullyParallel: false,
    workers: 1,
    reporter: [['list'], ['html', { open: 'never' }]],
    use: {
        baseURL: 'http://localhost:8080',
        trace: 'on-first-retry',
        screenshot: 'only-on-failure',
        video: 'retain-on-failure',
    },
    projects: [
        { name: 'chromium', use: { browserName: 'chromium', headless: true } }
    ],
    outputDir: 'test-results/',
    webServer: {
        command: 'python3 -m http.server 8080',
        port: 8080,
        timeout: 30000,
        reuseExistingServer: true,
    },
};
```

**Demo Recording (`playwright.demo.config.js`):**
```javascript
module.exports = {
    testDir: './tests',
    timeout: 300000,  // 5 minutes
    fullyParallel: false,
    retries: 0,
    workers: 1,
    reporter: 'list',
    use: {
        baseURL: 'http://localhost:8080',
        video: {
            mode: 'on',
            size: { width: 1280, height: 720 }
        },
        launchOptions: { slowMo: 100 },
        viewport: { width: 1280, height: 720 },
    },
    projects: [
        { name: 'demo-recording', use: { browserName: 'chromium', headless: true } }
    ],
    outputDir: 'demo-recording/',
    webServer: {
        command: 'python3 -m http.server 8080',
        port: 8080,
        reuseExistingServer: true,
    },
};
```

#### 4. NPM Scripts

```json
{
  "scripts": {
    "test": "npx playwright test",
    "test:ui": "npx playwright test --ui",
    "test:headed": "npx playwright test --headed",
    "demo:quickstart": "RECORD_DEMO=true npx playwright test demo-quickstart --config=playwright.demo.config.js",
    "demo:features": "RECORD_DEMO=true npx playwright test demo-features --config=playwright.demo.config.js",
    "demo:all": "npm run demo:quickstart && npm run demo:features"
  }
}
```

### Phase 4: CI/CD Integration

```yaml
# .github/workflows/playground-tests.yml
name: Playground E2E Tests

on:
  push:
    paths:
      - 'playground/**'
      - 'crates/utf8proj-wasm/**'
  pull_request:
    paths:
      - 'playground/**'
      - 'crates/utf8proj-wasm/**'

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Build WASM
        run: cd playground && ./build.sh

      - name: Install Playwright
        run: cd playground && npm ci && npx playwright install chromium

      - name: Run E2E Tests
        run: cd playground && npm test

      - name: Upload Test Results
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: playwright-report
          path: playground/test-results/
```

### Phase 5: Demo Publishing

```yaml
# .github/workflows/record-demos.yml (manual trigger)
name: Record Demo Videos

on:
  workflow_dispatch:
    inputs:
      demo:
        description: 'Demo to record'
        required: true
        default: 'all'
        type: choice
        options:
          - quickstart
          - features
          - all

jobs:
  record:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup
        run: |
          cd playground
          ./build.sh
          npm ci
          npx playwright install chromium

      - name: Record Demo
        run: cd playground && npm run demo:${{ inputs.demo }}

      - name: Convert to MP4
        run: |
          for f in playground/demo-recording/**/*.webm; do
            ffmpeg -i "$f" -c:v libx264 -crf 23 -pix_fmt yuv420p "${f%.webm}.mp4"
          done

      - name: Upload Videos
        uses: actions/upload-artifact@v4
        with:
          name: demo-videos
          path: playground/demo-recording/**/*.mp4
```

## Example Demo Script

```javascript
// tests/demo-quickstart.spec.js
const { test } = require('@playwright/test');

const skipDemo = process.env.RECORD_DEMO !== 'true';

(skipDemo ? test.skip : test)('Quick Start Demo', async ({ page }) => {
    // Setup
    await page.goto('/');
    await injectSubtitleOverlay(page);

    // SCENE 1: Introduction
    await showSubtitle(page, 'utf8proj: Explainable Project Scheduling', 3000);

    // SCENE 2: Type a project
    await showSubtitle(page, 'Create a simple project file', 2000);
    await typeProject(page, `
project "Website Redesign" {
    start: 2025-02-01
}

resource dev "Developer" { rate: 850/day }

task design "Design" { effort: 5d, assign: dev }
task build "Build" { effort: 10d, assign: dev, depends: design }
milestone launch "Launch" { depends: build }
`);

    // SCENE 3: Schedule
    await showSubtitle(page, 'Click "Schedule" to compute dates', 2000);
    await page.click('#schedule-btn');
    await waitForSchedule(page);
    await showSubtitle(page, 'CPM algorithm computes start/end dates', 2500);

    // SCENE 4: View Gantt
    await showSubtitle(page, 'Interactive Gantt chart with critical path', 2500);
    await page.click('#gantt-tab');
    await waitForGantt(page);
    await page.waitForTimeout(3000);

    // SCENE 5: Export
    await showSubtitle(page, 'Export to Excel for stakeholder reports', 2000);
    await page.click('#export-xlsx');
    await page.waitForTimeout(2000);

    // Finale
    await showSubtitle(page, 'Try it yourself at alanbld.github.io/utf8proj', 4000);
    await hideSubtitle(page);
});
```

## Migration Path

1. **Phase 1** (1 day): Add Playwright to playground, create basic test structure
2. **Phase 2** (2 days): Implement 39 E2E tests
3. **Phase 3** (1 day): Create demo recording infrastructure
4. **Phase 4** (1 day): Record 5 demo videos
5. **Phase 5** (1 day): CI/CD integration

**Total: ~6 days of effort**

## Success Criteria

- [ ] 39 E2E tests passing on CI (deferred)
- [x] 2 demo videos recorded (~15 minutes total)
- [x] Demo videos linked from README
- [ ] HTML test report published on failures (deferred)
- [ ] <5% test flakiness rate (deferred)

## Risks

1. **WASM loading time**: Tests may timeout waiting for WASM initialization
   - Mitigation: Increase timeouts, add retry logic

2. **Monaco editor interaction**: Complex to simulate typing
   - Mitigation: Use Playwright's `page.keyboard` API

3. **Video size**: WebM files can be large
   - Mitigation: Convert to H.264 MP4, compress with ffmpeg

## Alternatives Considered

1. **Cypress**: Good, but Playwright has better WASM support
2. **Puppeteer**: Lower-level, no built-in video recording
3. **Manual testing**: Not scalable, prone to human error

## References

- [Playwright Documentation](https://playwright.dev/)
- [Vehicle Configurator E2E Tests](https://production.eng.it/gitlab/gcatalan/multi-brand-dealer-network-vehicle-configurator)
- [utf8proj Playground](https://alanbld.github.io/utf8proj/)
