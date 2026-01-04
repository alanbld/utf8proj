# utf8proj: Design Refinement Survey - Part 1
**Version:** 1.0
**Date:** 2026-01-04
**Scope:** Sections A-E (Questions 1-17)
**Focus:** Core Scheduling Algorithm & History Systems

---

## SECTION A: PROGRESS-AWARE CPM ALGORITHM
**Current Confidence: 75% → Target: 95%**

### Q1. Effective Duration Calculation for Partially Complete Tasks

**Context:**
```rust
// Current linear approach
let remaining_pct = (100 - pct_complete) as f64 / 100.0;
effective_duration = original_duration.mul_f64(remaining_pct);
```

**Recommended Design:**

Support three duration calculation modes with `remaining_duration` as an explicit override:

```rust
pub enum DurationMode {
    /// Linear: remaining = original × (1 - pct_complete/100)
    Linear,
    /// Explicit: user provides remaining_duration directly
    Explicit { remaining: Duration },
    /// Performance-based: remaining = original × (1 - pct_complete/100) × performance_factor
    PerformanceBased { performance_factor: f64 },
}

impl Task {
    pub fn effective_remaining_duration(&self, status_date: NaiveDate) -> Duration {
        match &self.duration_mode {
            DurationMode::Linear => {
                let remaining_pct = (100.0 - self.percent_complete as f64) / 100.0;
                self.duration.mul_f64(remaining_pct)
            }
            DurationMode::Explicit { remaining } => *remaining,
            DurationMode::PerformanceBased { performance_factor } => {
                let remaining_pct = (100.0 - self.percent_complete as f64) / 100.0;
                self.duration.mul_f64(remaining_pct * performance_factor)
            }
        }
    }
}
```

**DSL Syntax:**
```proj
task backend_api "Backend API" {
    duration: 20d
    complete: 50%
    remaining: 12d    # Explicit override - takes precedence
}
```

**Rationale:**

1. **Linear is the sensible default** - Most project management tools use linear interpolation, and users expect this behavior. It's simple to understand and audit.

2. **Explicit override handles the "percentage paradox"** - When a PM knows that 50% complete doesn't mean 50% remaining work, they can specify `remaining: 12d` directly. This is clearer than complex S-curve formulas.

3. **Performance factor for systematic variance** - If a team consistently runs 20% slower than estimated, `performance_factor: 1.2` adjusts all remaining work proportionally without manual updates.

4. **Avoiding S-curves for v1.0** - Non-linear progress curves (S-curves, front-loaded, back-loaded) add complexity without proportional value. Most PMs don't think in these terms. Defer to v2.0 if user demand exists.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Linear only | Simple, predictable | Ignores real-world variance |
| S-curves | Theoretically accurate | Complex, hard to calibrate, rarely used |
| Explicit override | PM control, handles edge cases | Requires manual updates |
| Performance factor | Systematic adjustment | May not reflect task-specific issues |

**Implementation Notes:**

1. Default to `DurationMode::Linear` if no `remaining:` attribute specified
2. Validate: `remaining` cannot exceed `original_duration`
3. Validate: If `complete: 100%`, `remaining` must be `0d` (or omitted)
4. Store `remaining` separately from `duration` to preserve original estimate
5. Performance factor bounds: `0.5 ≤ factor ≤ 3.0` (warn outside range)

**Test Strategy:**

```rust
#[test]
fn linear_duration_50_percent() {
    let task = Task::new("test", Duration::days(20)).with_percent_complete(50);
    assert_eq!(task.effective_remaining_duration(), Duration::days(10));
}

#[test]
fn explicit_remaining_overrides_calculation() {
    let task = Task::new("test", Duration::days(20))
        .with_percent_complete(50)
        .with_remaining(Duration::days(15));
    assert_eq!(task.effective_remaining_duration(), Duration::days(15));
}

#[test]
fn performance_factor_adjusts_remaining() {
    let task = Task::new("test", Duration::days(20))
        .with_percent_complete(50)
        .with_performance_factor(1.5);
    assert_eq!(task.effective_remaining_duration(), Duration::days(15)); // 10 × 1.5
}

#[test]
fn completed_task_has_zero_remaining() {
    let task = Task::new("test", Duration::days(20)).with_percent_complete(100);
    assert_eq!(task.effective_remaining_duration(), Duration::days(0));
}
```

---

### Q2. Handling Dependencies with Partial Completion

**Context:**
```proj
task backend_api "Backend API" {
    duration: 20d
    complete: 50%    # 10d done, 10d remaining
}

task frontend_ui "Frontend UI" {
    depends: backend_api  # FS dependency
    duration: 15d
}
```

**Recommended Design:**

Standard FS dependency means successor waits for predecessor's **completion**, not just remaining work. However, support percentage-based triggers for advanced use cases:

```rust
pub enum DependencyTrigger {
    /// Standard: wait for 100% completion (default)
    Completion,
    /// Percentage: start when predecessor reaches X%
    Percentage(u8),
    /// Milestone: start when predecessor reaches named milestone
    Milestone(String),
}

pub struct Dependency {
    pub predecessor: TaskId,
    pub dep_type: DependencyType,  // FS, SS, FF, SF
    pub lag: Duration,
    pub trigger: DependencyTrigger,
}
```

**DSL Syntax:**
```proj
task frontend_ui "Frontend UI" {
    depends: backend_api           # Standard FS (wait for 100%)
    depends: backend_api.75%       # Start when backend is 75% complete
    depends: backend_api@api_ready # Start when milestone reached
    duration: 15d
}
```

**CPM Behavior:**

For progress-aware scheduling:

```rust
fn calculate_successor_start(
    predecessor: &ScheduledTask,
    dependency: &Dependency,
    status_date: NaiveDate,
) -> NaiveDate {
    match dependency.trigger {
        DependencyTrigger::Completion => {
            // If predecessor is in-progress, use forecasted finish
            // If predecessor is complete, use actual_finish
            let pred_finish = predecessor.actual_finish
                .unwrap_or(predecessor.forecast_finish);
            pred_finish + dependency.lag
        }
        DependencyTrigger::Percentage(target_pct) => {
            if predecessor.percent_complete >= target_pct {
                // Already past trigger - can start immediately (from status_date)
                max(status_date, predecessor.early_start) + dependency.lag
            } else {
                // Calculate when predecessor will reach target_pct
                let remaining_to_trigger = target_pct - predecessor.percent_complete;
                let days_to_trigger = predecessor.duration
                    .mul_f64(remaining_to_trigger as f64 / 100.0);
                status_date + days_to_trigger + dependency.lag
            }
        }
        // Similar for Milestone
    }
}
```

**Critical Path Impact:**

Partial completion affects critical path calculation:
1. **Completed tasks (100%)** - Use `actual_finish`, zero float
2. **In-progress tasks** - Use `remaining_duration` for forward pass
3. **Not-started tasks** - Use full `duration`

```rust
fn forward_pass_with_progress(tasks: &mut [ScheduledTask], status_date: NaiveDate) {
    for task in tasks.topological_order() {
        if task.percent_complete == 100 {
            // Locked - use actual dates
            task.early_start = task.actual_start.unwrap();
            task.early_finish = task.actual_finish.unwrap();
        } else if task.percent_complete > 0 {
            // In-progress - start is locked, finish is forecasted
            task.early_start = task.actual_start.unwrap();
            task.early_finish = status_date + task.effective_remaining_duration();
        } else {
            // Not started - normal CPM calculation from predecessors
            task.early_start = max(status_date, max_predecessor_finish(task));
            task.early_finish = task.early_start + task.duration;
        }
    }
}
```

**Rationale:**

1. **Standard FS means 100% complete** - This matches MS Project, Primavera, and user expectations. Changing this would be surprising.

2. **Percentage triggers enable fast-following** - Real projects often have overlapping work. "Start frontend when backend API is 75% done" is a legitimate pattern.

3. **Milestones are cleaner than percentages** - Instead of guessing "75%", define explicit milestones: `milestone api_ready { at: 75% }`. Deferred to v1.1.

4. **In-progress tasks anchor the schedule** - Once a task starts, its `actual_start` is fixed. Only remaining work can shift.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Always wait for 100% | Simple, standard | No fast-following |
| Percentage triggers | Flexible overlapping | Complex, percentage is arbitrary |
| Milestone triggers | Semantic meaning | Extra syntax, milestone definition |
| Soft dependencies | Maximum flexibility | Hard to reason about |

**Implementation Notes:**

1. Default trigger is `Completion` (100%)
2. Percentage trigger: `0 < pct < 100` (0% is SS, 100% is standard FS)
3. For backward pass, percentage triggers need careful handling (calculate reverse)
4. Circular dependency detection must consider trigger percentages
5. Gantt visualization should show dependency trigger point

**Test Strategy:**

```rust
#[test]
fn fs_dependency_waits_for_completion() {
    let schedule = schedule_with_progress(vec![
        task("A", 20).complete(50),  // 10d remaining
        task("B", 15).depends_on("A"),
    ], status_date("2026-02-10"));

    // B starts after A finishes (status_date + 10d remaining)
    assert_eq!(schedule["B"].early_start, date("2026-02-20"));
}

#[test]
fn percentage_trigger_allows_early_start() {
    let schedule = schedule_with_progress(vec![
        task("A", 20).complete(80),  // Past 75% trigger
        task("B", 15).depends_on_at("A", 75),
    ], status_date("2026-02-10"));

    // B can start immediately (A already past 75%)
    assert_eq!(schedule["B"].early_start, date("2026-02-10"));
}

#[test]
fn critical_path_uses_remaining_duration() {
    let schedule = schedule_with_progress(vec![
        task("A", 20).complete(50),  // 10d remaining, on critical path
        task("B", 15).depends_on("A"),
    ], status_date("2026-02-01"));

    // Critical path: 10d (A remaining) + 15d (B) = 25d
    assert_eq!(schedule.critical_path_duration(), Duration::days(25));
}
```

---

### Q3. Actual Dates vs Dependencies Conflict Resolution

**Context:**
```proj
task design "Design" {
    duration: 10d
    actual_finish: 2026-02-15
}

task implementation "Implementation" {
    depends: design          # FS: should start after 2026-02-15
    actual_start: 2026-02-10  # CONFLICT: Started before design finished!
}
```

**Recommended Design:**

**Warn but accept** - Actual dates represent reality; dependencies represent the plan. Reality wins.

```rust
pub enum ScheduleWarning {
    DependencyViolation {
        task: TaskId,
        predecessor: TaskId,
        expected_start: NaiveDate,
        actual_start: NaiveDate,
        violation_days: i64,
    },
    // ... other warnings
}

pub struct ScheduleResult {
    pub schedule: Schedule,
    pub warnings: Vec<ScheduleWarning>,
    pub errors: Vec<ScheduleError>,
}
```

**Behavior Matrix:**

| Predecessor State | Successor State | Behavior |
|-------------------|-----------------|----------|
| actual_finish set | actual_start set | **Warn if violated**, use actual dates |
| actual_finish set | not started | Successor must start after actual_finish |
| in-progress | actual_start set | **Warn if violated**, use actual dates |
| in-progress | not started | Successor waits for forecast_finish |
| not started | any | Normal CPM calculation |

**Warning Output:**
```
⚠ Warning: Dependency violation detected
  Task "Implementation" started 2026-02-10
  Predecessor "Design" finished 2026-02-15
  Dependency type: FS (Finish-to-Start)
  Violation: Started 5 days before predecessor finished

  This may indicate:
  - Parallel work (intentional)
  - Incorrect dependency definition
  - Data entry error

  The schedule uses actual dates. Update dependencies if this is intentional.
```

**Rationale:**

1. **Actual dates are ground truth** - The project management database reflects what actually happened. Rejecting it serves no purpose.

2. **Dependencies are planning tools** - They help forecast future work. Once work is done, the dependency served its purpose.

3. **Warnings enable auditing** - PMs can review violations and decide if dependencies need updating or if there was a tracking error.

4. **Strict mode for validation** - Offer `--strict` flag that errors on violations for users who want enforcement.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Error (reject) | Enforces consistency | Blocks valid scenarios, frustrates users |
| Warn (default) | Flexible, honest reporting | May hide data quality issues |
| Silent accept | No friction | Users unaware of violations |
| Auto-adjust | "Fixes" the issue | Changes user data without consent |

**Implementation Notes:**

```rust
fn validate_dependency_with_actuals(
    predecessor: &Task,
    successor: &Task,
    dep: &Dependency,
) -> Option<ScheduleWarning> {
    let (pred_finish, succ_start) = match (predecessor.actual_finish, successor.actual_start) {
        (Some(pf), Some(ss)) => (pf, ss),
        _ => return None,  // Can't validate without both actual dates
    };

    let expected_start = match dep.dep_type {
        DependencyType::FS => pred_finish + dep.lag,
        DependencyType::SS => predecessor.actual_start? + dep.lag,
        DependencyType::FF => pred_finish + dep.lag - successor.duration,
        DependencyType::SF => predecessor.actual_start? + dep.lag - successor.duration,
    };

    if succ_start < expected_start {
        Some(ScheduleWarning::DependencyViolation {
            task: successor.id.clone(),
            predecessor: predecessor.id.clone(),
            expected_start,
            actual_start: succ_start,
            violation_days: (expected_start - succ_start).num_days(),
        })
    } else {
        None
    }
}
```

**CLI Flags:**
```bash
utf8proj schedule project.proj              # Default: warn on violations
utf8proj schedule project.proj --strict     # Error on violations
utf8proj schedule project.proj --no-warn    # Suppress warnings
```

**Test Strategy:**

```rust
#[test]
fn actual_dates_override_dependencies_with_warning() {
    let result = schedule_project(vec![
        task("design").actual_finish(date("2026-02-15")),
        task("impl").depends_on("design").actual_start(date("2026-02-10")),
    ]);

    // Schedule succeeds with actual dates
    assert_eq!(result.schedule["impl"].start, date("2026-02-10"));

    // Warning is generated
    assert!(result.warnings.iter().any(|w| matches!(w,
        ScheduleWarning::DependencyViolation { task, .. } if task == "impl"
    )));
}

#[test]
fn strict_mode_rejects_violations() {
    let result = schedule_project_strict(vec![
        task("design").actual_finish(date("2026-02-15")),
        task("impl").depends_on("design").actual_start(date("2026-02-10")),
    ]);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ScheduleError::DependencyViolation { .. }));
}

#[test]
fn no_warning_when_dependency_satisfied() {
    let result = schedule_project(vec![
        task("design").actual_finish(date("2026-02-15")),
        task("impl").depends_on("design").actual_start(date("2026-02-16")),
    ]);

    assert!(result.warnings.is_empty());
}
```

---

### Q4. Resource Leveling with Partial Completion

**Context:**
```proj
resource dev "Developer" { capacity: 1 }

task backend "Backend" {
    duration: 10d
    assign: dev
    complete: 50%          # 5d done, 5d remaining
    actual_start: 2026-02-01
}

task frontend "Frontend" {
    duration: 10d
    assign: dev
    complete: 0%           # Not started
}
```

**Recommended Design:**

**In-progress tasks are immovable anchors.** Leveling only affects future work (tasks not yet started).

```rust
pub struct LevelingOptions {
    /// Only level tasks starting after this date (default: status_date)
    pub level_after: NaiveDate,
    /// Preserve in-progress task schedules
    pub preserve_in_progress: bool,  // default: true
    /// Allow splitting tasks (pause and resume)
    pub allow_splitting: bool,       // default: false
    /// Priority rules for conflict resolution
    pub priority_rules: PriorityRules,
}

pub enum TaskLevelingStatus {
    /// Cannot be moved - has actual_start or complete > 0
    Anchored,
    /// Can be moved within float
    Movable { max_delay: Duration },
    /// Can be moved freely (not on critical path)
    Flexible,
}
```

**Leveling Algorithm:**

```rust
fn level_resources(
    schedule: &mut Schedule,
    options: &LevelingOptions,
) -> LevelingResult {
    // 1. Identify anchored vs movable tasks
    for task in &mut schedule.tasks {
        task.leveling_status = if task.percent_complete > 0 || task.actual_start.is_some() {
            TaskLevelingStatus::Anchored
        } else if task.is_critical {
            TaskLevelingStatus::Movable { max_delay: Duration::zero() }
        } else {
            TaskLevelingStatus::Movable { max_delay: task.total_float }
        };
    }

    // 2. Detect over-allocations after status_date only
    let conflicts = detect_future_conflicts(schedule, options.level_after);

    // 3. Resolve conflicts by delaying lower-priority movable tasks
    for conflict in conflicts {
        let (keep, delay) = resolve_conflict(&conflict, &options.priority_rules);
        if delay.leveling_status == TaskLevelingStatus::Anchored {
            // Cannot delay anchored task - warn user
            result.warnings.push(LevelingWarning::CannotResolve { .. });
        } else {
            // Delay the task
            delay_task(schedule, delay.id, conflict.overlap_duration);
        }
    }

    result
}
```

**Priority Rules (default):**
1. **In-progress tasks** - Never moved (anchored)
2. **Critical path tasks** - Minimize delay (preserve project end date)
3. **Higher priority value** - Keep scheduled (explicit priority attribute)
4. **Earlier baseline start** - Keep scheduled (original plan precedence)
5. **Shorter duration** - Keep scheduled (get small tasks done)

**Handling Past Over-allocation:**

```rust
// Over-allocation before status_date is historical - warn but don't fix
fn detect_historical_conflicts(schedule: &Schedule, status_date: NaiveDate) -> Vec<Conflict> {
    schedule.conflicts()
        .filter(|c| c.period.start < status_date)
        .collect()
}

// Warn about historical over-allocation
// "Resource 'dev' was over-allocated 2026-01-15 to 2026-01-20 (historical - not adjusted)"
```

**Rationale:**

1. **Reality cannot be undone** - A task that's 50% complete has happened. Leveling can't change history.

2. **In-progress tasks have organizational momentum** - People are actively working on them. Suggesting they stop mid-task to work on something else is impractical.

3. **Leveling is forward-looking** - Its purpose is to create a feasible future schedule, not to rewrite history.

4. **Past over-allocation is informational** - It explains why the project might be behind. It's not something to "fix."

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Anchor in-progress | Realistic, respects work in flight | May leave conflicts unresolved |
| Allow moving in-progress | Maximum flexibility | Unrealistic, confusing |
| Split tasks | Can resolve more conflicts | Complex, often impractical |
| Warn on historical | Honest reporting | May seem like noise |

**Implementation Notes:**

1. `actual_start` OR `complete > 0` → task is anchored
2. Leveling should be idempotent (running twice produces same result)
3. Track which tasks were delayed and by how much for reporting
4. Allow manual override: `leveling: ignore` attribute to exclude task
5. Consider partial allocation: `assign: dev@50%` means capacity impact is 0.5

**Test Strategy:**

```rust
#[test]
fn in_progress_task_not_moved() {
    let mut schedule = schedule_project(vec![
        task("backend", 10).assign("dev").complete(50).actual_start(date("2026-02-01")),
        task("frontend", 10).assign("dev").complete(0),
    ]);
    let original_backend_start = schedule["backend"].start;

    level_resources(&mut schedule, &LevelingOptions::default());

    // Backend (in-progress) unchanged
    assert_eq!(schedule["backend"].start, original_backend_start);
    // Frontend delayed to avoid conflict
    assert!(schedule["frontend"].start >= schedule["backend"].forecast_finish);
}

#[test]
fn past_overallocation_generates_warning_only() {
    let schedule = schedule_project_at(vec![
        task("A", 10).assign("dev").actual_start(date("2026-01-01")).actual_finish(date("2026-01-10")),
        task("B", 10).assign("dev").actual_start(date("2026-01-05")).actual_finish(date("2026-01-15")),
    ], status_date("2026-01-20"));  // Both tasks complete

    let result = level_resources(&schedule, &LevelingOptions::default());

    // No changes (both are historical)
    assert!(result.changes.is_empty());
    // Warning about past over-allocation
    assert!(result.warnings.iter().any(|w|
        matches!(w, LevelingWarning::HistoricalOverallocation { .. })
    ));
}

#[test]
fn critical_path_task_prioritized() {
    let mut schedule = schedule_project(vec![
        task("critical", 10).assign("dev").critical(true),
        task("non_critical", 10).assign("dev").float(Duration::days(5)),
    ]);

    level_resources(&mut schedule, &LevelingOptions::default());

    // Critical task stays in place, non-critical delayed
    assert_eq!(schedule["critical"].start, date("2026-02-01"));
    assert!(schedule["non_critical"].start > date("2026-02-01"));
}
```

---

### Q5. Container Tasks with Mixed Child Progress

**Context:**
```proj
task development "Development Phase" {
    task frontend "Frontend" {
        duration: 10d
        complete: 100%
        actual_finish: 2026-02-15
    }

    task backend "Backend" {
        duration: 20d
        complete: 50%
        actual_start: 2026-02-05
    }

    task testing "Testing" {
        duration: 5d
        complete: 0%
    }
}
```

**Recommended Design:**

**Duration-weighted progress** for containers, with derived actual dates:

```rust
impl Task {
    /// Calculate container progress from children
    pub fn derive_progress(&self) -> DerivedProgress {
        if self.children.is_empty() {
            return DerivedProgress::Leaf {
                percent: self.percent_complete,
                actual_start: self.actual_start,
                actual_finish: self.actual_finish,
            };
        }

        // Duration-weighted average
        let total_duration: f64 = self.children.iter()
            .map(|c| c.duration.num_days() as f64)
            .sum();

        let weighted_progress: f64 = self.children.iter()
            .map(|c| {
                let child_progress = c.derive_progress();
                let weight = c.duration.num_days() as f64 / total_duration;
                child_progress.percent() as f64 * weight
            })
            .sum();

        // Actual start = earliest child actual_start
        let actual_start = self.children.iter()
            .filter_map(|c| c.actual_start)
            .min();

        // Actual finish = latest child actual_finish (only if ALL children complete)
        let actual_finish = if self.children.iter().all(|c| c.percent_complete == 100) {
            self.children.iter()
                .filter_map(|c| c.actual_finish)
                .max()
        } else {
            None
        };

        DerivedProgress::Container {
            percent: weighted_progress.round() as u8,
            actual_start,
            actual_finish,
        }
    }
}
```

**Calculation Example:**

```
Frontend:  10d × 100% = 10.0 weighted days complete
Backend:   20d ×  50% = 10.0 weighted days complete
Testing:    5d ×   0% =  0.0 weighted days complete
─────────────────────────────────────────────────────
Total:     35d          20.0 weighted days complete

Container progress = 20.0 / 35.0 = 57.1% ≈ 57%

Container actual_start = min(none, 2026-02-05, none) = 2026-02-05
Container actual_finish = none (not all children complete)
```

**User Override:**

Allow explicit container progress that overrides calculation:

```proj
task development "Development Phase" {
    complete: 60%    # User override (takes precedence)
    progress_mode: manual  # Explicit flag

    task frontend { ... }
    task backend { ... }
}
```

**Validation:**
```rust
fn validate_container_progress(container: &Task) -> Vec<ValidationWarning> {
    let mut warnings = vec![];

    if let Some(explicit) = container.explicit_percent_complete {
        let derived = container.derive_progress().percent();
        let diff = (explicit as i16 - derived as i16).abs();

        if diff > 20 {
            warnings.push(ValidationWarning::ProgressMismatch {
                task: container.id.clone(),
                explicit,
                derived,
                difference: diff as u8,
            });
        }
    }

    warnings
}
```

**Rationale:**

1. **Duration-weighted is most intuitive** - A 20-day task at 50% represents more work than a 5-day task at 50%. Weighting by duration reflects this.

2. **Simple average misleads** - (100 + 50 + 0) / 3 = 50% ignores that testing is trivial compared to backend work.

3. **Effort-based is better but complex** - Weighting by effort (person-hours) is more accurate but requires effort tracking, which not all projects have. Duration is always available.

4. **Derived actual_start is min of children** - The container "started" when any child started.

5. **Derived actual_finish requires all complete** - A container isn't "finished" until all children are done. Having 2/3 children complete doesn't give a finish date.

6. **Allow override with warning** - PMs sometimes have information not captured in child tasks. Allow override but warn if it differs significantly from calculation.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Simple average | Easy to understand | Misleading for mixed durations |
| Duration-weighted | Proportional to work | Ignores effort variance |
| Effort-weighted | Most accurate | Requires effort tracking |
| User override only | Full control | No automation, tedious |

**Implementation Notes:**

1. Recursive derivation for nested containers (grandchildren roll up)
2. Cache derived progress to avoid recomputation
3. Invalidate cache when any descendant changes
4. Consider effort-weighted option for v1.1 (`progress_weight: effort`)
5. Round to nearest integer percent for display

**Test Strategy:**

```rust
#[test]
fn container_progress_is_duration_weighted() {
    let container = container("dev", vec![
        task("frontend", 10).complete(100),
        task("backend", 20).complete(50),
        task("testing", 5).complete(0),
    ]);

    let progress = container.derive_progress();
    // (10×100 + 20×50 + 5×0) / 35 = 2000/35 = 57.14 ≈ 57%
    assert_eq!(progress.percent(), 57);
}

#[test]
fn container_actual_start_is_earliest_child() {
    let container = container("dev", vec![
        task("frontend", 10).actual_start(date("2026-02-10")),
        task("backend", 20).actual_start(date("2026-02-05")),
        task("testing", 5),  // Not started
    ]);

    let progress = container.derive_progress();
    assert_eq!(progress.actual_start(), Some(date("2026-02-05")));
}

#[test]
fn container_actual_finish_only_when_all_complete() {
    let container = container("dev", vec![
        task("frontend", 10).complete(100).actual_finish(date("2026-02-15")),
        task("backend", 20).complete(50),
        task("testing", 5).complete(0),
    ]);

    let progress = container.derive_progress();
    assert_eq!(progress.actual_finish(), None);  // Not all children complete
}

#[test]
fn container_actual_finish_is_latest_child() {
    let container = container("dev", vec![
        task("frontend", 10).complete(100).actual_finish(date("2026-02-15")),
        task("backend", 20).complete(100).actual_finish(date("2026-02-25")),
        task("testing", 5).complete(100).actual_finish(date("2026-02-28")),
    ]);

    let progress = container.derive_progress();
    assert_eq!(progress.actual_finish(), Some(date("2026-02-28")));
}

#[test]
fn explicit_progress_overrides_with_warning() {
    let container = container("dev", vec![
        task("frontend", 10).complete(100),
        task("backend", 20).complete(50),
    ]).explicit_complete(90);  // Override: 90%

    let progress = container.derive_progress();
    assert_eq!(progress.percent(), 90);  // Uses override

    let warnings = validate_container_progress(&container);
    assert_eq!(warnings.len(), 1);  // Warning about 90% vs derived ~67%
}
```

---

## SECTION B: CONTAINER TASK DERIVATION
**Current Confidence: 80% → Target: 95%**

### Q6. Container with Both Duration Attribute AND Children

**Context:**
```proj
task project "Project" {
    duration: 30d    # Explicit duration specified

    task phase1 { duration: 10d }
    task phase2 { duration: 15d, depends: phase1 }
    # Actual scheduled duration: 25d (10d + 15d)
}
```

**Recommended Design:**

**Parser warning, children win.** A container's duration is always derived from children; explicit duration is ignored with a warning.

```rust
pub enum ParserWarning {
    ContainerDurationIgnored {
        task: TaskId,
        specified: Duration,
        derived: Duration,
    },
    // ... other warnings
}

fn parse_task(input: &str) -> ParseResult<Task> {
    let task = parse_task_attributes(input)?;

    if !task.children.is_empty() && task.duration.is_some() {
        warnings.push(ParserWarning::ContainerDurationIgnored {
            task: task.id.clone(),
            specified: task.duration.unwrap(),
            derived: derive_container_duration(&task),
        });
        task.duration = None;  // Clear explicit duration
    }

    Ok(task)
}
```

**Warning Output:**
```
⚠ Warning: Container task duration ignored
  Task "Project" has explicit duration: 30d
  Derived duration from children: 25d
  Container duration is always derived from children.
  Remove 'duration: 30d' to suppress this warning.
```

**Rationale:**

1. **Children are the source of truth** - If a container has children, those children define the work. The container is just a grouping mechanism.

2. **Prevents data inconsistency** - If we allow both, users might update children but forget to update container duration, leading to confusion.

3. **Warning, not error** - Legacy files or imports might have this pattern. Error would break compatibility; warning educates.

4. **Clear mental model** - Containers don't have their own duration; they span their children. Period.

**Alternative Considered:**

"Container duration as constraint" - Use explicit duration as a target/deadline. Rejected because:
- Conflates scheduling attributes with constraints
- Use `must_finish_on` for deadlines instead
- Overcomplicates simple container concept

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Children always win | Clear, consistent | May surprise users with explicit duration |
| Container wins | User control | Children become meaningless |
| Error | Strict enforcement | Breaks imports, legacy files |
| Warn + ignore | Educational, compatible | Noise for intentional overrides |

**Implementation Notes:**

1. Check during parsing, not scheduling (early feedback)
2. Same rule applies to `effort` attribute on containers
3. Container can have `must_finish_on` constraint (that's different)
4. Allow `duration` on empty containers (placeholders - see Q8)

**Test Strategy:**

```rust
#[test]
fn container_duration_derived_from_children() {
    let project = parse_project(r#"
        task project "Project" {
            duration: 30d
            task phase1 { duration: 10d }
            task phase2 { duration: 15d, depends: phase1 }
        }
    "#).unwrap();

    // Explicit duration ignored
    assert_eq!(project.tasks["project"].duration, None);
    // Derived duration from schedule
    let schedule = schedule_project(&project);
    assert_eq!(schedule["project"].duration, Duration::days(25));
}

#[test]
fn container_duration_warning_generated() {
    let result = parse_project(r#"
        task project "Project" {
            duration: 30d
            task phase1 { duration: 10d }
        }
    "#);

    assert!(result.warnings.iter().any(|w|
        matches!(w, ParserWarning::ContainerDurationIgnored { task, .. } if task == "project")
    ));
}
```

---

### Q7. Container Progress vs Leaf Progress

**Context:**
```proj
task development "Development" {
    complete: 60%    # User manually sets container progress

    task frontend { complete: 100% }  # Calculated: 70% based on duration
    task backend { complete: 40% }
}
```

**Recommended Design:**

**Explicit container progress is allowed** but validated against derived value. Large discrepancies generate warnings.

```rust
pub struct Task {
    /// Explicit progress set by user (None = derive from children)
    pub explicit_percent_complete: Option<u8>,

    /// Progress derivation mode
    pub progress_mode: ProgressMode,
}

pub enum ProgressMode {
    /// Derive from children (default for containers)
    Derived,
    /// Use explicit value, validate against derived
    Explicit,
    /// Use explicit value, skip validation
    Manual,
}

impl Task {
    pub fn percent_complete(&self) -> u8 {
        match self.progress_mode {
            ProgressMode::Derived => self.derive_progress().percent(),
            ProgressMode::Explicit | ProgressMode::Manual => {
                self.explicit_percent_complete.unwrap_or(0)
            }
        }
    }
}
```

**Validation Rules:**

```rust
fn validate_progress_consistency(container: &Task) -> Vec<ValidationWarning> {
    if container.progress_mode == ProgressMode::Manual {
        return vec![];  // Skip validation
    }

    let explicit = match container.explicit_percent_complete {
        Some(p) => p,
        None => return vec![],
    };

    let derived = container.derive_progress().percent();
    let diff = (explicit as i16 - derived as i16).abs() as u8;

    if diff > 20 {
        vec![ValidationWarning::SignificantProgressMismatch {
            task: container.id.clone(),
            explicit,
            derived,
            difference: diff,
            suggestion: "Consider using 'progress_mode: manual' to suppress this warning",
        }]
    } else if diff > 10 {
        vec![ValidationWarning::ProgressMismatch {
            task: container.id.clone(),
            explicit,
            derived,
            difference: diff,
        }]
    } else {
        vec![]
    }
}
```

**DSL Syntax:**
```proj
task development "Development" {
    complete: 60%              # Explicit
    progress_mode: manual      # Suppress validation

    task frontend { ... }
    task backend { ... }
}
```

**Use Cases for Manual Progress:**

1. **High-level estimation** - PM knows overall status better than child sum
2. **Children not yet planned** - Container exists but breakdown is incomplete
3. **External system import** - Source system had different calculation
4. **Interim reporting** - Quick update without updating all children

**Rationale:**

1. **Flexibility for PMs** - Real projects often have nuance that child rollup doesn't capture.

2. **Validation catches errors** - If someone types 90% when children show 30%, that's likely a mistake.

3. **Manual mode for intentional override** - When PM explicitly wants to override, they can suppress warnings.

4. **Threshold of 20%** - Small differences (< 10%) are rounding noise. Large differences (> 20%) are likely errors or intentional overrides.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Always derive | Consistent | No PM override |
| Always explicit | Full control | Tedious, error-prone |
| Derive + validate | Best of both | Warning noise |
| Derive + manual mode | Flexible | Extra syntax |

**Implementation Notes:**

1. Default `progress_mode: derived` for containers
2. Default `progress_mode: explicit` for leaf tasks (leaf tasks always use their `complete:` value)
3. Validation runs during `schedule` command, not `parse`
4. Consider showing both values in reports: "60% (derived: 70%)"

**Test Strategy:**

```rust
#[test]
fn explicit_container_progress_used() {
    let container = parse_task(r#"
        task dev {
            complete: 60%
            task a { duration: 10d, complete: 100% }
            task b { duration: 10d, complete: 40% }
        }
    "#).unwrap();

    // Explicit value used
    assert_eq!(container.percent_complete(), 60);
}

#[test]
fn large_mismatch_generates_warning() {
    let container = parse_task(r#"
        task dev {
            complete: 90%
            task a { duration: 10d, complete: 50% }
            task b { duration: 10d, complete: 50% }
        }
    "#).unwrap();

    let warnings = validate_progress_consistency(&container);
    // Derived is 50%, explicit is 90%, diff is 40%
    assert!(warnings.iter().any(|w|
        matches!(w, ValidationWarning::SignificantProgressMismatch { difference: 40, .. })
    ));
}

#[test]
fn manual_mode_suppresses_validation() {
    let container = parse_task(r#"
        task dev {
            complete: 90%
            progress_mode: manual
            task a { duration: 10d, complete: 50% }
        }
    "#).unwrap();

    let warnings = validate_progress_consistency(&container);
    assert!(warnings.is_empty());
}
```

---

### Q8. Empty Containers (Placeholders)

**Context:**
```proj
task future_phase "Future Phase 3" {
    # No children, no duration/effort
    # Just a placeholder for future work
}
```

**Recommended Design:**

**Valid as milestone placeholder** with zero duration. Optionally allow explicit duration for rough estimation.

```rust
pub enum TaskType {
    /// Container with children - duration derived
    Container,
    /// Leaf task with work - has duration/effort
    Leaf,
    /// Milestone - zero duration, marks a point in time
    Milestone,
    /// Placeholder - no children yet, optional estimated duration
    Placeholder { estimated_duration: Option<Duration> },
}

fn determine_task_type(task: &Task) -> TaskType {
    if !task.children.is_empty() {
        TaskType::Container
    } else if task.is_milestone {
        TaskType::Milestone
    } else if task.duration.is_none() && task.effort.is_none() {
        TaskType::Placeholder {
            estimated_duration: task.estimated_duration
        }
    } else {
        TaskType::Leaf
    }
}
```

**DSL Syntax:**
```proj
# Option 1: True placeholder (zero duration until defined)
task future_phase "Future Phase 3" {
    placeholder: true
}

# Option 2: Placeholder with estimate (for rough planning)
task future_phase "Future Phase 3" {
    estimate: 20d    # Used for high-level timeline
}

# Option 3: Just empty (implicit placeholder)
task future_phase "Future Phase 3" {
    # Empty = placeholder milestone
}
```

**Scheduling Behavior:**

```rust
fn schedule_task(task: &Task, context: &ScheduleContext) -> ScheduledTask {
    match task.task_type() {
        TaskType::Placeholder { estimated_duration: None } => {
            // Zero duration milestone
            ScheduledTask {
                start: calculate_start(task, context),
                finish: calculate_start(task, context),  // Same as start
                duration: Duration::zero(),
                is_placeholder: true,
            }
        }
        TaskType::Placeholder { estimated_duration: Some(est) } => {
            // Use estimate for planning
            ScheduledTask {
                start: calculate_start(task, context),
                finish: calculate_start(task, context) + est,
                duration: est,
                is_placeholder: true,
                is_estimated: true,
            }
        }
        // ... other types
    }
}
```

**Gantt Visualization:**
- Placeholders shown with dashed outline
- Estimated duration shown with lighter fill
- Badge/icon indicating "placeholder" status

**Rationale:**

1. **Real projects have TBD sections** - Phase 3 might not be planned yet but needs to be in the project structure.

2. **Zero duration makes sense** - Until defined, it's a point in time, not a span. Dependencies to/from it work as milestones.

3. **Estimates enable high-level planning** - PM wants to see rough timeline even if breakdown isn't done.

4. **Distinguishable from real tasks** - UI should clearly show these are placeholders, not actual committed work.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Error (require duration) | Strict, complete data | Blocks iterative planning |
| Zero duration (milestone) | Simple, logical | May mislead timeline view |
| Estimate duration | Rough planning works | Users might confuse with real duration |
| Explicit placeholder flag | Clear intent | Extra syntax |

**Implementation Notes:**

1. Placeholders can have dependencies (useful for sequencing)
2. Placeholders can have resources (rough allocation planning)
3. Warning if placeholder is on critical path (uncertain duration affects project)
4. `check` command should report placeholders: "3 placeholder tasks need definition"
5. Placeholder with estimate != committed duration (different in reports)

**Test Strategy:**

```rust
#[test]
fn empty_task_is_placeholder() {
    let task = parse_task(r#"task future "Future Phase" {}"#).unwrap();
    assert!(matches!(task.task_type(), TaskType::Placeholder { estimated_duration: None }));
}

#[test]
fn placeholder_scheduled_as_milestone() {
    let schedule = schedule_project(r#"
        task phase1 { duration: 10d }
        task phase2 { }  # Placeholder
        task phase3 { duration: 5d, depends: phase2 }
    "#).unwrap();

    // Phase 2 has zero duration
    assert_eq!(schedule["phase2"].duration, Duration::zero());
    // Phase 3 starts right after phase 1 (phase 2 is zero-width)
    assert_eq!(schedule["phase3"].start, schedule["phase1"].finish);
}

#[test]
fn placeholder_with_estimate() {
    let schedule = schedule_project(r#"
        task phase1 { duration: 10d }
        task phase2 { estimate: 20d }  # Placeholder with estimate
        task phase3 { duration: 5d, depends: phase2 }
    "#).unwrap();

    // Phase 2 uses estimate
    assert_eq!(schedule["phase2"].duration, Duration::days(20));
    assert!(schedule["phase2"].is_estimated);
    // Phase 3 starts after phase 2's estimated finish
    assert_eq!(schedule["phase3"].start, schedule["phase1"].finish + Duration::days(20));
}

#[test]
fn placeholder_on_critical_path_warning() {
    let result = schedule_project(r#"
        task phase1 { duration: 10d }
        task phase2 { }  # Placeholder on critical path
    "#).unwrap();

    assert!(result.warnings.iter().any(|w|
        matches!(w, ScheduleWarning::PlaceholderOnCriticalPath { task, .. } if task == "phase2")
    ));
}
```

---

## SECTION C: HISTORY SYSTEM - SIDECAR FORMAT
**Current Confidence: 70% → Target: 95%**

### Q9. Sidecar File Format Choice

**Recommended Design:**

**YAML** with structured schema for readability and Git-friendliness.

```yaml
# project.proj.history
# utf8proj history file - DO NOT EDIT MANUALLY
version: "1.0"
project: "project.proj"
created: "2026-01-15T09:00:00Z"
last_modified: "2026-02-01T14:30:00Z"

snapshots:
  - id: "s001"
    timestamp: "2026-01-15T09:00:00Z"
    author: "alice"
    message: "Initial project setup"
    type: full
    schedule_hash: "a1b2c3d4"
    metrics:
      task_count: 15
      total_duration_days: 45
      critical_path_days: 30
    content: |
      project "My Project" {
          start: 2026-01-01
          ...
      }

  - id: "s002"
    timestamp: "2026-02-01T14:30:00Z"
    author: "bob"
    message: "Added testing phase"
    type: diff
    parent: "s001"
    schedule_hash: "e5f6g7h8"
    metrics:
      task_count: 18
      total_duration_days: 55
      critical_path_days: 35
    diff:
      added:
        - path: "project.development.testing"
          content: |
            task testing "Testing" {
                duration: 10d
                depends: backend
            }
      modified:
        - path: "project.development.backend"
          attribute: "duration"
          old: "15d"
          new: "20d"
      removed: []
```

**Rationale:**

1. **YAML is human-readable** - PMs and developers can inspect history without special tools.

2. **Git diffs work well** - YAML's line-oriented format produces meaningful Git diffs. JSON would be one-line blobs.

3. **Comments allowed** - Can include documentation or warnings in the file.

4. **Structured diffs** - Rather than raw text diffs, store semantic changes (task added, attribute modified). Easier to process and visualize.

5. **Metrics per snapshot** - Quick project health assessment without parsing full content.

**Format Comparison:**

| Format | Human Readable | Git Friendly | Comments | Schema | Ecosystem |
|--------|----------------|--------------|----------|--------|-----------|
| YAML | Excellent | Good | Yes | Optional | Wide |
| JSON | Poor (dense) | Poor | No | Excellent | Wide |
| TOML | Good | Good | Yes | Limited | Narrow |
| Custom | Variable | Variable | Variable | N/A | None |

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| YAML | Readable, comments, Git-friendly | Indentation-sensitive, slower parsing |
| JSON | Strict, fast parsing, universal | No comments, dense, poor diffs |
| TOML | Simple config | Not ideal for nested/repeated data |
| Custom binary | Compact, fast | No ecosystem, debugging hard |

**Implementation Notes:**

1. Use `serde_yaml` crate for serialization
2. Schema validation with `schemars` crate
3. File extension: `.proj.history` (associates with main file)
4. UTF-8 encoding, Unix line endings (LF)
5. Max line length in content blocks: 120 chars (soft wrap)
6. Escape sequences for special YAML characters in content

**Test Strategy:**

```rust
#[test]
fn history_file_round_trip() {
    let history = History {
        snapshots: vec![
            Snapshot::full("s001", content1),
            Snapshot::diff("s002", "s001", diff1),
        ],
    };

    let yaml = history.to_yaml().unwrap();
    let parsed = History::from_yaml(&yaml).unwrap();

    assert_eq!(history, parsed);
}

#[test]
fn history_file_git_diff_meaningful() {
    let history1 = History::with_snapshot(snapshot1);
    let history2 = history1.with_snapshot(snapshot2);

    let yaml1 = history1.to_yaml().unwrap();
    let yaml2 = history2.to_yaml().unwrap();

    let diff = compute_text_diff(&yaml1, &yaml2);
    // Diff should show only the new snapshot, not rewrite whole file
    assert!(diff.lines().count() < 30);
}
```

---

### Q10. Sidecar File Structure

**Recommended Design:**

**Hybrid structure** with periodic full snapshots and diffs in between.

```yaml
snapshots:
  # Full snapshot (anchor point)
  - id: "s001"
    type: full
    content: |
      [full .proj file - ~50KB]

  # Diff snapshots (small, efficient)
  - id: "s002"
    type: diff
    parent: "s001"
    diff: { added: [...], modified: [...], removed: [...] }

  - id: "s003"
    type: diff
    parent: "s002"
    diff: { ... }

  # ... up to N diffs ...

  # New full snapshot (every 10 snapshots or 100KB of diffs)
  - id: "s011"
    type: full
    content: |
      [full .proj file]
```

**Configuration:**
```rust
pub struct HistoryConfig {
    /// Create full snapshot every N snapshots (default: 10)
    pub full_snapshot_interval: usize,

    /// Create full snapshot when cumulative diff size exceeds this (default: 100KB)
    pub full_snapshot_threshold_bytes: usize,

    /// Compression for full snapshots (default: gzip level 6)
    pub compression: CompressionConfig,

    /// Maximum snapshots before suggesting migration to Git (default: 100)
    pub max_snapshots: usize,
}
```

**Snapshot Reconstruction:**
```rust
impl History {
    /// Reconstruct project state at snapshot
    pub fn reconstruct(&self, snapshot_id: &str) -> Result<String, HistoryError> {
        let snapshot = self.find_snapshot(snapshot_id)?;

        match &snapshot.snapshot_type {
            SnapshotType::Full { content } => {
                decompress(content)
            }
            SnapshotType::Diff { parent, diff } => {
                let parent_content = self.reconstruct(parent)?;
                apply_diff(&parent_content, diff)
            }
        }
    }
}
```

**Compression Strategy:**

```yaml
snapshots:
  - id: "s001"
    type: full
    encoding: gzip+base64
    content: "H4sIAAAAAAAAA6tWKkktLlGyUlAqS8wpTtVRSs7PS..."
```

- **Full snapshots**: gzip (level 6) + base64 encoding
- **Diff content**: plain text (usually small, compression overhead not worth it)
- **Typical compression ratio**: 5:1 for project files

**Rationale:**

1. **Diffs save space** - Most changes are small. Storing full copies wastes space.

2. **Periodic full snapshots for speed** - Reconstructing from 50 diffs is slow. Full snapshots every 10 versions caps reconstruction to max 10 diff applications.

3. **Threshold-based full snapshots** - If diffs accumulate to 100KB, we're not saving space anymore. Create a full snapshot.

4. **Compression for full only** - Small diffs don't benefit from compression. Overhead exceeds savings.

**Trade-offs:**

| Approach | Storage | Read Speed | Complexity |
|----------|---------|------------|------------|
| Full only | High (N × file size) | Fast (O(1)) | Low |
| Diff only | Low | Slow (O(N) diffs) | Medium |
| Hybrid | Medium | Bounded (O(interval)) | High |

**Garbage Collection:**

```rust
impl History {
    /// Remove old snapshots, keeping at least `keep` most recent
    pub fn gc(&mut self, keep: usize) {
        if self.snapshots.len() <= keep {
            return;
        }

        // Ensure we keep a full snapshot
        let oldest_to_keep = self.snapshots.len() - keep;
        let nearest_full = self.snapshots[..oldest_to_keep]
            .iter()
            .rposition(|s| s.is_full())
            .unwrap_or(0);

        // Remove snapshots before nearest_full
        self.snapshots.drain(..nearest_full);

        // Ensure first snapshot is now full
        if !self.snapshots[0].is_full() {
            let content = self.reconstruct(&self.snapshots[0].id);
            self.snapshots[0] = Snapshot::full_from_content(content);
        }
    }
}
```

**Implementation Notes:**

1. Use `flate2` crate for gzip compression
2. Use `base64` crate for encoding
3. Diff format should be semantic (task paths, attributes), not line-based
4. Track cumulative diff size to trigger full snapshot
5. Warn user when approaching max_snapshots: "Consider using Git for long-term history"

**Test Strategy:**

```rust
#[test]
fn hybrid_storage_efficient() {
    let mut history = History::new();
    let full_content = generate_project_content(100); // 100 tasks, ~50KB

    history.add_snapshot(Snapshot::full("s001", &full_content));

    for i in 2..=10 {
        let diff = generate_small_diff(); // ~500 bytes
        history.add_snapshot(Snapshot::diff(&format!("s{:03}", i), diff));
    }

    // Storage should be much less than 10 × 50KB
    assert!(history.storage_size() < 60_000); // ~50KB full + 9 × 500B diffs
}

#[test]
fn reconstruction_bounded_time() {
    let mut history = History::with_config(HistoryConfig {
        full_snapshot_interval: 5,
        ..Default::default()
    });

    // Add 20 snapshots
    for i in 1..=20 {
        if i % 5 == 1 {
            history.add_full_snapshot(content);
        } else {
            history.add_diff_snapshot(diff);
        }
    }

    // Reconstruct last snapshot should need max 4 diff applications
    let start = Instant::now();
    history.reconstruct("s020").unwrap();
    assert!(start.elapsed() < Duration::from_millis(100));
}

#[test]
fn gc_preserves_integrity() {
    let mut history = History::with_snapshots(20);
    history.gc(5);

    assert_eq!(history.snapshots.len(), 5);
    assert!(history.snapshots[0].is_full());

    // All remaining snapshots should be reconstructable
    for snapshot in &history.snapshots {
        assert!(history.reconstruct(&snapshot.id).is_ok());
    }
}
```

---

### Q11. Sidecar File Synchronization

**Recommended Design:**

**Explicit snapshot command** with optional auto-snapshot on schedule changes.

```bash
# Manual snapshot (default workflow)
utf8proj snapshot "Added testing phase"

# Auto-snapshot mode (opt-in)
utf8proj schedule --auto-snapshot

# View history
utf8proj history
utf8proj history --diff s001..s003
```

**Synchronization Strategy:**

```rust
pub struct SyncConfig {
    /// Create snapshot automatically on schedule command
    pub auto_snapshot: bool,  // default: false

    /// Create snapshot on specific events
    pub snapshot_triggers: Vec<SnapshotTrigger>,

    /// Commit history file with project (Git integration)
    pub git_commit_history: bool,  // default: true
}

pub enum SnapshotTrigger {
    /// After successful schedule command
    OnSchedule,
    /// When any task changes status (complete, actual dates)
    OnStatusChange,
    /// On explicit save (editor integration)
    OnSave,
    /// Never (manual only)
    Manual,
}
```

**Concurrent Edit Handling:**

```yaml
# project.proj.history
lock_info:
  holder: "alice@laptop"
  acquired: "2026-02-01T14:30:00Z"
  expires: "2026-02-01T15:30:00Z"
```

```rust
impl History {
    pub fn acquire_lock(&mut self, user: &str) -> Result<Lock, LockError> {
        if let Some(lock) = &self.lock_info {
            if lock.is_expired() {
                // Stale lock - take over
            } else {
                return Err(LockError::AlreadyLocked {
                    holder: lock.holder.clone(),
                    acquired: lock.acquired,
                });
            }
        }

        self.lock_info = Some(LockInfo::new(user));
        self.save()?;
        Ok(Lock::new(self))
    }
}
```

**Git Integration:**

```bash
# Recommended .gitignore entry
# (Don't ignore - history should be tracked)

# Recommended workflow
utf8proj snapshot "Sprint 5 baseline"
git add project.proj project.proj.history
git commit -m "Sprint 5 baseline"
```

**Merge Conflict Resolution:**

History file merges can conflict. Strategy:

```rust
fn merge_histories(base: &History, ours: &History, theirs: &History) -> MergeResult {
    // 1. Find common ancestor snapshot
    let common = find_common_ancestor(base, ours, theirs);

    // 2. Collect new snapshots from both branches
    let our_new = ours.snapshots_after(&common);
    let their_new = theirs.snapshots_after(&common);

    // 3. Interleave by timestamp
    let merged = interleave_by_timestamp(our_new, their_new);

    // 4. Assign new IDs to avoid collision
    let renumbered = renumber_snapshots(&merged);

    MergeResult::Success(History::from_snapshots(
        base.snapshots_up_to(&common)
            .chain(renumbered)
            .collect()
    ))
}
```

**Rationale:**

1. **Manual snapshots by default** - Automatic snapshots on every edit create noise. Users should consciously decide when to record history.

2. **Auto-snapshot opt-in** - Some users want automatic baselines. Enable with `--auto-snapshot` or config.

3. **Commit history to Git** - History file should be version controlled with the project. Two layers of history (internal + Git) provide redundancy.

4. **Advisory locking** - Prevents concurrent edits from corrupting history. Not foolproof (file could be edited directly) but helps.

5. **Timestamp-based merge** - When histories diverge, interleaving by time gives reasonable result.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Manual snapshot | User controls noise | May forget important baselines |
| Auto-snapshot | Complete history | Storage bloat, noise |
| Git-only | Leverage existing VCS | Lose semantic history (diffs are text) |
| Separate history file | Rich semantic data | Another file to manage |

**Implementation Notes:**

1. Lock timeout: 1 hour (configurable)
2. Lock is per-file, not per-project
3. Warn if history file not tracked by Git: "Consider adding project.proj.history to Git"
4. Merge uses 3-way merge if possible, else timestamp interleave
5. Snapshot IDs use UUIDs to avoid collision during merge

**Test Strategy:**

```rust
#[test]
fn manual_snapshot_workflow() {
    let mut project = Project::open("test.proj").unwrap();
    project.modify(add_task("testing"));
    project.save().unwrap();

    // History unchanged until explicit snapshot
    let history = History::load("test.proj.history").unwrap();
    assert_eq!(history.snapshots.len(), 1); // Just initial

    // Create snapshot
    project.snapshot("Added testing").unwrap();

    let history = History::load("test.proj.history").unwrap();
    assert_eq!(history.snapshots.len(), 2);
}

#[test]
fn concurrent_edit_blocked() {
    let project1 = Project::open("test.proj").unwrap();
    let lock1 = project1.history.acquire_lock("alice").unwrap();

    let project2 = Project::open("test.proj").unwrap();
    let result = project2.history.acquire_lock("bob");

    assert!(matches!(result, Err(LockError::AlreadyLocked { holder: "alice", .. })));

    drop(lock1);  // Release

    let lock2 = project2.history.acquire_lock("bob").unwrap();
    assert!(lock2.is_valid());
}

#[test]
fn history_merge_interleaves_by_timestamp() {
    let base = History::with_snapshot(s1);
    let ours = base.clone().with_snapshot(s2_10am);
    let theirs = base.clone().with_snapshot(s3_9am);

    let merged = merge_histories(&base, &ours, &theirs).unwrap();

    // s3 (9am) comes before s2 (10am)
    assert_eq!(merged.snapshots[1].message, s3.message);
    assert_eq!(merged.snapshots[2].message, s2.message);
}
```

---

## SECTION D: HISTORY SYSTEM - EMBEDDED FORMAT
**Current Confidence: 60% → Target: 95%**

### Q12. Embedded History Comment Format

**Recommended Design:**

**Defer embedded history to v2.0.** Focus on sidecar format for v1.0.

**Rationale:**

1. **Complexity vs value** - Embedded history adds significant parsing complexity for marginal benefit over sidecar.

2. **Text editor performance** - Large embedded history sections will slow down syntax highlighting and editing.

3. **Git conflicts** - Embedded history will conflict more often than sidecar (changes interleaved with project content).

4. **Single-file convenience is limited** - Users already manage `.gitignore`, `Cargo.toml`, etc. One more file isn't a burden.

5. **v1.0 focus** - Get core scheduling right first. History features are secondary.

**If Implemented in v2.0:**

```proj
# @history-start
# @snapshot id=s001 ts=2026-01-15T09:00:00Z author=alice
# @message Initial setup
# @content-hash sha256:a1b2c3d4...
# @snapshot id=s002 ts=2026-02-01 parent=s001
# @diff +task.testing duration=10d depends=backend
# @history-end

project "My Project" {
    start: 2026-01-01
    # ...
}
```

**Design Principles (for v2.0):**

1. Use comment syntax (`#`) so embedded history doesn't affect parsing
2. Clear delimiters (`@history-start`, `@history-end`)
3. Compact format (single-line per snapshot metadata)
4. Store hashes, not content (content is in the file itself)
5. Auto-migrate to sidecar when embedded exceeds 10KB

**Implementation Notes (deferred):**

1. Parser should skip history section in first pass
2. Lazy-load history only when requested
3. Consider folding support hints for editors
4. Test with vim, emacs, VS Code syntax highlighting

**Test Strategy:**

For v1.0: Test that embedded history comments don't break parsing:

```rust
#[test]
fn parser_ignores_history_comments() {
    let content = r#"
# @history-start
# @snapshot id=s001 ts=2026-01-15
# @history-end

project "Test" {
    start: 2026-01-01
}
"#;

    let project = parse_project(content).unwrap();
    assert_eq!(project.name, "Test");
}
```

---

### Q13. Embedded History Merge Conflicts

**Recommended Design:**

**Deferred to v2.0** (along with embedded history feature).

**Design Principles (for v2.0):**

1. **Append-only history** - New snapshots always added at end, reducing conflict probability

2. **Content hashes instead of content** - Embedded history stores hashes, not full content. Merging hashes is straightforward.

3. **Custom merge driver** - Git merge driver that understands history format:

```gitattributes
*.proj merge=utf8proj-history
```

```bash
# .git/config
[merge "utf8proj-history"]
    driver = utf8proj merge-driver %O %A %B %L %P
```

4. **Fallback** - If merge fails, convert to sidecar format which is easier to merge.

**Test Strategy (deferred):**

```rust
#[test]
fn embedded_history_merge_driver() {
    // Setup Git repo with merge driver
    // Create branch A with snapshot 1
    // Create branch B with snapshot 2
    // Merge - should succeed with both snapshots
}
```

---

### Q14. Embedded History Performance

**Recommended Design:**

**Deferred to v2.0.** Performance considerations for when implemented:

**Thresholds:**

| Metric | Threshold | Action |
|--------|-----------|--------|
| Embedded history size | > 10KB | Warn, suggest sidecar |
| Embedded history size | > 50KB | Auto-migrate to sidecar |
| Snapshot count | > 20 | Warn, suggest sidecar |
| File total size | > 500KB | Warn about editor performance |

**Lazy Loading:**

```rust
pub struct ProjectFile {
    /// Main project content (always loaded)
    pub content: String,

    /// History section (lazy-loaded on demand)
    pub history: LazyHistory,
}

pub enum LazyHistory {
    NotLoaded { byte_offset: usize, byte_length: usize },
    Loaded(History),
    External(PathBuf),  // Sidecar file
}

impl LazyHistory {
    pub fn load(&mut self) -> &History {
        if let LazyHistory::NotLoaded { offset, length } = self {
            // Read just the history section from file
            let history = read_history_section(offset, length);
            *self = LazyHistory::Loaded(history);
        }
        self.as_loaded().unwrap()
    }
}
```

**Editor Integration:**

Provide VS Code extension that:
- Folds history section by default
- Syntax highlights history markers
- Shows history in side panel instead of inline

**Test Strategy (deferred):**

```rust
#[test]
fn large_embedded_history_warns() {
    let project = create_project_with_history(50); // 50 snapshots
    let result = project.save_embedded();

    assert!(result.warnings.iter().any(|w|
        matches!(w, Warning::EmbeddedHistoryLarge { size, .. })
    ));
}

#[test]
fn very_large_history_auto_migrates() {
    let mut project = create_project_with_history(100);
    project.save().unwrap();

    // Should have created sidecar file
    assert!(Path::new("project.proj.history").exists());
    // Embedded history should be stub
    let content = fs::read_to_string("project.proj").unwrap();
    assert!(content.contains("# History: see project.proj.history"));
}
```

---

## SECTION E: PLAYBACK ENGINE
**Current Confidence: 65% → Target: 95%**

### Q15. Diff Algorithm for Schedule Changes

**Recommended Design:**

**Custom semantic diff** operating on the parsed project structure, not raw text.

```rust
pub struct ProjectDiff {
    pub tasks: TaskDiff,
    pub resources: ResourceDiff,
    pub dependencies: DependencyDiff,
    pub schedule: ScheduleDiff,
}

pub struct TaskDiff {
    pub added: Vec<Task>,
    pub removed: Vec<TaskId>,
    pub renamed: Vec<(TaskId, TaskId, f64)>,  // (old, new, similarity)
    pub modified: Vec<TaskModification>,
}

pub struct TaskModification {
    pub id: TaskId,
    pub changes: Vec<AttributeChange>,
}

pub enum AttributeChange {
    Duration { old: Duration, new: Duration },
    Progress { old: u8, new: u8 },
    Effort { old: Duration, new: Duration },
    StartConstraint { old: Option<NaiveDate>, new: Option<NaiveDate> },
    // ... other attributes
}
```

**Rename Detection:**

```rust
fn detect_renames(old_tasks: &[Task], new_tasks: &[Task]) -> Vec<(TaskId, TaskId, f64)> {
    let removed: HashSet<_> = old_tasks.iter()
        .filter(|t| !new_tasks.iter().any(|n| n.id == t.id))
        .collect();

    let added: HashSet<_> = new_tasks.iter()
        .filter(|t| !old_tasks.iter().any(|o| o.id == t.id))
        .collect();

    let mut renames = vec![];

    for old in &removed {
        for new in &added {
            let similarity = compute_similarity(old, new);
            if similarity > 0.7 {
                renames.push((old.id.clone(), new.id.clone(), similarity));
            }
        }
    }

    // Resolve conflicts (one old can match multiple new) by highest similarity
    resolve_rename_conflicts(renames)
}

fn compute_similarity(old: &Task, new: &Task) -> f64 {
    let mut score = 0.0;
    let mut weights = 0.0;

    // Name similarity (Levenshtein distance)
    let name_sim = 1.0 - (levenshtein(&old.name, &new.name) as f64
                         / max(old.name.len(), new.name.len()) as f64);
    score += name_sim * 3.0;  // Name is most important
    weights += 3.0;

    // Duration similarity
    if old.duration == new.duration {
        score += 2.0;
    } else if (old.duration.num_days() - new.duration.num_days()).abs() <= 2 {
        score += 1.0;
    }
    weights += 2.0;

    // Same parent container
    if old.parent == new.parent {
        score += 2.0;
    }
    weights += 2.0;

    // Similar dependencies
    let dep_overlap = jaccard_similarity(&old.dependencies, &new.dependencies);
    score += dep_overlap * 1.5;
    weights += 1.5;

    score / weights
}
```

**Schedule Diff:**

```rust
pub struct ScheduleDiff {
    pub project_start_delta: Option<i64>,
    pub project_finish_delta: Option<i64>,
    pub critical_path_changed: bool,
    pub task_date_changes: Vec<TaskDateChange>,
}

pub struct TaskDateChange {
    pub task_id: TaskId,
    pub start_delta: i64,   // days shifted
    pub finish_delta: i64,
    pub float_delta: i64,
    pub became_critical: bool,
    pub no_longer_critical: bool,
}
```

**Rationale:**

1. **Semantic diff is more meaningful** - "Task duration changed from 10d to 15d" is clearer than "+duration: 15d / -duration: 10d".

2. **Rename detection prevents false add/remove** - If user renames "backend_v1" to "backend", it should show as rename, not delete+add.

3. **Tree structure awareness** - Understanding task hierarchy allows detecting container changes correctly.

4. **Schedule impact is what users care about** - Raw file diff doesn't show "project delayed by 3 days". Semantic diff can.

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Text diff (Myers/Patience) | Simple, proven | Misses semantics, false positives |
| Semantic diff | Meaningful changes | More complex, custom implementation |
| Tree diff | Good for hierarchy | Not project-specific |
| Hybrid | Best of both | Highest complexity |

**Implementation Notes:**

1. Use `strsim` crate for string similarity (Levenshtein, Jaro-Winkler)
2. Rename threshold: 0.7 similarity (configurable)
3. Generate human-readable summary: "3 tasks added, 2 modified, 1 renamed"
4. Store diffs in history file for playback
5. Cache parsed project structures for repeated diff operations

**Test Strategy:**

```rust
#[test]
fn detects_task_rename() {
    let old = project_with_task("backend_api_v1", Duration::days(10));
    let new = project_with_task("backend_api", Duration::days(10));

    let diff = compute_diff(&old, &new);

    assert!(diff.tasks.renamed.len() == 1);
    assert_eq!(diff.tasks.renamed[0].0, "backend_api_v1");
    assert_eq!(diff.tasks.renamed[0].1, "backend_api");
    assert!(diff.tasks.added.is_empty());
    assert!(diff.tasks.removed.is_empty());
}

#[test]
fn detects_duration_change() {
    let old = project_with_task("backend", Duration::days(10));
    let new = project_with_task("backend", Duration::days(15));

    let diff = compute_diff(&old, &new);

    assert!(diff.tasks.modified.iter().any(|m|
        m.id == "backend" && m.changes.iter().any(|c|
            matches!(c, AttributeChange::Duration { old: 10, new: 15 })
        )
    ));
}

#[test]
fn schedule_diff_shows_project_delay() {
    let old = schedule_project(vec![task("a", 10), task("b", 10)]);
    let mut new = old.clone();
    new.tasks.get_mut("a").unwrap().duration = Duration::days(15);
    let new = reschedule(new);

    let diff = compute_schedule_diff(&old, &new);

    assert_eq!(diff.project_finish_delta, Some(5)); // 5 days later
}
```

---

### Q16. Impact Metrics for Playback

**Recommended Design:**

```rust
pub struct ScheduleImpact {
    // === Temporal Impact ===
    /// Project finish date change (positive = delayed)
    pub finish_delta_days: i64,
    /// Project duration change
    pub duration_delta_days: i64,

    // === Critical Path ===
    pub critical_path_changed: bool,
    pub critical_path_length_delta: i64,
    pub tasks_became_critical: Vec<TaskId>,
    pub tasks_no_longer_critical: Vec<TaskId>,

    // === Task Changes ===
    pub tasks_added: usize,
    pub tasks_removed: usize,
    pub tasks_renamed: usize,
    pub tasks_modified: usize,

    // === Progress ===
    pub overall_progress_delta: i8,  // -100 to +100
    pub completed_tasks_delta: i64,  // Tasks that became 100%

    // === Schedule Quality ===
    pub total_float_delta_days: i64,
    pub tasks_with_reduced_float: usize,
    pub over_allocations_delta: i64,

    // === Resource Impact ===
    pub resources_added: usize,
    pub resources_removed: usize,
    pub utilization_delta_pct: f64,

    // === Risk Indicators ===
    pub risk_score_delta: f64,  // Composite risk metric
}

impl ScheduleImpact {
    /// Categorize the overall impact severity
    pub fn severity(&self) -> ImpactSeverity {
        let score = self.compute_severity_score();
        match score {
            s if s < 0.2 => ImpactSeverity::Minor,
            s if s < 0.5 => ImpactSeverity::Moderate,
            s if s < 0.8 => ImpactSeverity::Significant,
            _ => ImpactSeverity::Critical,
        }
    }

    fn compute_severity_score(&self) -> f64 {
        let mut score = 0.0;

        // Project delay is most impactful
        if self.finish_delta_days > 0 {
            score += (self.finish_delta_days as f64 / 30.0).min(1.0) * 0.4;
        }

        // Critical path changes
        if self.critical_path_changed {
            score += 0.2;
        }

        // Many task changes
        let change_rate = (self.tasks_added + self.tasks_removed + self.tasks_modified) as f64
                          / self.total_tasks as f64;
        score += change_rate.min(1.0) * 0.2;

        // Float reduction (schedule tightening)
        if self.total_float_delta_days < 0 {
            score += (-self.total_float_delta_days as f64 / 50.0).min(1.0) * 0.1;
        }

        // Over-allocations
        if self.over_allocations_delta > 0 {
            score += 0.1;
        }

        score
    }
}

pub enum ImpactSeverity {
    Minor,       // Cosmetic changes, no schedule impact
    Moderate,    // Some tasks shifted, project on track
    Significant, // Project delayed or critical path changed
    Critical,    // Major delay or many constraints violated
}
```

**Visualization in Playback:**

```rust
pub struct PlaybackFrame {
    pub snapshot_id: String,
    pub timestamp: DateTime<Utc>,
    pub impact: ScheduleImpact,
    pub highlights: Vec<Highlight>,
}

pub enum Highlight {
    /// Task that changed significantly
    TaskChange { id: TaskId, change_type: ChangeType, color: Color },
    /// Critical path visualization
    CriticalPathChange { old_path: Vec<TaskId>, new_path: Vec<TaskId> },
    /// Project milestone change
    MilestoneShift { name: String, old_date: NaiveDate, new_date: NaiveDate },
}
```

**Rationale:**

1. **Project delay is king** - PMs care most about "is the project on track?". finish_delta_days is the primary metric.

2. **Critical path changes are significant** - Even if duration is same, a different critical path means different risks.

3. **Composite risk score** - Single number for quick assessment. Detailed metrics for drill-down.

4. **Severity categorization** - "Critical" change demands attention; "Minor" can be reviewed later.

5. **Actionable metrics** - Each metric suggests an action (reduce over-allocation, add float, etc.).

**Trade-offs:**

| Approach | Pros | Cons |
|----------|------|------|
| Few key metrics | Easy to understand | May miss important signals |
| Many detailed metrics | Complete picture | Information overload |
| Composite score | Single assessment | Hides details |
| Severity levels | Quick triage | Threshold arbitrary |

**Implementation Notes:**

1. Calculate metrics incrementally (diff-based) for performance
2. Store metrics in history file for quick playback
3. Allow user to configure severity thresholds
4. Provide drill-down: "Project delayed 5 days. Click to see which tasks shifted."
5. Export metrics to CSV for trend analysis

**Test Strategy:**

```rust
#[test]
fn project_delay_is_critical_impact() {
    let old_schedule = schedule_finishing(date("2026-03-01"));
    let new_schedule = schedule_finishing(date("2026-03-15"));  // 14 days late

    let impact = compute_impact(&old_schedule, &new_schedule);

    assert_eq!(impact.finish_delta_days, 14);
    assert_eq!(impact.severity(), ImpactSeverity::Significant);
}

#[test]
fn critical_path_change_is_moderate_impact() {
    let old = schedule_with_critical_path(vec!["A", "B", "C"]);
    let new = schedule_with_critical_path(vec!["A", "D", "C"]);  // Same length, different path

    let impact = compute_impact(&old, &new);

    assert!(impact.critical_path_changed);
    assert!(matches!(impact.severity(), ImpactSeverity::Moderate | ImpactSeverity::Significant));
}

#[test]
fn minor_changes_are_minor_impact() {
    let old = schedule_with_tasks(100);
    let new = old.with_modified_task("task_50", |t| t.name = "Task 50 Renamed");

    let impact = compute_impact(&old, &new);

    assert_eq!(impact.severity(), ImpactSeverity::Minor);
}
```

---

### Q17. Playback Animation Format

**Recommended Design:**

**HTML + JavaScript** as primary format for v1.0, with SVG frame export for advanced use cases.

```bash
# Generate interactive playback
utf8proj playback project.proj --output=timeline.html

# Generate SVG frames for video production
utf8proj playback project.proj --output=frames/ --format=svg

# Quick GIF for sharing (requires external tool)
utf8proj playback project.proj --output=frames/ --format=svg
ffmpeg -i frames/frame_%04d.svg -vf "fps=2" timeline.gif
```

**HTML Output Structure:**

```html
<!DOCTYPE html>
<html>
<head>
    <title>Project Timeline Playback</title>
    <style>
        /* Gantt chart styling */
        /* Controls styling */
        /* Responsive design */
    </style>
</head>
<body>
    <div id="playback-container">
        <div id="gantt-chart">
            <!-- SVG Gantt rendered here -->
        </div>

        <div id="controls">
            <button id="play-pause">▶ Play</button>
            <input type="range" id="timeline-slider" min="0" max="100">
            <span id="snapshot-label">Snapshot 1 of 10</span>
            <select id="speed">
                <option value="0.5">0.5x</option>
                <option value="1" selected>1x</option>
                <option value="2">2x</option>
            </select>
        </div>

        <div id="impact-panel">
            <h3>Changes in this version</h3>
            <ul id="change-list">
                <!-- Dynamic change list -->
            </ul>
            <div id="metrics">
                <span class="metric">Duration: <strong>+5 days</strong></span>
                <span class="metric">Progress: <strong>45% → 52%</strong></span>
            </div>
        </div>
    </div>

    <script>
        const snapshots = [/* JSON snapshot data */];
        // Playback logic
    </script>
</body>
</html>
```

**SVG Frame Structure:**

```rust
pub struct SvgFrame {
    pub snapshot_id: String,
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub content: String,  // SVG XML
}

fn render_frame(schedule: &Schedule, highlights: &[Highlight]) -> SvgFrame {
    let mut svg = SvgBuilder::new(1200, 800);

    // Header with date and snapshot info
    svg.add_header(&schedule.snapshot_info);

    // Gantt chart
    for task in &schedule.tasks {
        let bar = render_task_bar(task, &schedule.date_range);
        if highlights.contains(&task.id) {
            bar.add_highlight_animation();
        }
        svg.add(bar);
    }

    // Critical path overlay
    svg.add_critical_path(&schedule.critical_path);

    // Legend and metrics
    svg.add_legend();
    svg.add_metrics_panel(&schedule.metrics);

    SvgFrame {
        snapshot_id: schedule.snapshot_id.clone(),
        index: schedule.index,
        width: 1200,
        height: 800,
        content: svg.to_string(),
    }
}
```

**Animation Features:**

1. **Task bar transitions** - Bars smoothly shift position between snapshots
2. **Progress fill animation** - Progress bar fills/unfills with transition
3. **Critical path highlight** - Critical path pulses or uses distinct color
4. **Change callouts** - Annotations pointing to significant changes
5. **Timeline scrubbing** - User can drag to any point in history

**Rationale:**

1. **HTML is universally accessible** - No software installation required. Works in any browser.

2. **Interactive controls** - Play/pause, scrub, speed control add value over static formats.

3. **Responsive design** - Works on desktop, tablet, mobile.

4. **SVG for quality** - Vector graphics scale to any resolution. Good for print/video.

5. **GIF via FFmpeg** - Don't reinvent GIF encoding. Let FFmpeg handle it from SVG frames.

**Trade-offs:**

| Format | Accessibility | Interactivity | Quality | File Size | Effort |
|--------|--------------|---------------|---------|-----------|--------|
| HTML+JS | Excellent (browser) | Full | Vector | Small | Medium |
| SVG frames | Good | None | Vector | Medium | Low |
| GIF | Universal | None | Raster | Large | Medium (FFmpeg) |
| MP4 | Good | None | Raster | Medium | High |

**Implementation Notes:**

1. Use `svg` crate for SVG generation
2. Embed minimal JavaScript (no framework needed)
3. Include all data inline (single self-contained file)
4. Support dark/light themes
5. Export at multiple resolutions: 1280x720 (HD), 1920x1080 (FHD)
6. Frame rate: 1 frame per snapshot (not time-interpolated)

**Test Strategy:**

```rust
#[test]
fn html_playback_renders() {
    let history = History::with_snapshots(5);
    let html = render_playback_html(&history).unwrap();

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("snapshots = ["));
    assert!(html.contains("play-pause"));
}

#[test]
fn svg_frames_generated() {
    let history = History::with_snapshots(5);
    let frames = render_svg_frames(&history).unwrap();

    assert_eq!(frames.len(), 5);
    for frame in &frames {
        assert!(frame.content.starts_with("<svg"));
        assert!(frame.content.contains("</svg>"));
    }
}

#[test]
fn playback_shows_task_changes() {
    let history = History::new()
        .with_snapshot(project_with_task("A", 10))
        .with_snapshot(project_with_task_modified("A", 15));

    let html = render_playback_html(&history).unwrap();

    // Should highlight task A change
    assert!(html.contains("task-changed"));
    assert!(html.contains("duration: 10d → 15d"));
}
```

---

## SUMMARY - PART 1

### Decisions Made

| Question | Decision | Confidence |
|----------|----------|------------|
| Q1. Duration calculation | Linear default + explicit `remaining:` override | 95% |
| Q2. Dependencies with progress | Standard 100% trigger + optional percentage triggers | 95% |
| Q3. Actual vs dependency conflict | Warn but accept actual dates (reality wins) | 95% |
| Q4. Leveling with progress | In-progress tasks are anchored, level future only | 95% |
| Q5. Container progress | Duration-weighted average, derived actual dates | 95% |
| Q6. Container with duration + children | Warning, children win (ignore container duration) | 95% |
| Q7. Container vs leaf progress | Allow explicit with validation, manual mode to suppress | 95% |
| Q8. Empty containers | Valid as placeholder milestone, optional estimate | 95% |
| Q9. Sidecar format | YAML with structured schema | 95% |
| Q10. Sidecar structure | Hybrid (periodic full + diffs) | 95% |
| Q11. Sidecar synchronization | Manual snapshot default, auto opt-in, Git integration | 95% |
| Q12. Embedded history format | **Deferred to v2.0** | N/A |
| Q13. Embedded merge conflicts | **Deferred to v2.0** | N/A |
| Q14. Embedded performance | **Deferred to v2.0** | N/A |
| Q15. Diff algorithm | Custom semantic diff on parsed structure | 95% |
| Q16. Impact metrics | Comprehensive metrics with severity scoring | 95% |
| Q17. Animation format | HTML+JS primary, SVG frames for export | 95% |

### Section Confidence After Survey

| Section | Before | After |
|---------|--------|-------|
| A: Progress-Aware CPM | 75% | 95% |
| B: Container Derivation | 80% | 95% |
| C: History - Sidecar | 70% | 95% |
| D: History - Embedded | 60% | Deferred |
| E: Playback Engine | 65% | 95% |

---

**END OF PART 1**

Continue to Part 2 for Sections F-I (Excel Export, Resource Leveling, BDD/SAT, TaskJuggler Compatibility).
