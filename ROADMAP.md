# utf8proj Roadmap

**Last Updated:** 2026-01-30
**Current Version:** 0.15.1

---

## Completed Features

All original v1.0 roadmap items have been implemented:

| Feature | Status | Notes |
|---------|--------|-------|
| Interactive Gantt Chart (HTML/SVG) | **Done** | Zoom, tooltips, dependency arrows, dark theme, now line |
| WASM Playground | **Done** | https://alanbld.github.io/utf8proj/ |
| Resource Leveling | **Done** | RFC-0003, RFC-0014 (hybrid BDD) |
| Progress-Aware Scheduling | **Done** | RFC-0008, complete%, remaining duration |
| Baseline Management | **Done** | RFC-0013: save, list, compare |
| Excel Export | **Done** | RFC-0018: progress tracking, visual formatting |
| Now Line Rendering | **Done** | RFC-0017: status date marker on Gantt |
| Multiple Render Formats | **Done** | HTML, SVG, MermaidJS, PlantUML, Excel |
| Focus View | **Done** | RFC-0006: pattern-based filtering |
| LSP Support | **Done** | Diagnostics, hover, go-to-definition |

---

## Next Steps (Prioritized)

### 1. Project Status Dashboard (Recommended Next)

**Command:** `utf8proj status project.proj`

**Why This Matters for PMs:**
- Quick "How are we doing?" answer without parsing full schedule
- At-a-glance health metrics: overall progress, variance, critical path status
- Highlights issues: late tasks, blocked resources, at-risk milestones

**Proposed Output:**
```
╔══════════════════════════════════════════════════════╗
║  Project: CRM Migration                              ║
║  Status Date: 2026-01-30                             ║
╠══════════════════════════════════════════════════════╣
║  Overall Progress:  62%  ████████████░░░░░░░░        ║
║  Schedule Variance: +3 days (ahead)                  ║
║  Critical Path:     12 tasks, 45 days remaining      ║
╠══════════════════════════════════════════════════════╣
║  ⚠ 2 tasks behind schedule                          ║
║  ✓ 8 tasks completed this week                      ║
║  → 5 tasks starting next week                        ║
╚══════════════════════════════════════════════════════╝
```

**Complexity:** Low (data already computed, just needs formatting)
**Impact:** High (daily PM workflow)

---

### 2. CLI Progress Update

**Command:** `utf8proj progress --task=api_impl --complete=75`

**Why This Matters:**
- Update task progress without editing .proj files
- Batch updates via CSV import
- Integrates with CI/CD pipelines

**Examples:**
```bash
# Single task update
utf8proj progress project.proj --task=backend_api --complete=75

# With actual dates
utf8proj progress project.proj --task=frontend --complete=100 \
    --actual-start=2026-01-15 --actual-finish=2026-01-28

# Batch import
utf8proj progress project.proj --import=weekly_status.csv
```

**Complexity:** Medium (requires file modification logic)
**Impact:** High (reduces manual editing errors)

---

### 3. Extended Task Status

**Current:** `complete: 75%`

**Proposed:**
```proj
task api_impl "API Implementation" {
    duration: 10d
    status: blocked          # or: on_hold, at_risk, cancelled
    status_reason: "Waiting for security review"
    status_since: 2026-01-25
}
```

**Status Types:**
- `not_started` (default)
- `in_progress` (has actual_start, no actual_finish)
- `complete` (100% or has actual_finish)
- `blocked { reason, since }` - External dependency
- `on_hold { reason }` - Paused intentionally
- `at_risk { reason }` - May miss deadline
- `cancelled { reason }` - Removed from scope

**Complexity:** Medium (parser + domain model changes)
**Impact:** Medium (better project visibility)

---

### 4. Schedule Playback

**Command:** `utf8proj playback project.proj -o evolution.html`

**Why This Matters:**
- Visualize how the schedule evolved over time
- Answer "What changed since last week?"
- Stakeholder communication tool

**Features:**
- Timeline slider showing schedule at each baseline
- Highlight changes: tasks added/removed/delayed
- Impact metrics: duration change, critical path shifts

**Complexity:** High (requires multiple baselines, animation)
**Impact:** Medium (retrospectives, stakeholder reports)

---

### 5. Forecast Report

**Command:** `utf8proj forecast project.proj --baseline=original`

**Output:**
```
Forecast Report (vs baseline: original)
═══════════════════════════════════════

Projected Completion: 2026-05-15 (was 2026-04-30)
Schedule Variance:    +15 days
Cost Variance:        +€12,500

Top Delays:
  1. backend_api: +8 days (resource conflict)
  2. security_review: +5 days (external dependency)
  3. testing: +2 days (scope increase)

Recommendations:
  • Add resource to backend_api to recover 4 days
  • Escalate security_review with vendor
```

**Complexity:** Medium (builds on existing baseline compare)
**Impact:** High (proactive risk management)

---

## Post-v1.0 Features (Deferred)

| Feature | Notes |
|---------|-------|
| VS Code Extension | Syntax highlighting, diagnostics, preview |
| GitHub Action | CI/CD integration for schedule validation |
| EVM (Earned Value) | CPI, SPI, EAC, VAC metrics |
| Plugin System | Custom renderers, importers |
| Web UI | Browser-based editing with collaboration |
| AI Forecasting | ML-based completion predictions |

---

## Recommendation for Project Managers

**Start with: `utf8proj status`**

This command provides the highest value for the lowest implementation cost:

1. **Daily standup answer**: "Where are we?" in 2 seconds
2. **No workflow change**: Works with existing .proj files
3. **Builds foundation**: Status dashboard reuses schedule analysis already implemented
4. **Quick win**: Can be implemented and shipped in a few days

The existing `utf8proj schedule` and `utf8proj compare --baseline` commands provide detailed data, but PMs need a quick summary view for daily use. The status command fills this gap.

---

## Implementation Dependencies

```
utf8proj status (no deps, use existing schedule data)
    ↓
utf8proj progress (needs status to show result)
    ↓
Extended TaskStatus (enhances progress command)
    ↓
utf8proj forecast (combines status + baseline)
    ↓
utf8proj playback (needs multiple baselines)
```
