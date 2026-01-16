/**
 * Test helpers for utf8proj playground E2E tests.
 * Provides utilities for waiting on WASM operations and DOM rendering.
 */

/**
 * Wait for WASM module to initialize.
 * Detects completion via status bar showing "Ready" or console message.
 * @param {import('@playwright/test').Page} page
 */
export async function waitForWasm(page) {
    await page.waitForFunction(() => {
        // Check if status bar shows "Ready"
        const statusMsg = document.getElementById('status-message');
        if (statusMsg && statusMsg.textContent === 'Ready') {
            return true;
        }
        // Also check for run button being enabled (indirect signal)
        const runBtn = document.getElementById('run-btn');
        return runBtn && !runBtn.disabled;
    }, { timeout: 30000 });
    // Give a small buffer for initialization
    await page.waitForTimeout(500);
}

/**
 * Wait for the scheduler to complete and show output.
 * Detects success via status bar or schedule-info element.
 * @param {import('@playwright/test').Page} page
 * @param {object} options
 * @param {number} [options.timeout=20000] - Timeout in ms
 */
export async function waitForSchedule(page, options = {}) {
    const { timeout = 20000 } = options;
    await page.waitForFunction(() => {
        // Check status message for success
        const statusMsg = document.getElementById('status-message');
        if (statusMsg && statusMsg.textContent.includes('Scheduled successfully')) {
            return true;
        }
        // Check schedule-info shows task count
        const scheduleInfo = document.getElementById('schedule-info');
        if (scheduleInfo && scheduleInfo.textContent.includes('tasks')) {
            return true;
        }
        // Also accept if we have an error (schedule completed with error)
        if (statusMsg && (statusMsg.textContent.toLowerCase().includes('error') ||
                         statusMsg.textContent.toLowerCase().includes('failed'))) {
            return true;
        }
        return false;
    }, { timeout });
    await page.waitForTimeout(300);
}

/**
 * Wait for Gantt SVG to render with task bars.
 * The Gantt is rendered inside an iframe within #gantt-output.
 * @param {import('@playwright/test').Page} page
 */
export async function waitForGantt(page) {
    // Wait for the iframe to be created
    await page.waitForSelector('#gantt-output iframe', { timeout: 10000 });
    // Give time for the iframe content to render
    await page.waitForTimeout(1000);
}

/**
 * Wait for Monaco editor to be ready.
 * @param {import('@playwright/test').Page} page
 */
export async function waitForEditor(page) {
    await page.waitForFunction(() => {
        const editor = document.querySelector('.monaco-editor');
        return editor !== null;
    }, { timeout: 10000 });
    await page.waitForTimeout(200);
}

/**
 * Type project code into the Monaco editor.
 * Uses Monaco's API directly for reliability.
 * @param {import('@playwright/test').Page} page
 * @param {string} code - The project code to type
 */
export async function typeProject(page, code) {
    // Use Monaco's API to set the value directly
    await page.evaluate((code) => {
        const models = window.monaco?.editor?.getModels();
        if (models && models.length > 0) {
            models[0].setValue(code);
        }
    }, code);
    // Small delay to ensure the editor updates
    await page.waitForTimeout(200);
}

/**
 * Clear the editor content.
 * @param {import('@playwright/test').Page} page
 */
export async function clearEditor(page) {
    await page.evaluate(() => {
        const models = window.monaco?.editor?.getModels();
        if (models && models.length > 0) {
            models[0].setValue('');
        }
    });
    await page.waitForTimeout(100);
}

/**
 * Click the Schedule/Run button.
 * @param {import('@playwright/test').Page} page
 */
export async function clickSchedule(page) {
    await page.click('#run-btn');
}

/**
 * Click the Gantt tab.
 * @param {import('@playwright/test').Page} page
 */
export async function clickGanttTab(page) {
    await page.click('[data-tab="gantt"]');
}

/**
 * Get the JSON output text content.
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<string>}
 */
export async function getJsonOutput(page) {
    return await page.locator('#json-output').textContent();
}

/**
 * Get the status message text.
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<string>}
 */
export async function getStatusMessage(page) {
    return await page.locator('#status-message').textContent();
}

/**
 * Get the schedule info text.
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<string>}
 */
export async function getScheduleInfo(page) {
    return await page.locator('#schedule-info').textContent();
}

// Demo recording helpers

/**
 * Inject the subtitle overlay element for demo recording.
 * @param {import('@playwright/test').Page} page
 */
export async function injectSubtitleOverlay(page) {
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

/**
 * Show a subtitle on screen.
 * @param {import('@playwright/test').Page} page
 * @param {string} text - The subtitle text
 * @param {number} [duration=2000] - How long to show in ms
 */
export async function showSubtitle(page, text, duration = 2000) {
    await page.evaluate((text) => {
        const el = document.getElementById('demo-subtitle');
        if (el) {
            el.style.opacity = '1';
            el.textContent = text;
        }
    }, text);
    await page.waitForTimeout(duration);
}

/**
 * Hide the subtitle overlay.
 * @param {import('@playwright/test').Page} page
 */
export async function hideSubtitle(page) {
    await page.evaluate(() => {
        const el = document.getElementById('demo-subtitle');
        if (el) el.style.opacity = '0';
    });
    await page.waitForTimeout(300);
}

// Sample project code for tests

export const SAMPLE_PROJECTS = {
    simple: `project "Test Project" {
    start: 2025-02-01
}

resource dev "Developer" {
    rate: 850/day
}

task design "Design" {
    effort: 5d
    assign: dev
}

task build "Build" {
    effort: 10d
    assign: dev
    depends: design
}

milestone launch "Launch" {
    depends: build
}
`,

    withCalendar: `project "Calendar Test" {
    start: 2025-02-01
}

calendar "standard" {
    working_days: mon-fri
    working_hours: 09:00-17:00
}

resource dev "Developer" {
    rate: 850/day
    calendar: standard
}

task work "Work Task" {
    effort: 10d
    assign: dev
}
`,

    hierarchical: `project "Hierarchical Test" {
    start: 2025-02-01
}

resource dev "Developer" {
    rate: 850/day
}

task phase1 "Phase 1" {
    task design "Design" {
        effort: 3d
        assign: dev
    }
    task review "Review" {
        effort: 2d
        assign: dev
        depends: design
    }
}

task phase2 "Phase 2" {
    depends: phase1
    task implement "Implement" {
        effort: 5d
        assign: dev
    }
}
`,
};
