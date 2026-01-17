// @ts-check
/**
 * Features Demo - utf8proj Playground
 *
 * This demo showcases advanced features:
 * 1. Resource leveling
 * 2. Progress tracking
 * 3. Focus view
 * 4. Multiple export formats
 * 5. Share functionality
 *
 * Run with: RECORD_DEMO=true npm run demo:features
 */

import { test } from '@playwright/test';
import {
    waitForWasm,
    waitForSchedule,
    waitForGantt,
    waitForEditor,
    clickSchedule,
    clickGanttTab,
    injectSubtitleOverlay,
    showSubtitle,
    hideSubtitle,
} from './helpers.js';

// Skip this test unless RECORD_DEMO=true
const skipDemo = process.env.RECORD_DEMO !== 'true';

const LEVELING_PROJECT = `project "Software Release" {
    start: 2025-02-01
}

resource dev "Developer" {
    rate: 850/day
    capacity: 1.0
}

task feature1 "Feature 1" {
    effort: 5d
    assign: dev
}

task feature2 "Feature 2" {
    effort: 5d
    assign: dev
}

task feature3 "Feature 3" {
    effort: 5d
    assign: dev
}

task testing "Testing" {
    effort: 3d
    assign: dev
    depends: feature1, feature2, feature3
}

milestone release "Release" {
    depends: testing
}
`;

const PROGRESS_PROJECT = `project "Migration Project" {
    start: 2025-01-06
    status_date: 2025-01-20
}

resource dev "Developer" {
    rate: 850/day
}

task planning "Planning" {
    effort: 5d
    assign: dev
    complete: 100%
}

task design "Design" {
    effort: 10d
    assign: dev
    depends: planning
    complete: 60%
}

task implementation "Implementation" {
    effort: 15d
    assign: dev
    depends: design
}

milestone golive "Go Live" {
    depends: implementation
}
`;

(skipDemo ? test.skip : test)('Resource Leveling Demo', async ({ page }) => {
    await page.goto('/');
    await waitForWasm(page);
    await waitForEditor(page);
    await injectSubtitleOverlay(page);

    // SCENE 1: Introduction
    await showSubtitle(page, 'Resource Leveling: Resolving Over-allocation', 3000);
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // SCENE 2: Load project with parallel tasks
    await showSubtitle(page, 'Three tasks assigned to one developer', 2500);

    await page.evaluate((code) => {
        const models = window.monaco?.editor?.getModels();
        if (models && models.length > 0) {
            models[0].setValue(code);
        }
    }, LEVELING_PROJECT);

    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // SCENE 3: Schedule without leveling
    await showSubtitle(page, 'Without leveling: tasks overlap (over-allocated)', 2500);
    await clickSchedule(page);
    await waitForSchedule(page);
    await clickGanttTab(page);
    await waitForGantt(page);
    await hideSubtitle(page);
    await page.waitForTimeout(2000);

    // SCENE 4: Enable resource leveling
    await showSubtitle(page, 'Enable Resource Leveling checkbox', 2000);
    await page.locator('#leveling-checkbox').check();
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // SCENE 5: Re-schedule with leveling
    await showSubtitle(page, 'Tasks are now sequenced correctly', 2500);
    await clickSchedule(page);
    await waitForSchedule(page);
    await page.waitForTimeout(500);
    await hideSubtitle(page);
    await page.waitForTimeout(2000);

    // SCENE 6: Explain the result
    await showSubtitle(page, 'Developer works on one task at a time', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1500);

    await showSubtitle(page, 'L001 diagnostics explain each shift', 2000);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);
});

(skipDemo ? test.skip : test)('Progress Tracking Demo', async ({ page }) => {
    await page.goto('/');
    await waitForWasm(page);
    await waitForEditor(page);
    await injectSubtitleOverlay(page);

    // SCENE 1: Introduction
    await showSubtitle(page, 'Progress-Aware Scheduling (RFC-0008)', 3000);
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // SCENE 2: Load project with progress
    await showSubtitle(page, 'Track task completion with complete: X%', 2500);

    await page.evaluate((code) => {
        const models = window.monaco?.editor?.getModels();
        if (models && models.length > 0) {
            models[0].setValue(code);
        }
    }, PROGRESS_PROJECT);

    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // SCENE 3: Highlight status_date
    await showSubtitle(page, 'status_date defines the reporting date', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // SCENE 4: Schedule
    await showSubtitle(page, 'Scheduling respects completion status', 2000);
    await clickSchedule(page);
    await waitForSchedule(page);
    await clickGanttTab(page);
    await waitForGantt(page);
    await hideSubtitle(page);
    await page.waitForTimeout(2000);

    // SCENE 5: Explain completed tasks
    await showSubtitle(page, 'Completed tasks are locked to actual dates', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1500);

    // SCENE 6: Explain in-progress tasks
    await showSubtitle(page, 'In-progress tasks forecast remaining work', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1500);

    // SCENE 7: Finale
    await showSubtitle(page, 'Earned Value metrics: SPI, variance detection', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);
});

(skipDemo ? test.skip : test)('Export and Share Demo', async ({ page }) => {
    await page.goto('/');
    await waitForWasm(page);
    await waitForEditor(page);
    await injectSubtitleOverlay(page);

    // SCENE 1: Introduction
    await showSubtitle(page, 'Export and Share Your Projects', 3000);
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // SCENE 2: Load and schedule a project
    await page.selectOption('#example-select', 'native');
    await page.waitForTimeout(500);
    await clickSchedule(page);
    await waitForSchedule(page);

    // SCENE 3: Show export options
    await showSubtitle(page, 'Multiple export formats available', 2000);
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // Show HTML option
    await page.selectOption('#export-format-select', 'html');
    await showSubtitle(page, 'HTML: Standalone interactive Gantt', 2000);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // Show Excel option
    await page.selectOption('#export-format-select', 'xlsx');
    await showSubtitle(page, 'Excel: Formula-driven costing reports', 2000);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // Show Mermaid option
    await page.selectOption('#export-format-select', 'mermaid');
    await showSubtitle(page, 'Mermaid: For Markdown documentation', 2000);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // SCENE 4: Share functionality
    await showSubtitle(page, 'Share projects via URL', 2000);
    await page.click('#share-btn');
    await page.waitForTimeout(500);
    await hideSubtitle(page);

    await showSubtitle(page, 'URL contains compressed project data', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1500);

    // Close modal
    await page.click('#share-modal-close');
    await page.waitForTimeout(500);

    // SCENE 5: Finale
    await showSubtitle(page, 'Git-friendly text files, anywhere you need them', 3000);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);
});

(skipDemo ? test.skip : test)('Focus View Demo', async ({ page }) => {
    await page.goto('/');
    await waitForWasm(page);
    await waitForEditor(page);
    await injectSubtitleOverlay(page);

    // SCENE 1: Introduction - Large project problem
    await showSubtitle(page, 'Focus View: Cut Through the Noise (RFC-0006)', 3000);
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // SCENE 2: Load large project
    await showSubtitle(page, 'Large project with 20+ tasks across 4 streams', 2500);
    await page.selectOption('#example-select', 'focus');
    await page.waitForTimeout(500);
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // SCENE 3: Show overwhelming Gantt
    await clickSchedule(page);
    await waitForSchedule(page);
    await clickGanttTab(page);
    await waitForGantt(page);

    await showSubtitle(page, 'Full Gantt: hard to find what you need', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1500);

    // SCENE 4: Apply focus filter
    await showSubtitle(page, 'Enter "backend" in the Focus field', 2000);
    await page.fill('#focus-input', 'backend');
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // SCENE 5: Re-schedule with focus
    await clickSchedule(page);
    await waitForSchedule(page);
    await page.waitForTimeout(500);

    await showSubtitle(page, 'Now showing only backend-related tasks', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(2000);

    // SCENE 6: Finale
    await showSubtitle(page, 'Focus on what matters, hide the rest', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);
});
