# utf8proj Test Specifications

**Document Version:** 1.0  
**Created:** 2026-01-01  
**Purpose:** Define test cases derived from project management specifications, NOT from TaskJuggler source code.

---

## Legal Notice

These test specifications are derived from:
- Project management textbooks and standards (PMBOK, etc.)
- TaskJuggler user manual (public documentation)
- Critical Path Method (CPM) academic literature
- Industry-standard scheduling behaviors

**These tests are NOT copied from TaskJuggler test suite** to avoid GPL contamination.

---

## 1. Basic Scheduling Tests

### 1.1 Duration-Based Scheduling

**Spec:** A task with explicit duration takes that duration regardless of resources.

```rust
#[test]
fn duration_based_task_takes_explicit_duration() {
    // Spec: PM standard - duration is calendar time
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }  # Monday
        task impl "Implementation" {
            duration: 5d
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    let task = &schedule.tasks["impl"];
    
    // 5 working days = Mon-Fri
    assert_eq!(task.start, date(2025, 1, 6));
    assert_eq!(task.finish, date(2025, 1, 10));
}
```

### 1.2 Effort-Based Scheduling

**Spec:** A task with effort and one full-time resource takes effort/capacity days.

```rust
#[test]
fn effort_based_scheduling_single_resource() {
    // Spec: effort = person-time, duration = effort / (capacity * units)
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        resource dev "Developer" { capacity: 1.0 }
        task impl "Implementation" {
            effort: 10d
            assign: dev
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    let task = &schedule.tasks["impl"];
    
    // 10 person-days with 1 person = 10 calendar days
    assert_eq!(task.duration.as_days(), 10.0);
}

#[test]
fn effort_based_scheduling_half_time_resource() {
    // Spec: half-time resource doubles the calendar duration
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        resource dev "Developer" { capacity: 0.5 }
        task impl "Implementation" {
            effort: 10d
            assign: dev
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    let task = &schedule.tasks["impl"];
    
    // 10 person-days with 0.5 capacity = 20 calendar days
    assert_eq!(task.duration.as_days(), 20.0);
}

#[test]
fn effort_based_scheduling_multiple_resources() {
    // Spec: multiple resources reduce duration proportionally
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        resource dev1 "Developer 1" { capacity: 1.0 }
        resource dev2 "Developer 2" { capacity: 1.0 }
        task impl "Implementation" {
            effort: 10d
            assign: dev1, dev2
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    let task = &schedule.tasks["impl"];
    
    // 10 person-days with 2 people = 5 calendar days
    assert_eq!(task.duration.as_days(), 5.0);
}
```

### 1.3 Duration vs Effort Precedence

**Spec:** When both duration and effort are specified, duration takes precedence.

```rust
#[test]
fn duration_takes_precedence_over_effort() {
    // Spec: explicit duration overrides effort-based calculation
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        resource dev "Developer" {}
        task impl "Implementation" {
            effort: 10d
            duration: 5d
            assign: dev
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    let task = &schedule.tasks["impl"];
    
    // Duration (5d) takes precedence over effort (10d)
    assert_eq!(task.duration.as_days(), 5.0);
}
```

---

## 2. Dependency Tests

### 2.1 Finish-to-Start (Default)

**Spec:** FS dependency means successor starts after predecessor finishes.

```rust
#[test]
fn finish_to_start_dependency() {
    // Spec: CPM - FS is default dependency type
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }  # Monday
        task a "Task A" { duration: 5d }
        task b "Task B" { duration: 3d, depends: a }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // A: Mon-Fri (Jan 6-10)
    assert_eq!(schedule.tasks["a"].start, date(2025, 1, 6));
    assert_eq!(schedule.tasks["a"].finish, date(2025, 1, 10));
    
    // B starts Monday Jan 13 (next working day after A finishes)
    assert_eq!(schedule.tasks["b"].start, date(2025, 1, 13));
}
```

### 2.2 Start-to-Start

**Spec:** SS dependency means successor starts when predecessor starts.

```rust
#[test]
fn start_to_start_dependency() {
    // Spec: SS allows parallel work starting together
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { duration: 10d }
        task b "Task B" { duration: 5d, depends: a SS }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // Both start on same day
    assert_eq!(schedule.tasks["a"].start, schedule.tasks["b"].start);
}
```

### 2.3 Finish-to-Finish

**Spec:** FF dependency means successor finishes when predecessor finishes.

```rust
#[test]
fn finish_to_finish_dependency() {
    // Spec: FF ensures tasks complete together
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { duration: 10d }
        task b "Task B" { duration: 3d, depends: a FF }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // Both finish on same day
    assert_eq!(schedule.tasks["a"].finish, schedule.tasks["b"].finish);
}
```

### 2.4 Lag and Lead Time

**Spec:** Positive lag delays successor, negative lag (lead) allows overlap.

```rust
#[test]
fn dependency_with_lag() {
    // Spec: lag adds waiting time between tasks
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { duration: 5d }
        task b "Task B" { duration: 3d, depends: a + 2d }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // A finishes Fri Jan 10
    // B starts Wed Jan 15 (2 working days after A finishes)
    assert_eq!(schedule.tasks["b"].start, date(2025, 1, 15));
}

#[test]
fn dependency_with_lead() {
    // Spec: lead (negative lag) allows overlap
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { duration: 10d }
        task b "Task B" { duration: 5d, depends: a - 3d }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // B can start 3 days before A finishes
    // A: Jan 6-17, B starts Jan 15
    assert!(schedule.tasks["b"].start < schedule.tasks["a"].finish);
}
```

---

## 3. Critical Path Tests

### 3.1 Simple Critical Path

**Spec:** Critical path is the longest path through the project.

```rust
#[test]
fn critical_path_identification() {
    // Spec: CPM algorithm
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { duration: 5d }
        task b "Task B" { duration: 10d }
        task c "Task C" { duration: 3d, depends: a }
        task d "Task D" { duration: 3d, depends: b }
        task e "Task E" { duration: 2d, depends: c, d }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // Critical path: B → D → E (10 + 3 + 2 = 15 days)
    // Non-critical: A → C (5 + 3 = 8 days)
    assert!(schedule.tasks["b"].is_critical);
    assert!(schedule.tasks["d"].is_critical);
    assert!(schedule.tasks["e"].is_critical);
    assert!(!schedule.tasks["a"].is_critical);
    assert!(!schedule.tasks["c"].is_critical);
}
```

### 3.2 Slack Calculation

**Spec:** Slack is the amount a task can slip without delaying the project.

```rust
#[test]
fn slack_calculation() {
    // Spec: slack = late_start - early_start
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { duration: 5d }
        task b "Task B" { duration: 10d }
        task c "Task C" { duration: 3d, depends: a, b }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // A has slack (5 days shorter than B)
    assert!(schedule.tasks["a"].slack.as_days() > 0.0);
    
    // B is critical (no slack)
    assert_eq!(schedule.tasks["b"].slack.as_days(), 0.0);
    
    // C is critical
    assert_eq!(schedule.tasks["c"].slack.as_days(), 0.0);
}
```

---

## 4. Resource Leveling Tests

### 4.1 Resource Overallocation Detection

**Spec:** Resource cannot be allocated more than 100% at any time.

```rust
#[test]
fn detects_resource_overallocation() {
    // Spec: resource capacity constraint
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        resource dev "Developer" { capacity: 1.0 }
        task a "Task A" { duration: 5d, assign: dev }
        task b "Task B" { duration: 5d, assign: dev }
    "#).unwrap();
    
    let result = scheduler.is_feasible(&project);
    
    // Parallel tasks with same resource = overallocation
    // (unless one is leveled)
    assert!(!result.feasible || result.conflicts.iter().any(|c| 
        matches!(c.conflict_type, ConflictType::ResourceOverallocation)
    ));
}
```

### 4.2 Resource Leveling

**Spec:** Leveling delays tasks to resolve overallocation.

```rust
#[test]
fn resource_leveling_delays_tasks() {
    // Spec: resource leveling algorithm
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        resource dev "Developer" { capacity: 1.0 }
        task a "Task A" { duration: 5d, assign: dev, priority: 1000 }
        task b "Task B" { duration: 5d, assign: dev, priority: 500 }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // Higher priority task scheduled first
    assert!(schedule.tasks["a"].start <= schedule.tasks["b"].start);
    
    // Tasks don't overlap
    assert!(
        schedule.tasks["a"].finish <= schedule.tasks["b"].start ||
        schedule.tasks["b"].finish <= schedule.tasks["a"].start
    );
}
```

---

## 5. Calendar Tests

### 5.1 Weekend Handling

**Spec:** Tasks don't progress on non-working days.

```rust
#[test]
fn tasks_skip_weekends() {
    // Spec: calendar working days
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }  # Monday
        calendar "default" {
            working_days: mon-fri
        }
        task impl "Implementation" { duration: 10d }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    let task = &schedule.tasks["impl"];
    
    // 10 working days starting Monday Jan 6
    // = Jan 6-10 (5d) + Jan 13-17 (5d) = finishes Fri Jan 17
    assert_eq!(task.start, date(2025, 1, 6));
    assert_eq!(task.finish, date(2025, 1, 17));
}
```

### 5.2 Holiday Handling

**Spec:** Tasks don't progress on holidays.

```rust
#[test]
fn tasks_skip_holidays() {
    // Spec: calendar holidays
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        calendar "default" {
            working_days: mon-fri
            holiday "Day Off" 2025-01-08..2025-01-08  # Wednesday
        }
        task impl "Implementation" { duration: 5d }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    let task = &schedule.tasks["impl"];
    
    // 5 working days, but Wed is holiday
    // Mon(6), Tue(7), skip Wed, Thu(9), Fri(10), Mon(13)
    assert_eq!(task.finish, date(2025, 1, 13));
}
```

---

## 6. Constraint Tests

### 6.1 Start No Earlier Than

**Spec:** Task cannot start before specified date.

```rust
#[test]
fn start_no_earlier_than_constraint() {
    // Spec: SNET constraint
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task impl "Implementation" { 
            duration: 5d
            start_no_earlier_than: 2025-01-13
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // Task starts on or after Jan 13
    assert!(schedule.tasks["impl"].start >= date(2025, 1, 13));
}
```

### 6.2 Must Finish On

**Spec:** Task must finish exactly on specified date.

```rust
#[test]
fn must_finish_on_constraint() {
    // Spec: MFO constraint - hard deadline
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task impl "Implementation" { 
            duration: 5d
            must_finish_on: 2025-01-17
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // Task finishes exactly on Jan 17
    assert_eq!(schedule.tasks["impl"].finish, date(2025, 1, 17));
    // Therefore starts Jan 13
    assert_eq!(schedule.tasks["impl"].start, date(2025, 1, 13));
}
```

---

## 7. Hierarchy (WBS) Tests

### 7.1 Summary Task Duration

**Spec:** Summary task duration spans from earliest child start to latest child finish.

```rust
#[test]
fn summary_task_spans_children() {
    // Spec: WBS hierarchy
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task phase1 "Phase 1" {
            task a "Task A" { duration: 5d }
            task b "Task B" { duration: 3d, depends: a }
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    
    // Summary spans all children
    assert_eq!(schedule.tasks["phase1"].start, schedule.tasks["a"].start);
    assert_eq!(schedule.tasks["phase1"].finish, schedule.tasks["b"].finish);
}
```

---

## 8. Circular Dependency Detection

**Spec:** Circular dependencies make scheduling impossible.

```rust
#[test]
fn detects_circular_dependency() {
    // Spec: DAG requirement
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { duration: 5d, depends: c }
        task b "Task B" { duration: 5d, depends: a }
        task c "Task C" { duration: 5d, depends: b }
    "#).unwrap();
    
    let result = scheduler.schedule(&project);
    
    assert!(matches!(result, Err(ScheduleError::CircularDependency(_))));
}
```

---

## 9. Cost Calculation Tests

### 9.1 Task Cost

**Spec:** Task cost = sum of (resource rate × time allocated).

```rust
#[test]
fn task_cost_calculation() {
    // Spec: cost = rate × duration
    let project = parse(r#"
        project "Test" { start: 2025-01-06, currency: EUR }
        resource dev "Developer" { rate: 800/day }
        task impl "Implementation" { 
            effort: 10d
            assign: dev
        }
    "#).unwrap();
    
    let schedule = schedule(&project).unwrap();
    let assignment = &schedule.tasks["impl"].assignments[0];
    
    // 10 days × €800/day = €8000
    assert_eq!(assignment.cost.unwrap().amount, Decimal::from(8000));
}
```

---

## 10. BDD/SAT-Specific Tests

### 10.1 Feasibility Check

**Spec:** BDD can quickly determine if any valid schedule exists.

```rust
#[test]
#[cfg(feature = "bdd")]
fn bdd_feasibility_check() {
    // Spec: BDD symbolic reasoning
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { 
            duration: 10d
            must_finish_on: 2025-01-10  # Only 5 working days!
        }
    "#).unwrap();
    
    let bdd_scheduler = BddScheduler::new();
    let result = bdd_scheduler.is_feasible(&project);
    
    // 10-day task cannot finish in 5 days
    assert!(!result.feasible);
}
```

### 10.2 Solution Counting

**Spec:** BDD can count all valid schedules.

```rust
#[test]
#[cfg(feature = "bdd")]
fn bdd_solution_counting() {
    // Spec: BDD sat_count
    let project = parse(r#"
        project "Test" { start: 2025-01-06 }
        task a "Task A" { duration: 1d }
        task b "Task B" { duration: 1d }
        # No dependencies, both can be on any day
    "#).unwrap();
    
    let bdd_scheduler = BddScheduler::new();
    let count = bdd_scheduler.count_solutions(&project);
    
    // Multiple valid orderings exist
    assert!(count > BigUint::from(1u32));
}
```

---

## Test Utilities

```rust
// tests/common/mod.rs

use chrono::NaiveDate;
use utf8proj_core::*;
use utf8proj_parser::parse;
use utf8proj_solver::HeuristicScheduler;

pub fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

pub fn schedule(project: &Project) -> Result<Schedule, ScheduleError> {
    HeuristicScheduler::default().schedule(project)
}

/// Compare two schedules for behavioral equivalence
pub fn schedules_equivalent(a: &Schedule, b: &Schedule) -> bool {
    if a.tasks.len() != b.tasks.len() {
        return false;
    }
    
    for (task_id, task_a) in &a.tasks {
        let Some(task_b) = b.tasks.get(task_id) else {
            return false;
        };
        
        if task_a.start != task_b.start || task_a.finish != task_b.finish {
            return false;
        }
    }
    
    true
}
```

---

## Oracle Testing (Optional)

For TaskJuggler compatibility verification:

```rust
// tests/oracle/taskjuggler.rs

#[test]
#[ignore] // Requires TaskJuggler installed
fn matches_taskjuggler_schedule() {
    let tjp = include_str!("fixtures/sample.tjp");
    
    // Run through TaskJuggler (black-box oracle)
    let tj_output = Command::new("tj3")
        .args(["--check", "--output", "/dev/stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("tj3 installed");
    
    // Parse TJ output
    let tj_schedule = parse_tj_output(&tj_output);
    
    // Run through utf8proj
    let project = utf8proj_parser::tjp::parse(tjp).unwrap();
    let our_schedule = schedule(&project).unwrap();
    
    // Compare behavioral equivalence
    assert!(schedules_equivalent(&tj_schedule, &our_schedule));
}
```

---

**Remember:** These tests are derived from specifications, not from TaskJuggler code. Document the specification source in each test comment.
