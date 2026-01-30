# utf8proj Roadmap

**Last Updated:** 2026-01-30
**Current Version:** 0.16.0

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
| Project Status Dashboard | **Done** | RFC-0019: text/JSON output, WASM, Excel sheet |

---

## Next Steps (Prioritized)

### 1. CLI Progress Update

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

### 2. Extended Task Status

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

### 3. Schedule Playback

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

### 4. Forecast Report

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

**Use `utf8proj status` for daily standups:**

```bash
utf8proj status project.proj                 # Text dashboard
utf8proj status project.proj --format json   # JSON for automation
utf8proj status project.proj --as-of DATE    # Historical view
```

The status command provides:
1. **Daily standup answer**: "Where are we?" in 2 seconds
2. **Progress metrics**: Overall %, variance, earned value (SPI)
3. **Task breakdown**: Complete, in-progress, not-started, behind counts
4. **Critical path**: Number of tasks and days remaining

For Excel reports with status dashboard, use:
```bash
utf8proj gantt project.proj -o report.xlsx -f xlsx --include-status
```

---

## Implementation Dependencies

```
utf8proj status ✓ DONE (RFC-0019)
    ↓
utf8proj progress (needs status to show result)
    ↓
Extended TaskStatus (enhances progress command)
    ↓
utf8proj forecast (combines status + baseline)
    ↓
utf8proj playback (needs multiple baselines)
```
