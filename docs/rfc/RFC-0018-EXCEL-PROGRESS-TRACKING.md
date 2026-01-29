# RFC-0018: Excel Progress Tracking

**RFC Number:** 0018
**Status:** Accepted (Phase 1-4 Implemented)
**Created:** 2026-01-29
**Related:** RFC-0008 (Progress-Aware CPM), RFC-0017 (Now Line), RFC-0009 (Excel Auto-Fit)

---

## Summary

Add progress tracking visualization to Excel exports, including completion percentage, remaining duration, actual dates, and visual progress bars in the timeline. This RFC defines a `ProgressMode` configuration option that controls how progress data is rendered.

## Motivation

Project managers using Excel exports need to:
1. **Track actual vs planned**: See which tasks are complete, in-progress, or not started
2. **Identify slippage**: Quickly spot tasks behind schedule relative to status date
3. **Report to stakeholders**: Share progress snapshots in familiar spreadsheet format
4. **Update forecasts**: Adjust remaining work estimates based on actual progress

Currently, the Excel export shows only the scheduled plan with no visibility into:
- Task completion status
- Remaining work
- Actual start/finish dates
- Progress relative to status date

## Design

### Progress Mode Configuration

```rust
/// How to render progress information in Excel
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ProgressMode {
    /// No progress columns or visualization (current behavior)
    #[default]
    None,
    /// Add progress columns only (Complete %, Remaining, Actuals)
    Columns,
    /// Add progress columns + visual progress bars in timeline
    Visual,
    /// Full progress view with status date marker and variance analysis
    Full,
}
```

### CLI Interface

```bash
# Default: no progress (backwards compatible)
utf8proj gantt project.proj -o report.xlsx -f xlsx

# Add progress columns
utf8proj gantt project.proj -o report.xlsx -f xlsx --progress=columns

# Visual progress bars in timeline
utf8proj gantt project.proj -o report.xlsx -f xlsx --progress=visual

# Full progress view with status date and variance
utf8proj gantt project.proj -o report.xlsx -f xlsx --progress=full

# Combine with status date override
utf8proj gantt project.proj -o report.xlsx -f xlsx --progress=full --as-of 2026-01-20
```

### Rust API

```rust
impl ExcelConfig {
    /// Set progress visualization mode
    pub fn with_progress_mode(mut self, mode: ProgressMode) -> Self {
        self.progress_mode = mode;
        self
    }
}

// Usage
let config = ExcelConfig::new()
    .with_progress_mode(ProgressMode::Visual)
    .with_status_date(NaiveDate::from_ymd_opt(2026, 1, 20).unwrap());
```

### Column Layout by Progress Mode

#### `ProgressMode::None` (Default - Current Behavior)
```
| Task ID | Activity | Lvl | M | Profile | Depends On | Type | Lag | Effort | Start | End | W1 | W2 | ...
```

#### `ProgressMode::Columns`
```
| Task ID | Activity | Lvl | M | Complete | Remaining | Actual Start | Actual End | Profile | Depends On | Type | Lag | Effort | Start | End | W1 | W2 | ...
```

New columns:
| Column | Type | Description |
|--------|------|-------------|
| Complete | Percentage | `75%` - Task completion |
| Remaining | Duration | `3d` - Remaining work |
| Actual Start | Date | Actual start (if started) |
| Actual End | Date | Actual finish (if complete) |

#### `ProgressMode::Visual`

Same columns as `Columns`, plus:
- Timeline cells show **split progress bars**:
  - Completed portion: Solid fill (green)
  - Remaining portion: Striped/lighter fill (original color)
  - Past status date + incomplete: Red highlight

```
Timeline visualization:
                    Status Date
                         ↓
Week:    W1    W2    W3  │ W4    W5    W6
Task A:  ████  ████  ██  │              (100% complete)
Task B:  ████  ████  ▒▒  │ ▒▒▒▒  ▒▒    (40% complete, behind)
Task C:            ░░░░  │ ░░░░  ░░░░  (0% complete, not started)

Legend:
  ████ = Completed work
  ▒▒▒▒ = Remaining work (behind schedule - past status date)
  ░░░░ = Remaining work (on schedule - future)
```

#### `ProgressMode::Full`

Same as `Visual`, plus:
- **Status Date Column**: Highlighted vertical column at status date
- **Variance Columns**: Schedule variance (days early/late)
- **Status Column**: Visual indicator (✓ Complete, ● In Progress, ○ Not Started, ⚠ Behind)

```
| Task ID | Activity | Status | Complete | Remaining | Variance | Actual Start | Actual End | ... | SD | W4 | W5 | ...
                                                                                                    ↑
                                                                                          Status Date column
```

Additional columns in Full mode:
| Column | Type | Description |
|--------|------|-------------|
| Status | Icon | ✓ ● ○ ⚠ - Visual status indicator |
| Variance | Days | `+2d` (ahead) or `-3d` (behind) |
| SD | Marker | Status date column (highlighted) |

### Conditional Formatting Rules

#### Task Status Colors (Full mode)
| Status | Condition | Row Color |
|--------|-----------|-----------|
| Complete | `complete = 100%` | Light green background |
| On Track | `complete > 0% AND remaining fits before deadline` | No highlight |
| Behind | `complete < expected% as of status_date` | Light red background |
| Not Started | `complete = 0% AND start > status_date` | No highlight |
| Overdue | `complete = 0% AND start <= status_date` | Orange background |

#### Timeline Cell Colors
| Cell State | Fill Color | Pattern |
|------------|------------|---------|
| Completed work | Green (#C6EFCE) | Solid |
| Remaining (on schedule) | Task color | Solid |
| Remaining (behind) | Red (#FFC7CE) | Diagonal stripe |
| Status date column | Yellow (#FFEB9C) | Solid (header highlighted) |

### Progress Bar Implementation

For weekly granularity, each cell represents task presence in that week:
- **Empty**: Task not scheduled in this week
- **Filled**: Task scheduled (current behavior)
- **Split**: Partial completion visualization

Split calculation for a week cell:
```rust
fn calculate_week_progress(
    task_start: NaiveDate,
    task_end: NaiveDate,
    week_start: NaiveDate,
    week_end: NaiveDate,
    complete_percent: u8,
    status_date: NaiveDate,
) -> CellProgress {
    // Calculate what portion of this week's work is complete
    let task_days_in_week = overlap_days(task_start, task_end, week_start, week_end);
    if task_days_in_week == 0 {
        return CellProgress::Empty;
    }

    // Estimate completed days based on linear progress
    let total_duration = (task_end - task_start).num_days() as f64;
    let completed_duration = total_duration * (complete_percent as f64 / 100.0);
    let completed_end = task_start + Duration::days(completed_duration as i64);

    let completed_in_week = overlap_days(task_start, completed_end, week_start, week_end);
    let remaining_in_week = task_days_in_week - completed_in_week;

    let is_behind = week_end <= status_date && remaining_in_week > 0;

    CellProgress::Split {
        completed: completed_in_week,
        remaining: remaining_in_week,
        behind_schedule: is_behind,
    }
}
```

### Data Sources

Progress data comes from scheduled tasks:

```rust
pub struct ScheduledTask {
    // Existing fields...
    pub start: NaiveDate,
    pub finish: NaiveDate,

    // Progress fields (from RFC-0008)
    pub percent_complete: u8,           // 0-100
    pub remaining_duration: Duration,   // Calculated or explicit
    pub actual_start: Option<NaiveDate>,
    pub actual_finish: Option<NaiveDate>,
}
```

### WASM Playground Integration

The playground exposes all Excel configuration options through a unified API and UI panel.

#### WASM API

```rust
/// Complete Excel configuration passed from JavaScript
#[derive(Deserialize)]
pub struct ExcelExportConfig {
    // Timescale
    pub scale: String,           // "weekly" | "daily"
    pub auto_fit: bool,          // Auto-fit timeframe to project
    pub weeks: Option<u32>,      // Manual week count (if !auto_fit)
    pub days: Option<u32>,       // Manual day count (if !auto_fit && scale=daily)

    // Formatting
    pub currency: String,        // "EUR", "USD", "GBP", etc.
    pub hours_per_day: f64,      // Working hours (default: 8.0)

    // Content
    pub include_summary: bool,   // Include executive summary sheet
    pub show_dependencies: bool, // Show dependency columns

    // Progress (RFC-0018)
    pub progress_mode: String,   // "none" | "columns" | "visual" | "full"
}

#[wasm_bindgen]
impl Playground {
    /// Render Excel with full configuration
    #[wasm_bindgen]
    pub fn render_xlsx_configured(&self, config_json: &str) -> Vec<u8> {
        let config: ExcelExportConfig = serde_json::from_str(config_json)
            .unwrap_or_default();

        let progress_mode = match config.progress_mode.as_str() {
            "columns" => ProgressMode::Columns,
            "visual" => ProgressMode::Visual,
            "full" => ProgressMode::Full,
            _ => ProgressMode::None,
        };

        let mut excel_config = ExcelConfig {
            scale: config.scale,
            currency: config.currency,
            auto_fit: config.auto_fit,
            weeks: config.weeks.unwrap_or(20),
            days: config.days.unwrap_or(60),
            hours_per_day: config.hours_per_day,
            include_summary: config.include_summary,
            show_dependencies: config.show_dependencies,
            progress_mode,
        };

        // Use project status_date for progress calculations
        if let Some(project) = &self.project {
            if let Some(status_date) = project.status_date {
                excel_config.status_date = Some(status_date);
            }
        }

        let renderer = excel_config.to_renderer();
        // ... render
    }
}
```

#### Playground UI Design

When Excel (XLSX) format is selected, an "Excel Options" panel appears with configurable settings:

```
┌─────────────────────────────────────────────────────────────────┐
│ Export Format: [Excel (XLSX) ▼]                    [Export]     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─ Timescale ─────────────────────────────────────────────┐   │
│  │  Scale:     (•) Weekly  ( ) Daily                       │   │
│  │  Timeframe: [✓] Auto-fit to project                     │   │
│  │             [ ] Manual: [20] weeks                      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─ Formatting ────────────────────────────────────────────┐   │
│  │  Currency:      [EUR ▼]                                 │   │
│  │  Hours/Day:     [8.0  ]                                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─ Content ───────────────────────────────────────────────┐   │
│  │  [✓] Include Summary Sheet                              │   │
│  │  [✓] Show Dependencies (formula-driven scheduling)      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─ Progress Tracking ─────────────────────────────────────┐   │
│  │  Progress Mode: [Visual ▼]                              │   │
│  │                                                         │   │
│  │    • None    - No progress columns (default)            │   │
│  │    • Columns - Add Complete%, Remaining, Actuals        │   │
│  │    • Visual  - + Progress bars in timeline              │   │
│  │    • Full    - + Status icons, variance, markers        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### HTML Implementation

```html
<!-- Excel Options Panel (shown when xlsx selected) -->
<div id="excel-options" class="export-options hidden">
    <div class="option-group">
        <label class="option-group-title">Timescale</label>
        <div class="option-row">
            <label><input type="radio" name="excel-scale" value="weekly" checked> Weekly</label>
            <label><input type="radio" name="excel-scale" value="daily"> Daily</label>
        </div>
        <div class="option-row">
            <label>
                <input type="checkbox" id="excel-autofit" checked>
                Auto-fit to project duration
            </label>
        </div>
        <div class="option-row" id="excel-manual-weeks">
            <label>Weeks: <input type="number" id="excel-weeks" value="20" min="1" max="104"></label>
        </div>
    </div>

    <div class="option-group">
        <label class="option-group-title">Formatting</label>
        <div class="option-row">
            <label>Currency:
                <select id="excel-currency">
                    <option value="EUR">EUR (€)</option>
                    <option value="USD">USD ($)</option>
                    <option value="GBP">GBP (£)</option>
                    <option value="CHF">CHF</option>
                    <option value="JPY">JPY (¥)</option>
                </select>
            </label>
        </div>
        <div class="option-row">
            <label>Hours/Day: <input type="number" id="excel-hours" value="8" min="1" max="24" step="0.5"></label>
        </div>
    </div>

    <div class="option-group">
        <label class="option-group-title">Content</label>
        <div class="option-row">
            <label><input type="checkbox" id="excel-summary" checked> Include Summary Sheet</label>
        </div>
        <div class="option-row">
            <label><input type="checkbox" id="excel-deps" checked> Show Dependencies</label>
        </div>
    </div>

    <div class="option-group">
        <label class="option-group-title">Progress Tracking</label>
        <div class="option-row">
            <label>Progress Mode:
                <select id="excel-progress">
                    <option value="none">None</option>
                    <option value="columns">Columns</option>
                    <option value="visual" selected>Visual</option>
                    <option value="full">Full</option>
                </select>
            </label>
        </div>
        <div class="option-hint" id="excel-progress-hint">
            Progress bars showing completed vs remaining work
        </div>
    </div>
</div>
```

#### JavaScript Implementation

```javascript
// Show/hide Excel options based on format selection
document.getElementById('export-format-select').addEventListener('change', (e) => {
    const excelOptions = document.getElementById('excel-options');
    excelOptions.classList.toggle('hidden', e.target.value !== 'xlsx');
});

// Toggle manual weeks input based on auto-fit
document.getElementById('excel-autofit').addEventListener('change', (e) => {
    document.getElementById('excel-manual-weeks').classList.toggle('hidden', e.target.checked);
});

// Update progress mode hint
document.getElementById('excel-progress').addEventListener('change', (e) => {
    const hints = {
        'none': 'No progress information (clean schedule view)',
        'columns': 'Adds Complete%, Remaining, Actual Start/End columns',
        'visual': 'Progress bars showing completed vs remaining work',
        'full': 'Full tracking with status icons, variance, and markers'
    };
    document.getElementById('excel-progress-hint').textContent = hints[e.target.value];
});

// Export with configuration
function exportExcel() {
    const config = {
        scale: document.querySelector('input[name="excel-scale"]:checked').value,
        auto_fit: document.getElementById('excel-autofit').checked,
        weeks: parseInt(document.getElementById('excel-weeks').value),
        currency: document.getElementById('excel-currency').value,
        hours_per_day: parseFloat(document.getElementById('excel-hours').value),
        include_summary: document.getElementById('excel-summary').checked,
        show_dependencies: document.getElementById('excel-deps').checked,
        progress_mode: document.getElementById('excel-progress').value
    };

    const xlsxBytes = playground.render_xlsx_configured(JSON.stringify(config));
    downloadBinaryFile('project.xlsx', xlsxBytes,
        'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet');
}
```

#### CSS Styling

```css
.export-options {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 12px;
    margin-top: 8px;
    font-size: 13px;
}

.export-options.hidden {
    display: none;
}

.option-group {
    margin-bottom: 12px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border-color);
}

.option-group:last-child {
    margin-bottom: 0;
    padding-bottom: 0;
    border-bottom: none;
}

.option-group-title {
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    color: var(--text-secondary);
    display: block;
    margin-bottom: 8px;
}

.option-row {
    margin: 6px 0;
}

.option-row label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-right: 12px;
}

.option-row input[type="number"],
.option-row select {
    width: 80px;
    padding: 4px 8px;
    border: 1px solid var(--border-color);
    border-radius: 4px;
    background: var(--bg-primary);
    color: var(--text-primary);
}

.option-hint {
    font-size: 11px;
    color: var(--text-secondary);
    font-style: italic;
    margin-top: 4px;
}
```

#### Share URL Support

Excel options are included in share URLs for reproducibility:

```javascript
// Generate share URL with Excel options
function generateShareUrl() {
    const data = {
        code: editor.getValue(),
        format: document.getElementById('format-select').value,
        leveling: document.getElementById('leveling-checkbox').checked,
        nowLine: document.getElementById('nowline-checkbox').checked,
        // Excel options (only included if xlsx selected)
        excel: {
            scale: document.querySelector('input[name="excel-scale"]:checked')?.value,
            autoFit: document.getElementById('excel-autofit')?.checked,
            weeks: parseInt(document.getElementById('excel-weeks')?.value || 20),
            currency: document.getElementById('excel-currency')?.value,
            hoursPerDay: parseFloat(document.getElementById('excel-hours')?.value || 8),
            includeSummary: document.getElementById('excel-summary')?.checked,
            showDependencies: document.getElementById('excel-deps')?.checked,
            progressMode: document.getElementById('excel-progress')?.value
        }
    };
    return encodeShareData(data);
}
```

#### Default Configuration

When no options are explicitly set, the playground uses sensible defaults:

| Option | Default | Rationale |
|--------|---------|-----------|
| Scale | Weekly | Better overview for most projects |
| Auto-fit | ✓ Enabled | Automatically sizes to project duration |
| Currency | EUR | European market focus |
| Hours/Day | 8.0 | Standard workday |
| Include Summary | ✓ Enabled | Executive overview useful |
| Show Dependencies | ✓ Enabled | Formula-driven scheduling |
| Progress Mode | Visual | Good balance of info vs complexity |

### Example Output

#### Schedule Sheet with `ProgressMode::Full`

```
┌──────────┬─────────────────┬────────┬──────────┬───────────┬──────────┬─────────────┬────────────┬─────┬─────┬═════╤═════┬─────┐
│ Task ID  │ Activity        │ Status │ Complete │ Remaining │ Variance │ Act. Start  │ Act. End   │ ... │ W3  ║ SD  │ W4  │ W5  │
├──────────┼─────────────────┼────────┼──────────┼───────────┼──────────┼─────────────┼────────────┼─────┼─────╬═════╪═════┼─────┤
│ design   │ Design Phase    │   ✓    │   100%   │    0d     │   +1d    │ 2026-01-06  │ 2026-01-10 │     │ ████║     │     │     │
│ impl     │ Implementation  │   ●    │    60%   │    4d     │   -2d    │ 2026-01-13  │            │     │ ████║▒▒▒▒│▒▒▒▒│     │
│ test     │ Testing         │   ○    │    0%    │   10d     │    0d    │             │            │     │     ║    │░░░░│░░░░│
│ deploy   │ Deployment      │   ⚠    │    0%    │    5d     │   -5d    │             │            │     │     ║░░░░│    │     │
└──────────┴─────────────────┴────────┴──────────┴───────────┴──────────┴─────────────┴────────────┴─────┴─────╩═════╧═════┴─────┘
                                                                                                              ↑
                                                                                                    Status Date (2026-01-20)
```

Legend:
- `████` = Completed work (green)
- `▒▒▒▒` = Remaining work behind schedule (red striped)
- `░░░░` = Remaining work on schedule (blue)
- `SD` = Status date column (yellow highlight)

## Implementation Plan

### Phase 1: Progress Columns (MVP)
1. Add `ProgressMode` enum to `ExcelConfig`
2. Add `--progress` CLI flag
3. Implement column insertion for Complete/Remaining/Actuals
4. Unit tests for column data correctness
5. Update WASM `render_xlsx_with_config` to accept progress mode

### Phase 2: Visual Progress Bars
1. Implement `calculate_week_progress()` logic
2. Add conditional formatting for split cells
3. Color coding for completed vs remaining
4. Behind-schedule highlighting (red)

### Phase 3: Full Mode
1. Add Status column with icons
2. Add Variance column calculation
3. Status date column highlighting
4. Row-level conditional formatting
5. E2E tests for visual output

### Phase 4: Playground Excel Options Panel
1. Add collapsible Excel options panel to index.html
2. Implement option groups: Timescale, Formatting, Content, Progress
3. Add `render_xlsx_configured()` WASM method accepting JSON config
4. Wire up JavaScript event handlers for all options
5. Show/hide panel based on export format selection
6. Auto-fit toggle shows/hides manual weeks input
7. Progress mode dropdown with dynamic hint text

### Phase 5: Playground Polish
1. Add CSS styling for options panel (light/dark theme)
2. Include Excel options in share URL encoding
3. Restore Excel options from share URL on load
4. Add tooltips explaining each option
5. E2E tests for Excel options panel
6. E2E tests for share URL with Excel options

## Test Cases

### Unit Tests
1. **progress_columns_added**: Columns present in output when mode=Columns
2. **progress_data_correct**: Complete%, Remaining, Actuals match task data
3. **variance_calculation**: Positive/negative variance calculated correctly
4. **status_icons**: Correct icon for each task state
5. **behind_schedule_detection**: Tasks past status date flagged correctly

### Visual Tests (Phase 2)
6. **split_cell_calculation**: Week cells show correct completed/remaining split
7. **behind_schedule_coloring**: Red highlight applied to overdue cells
8. **status_date_column**: SD column highlighted yellow

### Integration Tests
9. **cli_progress_flag**: `--progress=visual` produces correct output
10. **wasm_progress_export**: Playground export respects progress mode
11. **combined_flags**: `--progress=full --as-of DATE` work together

### Playground E2E Tests
12. **excel_options_panel_visibility**: Panel shown only when XLSX selected
13. **excel_scale_selection**: Weekly/Daily radio buttons work
14. **excel_autofit_toggle**: Disabling auto-fit shows weeks input
15. **excel_currency_selection**: Currency dropdown updates config
16. **excel_progress_mode_selection**: All four modes selectable
17. **excel_progress_hint_updates**: Hint text changes with mode
18. **excel_export_with_options**: Export uses selected configuration
19. **excel_share_url_includes_options**: Share URL encodes Excel options
20. **excel_share_url_restores_options**: Loading URL sets Excel options

## Acceptance Criteria

### Phase 1: Progress Columns
- [ ] `--progress=columns` adds Complete/Remaining/Actual columns
- [ ] Column data matches scheduled task progress fields
- [ ] Default behavior unchanged (no progress columns)
- [ ] WASM playground can export with progress columns

### Phase 2: Visual Progress Bars
- [ ] Timeline cells show completed vs remaining portions
- [ ] Behind-schedule cells highlighted in red
- [ ] Visual distinction between complete/remaining/behind

### Phase 3: Full Mode
- [ ] Status column shows ✓ ● ○ ⚠ icons
- [ ] Variance column shows +/- days
- [ ] Status date column visually highlighted
- [ ] Row coloring based on task status

### Phase 4: Playground Excel Options Panel
- [ ] Excel options panel visible when XLSX format selected
- [ ] Panel hidden for other export formats
- [ ] Timescale options: Weekly/Daily radio buttons
- [ ] Auto-fit checkbox toggles manual weeks input visibility
- [ ] Currency dropdown with common currencies
- [ ] Hours/Day numeric input
- [ ] Include Summary checkbox
- [ ] Show Dependencies checkbox
- [ ] Progress Mode dropdown with all four modes
- [ ] Progress mode hint text updates dynamically
- [ ] All options passed to `render_xlsx_configured()` correctly
- [ ] Exported file reflects selected options

### Phase 5: Playground Polish
- [ ] Options panel styled for light and dark themes
- [ ] Share URL includes Excel options
- [ ] Loading share URL restores Excel options
- [ ] Tooltips on hover for each option
- [ ] E2E test: Excel options panel visibility toggle
- [ ] E2E test: Progress mode selection affects export
- [ ] E2E test: Share URL round-trip preserves Excel options

## Alternatives Considered

### 1. Separate Progress Sheet
Add a dedicated "Progress" sheet instead of modifying Schedule sheet.
- **Pro**: Doesn't change existing layout
- **Con**: Data duplication, harder to correlate with timeline

### 2. Always Show Progress
Make progress columns always visible.
- **Pro**: Simpler implementation
- **Con**: Breaks backwards compatibility, clutters output for simple schedules

### 3. Progress as Overlay
Use Excel comments/notes to show progress on hover.
- **Pro**: Doesn't add columns
- **Con**: Not visible in print, poor discoverability

**Decision**: Opt-in `ProgressMode` provides flexibility while maintaining backwards compatibility.

## Future Enhancements (Out of Scope)

- Earned Value columns (PV, EV, AC, SPI, CPI)
- Progress trend sparklines
- Forecast completion date based on SPI
- Interactive progress update (two-way sync)
- PDF progress report generation

## Appendix: Progress Mode Comparison

| Feature | None | Columns | Visual | Full |
|---------|------|---------|--------|------|
| Complete % column | ❌ | ✓ | ✓ | ✓ |
| Remaining column | ❌ | ✓ | ✓ | ✓ |
| Actual dates | ❌ | ✓ | ✓ | ✓ |
| Split progress bars | ❌ | ❌ | ✓ | ✓ |
| Behind highlighting | ❌ | ❌ | ✓ | ✓ |
| Status icons | ❌ | ❌ | ❌ | ✓ |
| Variance column | ❌ | ❌ | ❌ | ✓ |
| Status date marker | ❌ | ❌ | ❌ | ✓ |
| Row conditional formatting | ❌ | ❌ | ❌ | ✓ |
