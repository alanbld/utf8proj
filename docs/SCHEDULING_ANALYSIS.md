# utf8proj Scheduling Analysis: PMI/PERT/CPM Compliance Review

## Executive Summary

This document analyzes utf8proj's scheduling behavior against PMI (PMBOK), PERT, and CPM standards, identifying gaps and proposing improvements.

**Key Finding**: utf8proj correctly implements basic CPM (forward/backward pass, critical path, float calculation) but has a significant gap in **effort-to-duration calculation** that ignores resource allocation units.

---

## 1. Current Behavior Analysis

### 1.1 The Core Issue: Effort vs Duration

**PMI Standard Formula**:
```
Duration = Work (Effort) / Resource Units
```

**utf8proj Current Implementation** (`crates/utf8proj-solver/src/lib.rs:203-214`):
```rust
fn get_task_duration_days(task: &Task) -> i64 {
    if let Some(dur) = task.duration {
        return dur.as_days().ceil() as i64;
    }
    // BUG: Assumes 1 resource at 100%, ignores actual allocation
    if let Some(effort) = task.effort {
        return effort.as_days().ceil() as i64;
    }
    0
}
```

**Problem**: The `units` field in `ResourceRef` (allocation percentage) is completely ignored.

**Example**:
| Scenario | Effort | Allocation | utf8proj Result | PMI Correct Result |
|----------|--------|------------|-----------------|-------------------|
| 1 resource @ 100% | 40h | 1.0 | 5 days | 5 days |
| 1 resource @ 50% | 40h | 0.5 | 5 days | **10 days** |
| 2 resources @ 100% | 40h | 2.0 | 5 days | **2.5 days** |

This explains the difference with tj3 in our M2C comparison (119/233 tasks differed).

### 1.2 What utf8proj Does Correctly

| Feature | Status | Notes |
|---------|--------|-------|
| Forward Pass (ES, EF) | Correct | Standard CPM algorithm |
| Backward Pass (LS, LF) | Correct | Standard CPM algorithm |
| Critical Path Detection | Correct | Slack = 0 identification |
| Total Float/Slack | Correct | TF = LS - ES |
| Dependency Types (FS, SS, FF, SF) | Correct | With lag support |
| Calendar/Working Days | Correct | Holidays, weekends |
| Resource Leveling | Partial | Serializes conflicts, but doesn't adjust duration |
| Container Task Dates | Correct | Min/max of children |

### 1.3 What tj3 Does Differently

tj3 has two duration keywords:
- `effort`: Work effort that's divided by resource allocation → Duration
- `length`: Fixed calendar duration regardless of resources

When utf8proj parses `effort 5d` from a TJP file, it should calculate:
```
Duration = 5d / (sum of assigned resource units)
```

---

## 2. PMI Task Types (Missing Abstraction)

PMI/PMBOK defines three task types that utf8proj should support:

### 2.1 Fixed Duration
- Duration is constant regardless of resources
- Adding resources increases total work
- Use case: Meetings, training, curing concrete
- **TJP keyword**: `length`

### 2.2 Fixed Work (Effort-Driven)
- Work/Effort is constant
- Adding resources reduces duration
- Use case: Most development tasks
- **TJP keyword**: `effort`
- **Formula**: `Duration = Work / Units`

### 2.3 Fixed Units
- Resource allocation is constant
- Changing duration changes work
- Use case: Resource-constrained tasks

### Current utf8proj Behavior
utf8proj treats everything as Fixed Duration, which is incorrect for `effort`-based tasks.

---

## 3. Resource Leveling Comparison

### 3.1 PMI Definitions

**Resource Leveling** (extends schedule):
- Adjusts start/finish dates based on resource constraints
- May extend project duration
- Used when resources are over-allocated

**Resource Smoothing** (within float only):
- Adjusts only within available float
- Never extends project duration
- Creates more uniform utilization

### 3.2 utf8proj Current Leveling

The current leveling (`crates/utf8proj-solver/src/leveling.rs`) correctly:
- Detects over-allocations
- Shifts tasks to resolve conflicts
- Uses priority-based heuristics

But it doesn't recalculate duration based on allocation units.

---

## 4. Proposed Fixes

### 4.1 Fix Effort-to-Duration Calculation (Critical)

```rust
fn get_task_duration_days(task: &Task) -> i64 {
    // Fixed duration takes precedence
    if let Some(dur) = task.duration {
        return dur.as_days().ceil() as i64;
    }

    // Effort-driven: Duration = Effort / Total Units
    if let Some(effort) = task.effort {
        let total_units: f32 = if task.assigned.is_empty() {
            1.0 // Default: assume 1 resource at 100%
        } else {
            task.assigned.iter().map(|r| r.units).sum()
        };
        return (effort.as_days() / total_units as f64).ceil() as i64;
    }

    0 // Milestone
}
```

### 4.2 Add Task Type Enum (Enhancement)

```rust
/// Task scheduling type per PMI/PMBOK
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum TaskType {
    /// Duration fixed, work adjusts with resources
    FixedDuration,
    /// Work fixed, duration adjusts with resources (effort-driven)
    #[default]
    FixedWork,
    /// Resource units fixed
    FixedUnits,
}
```

### 4.3 Enhanced Duration Calculation

```rust
fn calculate_task_duration(task: &Task, project: &Project) -> i64 {
    let total_units = get_total_resource_units(task, project);

    match task.task_type {
        TaskType::FixedDuration => {
            task.duration.map(|d| d.as_days().ceil() as i64).unwrap_or(0)
        }
        TaskType::FixedWork => {
            if let Some(effort) = task.effort {
                (effort.as_days() / total_units as f64).ceil() as i64
            } else {
                0
            }
        }
        TaskType::FixedUnits => {
            // Work = Duration × Units, but we need work or duration as input
            task.duration.map(|d| d.as_days().ceil() as i64).unwrap_or(0)
        }
    }
}

fn get_total_resource_units(task: &Task, project: &Project) -> f32 {
    if task.assigned.is_empty() {
        return 1.0;
    }

    task.assigned.iter().map(|ref_| {
        // Consider resource efficiency if defined
        let efficiency = project.get_resource(&ref_.resource_id)
            .map(|r| r.efficiency)
            .unwrap_or(1.0);
        ref_.units * efficiency
    }).sum()
}
```

---

## 5. Additional PMI-Compliant Enhancements

### 5.1 Resource Efficiency
Resources can have efficiency < 100% (learning curve, part-time, etc.):
```rust
pub struct Resource {
    // ... existing fields
    /// Efficiency factor (0.0-1.0, default 1.0)
    pub efficiency: f32,
}
```

Effective units = allocation × efficiency

### 5.2 Resource Smoothing Option
Add smoothing that only uses float:
```rust
pub enum LevelingMode {
    /// Extend schedule to resolve conflicts
    Leveling,
    /// Only use float, never extend schedule
    Smoothing,
}
```

### 5.3 PERT Three-Point Estimation
Support optimistic/most likely/pessimistic:
```rust
pub struct PertEstimate {
    pub optimistic: Duration,
    pub most_likely: Duration,
    pub pessimistic: Duration,
}

impl PertEstimate {
    /// Expected duration: (O + 4M + P) / 6
    pub fn expected(&self) -> Duration {
        let o = self.optimistic.minutes as f64;
        let m = self.most_likely.minutes as f64;
        let p = self.pessimistic.minutes as f64;
        Duration::minutes(((o + 4.0 * m + p) / 6.0) as i64)
    }

    /// Standard deviation: (P - O) / 6
    pub fn std_dev(&self) -> f64 {
        (self.pessimistic.minutes - self.optimistic.minutes) as f64 / 6.0
    }
}
```

### 5.4 Free Float Calculation
Currently only Total Float is calculated. Add Free Float:
```rust
pub struct ScheduledTask {
    // ... existing fields
    /// Free float: delay without affecting any successor
    pub free_float: Duration,
}
```

---

## 6. Implementation Priority

| Priority | Fix | Impact | Effort |
|----------|-----|--------|--------|
| **P0** | Effort/Duration calculation | Fixes 119/233 tasks in M2C | Low |
| P1 | TaskType enum | PMI compliance | Medium |
| P2 | Resource efficiency | Realistic scheduling | Low |
| P2 | Free float | Complete CPM | Low |
| P3 | Resource smoothing | Optimization option | Medium |
| P3 | PERT estimation | Risk analysis | Medium |

---

## 7. Verification Test Cases

After implementing fixes, these should pass:

```rust
#[test]
fn effort_with_partial_allocation() {
    let mut project = Project::new("Test");
    project.resources.push(Resource::new("dev").capacity(1.0));
    project.tasks.push(
        Task::new("work")
            .effort(Duration::hours(40)) // 5 days of work
            .assign_with_units("dev", 0.5) // 50% allocation
    );

    let schedule = CpmSolver::new().schedule(&project).unwrap();
    let task = schedule.tasks.get("work").unwrap();

    // 40h / (0.5 * 8h/day) = 10 days
    assert_eq!(task.duration.as_days(), 10.0);
}

#[test]
fn effort_with_multiple_resources() {
    let mut project = Project::new("Test");
    project.resources.push(Resource::new("dev1"));
    project.resources.push(Resource::new("dev2"));
    project.tasks.push(
        Task::new("work")
            .effort(Duration::hours(40))
            .assign("dev1")
            .assign("dev2") // 2 resources @ 100% = 200%
    );

    let schedule = CpmSolver::new().schedule(&project).unwrap();
    let task = schedule.tasks.get("work").unwrap();

    // 40h / (2.0 * 8h/day) = 2.5 days
    assert_eq!(task.duration.as_days(), 2.5);
}
```

---

## 8. Conclusion

utf8proj's core CPM implementation is correct, but the **effort-to-duration conversion is non-compliant with PMI standards**. The fix is straightforward (P0 priority) and will significantly improve tj3 compatibility.

The additional enhancements (TaskType, efficiency, PERT) would make utf8proj a more complete PMI-compliant scheduler, but the P0 fix alone resolves the primary behavioral difference observed in testing.
