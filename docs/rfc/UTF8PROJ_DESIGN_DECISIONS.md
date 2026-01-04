# utf8proj: Design Decisions (Finalized)
**Version:** 1.0  
**Date:** 2026-01-04  
**Status:** Authoritative - Ready for Implementation  
**Confidence:** 95% (from 85%)

---

## EXECUTIVE SUMMARY

This document consolidates design decisions from the completed Design Refinement Survey, representing consensus across multiple LLM architects (AI Studio, DeepSeek, Claude Code CLI). All areas previously below 80% confidence have been resolved.

**Key Achievements:**
- ✅ Progress-aware CPM algorithm finalized (Linear + Explicit Override)
- ✅ Container derivation rules defined (Weighted by duration)
- ✅ History system architecture clarified (YAML sidecar, deferred embedded)
- ✅ Excel export strategy confirmed (rust_xlsxwriter, 4-sheet workbook)
- ✅ Resource leveling algorithm chosen (Critical path priority heuristic)
- ✅ BDD/SAT integration deferred to v2.0
- ✅ TaskJuggler compatibility scope set (Tier 1 for v1.0)

---

## PART I: PROGRESS-AWARE CPM ALGORITHM
**Confidence: 75% → 95%**

### Decision A1: Effective Duration Calculation

**CHOSEN APPROACH:** Linear interpolation with explicit override

**Implementation:**
```rust
pub enum DurationCalculation {
    Linear,                           // Default: remaining = original × (1 - pct/100)
    Explicit { remaining: Duration }, // User override
}

impl Task {
    fn effective_remaining_duration(&self) -> Duration {
        match &self.duration_calculation {
            DurationCalculation::Explicit { remaining } => *remaining,
            DurationCalculation::Linear => {
                let pct = self.percent_complete.unwrap_or(0) as f64 / 100.0;
                self.duration.mul_f64(1.0 - pct)
            }
        }
    }
}
```

**DSL Syntax:**
```proj
task backend "Backend API" {
    duration: 20d
    complete: 50%
    remaining: 12d    # Explicit override when linear fails
}
```

**Rationale:**
- Linear is industry standard and intuitive
- Explicit override handles "percentage paradox" (80% done, 50% remaining)
- Avoids S-curve complexity without proven demand
- PM retains control when estimates diverge from reality

**Test Requirements:**
- Linear calculation at 0%, 50%, 100% completion
- Explicit override takes precedence
- Validation: remaining ≤ original duration
- Validation: 100% complete → remaining must be 0

---

### Decision A2: Dependencies with Partial Completion

**CHOSEN APPROACH:** Standard Finish-to-Start with 100% completion requirement

**Implementation:**
```rust
// Dependency satisfied when predecessor reaches 100% complete
fn is_dependency_satisfied(pred: &ScheduledTask, dep: &Dependency) -> bool {
    match dep.dep_type {
        DependencyType::FinishToStart => 
            pred.percent_complete == 100 || pred.actual_finish.is_some(),
        // ... other types
    }
}
```

**Rationale:**
- Partial dependencies (`depends: task.75%`) deferred to v2.0
- Simpler graph logic for MVP
- Standard CPM behavior (FS requires 100%)
- Percent-lags can be added later without breaking changes

**Deferred to v2.0:**
```proj
task frontend "Frontend" {
    depends: backend.75%    # Start when backend 75% complete (future)
}
```

---

### Decision A3: Actual Dates vs Dependencies Conflicts

**CHOSEN APPROACH:** Warn and accept reality

**Implementation:**
```rust
// Reality wins, but warn about process violations
if task.actual_start < required_start_from_dependencies {
    warnings.push(ScheduleWarning::ActualDateConflict {
        task_id: task.id,
        actual: task.actual_start,
        required: required_start_from_dependencies,
    });
}
// Use actual_start anyway (respect reality)
```

**CLI Behavior:**
```bash
utf8proj schedule project.proj
# Warning: Task 'frontend' started 2026-02-10 before dependency 'backend' 
#          finished (required: 2026-02-15). Fast-tracking or data error?
```

**Rationale:**
- Projects often fast-track (overlap phases)
- Rejecting actual dates frustrates users
- Warnings alert to potential issues without blocking
- Strict mode (`--strict`) available for validation pipelines

---

### Decision A4: Resource Leveling with Partial Completion

**CHOSEN APPROACH:** Future-only leveling

**Implementation:**
```rust
fn level_resources(schedule: &mut Schedule, status_date: NaiveDate) {
    // Lock completed and in-progress work
    for task in &schedule.tasks {
        if task.percent_complete == 100 || task.actual_finish.is_some() {
            task.locked = true; // Cannot be moved
        }
        if task.actual_start < status_date {
            // Split task: lock past portion, level future portion
            split_and_lock_past(task, status_date);
        }
    }
    
    // Level only unlocked (future) work
    level_unlocked_tasks(schedule);
}
```

**Rationale:**
- Cannot move work already completed
- In-progress tasks split: past fixed, future levelable
- Respects reality while optimizing future allocations

---

### Decision A5: Container Progress Derivation

**CHOSEN APPROACH:** Weighted average by duration with manual override

**Implementation:**
```rust
impl Task {
    fn derived_progress(&self) -> Option<u8> {
        if self.children.is_empty() {
            return self.percent_complete; // Leaf task
        }
        
        let mut total_duration = 0.0;
        let mut weighted_sum = 0.0;
        
        for child in &self.children {
            if let Some(dur) = child.duration_in_days() {
                total_duration += dur;
                let child_pct = child.derived_progress().unwrap_or(0) as f64;
                weighted_sum += dur * child_pct;
            }
        }
        
        if total_duration > 0.0 {
            Some((weighted_sum / total_duration).round() as u8)
        } else {
            None // Cannot derive
        }
    }
}
```

**Manual Override (with warning):**
```proj
task development "Development" {
    complete: 60%    # Manual override (calculated would be 45%)
    
    task frontend { duration: 10d, complete: 100% }
    task backend { duration: 20d, complete: 20% }
}
# Warning: Container 'development' manual progress (60%) differs from 
#          calculated (45%) by 15%. Consider updating.
```

**Rationale:**
- Weighted by duration is accurate (10d@100% ≠ 100d@100%)
- Manual override for high-level estimates
- Warning at >20% mismatch prevents silent inconsistency

---

## PART II: CONTAINER TASK RULES
**Confidence: 80% → 95%**

### Decision B6: Container with Duration Attribute

**CHOSEN APPROACH:** Parser warning, children override

**Implementation:**
```rust
// Parser behavior
if task.has_children() && task.duration.is_some() {
    warnings.push(ParserWarning::ContainerDurationIgnored {
        task_id: task.id,
        specified: task.duration.unwrap(),
        reason: "Container duration derived from children",
    });
    // Clear duration attribute (children define schedule)
    task.duration = None;
}
```

**Rationale:**
- Users may specify container duration as placeholder
- Children represent actual work breakdown
- Warning informs without blocking

---

### Decision B7: Container Progress Manual vs Calculated

**CHOSEN APPROACH:** Calculated by default, manual override with validation

**Implementation:**
```rust
// Container progress resolution
let calculated = container.derived_progress().unwrap_or(0);
let manual = container.percent_complete;

if let Some(manual_pct) = manual {
    let diff = (manual_pct as i32 - calculated as i32).abs();
    if diff > 20 {
        warnings.push(ContainerProgressMismatch {
            task: container.id,
            manual: manual_pct,
            calculated,
            diff,
        });
    }
    return manual_pct; // Manual wins
}
return calculated; // No manual override
```

**Rationale:**
- Trust PM's high-level estimate
- Warn when significant discrepancy (>20%)
- Balance automation with user control

---

### Decision B8: Empty Containers (Placeholders)

**CHOSEN APPROACH:** Valid as zero-duration milestones

**Implementation:**
```proj
task future_phase "Phase 3 - To Be Defined" {
    is_placeholder: true
    # Zero duration, appears as milestone
}
```

**Rationale:**
- Supports roadmap planning
- Non-invasive (zero duration)
- Distinguishable from incomplete data

---

## PART III: HISTORY SYSTEM
**Confidence: 70% → 95%**

### Decision C9: Sidecar File Format

**CHOSEN APPROACH:** YAML with JSON Schema validation

**Format:**
```yaml
# project.proj.history
version: 1.0
schema: https://utf8proj.dev/schemas/history-v1.yaml
project: "Backend API v2.0"

snapshots:
  - id: snap-001
    timestamp: 2026-01-15T09:00:00Z
    type: full
    author: alice@company.com
    message: "Initial project setup"
    sha256: abc123...
    content: |
      project "Backend API v2.0" {
        start: 2026-02-01
        ...
      }
```

**Rationale:**
- YAML: human-readable, supports comments
- JSON Schema: machine validation
- More maintainable than pure JSON for human inspection

**Library:** `serde_yaml` + `schemars`

---

### Decision C10: Sidecar Structure

**CHOSEN APPROACH:** Hybrid - rolling full snapshots + diffs

**Implementation:**
```yaml
snapshots:
  # Full snapshot (every 10 versions or monthly)
  - id: full-001
    type: full
    content: "[full project]"
  
  # Diffs from full-001
  - id: diff-002
    type: diff
    parent: full-001
    operations:
      - add_task: { id: t1, ... }
      - update_progress: { task: t1, to: 50% }
  
  # Next full snapshot
  - id: full-011
    type: full
    parent: full-001
    content: "[full project]"
```

**Configuration:**
```proj
project "My Project" {
    history_config: {
        full_snapshot_interval: 10    # versions
        max_full_snapshots: 5         # retention
    }
}
```

**Rationale:**
- Full snapshots: fast random access
- Diffs: space efficiency
- Rolling: balance speed and storage

---

### Decision C11: Sidecar Synchronization

**CHOSEN APPROACH:** Auto-snapshot on state changes

**CLI Behavior:**
```bash
# Auto-snapshot (default)
utf8proj progress --task=dev --complete=75
# → Creates snapshot automatically

# Opt-out for batch operations
utf8proj progress --import=updates.csv --no-snapshot
utf8proj snapshot  # Manual snapshot after batch

# Git integration
utf8proj snapshot --git-commit -m "Weekly status"
```

**Triggers:**
- Progress updates
- Dependency changes
- Task additions/deletions
- Not triggered: reports, queries

---

### Decision D12-D14: Embedded History

**CHOSEN APPROACH:** Deferred to v2.0

**Rationale:**
- Sidecar YAML simpler and more maintainable
- Embedded adds complexity (merge conflicts, parsing overhead)
- 90% of users covered by Git + Sidecar
- Can add later if demand exists

**Migration Path:** Users can convert sidecar to embedded in future version

---

## PART IV: PLAYBACK ENGINE
**Confidence: 65% → 95%**

### Decision E15: Diff Algorithm

**CHOSEN APPROACH:** Semantic diff (ID-based with similarity matching)

**Implementation:**
```rust
fn schedule_diff(old: &Schedule, new: &Schedule) -> ScheduleDiff {
    let mut changes = Vec::new();
    
    // 1. Match by ID
    for new_task in &new.tasks {
        if let Some(old_task) = old.tasks.get(&new_task.id) {
            if new_task != old_task {
                changes.push(Change::Modified { 
                    id: new_task.id, 
                    diff: diff_fields(old_task, new_task) 
                });
            }
        } else {
            // Check for rename (high similarity)
            if let Some(similar) = find_similar_task(new_task, &old.tasks) {
                changes.push(Change::Renamed {
                    from: similar.id,
                    to: new_task.id,
                });
            } else {
                changes.push(Change::Added { task: new_task });
            }
        }
    }
    
    // 2. Find deletions
    for old_id in old.tasks.keys() {
        if !new.tasks.contains_key(old_id) {
            changes.push(Change::Deleted { id: old_id });
        }
    }
    
    ScheduleDiff { changes }
}
```

**Rationale:**
- Semantic diff more meaningful than text diff
- Rename detection prevents spurious delete+add
- Efficient: O(n) with hash maps

---

### Decision E16: Impact Metrics

**CHOSEN APPROACH:** Key metrics with significance scoring

**Metrics:**
```rust
struct ScheduleImpact {
    // Temporal
    project_duration_delta: Duration,
    finish_date_delta: Duration,
    
    // Critical path
    critical_path_changed: bool,
    critical_path_length_delta: Duration,
    
    // Task changes
    tasks_added: usize,
    tasks_removed: usize,
    tasks_modified: usize,
    
    // Significance score (0-100)
    significance: u8,
}

impl ScheduleImpact {
    fn calculate_significance(&self) -> u8 {
        let mut score = 0;
        if self.project_duration_delta.abs() > Duration::days(5) { score += 40; }
        if self.critical_path_changed { score += 30; }
        if self.tasks_added > 5 { score += 20; }
        if self.tasks_modified > 10 { score += 10; }
        score.min(100)
    }
}
```

**Rationale:**
- Focus on actionable metrics
- Significance scoring highlights major changes
- Configurable thresholds

---

### Decision E17: Playback Format

**CHOSEN APPROACH:** HTML+JavaScript primary, GIF secondary

**Output Options:**
```bash
# Interactive HTML (default)
utf8proj playback project.proj --output=timeline.html

# Shareable GIF
utf8proj playback project.proj --output=animation.gif --fps=2

# SVG sequence (for video editing)
utf8proj playback project.proj --output=frames/ --format=svg
```

**HTML Features:**
- Play/pause/scrub controls
- Click task for details
- Speed control (0.5x, 1x, 2x)
- Export current frame as PNG

**Rationale:**
- HTML: interactive exploration
- GIF: universal sharing
- SVG: professional editing

**Library:** `handlebars` (HTML), `gif` crate (GIF), existing SVG renderer

---

## PART V: EXCEL EXPORT
**Confidence: 70% → 95%**

### Decision F18: Excel Library

**CHOSEN APPROACH:** rust_xlsxwriter

**Rationale:**
- Mature, actively maintained
- Formula support confirmed
- Chart generation adequate
- Pure Rust (no C++ dependencies)

**Alternative considered:** `umya-spreadsheet` (read+write but less stable)

---

### Decision F19: Excel Sheet Structure

**CHOSEN APPROACH:** 4-sheet workbook

**Structure:**
1. **Dashboard:** Summary, KPIs, embedded charts
2. **Task List:** Full task table with formulas
3. **Timeline:** Gantt-style view (conditional formatting)
4. **Resources:** Allocation table, utilization %

**Key Formulas:**
```excel
// Variance (Task List sheet)
=IF(ISBLANK([@[Actual Finish]]), 
    [@[Forecast Finish]] - [@[Planned Finish]], 
    [@[Actual Finish]] - [@[Planned Finish]])

// Status indicator
=IF([@[% Complete]]=100, "✓",
   IF(AND([@Variance]>0, TODAY()>[@[Planned Finish]]), "⚠", 
   "○"))

// Conditional formatting rule
=$G2>0  # Variance > 0 → Red background
```

**Rationale:**
- Dashboard for executives
- Task List for PMs
- Timeline for visualization
- Resources for allocation tracking

---

### Decision F20: Excel Charts

**CHOSEN APPROACH:** Native Excel charts (via rust_xlsxwriter API)

**Charts Generated:**
1. Gantt chart (bar chart with date axis)
2. Progress pie chart
3. Resource utilization histogram

**Implementation:**
```rust
fn create_gantt_chart(wb: &mut Workbook) -> Result<()> {
    let mut chart = Chart::new(ChartType::Bar);
    chart.add_series(ChartSeries::new()
        .set_categories("'Task List'!$B$2:$B$100")  // Task names
        .set_values("'Task List'!$C$2:$D$100"));     // Start, Finish
    chart.set_x_axis(ChartAxis::new().set_date_axis(true));
    wb.insert_chart(0, 10, &chart)?;
    Ok(())
}
```

**Rationale:**
- Native charts are interactive
- Update when data changes
- Professional appearance

---

## PART VI: RESOURCE LEVELING
**Confidence: 65% → 95%**

### Decision G21: Leveling Algorithm

**CHOSEN APPROACH:** Critical path priority heuristic

**Algorithm:**
```rust
fn level_resources(schedule: &mut Schedule) -> LevelingResult {
    let mut iterations = 0;
    
    while has_conflicts(schedule) && iterations < 100 {
        for time_slot in schedule.start..=schedule.end {
            for resource in &schedule.resources {
                let tasks = tasks_using_resource_at(resource, time_slot);
                
                if tasks.len() > resource.capacity {
                    // Sort: critical tasks first, then by priority
                    tasks.sort_by(|a, b| {
                        match (a.is_critical, b.is_critical) {
                            (true, false) => Ordering::Less,
                            (false, true) => Ordering::Greater,
                            _ => b.priority.cmp(&a.priority),
                        }
                    });
                    
                    // Delay non-critical tasks
                    for task in tasks.iter().skip(resource.capacity) {
                        if can_delay_without_violating_constraints(task) {
                            delay_task(schedule, task, 1);
                            break;
                        }
                    }
                }
            }
        }
        iterations += 1;
    }
    
    if iterations == 100 {
        LevelingResult::Partial { warnings: vec![...] }
    } else {
        LevelingResult::Success
    }
}
```

**Rationale:**
- Fast (O(n) iterations)
- Preserves critical path when possible
- Predictable behavior
- Good enough for 90% of projects

**Deferred to v2.0:** Constraint programming (CP-SAT) for optimal leveling

---

### Decision G22: Over-Allocation Handling

**CHOSEN APPROACH:** Configurable (warn | auto | error)

**CLI:**
```bash
# Default: warn only
utf8proj schedule project.proj
# Warning: Resource 'dev' over-allocated on 2026-02-15 (150%)

# Auto-level
utf8proj schedule project.proj --leveling=auto

# Strict validation
utf8proj schedule project.proj --leveling=error
# Error: Cannot schedule with over-allocations
```

**Project-level config:**
```proj
project "My Project" {
    leveling_mode: warn    # warn | auto | error
}
```

**Rationale:**
- Default warn: informative without changing dates
- Auto: convenience for quick scheduling
- Error: validation for critical projects

---

### Decision G23: Leveling with Constraints

**CHOSEN APPROACH:** Constraints are inviolable

**Implementation:**
```rust
fn level_with_constraints(schedule: &mut Schedule) -> LevelingResult {
    // Attempt leveling
    level_resources(schedule);
    
    // Validate constraints
    for task in &schedule.tasks {
        for constraint in &task.constraints {
            if !satisfies(task, constraint) {
                return LevelingResult::Failed {
                    reason: format!("Leveling violates {} on task {}", 
                                   constraint, task.id),
                    suggestions: vec![
                        "Add more resources",
                        "Relax constraint",
                        "Reduce scope",
                    ],
                };
            }
        }
    }
    
    LevelingResult::Success
}
```

**Rationale:**
- Hard constraints are requirements
- Violation = invalid schedule
- Helpful suggestions guide resolution

---

## PART VII: BDD/SAT INTEGRATION
**Confidence: 50% → Deferred**

### Decision H24-H26: BDD/SAT Features

**CHOSEN APPROACH:** Defer to v2.0

**Rationale:**
- Complexity too high for MVP
- Heuristic CPM provides 80% of value
- Can add later as opt-in feature
- Early users will inform necessity

**Future Design Notes:**
- Use OxiDD library for BDD operations
- Time discretization: week granularity
- What-if analysis: instant constraint changes
- Critical constraint identification

**V1.0 Alternative:** Re-run CPM for what-if (good enough)

---

## PART VIII: TASKJUGGLER COMPATIBILITY
**Confidence: 75% → 95%**

### Decision I27: TJP Feature Scope

**CHOSEN APPROACH:** Tier 1 for v1.0

**Tier 1 (v1.0):**
- ✅ Tasks (hierarchical WBS)
- ✅ Resources (capacity, rates)
- ✅ Dependencies (FS, SS, FF, SF)
- ✅ Effort-based scheduling
- ✅ Calendars (working hours, holidays)
- ✅ Basic reports

**Tier 2 (deferred):**
- ⏳ Scenarios
- ⏳ Shifts
- ⏳ Flags
- ⏳ Bookings

**Tier 3 (not planned):**
- ❌ Journal entries
- ❌ Rich text reports
- ❌ Complex accounts

**Coverage:** ~70% of typical TJP files supported

---

### Decision I28: TJP Parser Strategy

**CHOSEN APPROACH:** Hybrid - parse all, warn unsupported

**Implementation:**
```rust
impl TjpParser {
    fn parse_statement(&mut self, stmt: TjpStatement) -> Result<Option<ProjStmt>> {
        match stmt {
            TjpStatement::Task(_) => Ok(Some(convert_task(stmt)?)),
            TjpStatement::Shift(_) => {
                warnings.push("Unsupported: shift (use calendar instead)");
                Ok(None) // Skip unsupported
            }
            _ => Ok(Some(preserve_as_comment(stmt))), // Unknown → comment
        }
    }
}
```

**Rationale:**
- Maximizes compatibility
- Graceful degradation
- Clear warnings
- Future-proof (unsupported → comments)

---

### Decision I29: TJP to .proj Conversion

**CHOSEN APPROACH:** One-way migration with preservation

**CLI:**
```bash
# Basic conversion
utf8proj import legacy.tjp --output=modern.proj

# With validation
utf8proj import legacy.tjp --validate
# Reports differences between TJP schedule and .proj schedule

# Preserve unsupported features
utf8proj import legacy.tjp --preserve-all
# Unsupported features preserved as comments
```

**Rationale:**
- Clear migration path TJP → utf8proj
- No round-trip commitment (reduces complexity)
- Validation helps verify conversion
- Preservation allows future recovery

---

## IMPLEMENTATION PRIORITY

### Phase 1: Core CPM with Progress (Weeks 1-4)
1. **Week 1:** Domain model extensions (A1-A5 decisions)
2. **Week 2:** Progress-aware CPM solver
3. **Week 3:** CLI commands (status, progress, forecast)
4. **Week 4:** Testing and documentation

### Phase 2: History System (Weeks 5-6)
1. **Week 5:** YAML sidecar implementation (C9-C11)
2. **Week 6:** Playback engine (E15-E17)

### Phase 3: Export & Compatibility (Weeks 7-9)
1. **Week 7:** Excel export (F18-F20)
2. **Week 8:** Resource leveling (G21-G23)
3. **Week 9:** TJP import (I27-I29)

### Phase 4: Polish & Release (Week 10)
1. Documentation
2. Performance optimization
3. Integration tests
4. Example projects
5. v1.0 release

---

## CONFIDENCE TRACKING

| Component | Before | After | Improvement |
|-----------|--------|-------|-------------|
| Progress-Aware CPM | 75% | 95% | +20% |
| Container Derivation | 80% | 95% | +15% |
| History - Sidecar | 70% | 95% | +25% |
| History - Embedded | 60% | Deferred | N/A |
| Playback Engine | 65% | 95% | +30% |
| Excel Export | 70% | 95% | +25% |
| Resource Leveling | 65% | 95% | +30% |
| BDD/SAT | 50% | Deferred | N/A |
| TJP Compatibility | 75% | 95% | +20% |
| **OVERALL** | **85%** | **95%** | **+10%** |

---

## SUCCESS CRITERIA

**Design complete when:**
- ✅ All critical areas at 95% confidence
- ✅ Implementation roadmap finalized
- ✅ Test strategy defined for each component
- ✅ No unresolved architectural conflicts

**v1.0 MVP ready when:**
- All Phase 1-3 deliverables complete
- Test coverage >85%
- Documentation comprehensive
- 5+ example projects
- Performance validated (1000+ task projects)

---

**END OF DESIGN DECISIONS**

This document is the authoritative reference for implementation. All code should align with these decisions. Changes require RFC update and rationale documentation.
