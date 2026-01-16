# RFC-0009: Excel WASM Export with Auto-Fit Timeframe

**Status:** Draft
**Author:** Claude
**Created:** 2026-01-16

## Summary

Enhance the Excel renderer to automatically calculate the required timeframe from the schedule, and expose full configuration options through the WASM Playground API.

## Motivation

The current ExcelRenderer has hardcoded defaults:
- `schedule_weeks: 18` (arbitrary, may be too short or too long)
- `schedule_days: 60` (arbitrary)

This creates poor user experience:
1. Short projects waste columns with empty weeks
2. Long projects get truncated, missing critical end dates
3. WASM playground has no way to configure these settings

## Design Goals

1. **Zero-configuration works perfectly**: Auto-fit to project duration by default
2. **Full control when needed**: Expose all options through fluent builder API
3. **WASM-friendly**: All configuration serializable as JSON for browser UI
4. **Backward compatible**: Existing CLI behavior unchanged

## Specification

### 1. Auto-Fit Timeframe (Default Behavior)

When `schedule_weeks` or `schedule_days` is not explicitly set, calculate from schedule:

```rust
/// Calculate required weeks to cover full project + buffer
fn auto_fit_weeks(schedule: &Schedule, project_start: NaiveDate) -> u32 {
    let project_end = schedule.project_end;
    let days = (project_end - project_start).num_days().max(0) as u32;
    let weeks = (days + 6) / 7;  // Round up to complete weeks
    let buffer = (weeks / 10).max(1);  // 10% buffer, minimum 1 week
    weeks + buffer
}

/// Calculate required days for daily mode
fn auto_fit_days(schedule: &Schedule, project_start: NaiveDate) -> u32 {
    let project_end = schedule.project_end;
    let days = (project_end - project_start).num_days().max(0) as u32;
    let buffer = (days / 10).max(5);  // 10% buffer, minimum 5 days
    days + buffer
}
```

### 2. Period of Interest (Optional Filtering)

For large projects, allow exporting only a slice:

```rust
pub struct ExcelRenderer {
    // ... existing fields ...

    /// Start date for the export window (default: project start)
    pub view_start: Option<NaiveDate>,

    /// End date for the export window (default: auto-fit to project end)
    pub view_end: Option<NaiveDate>,

    /// Auto-fit mode (default: true)
    pub auto_fit: bool,
}
```

Builder methods:
```rust
impl ExcelRenderer {
    /// Set explicit timeframe (disables auto-fit)
    pub fn timeframe(mut self, start: NaiveDate, end: NaiveDate) -> Self {
        self.view_start = Some(start);
        self.view_end = Some(end);
        self.auto_fit = false;
        self
    }

    /// Disable auto-fit and use explicit weeks/days
    pub fn no_auto_fit(mut self) -> Self {
        self.auto_fit = false;
        self
    }
}
```

### 3. WASM Configuration API

Add a configuration struct that can be passed from JavaScript:

```rust
/// Excel export configuration for WASM
#[derive(Serialize, Deserialize, Default)]
#[wasm_bindgen]
pub struct ExcelConfig {
    /// Scale: "daily" or "weekly" (default: "weekly")
    pub scale: String,

    /// Currency symbol (default: "EUR")
    pub currency: String,

    /// Auto-fit to project duration (default: true)
    pub auto_fit: bool,

    /// Number of weeks (only if auto_fit=false, scale=weekly)
    pub weeks: Option<u32>,

    /// Number of days (only if auto_fit=false, scale=daily)
    pub days: Option<u32>,

    /// Working hours per day (default: 8)
    pub hours_per_day: f64,

    /// Include executive summary sheet (default: true)
    pub include_summary: bool,

    /// Include dependency columns (default: true)
    pub show_dependencies: bool,
}
```

Playground method:
```rust
#[wasm_bindgen]
impl Playground {
    /// Render as Excel workbook with configuration
    ///
    /// # Arguments
    /// * `config` - Optional JSON configuration object
    ///
    /// # Returns
    /// Raw bytes of the XLSX file
    pub fn render_xlsx_with_config(&self, config: JsValue) -> Vec<u8> {
        let config: ExcelConfig = serde_wasm_bindgen::from_value(config)
            .unwrap_or_default();

        match (&self.project, &self.schedule) {
            (Some(project), Some(schedule)) => {
                let mut renderer = ExcelRenderer::new()
                    .currency(&config.currency);

                // Apply scale
                if config.scale == "daily" {
                    renderer = renderer.daily();
                    if !config.auto_fit {
                        if let Some(days) = config.days {
                            renderer = renderer.days(days);
                        }
                    }
                } else if !config.auto_fit {
                    if let Some(weeks) = config.weeks {
                        renderer = renderer.weeks(weeks);
                    }
                }

                // Auto-fit is the default, applied in render_to_bytes
                renderer.render(project, schedule).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }
}
```

### 4. Playground UI Updates

Add export configuration panel:

```html
<!-- Export Options -->
<div id="export-options" class="export-options hidden">
    <label>Scale:
        <select id="excel-scale">
            <option value="weekly">Weekly</option>
            <option value="daily">Daily</option>
        </select>
    </label>
    <label>
        <input type="checkbox" id="excel-auto-fit" checked>
        Auto-fit timeframe
    </label>
    <label class="manual-weeks">Weeks:
        <input type="number" id="excel-weeks" value="18" min="1" max="104">
    </label>
</div>
```

### 5. Backward Compatibility

- **CLI unchanged**: Existing `--weeks` flag continues to work
- **Default behavior improved**: Auto-fit produces better results than fixed 18 weeks
- **API backward compatible**: `ExcelRenderer::new().render()` still works

## Implementation Plan

### Phase 1: Core Auto-Fit (This PR)
1. Add `auto_fit` field to ExcelRenderer
2. Implement `auto_fit_weeks()` and `auto_fit_days()`
3. Apply auto-fit in `render_to_bytes()` when enabled
4. Add TDD tests for auto-fit calculation

### Phase 2: WASM Configuration
1. Add `ExcelConfig` struct with wasm_bindgen
2. Add `render_xlsx_with_config()` to Playground
3. Update playground UI with export options

### Phase 3: Period of Interest (Future)
1. Add `view_start`/`view_end` fields
2. Filter tasks to only those overlapping the window
3. Add UI date range picker

## Test Cases

See `crates/utf8proj-render/tests/excel_auto_fit.rs` for TDD tests.

## Alternatives Considered

1. **Fixed weeks based on project size categories**: Rejected as too coarse
2. **User always specifies weeks**: Rejected as poor UX for zero-config
3. **Separate WASM struct from core**: Rejected to avoid code duplication

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-01-16 | Auto-fit on by default | Zero-config should "just work" |
| 2026-01-16 | 10% buffer with minimums | Prevents cramped layouts |
| 2026-01-16 | JSON config for WASM | JavaScript-friendly, easy UI binding |
