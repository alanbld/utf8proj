# utf8proj: Design Refinement Survey - Part 2
**Version:** 1.0
**Date:** 2026-01-04
**Scope:** Sections F-I (Questions 18-29)
**Focus:** Export, Resource Leveling, BDD/SAT & TaskJuggler Compatibility

---

## SECTION F: EXCEL EXPORT
**Current Confidence: 70% → Target: 95%**

### Q18. Rust Excel Library Choice

**Recommended Design:**

**rust_xlsxwriter** for v1.0 - write-only is sufficient, best maintained, full feature set.

```toml
# Cargo.toml
[dependencies]
rust_xlsxwriter = "0.64"  # Check for latest version
```

**Library Comparison:**

| Feature | rust_xlsxwriter | umya-spreadsheet | calamine |
|---------|-----------------|------------------|----------|
| Write .xlsx | Yes | Yes | No |
| Read .xlsx | No | Yes | Yes |
| Charts | Yes | Limited | No |
| Formulas | Yes | Yes | No |
| Conditional formatting | Yes | Partial | No |
| Performance | Excellent | Good | Excellent |
| Maintenance | Active | Active | Active |
| Documentation | Excellent | Limited | Good |

**Usage Example:**

```rust
use rust_xlsxwriter::{Workbook, Format, Chart, ChartType};

pub fn export_to_excel(schedule: &Schedule, path: &Path) -> Result<(), ExportError> {
    let mut workbook = Workbook::new();

    // Sheet 1: Task List
    let task_sheet = workbook.add_worksheet();
    task_sheet.set_name("Task List")?;
    write_task_list(task_sheet, schedule)?;

    // Sheet 2: Gantt Data
    let gantt_sheet = workbook.add_worksheet();
    gantt_sheet.set_name("Gantt Chart")?;
    write_gantt_data(gantt_sheet, schedule)?;
    add_gantt_chart(gantt_sheet, schedule)?;

    // Sheet 3: Resource Allocation
    let resource_sheet = workbook.add_worksheet();
    resource_sheet.set_name("Resources")?;
    write_resource_allocation(resource_sheet, schedule)?;

    workbook.save(path)?;
    Ok(())
}
```

**Rationale:**

1. **Write-only is fine** - utf8proj generates reports; users don't edit Excel and reimport.

2. **Chart support is essential** - Gantt charts and resource histograms are expected outputs.

3. **Formula support** - Allows computed columns (variance, SPI, CPI).

4. **Active maintenance** - rust_xlsxwriter has regular updates and responsive maintainer.

5. **Python xlsxwriter heritage** - Based on proven Python library with extensive documentation.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| rust_xlsxwriter | Full features, maintained | Write-only |
| umya-spreadsheet | Read+Write | Less mature, limited charts |
| calamine + manual write | Read existing | No write capability |
| Template-based | Use existing xlsx | Requires template distribution |

**Implementation Notes:**

1. Create format presets for consistent styling
2. Use named ranges for chart data sources
3. Set print areas and page breaks for good printing
4. Freeze header row and task name column
5. Auto-fit column widths where possible

**Test Strategy:**

```rust
#[test]
fn excel_export_creates_valid_file() {
    let schedule = create_test_schedule(10);
    let path = temp_path("test.xlsx");

    export_to_excel(&schedule, &path).unwrap();

    assert!(path.exists());
    // Validate with external tool or manual inspection
}

#[test]
fn excel_export_performance_1000_tasks() {
    let schedule = create_test_schedule(1000);
    let path = temp_path("large.xlsx");

    let start = Instant::now();
    export_to_excel(&schedule, &path).unwrap();
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_secs(5));
}
```

---

### Q19. Excel Sheet Structure

**Recommended Design:**

Four-sheet workbook structure optimized for PM workflows:

```
Sheet 1: "Task List" (Primary data view)
Sheet 2: "Gantt Chart" (Visual timeline)
Sheet 3: "Resources" (Allocation and utilization)
Sheet 4: "Variance Report" (Planned vs Actual)
```

**Sheet 1: Task List**

```rust
fn write_task_list(sheet: &mut Worksheet, schedule: &Schedule) -> Result<()> {
    // Headers
    let headers = [
        "WBS", "Task ID", "Task Name", "Duration", "Start", "Finish",
        "% Complete", "Baseline Start", "Baseline Finish",
        "Start Variance", "Finish Variance", "Total Float",
        "Critical", "Resources", "Predecessors"
    ];

    for (col, header) in headers.iter().enumerate() {
        sheet.write_string(0, col as u16, header)?;
    }

    // Data rows
    for (row, task) in schedule.tasks.iter().enumerate() {
        let row = (row + 1) as u32;

        sheet.write_string(row, 0, &task.wbs)?;
        sheet.write_string(row, 1, &task.id)?;
        sheet.write_string(row, 2, &task.name)?;
        sheet.write_number(row, 3, task.duration.num_days() as f64)?;
        sheet.write_date(row, 4, task.start)?;
        sheet.write_date(row, 5, task.finish)?;
        sheet.write_number(row, 6, task.percent_complete as f64 / 100.0)?;

        // Baseline dates (if available)
        if let Some(baseline) = &task.baseline {
            sheet.write_date(row, 7, baseline.start)?;
            sheet.write_date(row, 8, baseline.finish)?;

            // Variance formulas
            sheet.write_formula(row, 9, &format!("=E{}-H{}", row+1, row+1))?;
            sheet.write_formula(row, 10, &format!("=F{}-I{}", row+1, row+1))?;
        }

        sheet.write_number(row, 11, task.total_float.num_days() as f64)?;
        sheet.write_boolean(row, 12, task.is_critical)?;
        sheet.write_string(row, 13, &task.resources.join(", "))?;
        sheet.write_string(row, 14, &format_predecessors(&task.predecessors))?;
    }

    // Formatting
    apply_task_list_formatting(sheet, schedule.tasks.len())?;

    Ok(())
}

fn apply_task_list_formatting(sheet: &mut Worksheet, row_count: usize) -> Result<()> {
    // Freeze panes (header row + ID/Name columns)
    sheet.freeze_panes(1, 2)?;

    // Header format
    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);
    sheet.set_row_format(0, &header_format)?;

    // Conditional formatting for critical tasks
    let critical_format = ConditionalFormat::new()
        .set_type(ConditionalFormatType::Cell)
        .set_criteria(ConditionalFormatCriteria::Equal)
        .set_value("TRUE")
        .set_format(Format::new().set_background_color(Color::RGB(0xFFCCCC)));
    sheet.add_conditional_format(1, 12, row_count as u32, 12, &critical_format)?;

    // Conditional formatting for variance (red if positive = late)
    let late_format = ConditionalFormat::new()
        .set_type(ConditionalFormatType::Cell)
        .set_criteria(ConditionalFormatCriteria::GreaterThan)
        .set_value(0)
        .set_format(Format::new().set_font_color(Color::Red));
    sheet.add_conditional_format(1, 9, row_count as u32, 10, &late_format)?;

    // Percent complete as percentage format
    let pct_format = Format::new().set_num_format("0%");
    sheet.set_column_format(6, 6, &pct_format)?;

    // Date columns
    let date_format = Format::new().set_num_format("yyyy-mm-dd");
    sheet.set_column_format(4, 5, &date_format)?;
    sheet.set_column_format(7, 8, &date_format)?;

    Ok(())
}
```

**Sheet 2: Gantt Chart**

```rust
fn write_gantt_data(sheet: &mut Worksheet, schedule: &Schedule) -> Result<()> {
    // Data table for chart (task name, start offset, duration, progress duration)
    let headers = ["Task", "Start Day", "Duration", "Complete Days"];

    let project_start = schedule.project_start;

    for (row, task) in schedule.tasks.iter().enumerate() {
        let row = (row + 1) as u32;

        sheet.write_string(row, 0, &task.name)?;

        let start_offset = (task.start - project_start).num_days() as f64;
        sheet.write_number(row, 1, start_offset)?;

        let duration = task.duration.num_days() as f64;
        sheet.write_number(row, 2, duration)?;

        let complete_days = duration * (task.percent_complete as f64 / 100.0);
        sheet.write_number(row, 3, complete_days)?;
    }

    Ok(())
}

fn add_gantt_chart(sheet: &mut Worksheet, schedule: &Schedule) -> Result<()> {
    let task_count = schedule.tasks.len();

    // Stacked bar chart for Gantt effect
    let mut chart = Chart::new(ChartType::BarStacked);

    // Series 1: Start offset (invisible - creates gap)
    chart.add_series()
        .set_categories(("Gantt Chart", 1, 0, task_count as u32, 0))
        .set_values(("Gantt Chart", 1, 1, task_count as u32, 1))
        .set_fill_none();  // Invisible

    // Series 2: Completed portion (dark blue)
    chart.add_series()
        .set_categories(("Gantt Chart", 1, 0, task_count as u32, 0))
        .set_values(("Gantt Chart", 1, 3, task_count as u32, 3))
        .set_fill_color(Color::RGB(0x4472C4))
        .set_name("Complete");

    // Series 3: Remaining portion (light blue)
    chart.add_series()
        .set_categories(("Gantt Chart", 1, 0, task_count as u32, 0))
        .set_values_formula("=Gantt!$C$2:$C$N - Gantt!$D$2:$D$N")
        .set_fill_color(Color::RGB(0xB4C6E7))
        .set_name("Remaining");

    chart.set_title("Project Gantt Chart");
    chart.set_x_axis_name("Days from Project Start");

    // Insert chart
    sheet.insert_chart(1, 6, &chart)?;

    Ok(())
}
```

**Sheet 3: Resources**

```rust
fn write_resource_allocation(sheet: &mut Worksheet, schedule: &Schedule) -> Result<()> {
    // Resource summary table
    let headers = ["Resource", "Capacity", "Assigned Tasks", "Total Effort",
                   "Utilization %", "Over-allocated Days"];

    for (row, resource) in schedule.resources.iter().enumerate() {
        let row = (row + 1) as u32;
        let allocation = schedule.get_resource_allocation(&resource.id);

        sheet.write_string(row, 0, &resource.name)?;
        sheet.write_number(row, 1, resource.capacity)?;
        sheet.write_number(row, 2, allocation.assigned_tasks.len() as f64)?;
        sheet.write_number(row, 3, allocation.total_effort.num_hours() as f64)?;
        sheet.write_number(row, 4, allocation.utilization_percent / 100.0)?;
        sheet.write_number(row, 5, allocation.over_allocated_days as f64)?;
    }

    // Resource histogram (weekly allocation)
    add_resource_histogram(sheet, schedule)?;

    Ok(())
}
```

**Sheet 4: Variance Report**

```rust
fn write_variance_report(sheet: &mut Worksheet, schedule: &Schedule) -> Result<()> {
    // Summary metrics
    sheet.write_string(0, 0, "Variance Summary")?;

    let metrics = [
        ("Project Start Variance", schedule.start_variance_days()),
        ("Project Finish Variance", schedule.finish_variance_days()),
        ("Schedule Performance Index (SPI)", schedule.spi()),
        ("Tasks On Track", schedule.tasks_on_track_count() as f64),
        ("Tasks Behind", schedule.tasks_behind_count() as f64),
        ("Tasks Ahead", schedule.tasks_ahead_count() as f64),
    ];

    for (row, (label, value)) in metrics.iter().enumerate() {
        sheet.write_string((row + 2) as u32, 0, label)?;
        sheet.write_number((row + 2) as u32, 1, *value)?;
    }

    // Task-level variance detail
    write_task_variance_detail(sheet, schedule, 10)?;

    // Variance trend chart (if baseline history available)
    if schedule.has_baseline_history() {
        add_variance_trend_chart(sheet, schedule)?;
    }

    Ok(())
}
```

**Rationale:**

1. **Task List is primary** - Most PM work happens here. Comprehensive data with formulas.

2. **Gantt is visual summary** - Quick overview, not for editing. Stacked bar creates Gantt effect.

3. **Resources separate** - Dedicated sheet for resource-focused analysis.

4. **Variance for tracking** - Planned vs actual is core PM metric. Dedicated sheet emphasizes it.

5. **Formulas over static values** - Variance columns compute from dates. If user updates data, formulas recalculate.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| 4 sheets | Organized, focused | Navigation required |
| Single sheet | Everything visible | Cluttered, hard to print |
| Template-based | User customization | Template management burden |
| Minimal (data only) | Simple, fast | No visualization value |

**Implementation Notes:**

1. Named ranges for chart references (survive row insertion)
2. Print area set per sheet
3. Page breaks at logical task groups
4. Column widths optimized per content type
5. Data validation for date columns (prevent invalid edits)
6. Document properties (author: utf8proj, version)

**Test Strategy:**

```rust
#[test]
fn excel_has_all_sheets() {
    let schedule = create_test_schedule(10);
    let path = temp_path("test.xlsx");
    export_to_excel(&schedule, &path).unwrap();

    let workbook = open_xlsx(&path).unwrap();
    assert_eq!(workbook.sheet_names(), vec![
        "Task List", "Gantt Chart", "Resources", "Variance Report"
    ]);
}

#[test]
fn task_list_has_correct_columns() {
    let schedule = create_test_schedule(5);
    let path = temp_path("test.xlsx");
    export_to_excel(&schedule, &path).unwrap();

    let workbook = open_xlsx(&path).unwrap();
    let task_sheet = workbook.worksheet("Task List").unwrap();

    assert_eq!(task_sheet.get_value((0, 0)), "WBS");
    assert_eq!(task_sheet.get_value((0, 1)), "Task ID");
    // ... validate all headers
}

#[test]
fn variance_formulas_calculate_correctly() {
    let schedule = create_schedule_with_baseline();
    let path = temp_path("test.xlsx");
    export_to_excel(&schedule, &path).unwrap();

    // Open in Excel/LibreOffice and verify formulas
    // Or use formula evaluation library
}
```

---

### Q20. Excel Chart Generation

**Recommended Design:**

**Generate core charts in Rust**, let users customize in Excel.

**Charts to Generate:**

1. **Gantt Chart** (stacked bar) - Primary visualization
2. **Resource Histogram** (column chart) - Utilization over time
3. **Progress Burndown** (line chart) - Work remaining over time
4. **Variance Trend** (line chart) - Schedule variance over snapshots

```rust
pub fn generate_charts(workbook: &mut Workbook, schedule: &Schedule) -> Result<()> {
    // 1. Gantt Chart (in Gantt Chart sheet)
    generate_gantt_chart(workbook, schedule)?;

    // 2. Resource Histogram (in Resources sheet)
    generate_resource_histogram(workbook, schedule)?;

    // 3. Progress Burndown (in Variance Report sheet)
    if schedule.has_progress_history() {
        generate_burndown_chart(workbook, schedule)?;
    }

    // 4. Variance Trend (in Variance Report sheet)
    if schedule.has_baseline() {
        generate_variance_chart(workbook, schedule)?;
    }

    Ok(())
}

fn generate_gantt_chart(workbook: &mut Workbook, schedule: &Schedule) -> Result<()> {
    let sheet = workbook.worksheet_mut("Gantt Chart")?;
    let task_count = schedule.tasks.len();

    let mut chart = Chart::new(ChartType::BarStacked);

    // Configure as Gantt (see Q19 implementation)
    // ...

    // Styling
    chart.set_style(10);  // Built-in Excel style
    chart.legend().set_position(LegendPosition::Bottom);

    // Size and position
    chart.set_size(800, 400);
    sheet.insert_chart_with_offset(0, 5, &chart, 10, 10)?;

    Ok(())
}

fn generate_resource_histogram(workbook: &mut Workbook, schedule: &Schedule) -> Result<()> {
    let sheet = workbook.worksheet_mut("Resources")?;

    // First, write weekly utilization data
    let weeks = schedule.get_weekly_periods();
    let resources = &schedule.resources;

    // Header row: Resource names
    for (col, resource) in resources.iter().enumerate() {
        sheet.write_string(10, (col + 1) as u16, &resource.name)?;
    }

    // Data rows: Week, utilization per resource
    for (row, week) in weeks.iter().enumerate() {
        sheet.write_string((row + 11) as u32, 0, &week.label)?;

        for (col, resource) in resources.iter().enumerate() {
            let util = schedule.get_utilization(&resource.id, week);
            sheet.write_number((row + 11) as u32, (col + 1) as u16, util)?;
        }
    }

    // Create stacked column chart
    let mut chart = Chart::new(ChartType::ColumnStacked);

    for (col, resource) in resources.iter().enumerate() {
        chart.add_series()
            .set_name(&resource.name)
            .set_categories(("Resources", 11, 0, (11 + weeks.len()) as u32, 0))
            .set_values(("Resources", 11, (col + 1) as u16,
                        (11 + weeks.len()) as u32, (col + 1) as u16));
    }

    chart.set_title("Resource Allocation by Week");
    chart.y_axis().set_name("Hours");

    // Add capacity line (100% = full capacity)
    let capacity_line = chart.add_series()
        .set_chart_type(ChartType::Line)
        .set_name("Capacity");
    // ... configure capacity reference line

    sheet.insert_chart(10, (resources.len() + 3) as u16, &chart)?;

    Ok(())
}

fn generate_burndown_chart(workbook: &mut Workbook, schedule: &Schedule) -> Result<()> {
    let sheet = workbook.worksheet_mut("Variance Report")?;

    // Burndown data: Date, Planned Remaining, Actual Remaining
    let history = schedule.progress_history();

    for (row, snapshot) in history.iter().enumerate() {
        sheet.write_date((row + 20) as u32, 0, snapshot.date)?;
        sheet.write_number((row + 20) as u32, 1, snapshot.planned_remaining_days)?;
        sheet.write_number((row + 20) as u32, 2, snapshot.actual_remaining_days)?;
    }

    let mut chart = Chart::new(ChartType::Line);

    chart.add_series()
        .set_name("Planned")
        .set_categories(("Variance Report", 20, 0, (20 + history.len()) as u32, 0))
        .set_values(("Variance Report", 20, 1, (20 + history.len()) as u32, 1))
        .set_line_color(Color::Gray);

    chart.add_series()
        .set_name("Actual")
        .set_categories(("Variance Report", 20, 0, (20 + history.len()) as u32, 0))
        .set_values(("Variance Report", 20, 2, (20 + history.len()) as u32, 2))
        .set_line_color(Color::Blue);

    chart.set_title("Progress Burndown");
    chart.y_axis().set_name("Days Remaining");

    sheet.insert_chart(20, 4, &chart)?;

    Ok(())
}
```

**Rationale:**

1. **Generate, don't template** - Templates require distribution and versioning. Code-generated charts are reproducible.

2. **Core charts only** - Gantt, histogram, burndown cover 80% of needs. Users can add more in Excel.

3. **Data always available** - Even if chart rendering has issues, underlying data is in sheets for manual charting.

4. **Styling consistency** - Use Excel's built-in styles for professional look without custom CSS-like config.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Generate in Rust | Reproducible, no template | Limited chart types |
| Excel templates | Full customization | Template management |
| Data only + instructions | Simple | User effort required |
| External charting lib | Better charts | Additional dependency |

**Implementation Notes:**

1. rust_xlsxwriter supports most Excel chart types
2. Test charts open correctly in Excel, LibreOffice, Google Sheets
3. Provide data ranges so users can create additional charts
4. Chart titles should include project name and export date
5. Consider dark/light theme support (configurable colors)

**Test Strategy:**

```rust
#[test]
fn gantt_chart_renders() {
    let schedule = create_test_schedule(10);
    let path = temp_path("gantt.xlsx");
    export_to_excel(&schedule, &path).unwrap();

    // Verify chart exists (would need xlsx parsing library)
    // Manual verification: open in Excel, confirm chart displays
}

#[test]
fn histogram_shows_all_resources() {
    let schedule = create_schedule_with_resources(3);
    let path = temp_path("resources.xlsx");
    export_to_excel(&schedule, &path).unwrap();

    // Verify 3 series in histogram chart
}

#[test]
fn burndown_requires_history() {
    let schedule_no_history = create_test_schedule(10);
    let schedule_with_history = create_schedule_with_progress_history();

    // No burndown for schedule without history
    let charts1 = generate_charts(&schedule_no_history);
    assert!(!charts1.contains_burndown());

    // Has burndown with history
    let charts2 = generate_charts(&schedule_with_history);
    assert!(charts2.contains_burndown());
}
```

---

## SECTION G: RESOURCE LEVELING
**Current Confidence: 65% → Target: 95%**

### Q21. Resource Leveling Algorithm

**Recommended Design:**

**Critical Path Priority heuristic** with iterative refinement for v1.0.

```rust
pub struct LevelingConfig {
    /// Maximum iterations for refinement (default: 100)
    pub max_iterations: usize,

    /// Priority rules for conflict resolution
    pub priority_rules: Vec<PriorityRule>,

    /// Allow task splitting (default: false)
    pub allow_splitting: bool,

    /// Level only tasks after this date
    pub level_after: Option<NaiveDate>,
}

#[derive(Clone, Copy)]
pub enum PriorityRule {
    /// Keep critical path tasks scheduled
    CriticalPath,
    /// Higher priority value wins
    ExplicitPriority,
    /// Earlier baseline start wins
    BaselineStart,
    /// Less float = higher priority
    LessFloat,
    /// Shorter duration wins
    ShorterDuration,
    /// Already started tasks stay
    InProgress,
}

impl Default for LevelingConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            priority_rules: vec![
                PriorityRule::InProgress,
                PriorityRule::CriticalPath,
                PriorityRule::ExplicitPriority,
                PriorityRule::LessFloat,
                PriorityRule::ShorterDuration,
            ],
            allow_splitting: false,
            level_after: None,
        }
    }
}

pub fn level_resources(schedule: &mut Schedule, config: &LevelingConfig) -> LevelingResult {
    let mut result = LevelingResult::new();
    let mut iterations = 0;

    loop {
        // 1. Detect over-allocations
        let conflicts = detect_resource_conflicts(schedule, config.level_after);

        if conflicts.is_empty() {
            result.status = LevelingStatus::Success;
            break;
        }

        if iterations >= config.max_iterations {
            result.status = LevelingStatus::PartialSuccess {
                remaining_conflicts: conflicts.len(),
            };
            result.warnings.push(LevelingWarning::MaxIterationsReached);
            break;
        }

        // 2. Resolve highest-priority conflict
        let conflict = &conflicts[0];
        let resolution = resolve_conflict(schedule, conflict, &config.priority_rules);

        match resolution {
            Resolution::DelayTask { task_id, by_days } => {
                delay_task(schedule, &task_id, by_days);
                result.changes.push(LevelingChange::TaskDelayed {
                    task_id,
                    by_days,
                    reason: format!("Resource conflict with {}", conflict.competing_task),
                });
            }
            Resolution::CannotResolve { reason } => {
                result.warnings.push(LevelingWarning::UnresolvedConflict {
                    task: conflict.task_id.clone(),
                    resource: conflict.resource_id.clone(),
                    reason,
                });
            }
        }

        // 3. Recalculate schedule (CPM)
        recalculate_cpm(schedule);

        iterations += 1;
    }

    result.iterations = iterations;
    result
}

fn resolve_conflict(
    schedule: &Schedule,
    conflict: &ResourceConflict,
    rules: &[PriorityRule],
) -> Resolution {
    let task_a = &schedule.tasks[&conflict.task_a];
    let task_b = &schedule.tasks[&conflict.task_b];

    // Determine which task to delay based on priority rules
    let priority_a = compute_priority(task_a, rules);
    let priority_b = compute_priority(task_b, rules);

    let (keep, delay) = if priority_a >= priority_b {
        (task_a, task_b)
    } else {
        (task_b, task_a)
    };

    // Check if delay task can be moved
    if delay.is_anchored() {
        return Resolution::CannotResolve {
            reason: "Task is in-progress or has actual dates".to_string(),
        };
    }

    let delay_amount = conflict.overlap_days + 1;  // Clear the conflict

    // Check if delay violates constraints
    if let Some(constraint) = &delay.finish_constraint {
        let new_finish = delay.finish + Duration::days(delay_amount as i64);
        if new_finish > *constraint {
            return Resolution::CannotResolve {
                reason: format!("Would violate must_finish_on constraint: {}", constraint),
            };
        }
    }

    Resolution::DelayTask {
        task_id: delay.id.clone(),
        by_days: delay_amount,
    }
}

fn compute_priority(task: &Task, rules: &[PriorityRule]) -> i64 {
    let mut priority = 0i64;
    let mut weight = 1000000i64;  // Decreasing weights for rule order

    for rule in rules {
        let score = match rule {
            PriorityRule::InProgress => {
                if task.percent_complete > 0 { 1 } else { 0 }
            }
            PriorityRule::CriticalPath => {
                if task.is_critical { 1 } else { 0 }
            }
            PriorityRule::ExplicitPriority => {
                task.priority.unwrap_or(500) as i64
            }
            PriorityRule::LessFloat => {
                1000 - task.total_float.num_days().min(1000)
            }
            PriorityRule::ShorterDuration => {
                1000 - task.duration.num_days().min(1000)
            }
            PriorityRule::BaselineStart => {
                if let Some(baseline) = &task.baseline {
                    -baseline.start.num_days_from_ce() as i64
                } else {
                    0
                }
            }
        };

        priority += score * weight;
        weight /= 100;
    }

    priority
}
```

**Rationale:**

1. **Heuristic over optimization** - NP-hard optimal leveling is too slow for interactive use. Heuristics give good-enough results quickly.

2. **Critical path priority** - Preserving project end date is usually more important than perfect leveling.

3. **Configurable rules** - Different projects have different priorities. Allow customization.

4. **Iterative refinement** - Resolve one conflict at a time, recalculate, repeat. Simple and predictable.

5. **Bounded iterations** - Prevent infinite loops. Report partial success if can't resolve all conflicts.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Priority heuristic | Fast (O(n²)), predictable | May not find optimal |
| Constraint solver (Z3) | Optimal | Slow, complex dependency |
| Genetic algorithm | Good solutions | Non-deterministic |
| Manual only | User control | Tedious for large projects |

**Implementation Notes:**

1. Conflict detection: O(n × m × t) where n=tasks, m=resources, t=time periods
2. Use interval trees for efficient overlap detection
3. Cache priority calculations (don't recompute each iteration)
4. Report all changes for undo capability
5. Consider parallel conflict resolution in v2.0

**Test Strategy:**

```rust
#[test]
fn leveling_resolves_simple_conflict() {
    let mut schedule = schedule_project(vec![
        task("A", 10).assign("dev").start(date("2026-02-01")),
        task("B", 10).assign("dev").start(date("2026-02-05")),  // Overlaps with A
    ]);

    let result = level_resources(&mut schedule, &LevelingConfig::default());

    assert_eq!(result.status, LevelingStatus::Success);
    // B should be delayed to start after A finishes
    assert!(schedule["B"].start >= schedule["A"].finish);
}

#[test]
fn leveling_preserves_critical_path() {
    let mut schedule = schedule_project(vec![
        task("critical", 10).assign("dev").critical(true),
        task("non_critical", 10).assign("dev").float(5),
    ]);
    let original_critical_start = schedule["critical"].start;

    level_resources(&mut schedule, &LevelingConfig::default());

    // Critical task unchanged
    assert_eq!(schedule["critical"].start, original_critical_start);
    // Non-critical delayed
    assert!(schedule["non_critical"].start > original_critical_start);
}

#[test]
fn leveling_respects_constraints() {
    let mut schedule = schedule_project(vec![
        task("A", 10).assign("dev"),
        task("B", 10).assign("dev").must_finish_on(date("2026-02-20")),
    ]);

    let result = level_resources(&mut schedule, &LevelingConfig::default());

    // B has constraint, so A should be delayed instead
    assert!(schedule["B"].finish <= date("2026-02-20"));
}

#[test]
fn leveling_reports_unresolvable() {
    let mut schedule = schedule_project(vec![
        task("A", 10).assign("dev").must_finish_on(date("2026-02-15")),
        task("B", 10).assign("dev").must_finish_on(date("2026-02-15")),
    ]);

    let result = level_resources(&mut schedule, &LevelingConfig::default());

    assert!(matches!(result.status, LevelingStatus::PartialSuccess { .. }));
    assert!(!result.warnings.is_empty());
}
```

---

### Q22. Over-Allocation Handling

**Recommended Design:**

**Warn by default**, with configurable behavior.

```rust
pub enum OverAllocationMode {
    /// Schedule as-is, warn about conflicts (default)
    Warn,
    /// Automatically level to resolve conflicts
    AutoLevel,
    /// Error if any over-allocation detected
    Error,
    /// Ignore over-allocations entirely
    Ignore,
}

pub fn schedule_project(
    project: &Project,
    options: &ScheduleOptions,
) -> ScheduleResult {
    let schedule = run_cpm(project)?;

    let over_allocations = detect_over_allocations(&schedule);

    match options.over_allocation_mode {
        OverAllocationMode::Warn => {
            for conflict in &over_allocations {
                result.warnings.push(format_over_allocation_warning(conflict));
            }
        }
        OverAllocationMode::AutoLevel => {
            level_resources(&mut schedule, &options.leveling_config)?;
        }
        OverAllocationMode::Error => {
            if !over_allocations.is_empty() {
                return Err(ScheduleError::OverAllocation {
                    conflicts: over_allocations,
                });
            }
        }
        OverAllocationMode::Ignore => {
            // Do nothing
        }
    }

    Ok(result)
}
```

**Warning Output:**

```
⚠ Resource over-allocation detected

Resource: dev (Developer)
  Capacity: 1.0 (8h/day)
  Period: 2026-02-05 to 2026-02-14

  Conflicting tasks:
    • "Backend API" (2026-02-01 to 2026-02-14) - 100% allocation
    • "Frontend UI" (2026-02-05 to 2026-02-20) - 100% allocation

  Over-allocation: 200% (requires 2.0 capacity, have 1.0)

Suggestions:
  1. Run 'utf8proj level' to automatically resolve
  2. Add another developer: resource dev2 "Developer 2" { capacity: 1 }
  3. Reduce allocation: assign: dev@50%
  4. Delay "Frontend UI" to start after "Backend API"
```

**CLI Interface:**

```bash
# Default: warn
utf8proj schedule project.proj
# Output shows warnings but produces schedule

# Auto-level
utf8proj schedule project.proj --level
# Or
utf8proj level project.proj

# Strict mode (error on conflict)
utf8proj schedule project.proj --strict

# Quiet mode (ignore)
utf8proj schedule project.proj --ignore-overallocation
```

**Partial Allocation Handling:**

```rust
fn calculate_allocation(task: &Task, resource_id: &str) -> f64 {
    task.assignments.iter()
        .find(|a| a.resource_id == resource_id)
        .map(|a| a.percentage / 100.0)
        .unwrap_or(0.0)
}

fn detect_over_allocation(
    schedule: &Schedule,
    resource: &Resource,
    date: NaiveDate,
) -> Option<OverAllocation> {
    let total_allocation: f64 = schedule.tasks.values()
        .filter(|t| t.is_active_on(date))
        .map(|t| calculate_allocation(t, &resource.id))
        .sum();

    if total_allocation > resource.capacity {
        Some(OverAllocation {
            resource_id: resource.id.clone(),
            date,
            required: total_allocation,
            available: resource.capacity,
        })
    } else {
        None
    }
}
```

**Rationale:**

1. **Warn is safest default** - Users see the problem but schedule is produced. They can decide how to fix.

2. **Auto-level is opt-in** - Automatic changes may surprise users. Require explicit request.

3. **Error mode for CI/CD** - In automated pipelines, fail fast if schedule is infeasible.

4. **Partial allocation is common** - 50% dev on task A, 50% on task B is valid. Handle correctly.

5. **Actionable suggestions** - Don't just report problem; suggest solutions.

**Trade-offs:**

| Mode | Pros | Cons |
|------|------|------|
| Warn | User awareness, produces output | Invalid schedule may be used |
| AutoLevel | Automatic resolution | May make unexpected changes |
| Error | Strict validation | Blocks progress on issues |
| Ignore | No noise | Problems hidden |

**Implementation Notes:**

1. Check each day in project span (or use interval arithmetic)
2. Consider calendar (non-working days don't count)
3. Group conflicts by resource and period for readable output
4. Severity levels: minor (101-120%), moderate (121-150%), severe (151%+)
5. Track historical over-allocation separately (in the past, can't change)

**Test Strategy:**

```rust
#[test]
fn over_allocation_detected() {
    let schedule = schedule_project(vec![
        task("A", 10).assign("dev@100%"),
        task("B", 10).assign("dev@100%"),  // Both run simultaneously
    ]);

    let conflicts = detect_over_allocations(&schedule);

    assert!(!conflicts.is_empty());
    assert_eq!(conflicts[0].resource_id, "dev");
    assert_eq!(conflicts[0].required, 2.0);  // 200%
}

#[test]
fn partial_allocation_no_conflict() {
    let schedule = schedule_project(vec![
        task("A", 10).assign("dev@50%"),
        task("B", 10).assign("dev@50%"),
    ]);

    let conflicts = detect_over_allocations(&schedule);

    assert!(conflicts.is_empty());  // 100% total = OK
}

#[test]
fn strict_mode_errors_on_conflict() {
    let result = schedule_project_with_options(
        vec![task("A", 10).assign("dev"), task("B", 10).assign("dev")],
        ScheduleOptions { over_allocation_mode: OverAllocationMode::Error, .. },
    );

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ScheduleError::OverAllocation { .. }));
}

#[test]
fn auto_level_resolves_conflict() {
    let schedule = schedule_project_with_options(
        vec![
            task("A", 10).assign("dev").start(date("2026-02-01")),
            task("B", 10).assign("dev").start(date("2026-02-01")),
        ],
        ScheduleOptions { over_allocation_mode: OverAllocationMode::AutoLevel, .. },
    ).unwrap();

    // After leveling, no conflict
    let conflicts = detect_over_allocations(&schedule);
    assert!(conflicts.is_empty());
}
```

---

### Q23. Resource Leveling with Constraints

**Recommended Design:**

**Constraints are inviolable.** Leveling fails (with warning) rather than violating constraints.

```rust
pub struct ConstraintViolation {
    pub task_id: TaskId,
    pub constraint_type: ConstraintType,
    pub constraint_value: NaiveDate,
    pub would_be_value: NaiveDate,
    pub violation_days: i64,
}

pub enum LevelingFailureReason {
    /// Task has constraint that would be violated
    ConstraintViolation(ConstraintViolation),
    /// Task is anchored (in-progress or actual dates)
    TaskAnchored(TaskId),
    /// All tasks have higher priority
    NoPriorityWinner,
    /// Would create circular dependency
    CircularDependency,
}

fn level_with_constraints(
    schedule: &mut Schedule,
    config: &LevelingConfig,
) -> LevelingResult {
    let mut result = LevelingResult::new();

    for conflict in detect_conflicts(schedule) {
        let resolution = try_resolve_conflict(schedule, &conflict, config);

        match resolution {
            Ok(change) => {
                apply_change(schedule, &change);
                result.changes.push(change);
            }
            Err(reason) => {
                result.unresolved.push(UnresolvedConflict {
                    conflict: conflict.clone(),
                    reason,
                });
            }
        }
    }

    if !result.unresolved.is_empty() {
        result.suggestions = generate_suggestions(&result.unresolved);
    }

    result
}

fn try_resolve_conflict(
    schedule: &Schedule,
    conflict: &ResourceConflict,
    config: &LevelingConfig,
) -> Result<LevelingChange, LevelingFailureReason> {
    let tasks = [&conflict.task_a, &conflict.task_b];

    // Sort by priority (highest first)
    let mut candidates: Vec<_> = tasks.iter()
        .map(|id| (id, compute_priority(&schedule.tasks[*id], &config.priority_rules)))
        .collect();
    candidates.sort_by_key(|(_, p)| std::cmp::Reverse(*p));

    // Try to delay lower-priority tasks
    for (task_id, _) in candidates.iter().skip(1) {
        let task = &schedule.tasks[*task_id];

        // Check if anchored
        if task.is_anchored() {
            continue;
        }

        // Calculate required delay
        let delay_days = conflict.overlap_days + 1;
        let new_start = task.start + Duration::days(delay_days as i64);
        let new_finish = task.finish + Duration::days(delay_days as i64);

        // Check constraints
        if let Some(must_start) = task.must_start_on {
            if new_start != must_start {
                return Err(LevelingFailureReason::ConstraintViolation(
                    ConstraintViolation {
                        task_id: (*task_id).clone(),
                        constraint_type: ConstraintType::MustStartOn,
                        constraint_value: must_start,
                        would_be_value: new_start,
                        violation_days: (new_start - must_start).num_days(),
                    }
                ));
            }
        }

        if let Some(must_finish) = task.must_finish_on {
            if new_finish > must_finish {
                return Err(LevelingFailureReason::ConstraintViolation(
                    ConstraintViolation {
                        task_id: (*task_id).clone(),
                        constraint_type: ConstraintType::MustFinishOn,
                        constraint_value: must_finish,
                        would_be_value: new_finish,
                        violation_days: (new_finish - must_finish).num_days(),
                    }
                ));
            }
        }

        // Can delay this task
        return Ok(LevelingChange::DelayTask {
            task_id: (*task_id).clone(),
            original_start: task.start,
            new_start,
            reason: format!("Resource conflict with {}", conflict.other_task(*task_id)),
        });
    }

    // All tasks have constraints or are anchored
    Err(LevelingFailureReason::NoPriorityWinner)
}

fn generate_suggestions(unresolved: &[UnresolvedConflict]) -> Vec<String> {
    let mut suggestions = vec![];

    for conflict in unresolved {
        match &conflict.reason {
            LevelingFailureReason::ConstraintViolation(cv) => {
                suggestions.push(format!(
                    "Consider relaxing constraint on '{}': {} {}",
                    cv.task_id, cv.constraint_type, cv.constraint_value
                ));
                suggestions.push(format!(
                    "Or add {} more days to project timeline to accommodate both constraints",
                    cv.violation_days.abs()
                ));
            }
            LevelingFailureReason::TaskAnchored(task_id) => {
                suggestions.push(format!(
                    "Task '{}' is in-progress and cannot be moved. \
                     Consider adding another resource to share the work.",
                    task_id
                ));
            }
            LevelingFailureReason::NoPriorityWinner => {
                suggestions.push(format!(
                    "Both '{}' and '{}' have equal priority. \
                     Set explicit priority: 'priority: 100' on one task.",
                    conflict.conflict.task_a, conflict.conflict.task_b
                ));
            }
            _ => {}
        }
    }

    suggestions.sort();
    suggestions.dedup();
    suggestions
}
```

**Rationale:**

1. **Constraints represent business requirements** - "Must finish by Feb 15" is a real deadline. Breaking it defeats the purpose.

2. **Leveling is optimization, not magic** - If constraints make schedule infeasible, user needs to know.

3. **Clear failure reporting** - Don't silently fail. Explain exactly why leveling couldn't succeed.

4. **Actionable suggestions** - Help user understand options: relax constraint, add resources, extend timeline.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Constraints inviolable | Respects user intent | May report impossible |
| Soft constraints | More flexible | Unclear which are "real" |
| Constraint priorities | Nuanced control | Complex configuration |
| Auto-relax constraints | Always produces result | Dangerous - may miss deadlines |

**Implementation Notes:**

1. Check constraints before applying any change
2. Aggregate multiple constraint violations into single report
3. Consider constraint "hardness" in v2.0 (must vs should)
4. What-if integration: "If we add 1 dev, this becomes feasible"
5. Visual indicator in Gantt for tasks blocked by constraints

**Test Strategy:**

```rust
#[test]
fn leveling_respects_must_finish_on() {
    let mut schedule = schedule_project(vec![
        task("A", 10).assign("dev").start(date("2026-02-01")),
        task("B", 10).assign("dev").start(date("2026-02-01"))
            .must_finish_on(date("2026-02-15")),
    ]);

    let result = level_resources(&mut schedule, &LevelingConfig::default());

    // B cannot be delayed (would violate constraint), so A must be
    assert!(schedule["A"].start > date("2026-02-01"));
    assert!(schedule["B"].finish <= date("2026-02-15"));
}

#[test]
fn leveling_reports_constraint_conflict() {
    let mut schedule = schedule_project(vec![
        task("A", 15).assign("dev").must_finish_on(date("2026-02-15")),
        task("B", 15).assign("dev").must_finish_on(date("2026-02-15")),
    ]);

    let result = level_resources(&mut schedule, &LevelingConfig::default());

    assert!(!result.unresolved.is_empty());
    assert!(result.suggestions.iter().any(|s| s.contains("relaxing constraint")));
}

#[test]
fn leveling_suggests_adding_resources() {
    let mut schedule = schedule_project(vec![
        task("A", 10).assign("dev").in_progress(),
        task("B", 10).assign("dev"),
    ]);

    let result = level_resources(&mut schedule, &LevelingConfig::default());

    // A is anchored, but we should still suggest solutions
    assert!(result.suggestions.iter().any(|s| s.contains("another resource")));
}
```

---

## SECTION H: BDD/SAT INTEGRATION
**Current Confidence: 50% → Target: 95%**

### Q24. BDD/SAT Necessity for MVP

**Recommended Design:**

**Defer BDD/SAT to v2.0.** Heuristics and CPM are sufficient for v1.0.

**Rationale:**

1. **80/20 rule** - CPM + priority-based leveling solves 80% of use cases. BDD/SAT adds 20% more capability for 5x complexity.

2. **Learning curve** - BDD/SAT concepts are foreign to most project managers. Core features should be intuitive.

3. **Performance uncertainty** - Encoding 1000 tasks × 365 days = 365,000 boolean variables. Performance is unproven.

4. **Dependency burden** - OxiDD or Z3 add significant compile time and binary size.

5. **Market validation first** - Ship v1.0, get user feedback. If "what-if" analysis is highly requested, invest in v2.0.

**What-If Without BDD:**

Simple what-if analysis can use re-scheduling:

```rust
pub fn what_if_add_resource(
    schedule: &Schedule,
    resource: &Resource,
) -> WhatIfResult {
    let mut modified = schedule.clone();
    modified.add_resource(resource);

    // Re-run CPM and leveling
    let new_schedule = schedule_and_level(&modified);

    WhatIfResult {
        original_finish: schedule.project_finish,
        new_finish: new_schedule.project_finish,
        duration_delta: new_schedule.project_duration - schedule.project_duration,
        conflicts_resolved: count_resolved_conflicts(schedule, &new_schedule),
    }
}
```

This is O(n log n) instead of BDD's O(1) for queries, but fast enough for interactive use.

**v2.0 BDD Features (placeholder):**

If demand exists, v2.0 could add:
- Formal feasibility checking
- Constraint criticality analysis
- Valid schedule enumeration
- Optimal schedule search (within bounds)

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Include in v1.0 | Powerful analysis | Complexity, risk, delay |
| Defer to v2.0 | Faster v1.0 ship | Missing advanced features |
| Never implement | Simplicity | Can't match enterprise tools |
| External tool integration | Use existing SAT solvers | Integration complexity |

**Implementation Notes:**

1. Mark placeholder in architecture: `mod bdd;` with `// TODO: v2.0`
2. Design interfaces that could use BDD later
3. Document the "simple what-if" approach for v1.0
4. Track user requests for advanced analysis features

**Test Strategy:**

```rust
#[test]
fn simple_whatif_shows_impact() {
    let schedule = create_overloaded_schedule();

    let result = what_if_add_resource(&schedule, &Resource::new("dev2"));

    assert!(result.duration_delta.num_days() < 0);  // Project shorter
    assert!(result.conflicts_resolved > 0);
}
```

---

### Q25. BDD Encoding of Constraints (If Including in v1.0)

**Recommended Design:**

**Deferred to v2.0.** Placeholder design for future:

```rust
// Placeholder - not implemented in v1.0
pub mod bdd {
    use oxidd::bdd::BDD;

    pub struct ScheduleBDD {
        manager: BDD,
        task_vars: HashMap<TaskId, Vec<BDDVar>>,  // task → [var per time slot]
    }

    impl ScheduleBDD {
        pub fn new(project: &Project, time_horizon: usize) -> Self {
            // Encode:
            // - Each task starts exactly once
            // - Dependencies: start_B >= finish_A
            // - Resource capacity: sum of concurrent tasks <= capacity
            todo!("v2.0 implementation")
        }

        pub fn is_feasible(&self) -> bool {
            !self.manager.is_false()
        }

        pub fn count_solutions(&self) -> u64 {
            self.manager.sat_count()
        }
    }
}
```

**Encoding Strategy (for v2.0):**

1. **Time discretization** - Daily granularity (weekly for large projects)
2. **Task variables** - `task_i_starts_day_d` boolean for each task × day
3. **Exactly-one constraint** - Each task starts exactly one day
4. **Dependency constraints** - Implication chains
5. **Resource constraints** - Cardinality constraints or sequential counters

**Test Strategy (deferred):**

```rust
#[test]
#[ignore]  // v2.0
fn bdd_feasibility_check() {
    let project = create_infeasible_project();
    let bdd = ScheduleBDD::new(&project, 365);

    assert!(!bdd.is_feasible());
}
```

---

### Q26. BDD What-If Analysis

**Recommended Design:**

**Deferred to v2.0.** Simple CPM-based what-if for v1.0 (see Q24).

**v2.0 Design Sketch:**

```rust
// v2.0 placeholder
pub struct WhatIfAnalysis {
    base_bdd: ScheduleBDD,
}

impl WhatIfAnalysis {
    pub fn add_resource(&self, resource: &Resource) -> WhatIfResult {
        // Modify capacity constraints in BDD
        // Recompute solution count
        // Compare earliest finish dates
        todo!("v2.0")
    }

    pub fn remove_task(&self, task_id: &TaskId) -> WhatIfResult {
        // Remove task constraints from BDD
        // Recompute
        todo!("v2.0")
    }

    pub fn add_constraint(&self, constraint: &Constraint) -> WhatIfResult {
        // Add constraint to BDD
        // Check if still feasible
        todo!("v2.0")
    }
}
```

**User Value Proposition (for v2.0):**

- "How many valid schedules exist?" → Measures flexibility
- "Which constraint is most restrictive?" → Identifies bottlenecks
- "Is this deadline achievable?" → Instant yes/no vs re-running CPM

**Test Strategy (deferred):**

```rust
#[test]
#[ignore]  // v2.0
fn whatif_add_resource_increases_solutions() {
    let analysis = WhatIfAnalysis::new(&project);

    let before = analysis.base_bdd.count_solutions();
    let after = analysis.add_resource(&new_resource).solution_count;

    assert!(after >= before);
}
```

---

## SECTION I: TASKJUGGLER COMPATIBILITY
**Current Confidence: 75% → Target: 95%**

### Q27. TJP Feature Subset for v1.0

**Recommended Design:**

**Tier 1 (Core) for v1.0**, with graceful degradation for Tier 2/3.

**Tier 1: Core Scheduling (v1.0 - Must Have)**

| Feature | Status | Notes |
|---------|--------|-------|
| Tasks (hierarchical) | ✅ Implemented | Full WBS support |
| Resources (capacity, efficiency) | ✅ Implemented | Basic model |
| Dependencies (FS, SS, FF, SF) | ✅ Implemented | With lag |
| Effort-based scheduling | ✅ Implemented | effort attribute |
| Calendars (working hours) | ✅ Implemented | workinghours, holiday |
| Duration-based tasks | ✅ Implemented | duration attribute |
| Basic constraints | ✅ Implemented | must_start_on |

**Tier 2: Advanced Features (v1.1 - Should Have)**

| Feature | Status | Notes |
|---------|--------|-------|
| Scenarios | ❌ Planned | What-if comparison |
| Resource limits | ❌ Planned | maxend, limits |
| Task flags | ⚠️ Partial | milestone yes, flags |
| Shifts | ❌ Planned | Time zones |
| Vacation tracking | ❌ Planned | vacation attribute |
| Cost accounting | ❌ Planned | rate, cost tracking |

**Tier 3: Optional (v2.0+ - Nice to Have)**

| Feature | Status | Notes |
|---------|--------|-------|
| Bookings | ❌ Deferred | Actual time entries |
| Journal entries | ❌ Deferred | Notes, logs |
| Rich text reports | ❌ Deferred | HTML generation |
| Time sheets | ❌ Deferred | Time tracking |
| Charge sets | ❌ Deferred | Complex cost models |

**Migration Path for Unsupported Features:**

```rust
pub enum UnsupportedFeatureHandling {
    /// Parse but ignore (preserve in comments)
    Warn,
    /// Skip entirely, log warning
    Skip,
    /// Fail parsing
    Error,
}

fn parse_tjp_feature(feature: &str, handling: UnsupportedFeatureHandling) -> ParseResult {
    if !is_supported(feature) {
        match handling {
            UnsupportedFeatureHandling::Warn => {
                warnings.push(format!("Unsupported TJP feature '{}' - ignored", feature));
                // Store as comment/metadata for round-trip
                Ok(ParsedFeature::Ignored(feature.to_string()))
            }
            UnsupportedFeatureHandling::Skip => {
                log::warn!("Skipping unsupported feature: {}", feature);
                Ok(ParsedFeature::Skipped)
            }
            UnsupportedFeatureHandling::Error => {
                Err(ParseError::UnsupportedFeature(feature.to_string()))
            }
        }
    } else {
        parse_supported_feature(feature)
    }
}
```

**Rationale:**

1. **Tier 1 covers most projects** - Basic scheduling is what 90% of users need.

2. **Graceful degradation over strict parsing** - Import should work with warnings, not fail.

3. **Clear roadmap** - Users know what's coming in v1.1, v2.0.

4. **Focus on core quality** - Better to have excellent Tier 1 than mediocre Tier 1+2.

**Trade-offs:**

| Scope | Pros | Cons |
|-------|------|------|
| Tier 1 only | Fast ship, focused | May not import complex TJP files |
| Tier 1+2 | More compatibility | Longer development, more bugs |
| Full TJP | Complete compatibility | Years of work, feature bloat |

**Implementation Notes:**

1. Document supported features in README
2. `utf8proj check project.tjp` reports unsupported features
3. Store unsupported features as comments for potential round-trip
4. Track feature requests to prioritize Tier 2 order

**Test Strategy:**

```rust
#[test]
fn tier1_features_parse_correctly() {
    let tjp = r#"
        project "Test" 2026-01-01 +3m {}
        resource dev "Developer" {}
        task main "Main" {
            task sub "Sub" {
                effort 5d
                allocate dev
            }
            depends !sub
        }
    "#;

    let project = parse_tjp(tjp).unwrap();
    assert_eq!(project.tasks.len(), 2);
}

#[test]
fn unsupported_features_warn_not_error() {
    let tjp = r#"
        project "Test" 2026-01-01 +3m {}
        scenario plan "Plan" {}  // Unsupported
        task main "Main" { duration 5d }
    "#;

    let result = parse_tjp(tjp);
    assert!(result.is_ok());
    assert!(result.unwrap().warnings.iter().any(|w| w.contains("scenario")));
}
```

---

### Q28. TJP Parser Strategy

**Recommended Design:**

**Option C: Hybrid - Parse All, Warn Unsupported**

```rust
pub struct TjpParser {
    /// How to handle unsupported features
    pub unsupported_handling: UnsupportedFeatureHandling,

    /// Preserve unsupported constructs for round-trip
    pub preserve_unsupported: bool,

    /// Collected warnings during parsing
    pub warnings: Vec<ParseWarning>,
}

impl TjpParser {
    pub fn parse(&mut self, input: &str) -> Result<TjpProject, ParseError> {
        let ast = self.parse_full_grammar(input)?;
        let project = self.convert_to_model(ast)?;
        Ok(project)
    }

    fn parse_full_grammar(&mut self, input: &str) -> Result<TjpAst, ParseError> {
        // Parse ALL TJP syntax, even unsupported
        // This prevents "unrecognized token" errors
        let pairs = TjpGrammar::parse(Rule::project, input)?;
        self.build_ast(pairs)
    }

    fn convert_to_model(&mut self, ast: TjpAst) -> Result<TjpProject, ParseError> {
        let mut project = TjpProject::new();

        for node in ast.nodes {
            match node {
                AstNode::Task(t) => project.tasks.push(self.convert_task(t)?),
                AstNode::Resource(r) => project.resources.push(self.convert_resource(r)?),
                AstNode::Scenario(s) => {
                    self.handle_unsupported("scenario", &s);
                    if self.preserve_unsupported {
                        project.preserved.push(PreservedNode::Scenario(s));
                    }
                }
                AstNode::Report(r) => {
                    self.handle_unsupported("report", &r);
                    // Reports are output-only, safe to ignore
                }
                // ... other node types
            }
        }

        Ok(project)
    }

    fn handle_unsupported(&mut self, feature: &str, _node: &impl Debug) {
        match self.unsupported_handling {
            UnsupportedFeatureHandling::Warn => {
                self.warnings.push(ParseWarning::UnsupportedFeature {
                    feature: feature.to_string(),
                    suggestion: get_suggestion(feature),
                });
            }
            UnsupportedFeatureHandling::Error => {
                // Handled at caller
            }
            UnsupportedFeatureHandling::Skip => {
                log::debug!("Skipping unsupported: {}", feature);
            }
        }
    }
}
```

**Grammar Coverage:**

```pest
// tjp.pest - Parse full TJP syntax

project = { "project" ~ string ~ date ~ duration_spec ~ "{" ~ project_body ~ "}" }

project_body = { (task | resource | scenario | report | account | ... )* }

// Parse scenarios even though not supported
scenario = { "scenario" ~ identifier ~ string ~ "{" ~ scenario_body ~ "}" }

// Parse reports even though not supported
report = { ("taskreport" | "resourcereport" | "textreport") ~ ... }
```

**Round-Trip Preservation:**

```rust
pub struct PreservedNode {
    pub original_text: String,
    pub line_number: usize,
    pub node_type: String,
}

impl TjpProject {
    pub fn to_tjp(&self) -> String {
        let mut output = String::new();

        // Write supported content
        output.push_str(&self.render_project_header());
        for resource in &self.resources {
            output.push_str(&resource.to_tjp());
        }
        for task in &self.tasks {
            output.push_str(&task.to_tjp());
        }

        // Write preserved unsupported content as comments
        if !self.preserved.is_empty() {
            output.push_str("\n# Preserved unsupported content:\n");
            for node in &self.preserved {
                output.push_str(&format!("# {}\n", node.original_text));
            }
        }

        output
    }
}
```

**Rationale:**

1. **Full grammar prevents parse errors** - Users won't see cryptic "unexpected token" errors on valid TJP files.

2. **Warnings are educational** - Users learn what's not supported and can adjust.

3. **Preservation enables migration** - Import TJP, work in utf8proj, export back without losing unsupported sections.

4. **No bug-for-bug compatibility** - We parse TJP syntax, not replicate TJ3 bugs.

**Trade-offs:**

| Strategy | Pros | Cons |
|----------|------|------|
| Full grammar + warn | Maximum compatibility | Large grammar to maintain |
| Subset grammar | Simpler parser | Fails on many TJP files |
| Error on unsupported | Clear boundaries | Poor UX |
| Ignore unsupported | Silent operation | Users don't know what's lost |

**Implementation Notes:**

1. Use pest for grammar (already in use for native DSL)
2. Grammar can be auto-generated from TJP documentation
3. Test against real TJP files from msproject-to-taskjuggler examples
4. Performance: Parse time should be < 100ms for 1000-task files

**Test Strategy:**

```rust
#[test]
fn parse_complex_tjp_with_warnings() {
    let tjp = include_str!("../../examples/ttg_02_deps.tjp");

    let result = TjpParser::new()
        .with_handling(UnsupportedFeatureHandling::Warn)
        .parse(tjp);

    assert!(result.is_ok());
    let project = result.unwrap();
    assert!(!project.tasks.is_empty());

    // Check warnings were generated for any unsupported features
    // (depends on what's in the example file)
}

#[test]
fn round_trip_preserves_unsupported() {
    let original_tjp = r#"
        project "Test" 2026-01-01 +1m {}
        scenario plan "Plan" { active yes }
        task main "Main" { duration 5d }
    "#;

    let project = TjpParser::new()
        .with_preserve(true)
        .parse(original_tjp)
        .unwrap();

    let exported = project.to_tjp();

    // Scenario should be preserved as comment
    assert!(exported.contains("scenario"));
    assert!(exported.contains("Plan"));
}
```

---

### Q29. TJP to .proj Conversion

**Recommended Design:**

**One-time migration as primary workflow**, with export-to-TJP for interoperability.

```bash
# Import TJP → .proj (one-time migration)
utf8proj import legacy.tjp --output=modern.proj

# Work in .proj format
utf8proj schedule modern.proj

# Export back to TJP if needed (for TJ3 users)
utf8proj export modern.proj --format=tjp --output=for_tj3.tjp
```

**Conversion Pipeline:**

```rust
pub fn import_tjp(tjp_path: &Path, proj_path: &Path) -> Result<ImportResult, ImportError> {
    // 1. Parse TJP
    let tjp_project = TjpParser::new()
        .with_handling(UnsupportedFeatureHandling::Warn)
        .with_preserve(true)
        .parse_file(tjp_path)?;

    // 2. Convert to native model
    let native_project = convert_tjp_to_native(&tjp_project)?;

    // 3. Validate
    let validation = validate_project(&native_project)?;

    // 4. Write .proj
    let proj_content = native_project.to_proj_string();
    fs::write(proj_path, &proj_content)?;

    Ok(ImportResult {
        tasks_imported: native_project.tasks.len(),
        resources_imported: native_project.resources.len(),
        warnings: tjp_project.warnings,
        unsupported_preserved: tjp_project.preserved.len(),
    })
}

fn convert_tjp_to_native(tjp: &TjpProject) -> Result<Project, ConversionError> {
    let mut project = Project::new(&tjp.name);
    project.start = tjp.start;

    // Convert resources
    for tjp_resource in &tjp.resources {
        project.resources.push(Resource {
            id: tjp_resource.id.clone(),
            name: tjp_resource.name.clone(),
            capacity: tjp_resource.limits.daily_max.unwrap_or(1.0),
            efficiency: tjp_resource.efficiency.unwrap_or(1.0),
            // ... map other attributes
        });
    }

    // Convert tasks (recursive for hierarchy)
    for tjp_task in &tjp.tasks {
        project.tasks.push(convert_task(tjp_task)?);
    }

    Ok(project)
}

fn convert_task(tjp: &TjpTask) -> Result<Task, ConversionError> {
    let mut task = Task::new(&tjp.id, &tjp.name);

    // Duration or effort
    if let Some(duration) = tjp.duration {
        task.duration = Some(duration);
    } else if let Some(effort) = tjp.effort {
        task.effort = Some(effort);
    }

    // Dependencies
    for dep in &tjp.depends {
        task.dependencies.push(Dependency {
            predecessor: dep.task_id.clone(),
            dep_type: convert_dep_type(&dep.dep_type),
            lag: dep.gap.unwrap_or(Duration::zero()),
        });
    }

    // Resource assignments
    for alloc in &tjp.allocations {
        task.assignments.push(Assignment {
            resource_id: alloc.resource_id.clone(),
            percentage: alloc.percentage.unwrap_or(100),
        });
    }

    // Constraints
    if let Some(start) = tjp.start {
        task.constraints.push(Constraint::MustStartOn(start));
    }

    // Children (recursive)
    for child in &tjp.children {
        task.children.push(convert_task(child)?);
    }

    Ok(task)
}
```

**Export to TJP:**

```rust
pub fn export_to_tjp(project: &Project, tjp_path: &Path) -> Result<ExportResult, ExportError> {
    let mut output = String::new();

    // Project header
    output.push_str(&format!(
        "project \"{}\" {} +{}m {{\n",
        project.name,
        project.start.format("%Y-%m-%d"),
        project.duration_months(),
    ));

    // Timezone
    output.push_str("  timezone \"UTC\"\n");
    output.push_str("}\n\n");

    // Resources
    for resource in &project.resources {
        output.push_str(&format!(
            "resource {} \"{}\" {{\n",
            resource.id, resource.name
        ));
        if resource.efficiency != 1.0 {
            output.push_str(&format!("  efficiency {}\n", resource.efficiency));
        }
        output.push_str("}\n\n");
    }

    // Tasks
    for task in &project.tasks {
        output.push_str(&render_task_tjp(task, 0));
    }

    fs::write(tjp_path, &output)?;

    Ok(ExportResult {
        tasks_exported: project.task_count(),
        warnings: vec![],
    })
}

fn render_task_tjp(task: &Task, indent: usize) -> String {
    let ind = "  ".repeat(indent);
    let mut output = format!("{}task {} \"{}\" {{\n", ind, task.id, task.name);

    if let Some(duration) = task.duration {
        output.push_str(&format!("{}  duration {}d\n", ind, duration.num_days()));
    } else if let Some(effort) = task.effort {
        output.push_str(&format!("{}  effort {}d\n", ind, effort.num_days()));
    }

    for dep in &task.dependencies {
        let dep_str = match dep.dep_type {
            DependencyType::FS => format!("{}", dep.predecessor),
            DependencyType::SS => format!("!{}", dep.predecessor),
            DependencyType::FF => format!("{}~", dep.predecessor),
            DependencyType::SF => format!("!{}~", dep.predecessor),
        };
        output.push_str(&format!("{}  depends {}\n", ind, dep_str));
    }

    for assign in &task.assignments {
        output.push_str(&format!("{}  allocate {}\n", ind, assign.resource_id));
    }

    // Children
    for child in &task.children {
        output.push_str(&render_task_tjp(child, indent + 1));
    }

    output.push_str(&format!("{}}}\n", ind));
    output
}
```

**Round-Trip Guarantee:**

**NOT guaranteed.** Semantic equivalence is the goal, not textual identity.

```
legacy.tjp → import → modern.proj → export → for_tj3.tjp
                                              ↓
                                        Semantically equivalent
                                        Not textually identical
```

Reasons:
- Formatting differences (whitespace, ordering)
- Attribute defaults (explicit vs implicit)
- Unsupported features (preserved as comments)
- Native DSL features (may not have TJP equivalent)

**Rationale:**

1. **Migration is primary use case** - Most users want to move FROM TJP, not stay in it.

2. **TJP export for interoperability** - Share with TJ3 users without requiring them to switch.

3. **Semantic equivalence is practical** - Textual round-trip is unrealistic and unnecessary.

4. **Warnings for data loss** - If export can't represent something, warn user.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| One-way import | Simple, focused | Can't go back to TJP |
| Round-trip | Full compatibility | Complex, maintenance burden |
| Semantic equivalence | Practical balance | Some information loss |
| TJP as native format | No conversion needed | Tied to TJ ecosystem |

**Implementation Notes:**

1. Import preserves TJP-specific attributes in metadata
2. Export uses preserved metadata when available
3. Warn if .proj has features without TJP equivalent
4. Test with real-world TJP files from examples directory
5. Consider `--strict` mode that errors on any data loss

**Test Strategy:**

```rust
#[test]
fn import_export_semantic_equivalence() {
    // Import TJP
    let tjp_path = Path::new("examples/ttg_02_deps.tjp");
    let proj_path = temp_path("converted.proj");

    import_tjp(tjp_path, &proj_path).unwrap();

    // Schedule both
    let original_schedule = schedule_tjp(tjp_path).unwrap();
    let converted_schedule = schedule_proj(&proj_path).unwrap();

    // Same results
    assert_eq!(original_schedule.project_finish, converted_schedule.project_finish);
    assert_eq!(original_schedule.critical_path, converted_schedule.critical_path);
}

#[test]
fn export_produces_valid_tjp() {
    let project = create_test_project();
    let tjp_path = temp_path("export.tjp");

    export_to_tjp(&project, &tjp_path).unwrap();

    // Should parse with our parser
    let reimported = TjpParser::new().parse_file(&tjp_path).unwrap();
    assert_eq!(reimported.tasks.len(), project.tasks.len());

    // Should be valid for tj3 (if available)
    if which::which("tj3").is_ok() {
        let output = Command::new("tj3")
            .arg("--check")
            .arg(&tjp_path)
            .output()
            .unwrap();
        assert!(output.status.success());
    }
}

#[test]
fn import_warns_on_unsupported() {
    let tjp = r#"
        project "Test" 2026-01-01 +1m {}
        scenario optimistic "Optimistic" {}
        task main "Main" { duration 5d }
    "#;

    let result = import_tjp_string(tjp);

    assert!(result.warnings.iter().any(|w| w.contains("scenario")));
}
```

---

## SUMMARY - PART 2

### Decisions Made

| Question | Decision | Confidence |
|----------|----------|------------|
| Q18. Excel library | rust_xlsxwriter (write-only, full features) | 95% |
| Q19. Excel structure | 4 sheets: Task List, Gantt, Resources, Variance | 95% |
| Q20. Chart generation | Generate in Rust (Gantt, histogram, burndown) | 95% |
| Q21. Leveling algorithm | Critical path priority heuristic + iteration | 95% |
| Q22. Over-allocation | Warn by default, configurable modes | 95% |
| Q23. Leveling + constraints | Constraints inviolable, fail with suggestions | 95% |
| Q24. BDD/SAT necessity | **Deferred to v2.0** | N/A |
| Q25. BDD encoding | **Deferred to v2.0** | N/A |
| Q26. BDD what-if | **Deferred to v2.0** (use CPM re-scheduling) | N/A |
| Q27. TJP feature subset | Tier 1 for v1.0, graceful degradation | 95% |
| Q28. TJP parser strategy | Hybrid: parse all, warn unsupported, preserve | 95% |
| Q29. TJP conversion | One-time migration primary, semantic equivalence | 95% |

### Section Confidence After Survey

| Section | Before | After |
|---------|--------|-------|
| F: Excel Export | 70% | 95% |
| G: Resource Leveling | 65% | 95% |
| H: BDD/SAT | 50% | Deferred |
| I: TJP Compatibility | 75% | 95% |

---

## OVERALL SURVEY COMPLETION

### Final Confidence Levels

| Component | Initial | After Survey |
|-----------|---------|--------------|
| Progress-Aware CPM | 75% | 95% |
| Container Derivation | 80% | 95% |
| History - Sidecar | 70% | 95% |
| History - Embedded | 60% | Deferred |
| Playback Engine | 65% | 95% |
| Excel Export | 70% | 95% |
| Resource Leveling | 65% | 95% |
| BDD/SAT Integration | 50% | Deferred |
| TJP Compatibility | 75% | 95% |
| **Overall** | **85%** | **95%** |

### Deferred Features (v2.0+)

1. **Embedded History** - Complexity vs value doesn't justify for v1.0
2. **BDD/SAT Integration** - Advanced analysis, needs market validation
3. **TJP Tier 2/3 Features** - Scenarios, bookings, time sheets

### Implementation Priority

Based on survey answers, recommended implementation order:

1. **Phase 1**: Core CPM with progress awareness (Q1-Q5)
2. **Phase 2**: Container task derivation (Q6-Q8)
3. **Phase 3**: Resource leveling (Q21-Q23)
4. **Phase 4**: TJP parser enhancement (Q27-Q29)
5. **Phase 5**: History system - sidecar (Q9-Q11)
6. **Phase 6**: Playback engine (Q15-Q17)
7. **Phase 7**: Excel export (Q18-Q20)

---

**END OF DESIGN REFINEMENT SURVEY**

**Next Steps:**
1. Integrate decisions into UTF8PROJ_RFC_MASTER.md
2. Create implementation tasks from survey answers
3. Begin Phase 1 development with test-driven approach
