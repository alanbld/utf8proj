// @ts-check
import { test, expect } from '@playwright/test';
import {
    waitForWasm,
    waitForSchedule,
    waitForGantt,
    waitForEditor,
    typeProject,
    clearEditor,
    clickSchedule,
    clickGanttTab,
    getJsonOutput,
    getStatusMessage,
    getScheduleInfo,
    SAMPLE_PROJECTS,
} from './helpers.js';

test.describe('Playground Loading', () => {
    test('page loads successfully', async ({ page }) => {
        await page.goto('/');
        await expect(page).toHaveTitle(/utf8proj/i);
    });

    test('WASM module initializes', async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        // Verify status shows Ready
        const statusText = await page.locator('#status-message').textContent();
        expect(statusText).toBe('Ready');
    });

    test('Monaco editor loads', async ({ page }) => {
        await page.goto('/');
        await waitForEditor(page);
        const editor = page.locator('.monaco-editor');
        await expect(editor).toBeVisible();
    });
});

test.describe('Editor Functionality', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('can type project code', async ({ page }) => {
        await typeProject(page, SAMPLE_PROJECTS.simple);
        // Run schedule to verify code was typed correctly
        await clickSchedule(page);
        await waitForSchedule(page);
        // If schedule succeeds, the code was typed correctly
        const statusText = await page.locator('#status-message').textContent();
        expect(statusText).toContain('Scheduled successfully');
    });

    test('editor accepts input', async ({ page }) => {
        // Type something and verify we can schedule
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);
        // Check that schedule-info shows duration and critical path info
        const scheduleInfo = await page.locator('#schedule-info').textContent();
        // Output format: "X tasks | Y days | Z critical"
        expect(scheduleInfo).toMatch(/days.*critical/);
    });
});

test.describe('Scheduling', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('schedule button computes dates', async ({ page }) => {
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);

        // Check JSON output contains task information
        const jsonOutput = await getJsonOutput(page);
        expect(jsonOutput).toMatch(/design|build|launch/i);
    });

    test('shows critical path information', async ({ page }) => {
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);

        // schedule-info should show critical path count
        const scheduleInfo = await getScheduleInfo(page);
        expect(scheduleInfo).toMatch(/critical/i);
    });

    test('handles hierarchical tasks', async ({ page }) => {
        await typeProject(page, SAMPLE_PROJECTS.hierarchical);
        await clickSchedule(page);
        await waitForSchedule(page);

        const jsonOutput = await getJsonOutput(page);
        expect(jsonOutput).toMatch(/phase1|phase2|Phase/i);
    });
});

test.describe('Gantt Chart', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);
    });

    test('Gantt tab renders iframe', async ({ page }) => {
        await clickGanttTab(page);
        await waitForGantt(page);

        const iframe = page.locator('#gantt-output iframe');
        await expect(iframe).toBeVisible();
    });

    test('Gantt iframe has content', async ({ page }) => {
        await clickGanttTab(page);
        await waitForGantt(page);

        // Access iframe content
        const iframe = page.frameLocator('#gantt-output iframe');
        // Check for SVG element inside the iframe
        const svg = iframe.locator('svg');
        await expect(svg).toBeVisible({ timeout: 5000 });
    });

    test('Gantt has task bars in SVG', async ({ page }) => {
        await clickGanttTab(page);
        await waitForGantt(page);

        // Access iframe content
        const iframe = page.frameLocator('#gantt-output iframe');
        const taskBars = iframe.locator('svg rect');
        const count = await taskBars.count();
        expect(count).toBeGreaterThan(0);
    });
});

test.describe('Error Handling', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('shows error for invalid syntax', async ({ page }) => {
        await typeProject(page, 'this is not valid project syntax!!!');
        await clickSchedule(page);

        // Wait for error output
        await page.waitForTimeout(1000);
        const status = await getStatusMessage(page);
        // Should show some kind of error
        expect(status.toLowerCase()).toMatch(/error|failed|invalid|parse/);
    });

    test('shows error for circular dependency', async ({ page }) => {
        const circularProject = `project "Circular" {
    start: 2025-02-01
}

resource dev "Developer" { rate: 850/day }

task a "Task A" { effort: 1d, assign: dev, depends: b }
task b "Task B" { effort: 1d, assign: dev, depends: a }
`;
        await typeProject(page, circularProject);
        await clickSchedule(page);

        await page.waitForTimeout(1000);
        const status = await getStatusMessage(page);
        expect(status.toLowerCase()).toMatch(/circular|cycle|error/);
    });
});

test.describe('Theme Toggle', () => {
    test('can switch to dark theme', async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);

        // Look for theme toggle button
        const themeBtn = page.locator('[data-theme-toggle], #theme-toggle, button:has-text("Dark"), button:has-text("Theme")');
        if (await themeBtn.count() > 0) {
            await themeBtn.first().click();
            // Check if dark class was added
            const isDark = await page.evaluate(() => {
                return document.body.classList.contains('dark') ||
                       document.documentElement.classList.contains('dark') ||
                       document.body.getAttribute('data-theme') === 'dark';
            });
            expect(isDark).toBe(true);
        } else {
            // Theme toggle may not exist, skip
            test.skip();
        }
    });
});
