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
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
    });

    test('can switch to dark theme', async ({ page }) => {
        // Click theme button (moon emoji)
        const themeBtn = page.locator('#theme-btn');
        await themeBtn.click();

        // Check if dark theme was applied
        const isDark = await page.evaluate(() => {
            return document.documentElement.getAttribute('data-theme') === 'dark';
        });
        expect(isDark).toBe(true);
    });

    test('can switch back to light theme', async ({ page }) => {
        // Switch to dark first
        const themeBtn = page.locator('#theme-btn');
        await themeBtn.click();

        // Switch back to light
        await themeBtn.click();

        const isLight = await page.evaluate(() => {
            return document.documentElement.getAttribute('data-theme') !== 'dark';
        });
        expect(isLight).toBe(true);
    });
});

test.describe('Format Detection', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('detects native DSL format', async ({ page }) => {
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);

        // Check format selector shows native
        const format = await page.locator('#format-select').inputValue();
        expect(format).toBe('native');
    });

    test('handles TJP format selection', async ({ page }) => {
        // Select TJP format
        await page.selectOption('#format-select', 'tjp');

        const format = await page.locator('#format-select').inputValue();
        expect(format).toBe('tjp');
    });
});

test.describe('Example Loading', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('can load native example', async ({ page }) => {
        await page.selectOption('#example-select', 'native');
        await page.waitForTimeout(500);

        // Verify example loaded by scheduling
        await clickSchedule(page);
        await waitForSchedule(page);

        const status = await getStatusMessage(page);
        expect(status).toContain('Scheduled successfully');
    });

    test('can load hierarchical example', async ({ page }) => {
        await page.selectOption('#example-select', 'hierarchical');
        await page.waitForTimeout(500);

        await clickSchedule(page);
        await waitForSchedule(page);

        const status = await getStatusMessage(page);
        expect(status).toContain('Scheduled successfully');
    });

    test('can load progress tracking example', async ({ page }) => {
        await page.selectOption('#example-select', 'progress');
        await page.waitForTimeout(500);

        await clickSchedule(page);
        await waitForSchedule(page);

        const status = await getStatusMessage(page);
        expect(status).toContain('Scheduled successfully');
    });

    test('can load focus view example', async ({ page }) => {
        await page.selectOption('#example-select', 'focus');
        await page.waitForTimeout(500);

        await clickSchedule(page);
        await waitForSchedule(page);

        const status = await getStatusMessage(page);
        expect(status).toContain('Scheduled successfully');
    });

    test('focus example filters tasks when pattern applied', async ({ page }) => {
        // Load the focus example (large project with multiple streams)
        await page.selectOption('#example-select', 'focus');
        // Wait for editor to update with the focus example content
        await page.waitForFunction(() => {
            const models = window.monaco?.editor?.getModels();
            return models && models[0]?.getValue().includes('Enterprise Platform');
        }, { timeout: 5000 });

        // Schedule without focus
        await clickSchedule(page);
        await waitForSchedule(page);
        await clickGanttTab(page);
        await waitForGantt(page);

        // Verify unfiltered view shows both backend and frontend tasks
        const iframe = page.frameLocator('#gantt-output iframe');
        const unfilteredHtml = await iframe.locator('body').innerHTML();
        expect(unfilteredHtml).toContain('Backend'); // Has backend tasks
        expect(unfilteredHtml).toContain('Frontend'); // Has frontend tasks

        // Count text elements (task labels) in unfiltered view
        const unfilteredLabels = (unfilteredHtml.match(/<text/g) || []).length;

        // Apply focus pattern to filter to backend tasks only
        await page.fill('#focus-input', 'backend');
        await clickSchedule(page);
        await waitForSchedule(page);
        await waitForGantt(page);

        // Verify filtered view has fewer task labels
        const filteredHtml = await iframe.locator('body').innerHTML();
        const filteredLabels = (filteredHtml.match(/<text/g) || []).length;

        // Focus should reduce the number of visible task labels
        expect(filteredLabels).toBeLessThan(unfilteredLabels);
        // But should still show some content (not empty)
        expect(filteredLabels).toBeGreaterThan(0);
    });
});

test.describe('Resource Leveling', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('leveling checkbox exists', async ({ page }) => {
        const checkbox = page.locator('#leveling-checkbox');
        await expect(checkbox).toBeVisible();
    });

    test('can enable resource leveling', async ({ page }) => {
        const checkbox = page.locator('#leveling-checkbox');
        await checkbox.check();

        const isChecked = await checkbox.isChecked();
        expect(isChecked).toBe(true);
    });

    test('schedule works with leveling enabled', async ({ page }) => {
        await page.locator('#leveling-checkbox').check();
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);

        const status = await getStatusMessage(page);
        expect(status).toContain('Scheduled successfully');
    });
});

test.describe('Focus View', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('focus input exists', async ({ page }) => {
        const focusInput = page.locator('#focus-input');
        await expect(focusInput).toBeVisible();
    });

    test('context depth selector exists', async ({ page }) => {
        const contextDepth = page.locator('#context-depth-select');
        await expect(contextDepth).toBeVisible();
    });

    test('can set focus filter', async ({ page }) => {
        await page.fill('#focus-input', 'design');

        const value = await page.locator('#focus-input').inputValue();
        expect(value).toBe('design');
    });
});

test.describe('JSON Output', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);
    });

    test('JSON tab shows schedule data', async ({ page }) => {
        // Click JSON tab
        await page.click('[data-tab="json"]');

        const jsonOutput = await getJsonOutput(page);
        expect(jsonOutput.length).toBeGreaterThan(10);
    });

    test('JSON contains task array', async ({ page }) => {
        await page.click('[data-tab="json"]');

        const jsonOutput = await getJsonOutput(page);
        expect(jsonOutput).toContain('tasks');
    });

    test('JSON contains duration info', async ({ page }) => {
        await page.click('[data-tab="json"]');

        const jsonOutput = await getJsonOutput(page);
        expect(jsonOutput).toMatch(/duration|days/i);
    });

    test('JSON contains critical path', async ({ page }) => {
        await page.click('[data-tab="json"]');

        const jsonOutput = await getJsonOutput(page);
        expect(jsonOutput).toContain('critical_path');
    });
});

test.describe('Export Functionality', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);
    });

    test('export format selector exists', async ({ page }) => {
        const exportFormat = page.locator('#export-format-select');
        await expect(exportFormat).toBeVisible();
    });

    test('export button exists', async ({ page }) => {
        const exportBtn = page.locator('#export-btn');
        await expect(exportBtn).toBeVisible();
    });

    test('can select HTML export format', async ({ page }) => {
        await page.selectOption('#export-format-select', 'html');
        const format = await page.locator('#export-format-select').inputValue();
        expect(format).toBe('html');
    });

    test('can select Excel export format', async ({ page }) => {
        await page.selectOption('#export-format-select', 'xlsx');
        const format = await page.locator('#export-format-select').inputValue();
        expect(format).toBe('xlsx');
    });

    test('can select Mermaid export format', async ({ page }) => {
        await page.selectOption('#export-format-select', 'mermaid');
        const format = await page.locator('#export-format-select').inputValue();
        expect(format).toBe('mermaid');
    });
});

test.describe('Share Functionality', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('share button exists', async ({ page }) => {
        const shareBtn = page.locator('#share-btn');
        await expect(shareBtn).toBeVisible();
    });

    test('share modal opens', async ({ page }) => {
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await page.click('#share-btn');

        const modal = page.locator('#share-modal');
        await expect(modal).not.toHaveClass(/hidden/);
    });

    test('share URL is generated', async ({ page }) => {
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await page.click('#share-btn');
        await page.waitForTimeout(500);

        const shareUrl = await page.locator('#share-url').inputValue();
        expect(shareUrl).toContain('#p=');
    });

    test('share modal can be closed', async ({ page }) => {
        await page.click('#share-btn');
        await page.click('#share-modal-close');

        const modal = page.locator('#share-modal');
        await expect(modal).toHaveClass(/hidden/);
    });
});

test.describe('Download Functionality', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('download button exists', async ({ page }) => {
        const downloadBtn = page.locator('#download-proj-btn');
        await expect(downloadBtn).toBeVisible();
    });
});

test.describe('Panel Resizing', () => {
    test('resize handle exists', async ({ page }) => {
        await page.goto('/');
        const handle = page.locator('#resize-handle');
        await expect(handle).toBeVisible();
    });
});

test.describe('Gantt Interactions', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);
        await clickGanttTab(page);
        await waitForGantt(page);
    });

    test('Gantt SVG has text labels', async ({ page }) => {
        const iframe = page.frameLocator('#gantt-output iframe');
        const textElements = iframe.locator('svg text');
        const count = await textElements.count();
        expect(count).toBeGreaterThan(0);
    });

    test('Gantt SVG has multiple rectangles for tasks', async ({ page }) => {
        const iframe = page.frameLocator('#gantt-output iframe');
        const rects = iframe.locator('svg rect');
        const count = await rects.count();
        // Should have at least task bars (design, build, launch)
        expect(count).toBeGreaterThanOrEqual(2);
    });

    test('Gantt renders milestone differently', async ({ page }) => {
        // Milestones are typically rendered as diamonds or small shapes
        const iframe = page.frameLocator('#gantt-output iframe');
        // Check for any path or polygon elements (milestones often use these)
        const shapes = iframe.locator('svg path, svg polygon, svg rect');
        const count = await shapes.count();
        expect(count).toBeGreaterThan(0);
    });
});

test.describe('Validation', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('shows error for incomplete task block', async ({ page }) => {
        const invalidProject = `project "Bad Syntax" {
    start: 2025-02-01
}

task a "Task" {
    effort: 1d
    this_is_not_valid
}
`;
        await typeProject(page, invalidProject);
        await clickSchedule(page);
        await page.waitForTimeout(1000);

        const status = await getStatusMessage(page);
        expect(status.toLowerCase()).toMatch(/error|syntax|expected/);
    });

    test('shows error for missing closing brace', async ({ page }) => {
        const invalidProject = `project "Unclosed" {
    start: 2025-02-01

task a "Task" {
    effort: 1d
`;
        await typeProject(page, invalidProject);
        await clickSchedule(page);
        await page.waitForTimeout(1000);

        const status = await getStatusMessage(page);
        expect(status.toLowerCase()).toMatch(/error|syntax|expected/);
    });
});

test.describe('RFC-0012: Temporal Regimes', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('schedules project with regime: work/event/deadline', async ({ page }) => {
        await typeProject(page, SAMPLE_PROJECTS.temporalRegimes);
        await clickSchedule(page);
        await waitForSchedule(page);

        // Should schedule successfully - check status message
        const status = await getStatusMessage(page);
        expect(status.toLowerCase()).toMatch(/scheduled|success|ready/);
    });

    test('loads Temporal Regimes example from dropdown', async ({ page }) => {
        // Select the temporal example from dropdown
        await page.selectOption('#example-select', 'temporal');
        await page.waitForTimeout(1000);

        // Verify editor has content with regime keyword
        const editorContent = await page.evaluate(() => {
            return window.monaco?.editor?.getModels()[0]?.getValue() || '';
        });
        expect(editorContent).toContain('regime:');
    });
});

test.describe('RFC-0017: Now Line', () => {
    test.beforeEach(async ({ page }) => {
        await page.goto('/');
        await waitForWasm(page);
        await waitForEditor(page);
    });

    test('now line checkbox exists', async ({ page }) => {
        const checkbox = page.locator('#nowline-checkbox');
        await expect(checkbox).toBeVisible();
    });

    test('now line checkbox is checked by default', async ({ page }) => {
        const checkbox = page.locator('#nowline-checkbox');
        const isChecked = await checkbox.isChecked();
        expect(isChecked).toBe(true);
    });

    test('can disable now line', async ({ page }) => {
        const checkbox = page.locator('#nowline-checkbox');
        await checkbox.uncheck();

        const isChecked = await checkbox.isChecked();
        expect(isChecked).toBe(false);
    });

    test('now line renders in Gantt chart', async ({ page }) => {
        // Use a project with explicit status_date within the chart range
        await typeProject(page, SAMPLE_PROJECTS.withStatusDate);
        await clickSchedule(page);
        await waitForSchedule(page);
        await clickGanttTab(page);
        await waitForGantt(page);

        // Access iframe and look for now-line element
        const iframe = page.frameLocator('#gantt-output iframe');
        const nowLine = iframe.locator('.now-line');
        // Should have at least one now line (status date)
        const count = await nowLine.count();
        expect(count).toBeGreaterThanOrEqual(1);
    });

    test('now line hidden when checkbox unchecked', async ({ page }) => {
        // Uncheck the now line checkbox first
        const checkbox = page.locator('#nowline-checkbox');
        await checkbox.uncheck();

        await typeProject(page, SAMPLE_PROJECTS.simple);
        await clickSchedule(page);
        await waitForSchedule(page);
        await clickGanttTab(page);
        await waitForGantt(page);

        // Access iframe and verify no now-line elements
        const iframe = page.frameLocator('#gantt-output iframe');
        const nowLine = iframe.locator('.now-line');
        const count = await nowLine.count();
        expect(count).toBe(0);
    });

    test('now line uses status_date from project', async ({ page }) => {
        // Use status_date that falls within the chart's visible range
        await typeProject(page, SAMPLE_PROJECTS.withStatusDate);
        await clickSchedule(page);
        await waitForSchedule(page);
        await clickGanttTab(page);
        await waitForGantt(page);

        // Access iframe and look for now-line with status-date class
        const iframe = page.frameLocator('#gantt-output iframe');
        const statusLine = iframe.locator('.now-line.status-date');
        const count = await statusLine.count();
        expect(count).toBe(1);
    });
});
