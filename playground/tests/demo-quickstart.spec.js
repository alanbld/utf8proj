// @ts-check
/**
 * Quick Start Demo - utf8proj Playground
 *
 * This demo shows the basic workflow:
 * 1. Load the playground
 * 2. Type a simple project
 * 3. Schedule it
 * 4. View the Gantt chart
 *
 * Run with: RECORD_DEMO=true npm run demo:quickstart
 */

import { test } from '@playwright/test';
import {
    waitForWasm,
    waitForSchedule,
    waitForGantt,
    waitForEditor,
    typeProject,
    clickSchedule,
    clickGanttTab,
    injectSubtitleOverlay,
    showSubtitle,
    hideSubtitle,
} from './helpers.js';

// Skip this test unless RECORD_DEMO=true
const skipDemo = process.env.RECORD_DEMO !== 'true';

const DEMO_PROJECT = `project "Website Redesign" {
    start: 2025-02-01
}

resource designer "UI Designer" {
    rate: 750/day
}

resource developer "Developer" {
    rate: 850/day
}

task design "Design Phase" {
    task wireframes "Wireframes" {
        effort: 3d
        assign: designer
    }
    task mockups "Mockups" {
        effort: 5d
        assign: designer
        depends: wireframes
    }
}

task development "Development" {
    depends: design

    task frontend "Frontend" {
        effort: 10d
        assign: developer
    }
    task backend "Backend" {
        effort: 8d
        assign: developer
    }
}

milestone launch "Launch" {
    depends: development
}
`;

(skipDemo ? test.skip : test)('Quick Start Demo', async ({ page }) => {
    // Setup - navigate and wait for initialization
    await page.goto('/');
    await waitForWasm(page);
    await waitForEditor(page);
    await injectSubtitleOverlay(page);

    // SCENE 1: Introduction (3s)
    await showSubtitle(page, 'utf8proj: Explainable Project Scheduling', 3000);
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    // SCENE 2: Explain the editor (2s)
    await showSubtitle(page, 'Write your project in plain text', 2000);
    await hideSubtitle(page);

    // SCENE 3: Type the project code (with visible typing effect)
    await showSubtitle(page, 'Define resources, tasks, and dependencies', 2500);

    // Clear default content and type our demo project
    await page.evaluate(() => {
        const models = window.monaco?.editor?.getModels();
        if (models && models.length > 0) {
            models[0].setValue('');
        }
    });
    await page.waitForTimeout(300);

    // Type the project line by line for visual effect
    const lines = DEMO_PROJECT.trim().split('\n');
    for (let i = 0; i < lines.length; i++) {
        await page.evaluate((line) => {
            const models = window.monaco?.editor?.getModels();
            if (models && models.length > 0) {
                const current = models[0].getValue();
                models[0].setValue(current + (current ? '\n' : '') + line);
            }
        }, lines[i]);
        // Faster for demo, but still visible
        await page.waitForTimeout(50);
    }

    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // SCENE 4: Click Schedule button
    await showSubtitle(page, 'Click "Run" to schedule the project', 2000);
    await hideSubtitle(page);
    await page.waitForTimeout(500);

    await clickSchedule(page);
    await waitForSchedule(page);

    await showSubtitle(page, 'CPM algorithm computes optimal dates', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // SCENE 5: View Gantt chart
    await showSubtitle(page, 'View the interactive Gantt chart', 2000);
    await clickGanttTab(page);
    await waitForGantt(page);
    await hideSubtitle(page);
    await page.waitForTimeout(2000);

    // SCENE 6: Highlight features
    await showSubtitle(page, 'Critical path highlighted in red', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1500);

    await showSubtitle(page, 'Hover for task details', 2000);
    await hideSubtitle(page);
    await page.waitForTimeout(1500);

    // SCENE 7: Mention export options
    await showSubtitle(page, 'Export to HTML, Excel, Mermaid, or PlantUML', 2500);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);

    // SCENE 8: Finale
    await showSubtitle(page, 'Try it yourself at alanbld.github.io/utf8proj', 4000);
    await hideSubtitle(page);
    await page.waitForTimeout(1000);
});
