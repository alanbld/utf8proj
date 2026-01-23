# Scaling utf8proj: Resource Leveling Optimization

**RFC Number:** 0014 (Draft)
**Status:** Investigation
**Created:** 2026-01-21
**Author:** utf8proj contributors
**Related:** RFC-0001 (Architecture), Performance Summary v0.11.1
**Target Version:** 0.12.0

---

## Executive Summary

utf8proj's CPM scheduler already handles million-task projects efficiently (73k tasks/sec). The scaling bottleneck is the **O(n²) resource leveling algorithm**, which degrades to >2 minutes for 10,000-task projects with resource conflicts.

**Recommended approach:** Hybrid leveling that combines BDD's conflict detection speed (2x faster) with heuristic resolution efficiency (22x faster under heavy conflicts).

This is **not** a migration away from BDD—it's leveraging BDD where it excels while addressing the heuristic algorithm's quadratic complexity.

---

## 1. Current Performance Profile

### 1.1 What Works Well

| Component | Scale | Performance | Status |
|-----------|-------|-------------|--------|
| CPM Scheduling | 100k tasks | 156k tasks/sec | ✅ Excellent |
| CPM Scheduling | 1M tasks | 73k tasks/sec | ✅ Excellent |
| BDD Conflict Detection | Few conflicts | 2x faster than heuristic | ✅ Excellent |
| Parser/Renderer | 244 real tasks | <1s total | ✅ Production-ready |

**Conclusion:** Core scheduling infrastructure requires no changes.

### 1.2 The Actual Bottleneck

| Project Scale | Leveling Time | Issue |
|---------------|---------------|-------|
| 15 tasks (tutorial) | 0.3s | ✅ Acceptable |
| 1,000 tasks | 0.4s | ✅ Acceptable |
| 5,000 tasks | ~1s | ⚠️ Noticeable |
| 10,000 tasks | >2 min | ❌ Too slow |

**Root cause:** `find_available_slot()` in `leveling.rs` performs O(n) scans per task, yielding O(n²) overall complexity when many tasks compete for the same resources.

**v0.11.1 stopgap:** The infinite loop bug was fixed by adding a 2000 working day search limit with L002 diagnostic. This prevents hangs but doesn't address the underlying O(n²) complexity—it just bounds the worst case. Hybrid leveling would supersede this limit by guaranteeing termination through BDD-validated feasible windows.

### 1.3 BDD vs Heuristic Characteristics

| Scenario | BDD | Heuristic | Insight |
|----------|-----|-----------|---------|
| Few conflicts (single resource) | 3.7ms | 7.5ms | BDD 2x faster |
| Many conflicts (multi-resource) | 56ms | 2.6ms | Heuristic 22x faster |

**Key insight:** These aren't competing approaches—they excel at different phases of the leveling problem.

---

## 2. Problem Analysis

### 2.1 Why O(n²) Emerges

```
Current algorithm (simplified):

for each task in priority order:           # O(n)
    for each time slot until available:    # O(n) worst case
        check resource availability
    assign task to slot
```

When resources are heavily contended:
- Each task may scan many slots before finding availability
- Later tasks scan increasingly far into the future
- Worst case: task N scans N-1 previously scheduled tasks

### 2.2 Why BDD Helps (But Doesn't Solve Everything)

BDD represents the constraint space compactly:
- **Conflict detection:** "Is there *any* valid slot?" → BDD answers in O(1) amortized
- **Conflict enumeration:** "Which slots are valid?" → BDD can enumerate efficiently
- **Conflict resolution:** "Schedule N tasks to minimize makespan" → BDD alone isn't optimal

The 22x slowdown under many conflicts occurs because BDD encodes all possibilities but doesn't provide a heuristic for *choosing* among them efficiently.

---

## 3. Proposed Solution: Hybrid Leveling

### 3.1 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Hybrid Resource Leveler                   │
│                                                              │
│  Phase 1: BDD Conflict Analysis                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  • Build resource constraint BDD per resource          │  │
│  │  • Identify conflict clusters (tasks competing)        │  │
│  │  • Compute earliest feasible windows per task          │  │
│  │  • O(n) with BDD operations                            │  │
│  └────────────────────────────────────────────────────────┘  │
│                           │                                  │
│                           ▼                                  │
│  Phase 2: Heuristic Resolution                               │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  • Process conflict clusters independently             │  │
│  │  • Use priority rules within known-feasible windows    │  │
│  │  • Skip impossible slots (BDD already ruled out)       │  │
│  │  • O(n log n) with preprocessing                       │  │
│  └────────────────────────────────────────────────────────┘  │
│                           │                                  │
│                           ▼                                  │
│  Output: Leveled schedule with explanations                  │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Phase 1: BDD Conflict Analysis

> **Implementation Note:** The `extract_feasible_windows()` function assumes OxiDD can efficiently enumerate satisfying assignments within a time range. This requires verification—if OxiDD's enumeration is O(solutions) rather than O(1), an alternative approach using BDD projection operations may be needed.

```rust
/// Conflict analysis result from BDD
pub struct ConflictAnalysis {
    /// Tasks grouped by competing resource sets
    pub clusters: Vec<ConflictCluster>,

    /// Per-task feasibility windows (precomputed from BDD)
    pub windows: HashMap<TaskId, Vec<TimeWindow>>,

    /// Tasks with no conflicts (can be scheduled immediately)
    pub unconstrained: Vec<TaskId>,
}

pub struct ConflictCluster {
    pub tasks: Vec<TaskId>,
    pub resources: Vec<ResourceId>,
    pub estimated_contention: f32,  // 0.0 = no conflicts, 1.0 = fully serialized
}

impl BddLeveler {
    /// Analyze conflicts without scheduling
    /// Returns in O(n) time using BDD operations
    pub fn analyze_conflicts(&self, project: &Project) -> ConflictAnalysis {
        let mut clusters = Vec::new();
        let mut windows = HashMap::new();

        // Build per-resource BDDs
        for resource in &project.resources {
            let bdd = self.build_resource_bdd(resource, &project.tasks);

            // Extract feasible windows for each task on this resource
            for task in project.tasks_using(resource) {
                let task_windows = bdd.extract_feasible_windows(task);
                windows.entry(task.id)
                    .or_insert_with(Vec::new)
                    .extend(task_windows);
            }

            // Identify conflict clusters
            let conflicts = bdd.find_conflicts();
            if !conflicts.is_empty() {
                clusters.push(ConflictCluster {
                    tasks: conflicts,
                    resources: vec![resource.id],
                    estimated_contention: bdd.contention_ratio(),
                });
            }
        }

        ConflictAnalysis { clusters, windows, unconstrained: self.find_unconstrained() }
    }
}
```

### 3.3 Phase 2: Heuristic Resolution

```rust
impl HybridLeveler {
    pub fn level(&self, project: &Project) -> Result<Schedule, LevelingError> {
        // Phase 1: BDD analysis (fast)
        let analysis = self.bdd.analyze_conflicts(project);

        // Schedule unconstrained tasks immediately
        let mut schedule = Schedule::new();
        for task_id in &analysis.unconstrained {
            schedule.assign_at_earliest(task_id);
        }

        // Phase 2: Process conflict clusters
        for cluster in &analysis.clusters {
            self.resolve_cluster(&mut schedule, cluster, &analysis.windows)?;
        }

        Ok(schedule)
    }

    fn resolve_cluster(
        &self,
        schedule: &mut Schedule,
        cluster: &ConflictCluster,
        windows: &HashMap<TaskId, Vec<TimeWindow>>,
    ) -> Result<(), LevelingError> {
        // Sort tasks by priority rule
        let mut tasks: Vec<_> = cluster.tasks.iter()
            .map(|id| (id, self.compute_priority(id)))
            .collect();
        tasks.sort_by(|a, b| b.1.cmp(&a.1));

        for (task_id, _priority) in tasks {
            // Only search within BDD-validated windows
            let feasible_windows = windows.get(task_id)
                .ok_or(LevelingError::NoFeasibleWindow(*task_id))?;

            // Find first available slot within feasible windows
            let slot = self.find_slot_in_windows(schedule, task_id, feasible_windows)?;
            schedule.assign(task_id, slot);
        }

        Ok(())
    }
}
```

### 3.4 Complexity Analysis

| Phase | Current | Hybrid |
|-------|---------|--------|
| Conflict detection | O(n²) implicit | O(n) via BDD |
| Slot search | O(n) per task | O(log k) per task* |
| Total | O(n²) | O(n log n) |

*k = number of feasible windows, typically small

---

## 4. Alternative Approaches

### 4.1 Interval Tree for Slot Lookup

Replace linear slot search with interval tree:

```rust
use intervaltree::IntervalTree;

pub struct ResourceTimeline {
    /// Interval tree of (start, end) -> task_id
    occupied: IntervalTree<NaiveDate, TaskId>,
}

impl ResourceTimeline {
    /// Find gaps in O(log n) instead of O(n)
    pub fn find_gap(&self, duration: Duration, after: NaiveDate) -> Option<TimeWindow> {
        // Query overlapping intervals
        let overlaps = self.occupied.query(after..NaiveDate::MAX);
        // Find first gap >= duration
        // ...
    }
}
```

**Complexity improvement:** O(n log n) for all slot lookups combined.

**Trade-off:** Doesn't provide the what-if analysis capability that BDD offers.

### 4.2 SAT/CP for Optimal Leveling

For cases where optimal (not just feasible) leveling matters:

```rust
#[cfg(feature = "optimal-leveling")]
pub fn level_optimal(project: &Project) -> Schedule {
    use pumpkin_solver::Solver;

    let mut solver = Solver::default();

    // Decision variables: task start times
    let starts: Vec<_> = project.tasks.iter()
        .map(|t| solver.new_int_var(0, project.horizon()))
        .collect();

    // Cumulative constraint for each resource
    for resource in &project.resources {
        let tasks_on_resource: Vec<_> = project.tasks
            .iter()
            .filter(|t| t.uses_resource(resource.id))
            .collect();

        solver.add_cumulative(
            tasks_on_resource.iter().map(|t| starts[t.index]),
            tasks_on_resource.iter().map(|t| t.duration),
            tasks_on_resource.iter().map(|t| t.resource_demand(resource.id)),
            resource.capacity,
        );
    }

    // Minimize makespan
    let makespan = solver.new_int_var(0, project.horizon());
    for (i, task) in project.tasks.iter().enumerate() {
        solver.add_constraint(starts[i] + task.duration <= makespan);
    }
    solver.minimize(makespan);

    solver.solve().expect("Feasible solution exists")
}
```

**When to use:**
- Tight resource constraints where heuristic quality matters
- Benchmark comparisons
- Projects where 10-30 second solve time is acceptable

**Trade-off:** Slower than heuristic, but guarantees optimal makespan.

### 4.3 Parallel Leveling

Independent resource timelines can be processed concurrently:

```rust
use rayon::prelude::*;

pub fn level_parallel(project: &Project) -> Schedule {
    // Group tasks by primary resource
    let resource_groups = group_by_resource(&project.tasks);

    // Level each resource group in parallel
    let partial_schedules: Vec<_> = resource_groups
        .par_iter()
        .map(|(resource, tasks)| level_single_resource(resource, tasks))
        .collect();

    // Merge partial schedules (handle cross-resource dependencies)
    merge_schedules(partial_schedules)
}
```

**Speedup potential:** 2-4x on typical multi-core machines for projects with independent resource pools.

---

## 5. Implementation Plan

### Phase 0: Quick Win (v0.11.2) ✅ COMPLETE

**BTreeMap Slot Lookup Optimization** — Shipped in v0.11.2

- [x] Changed `ResourceTimeline.usage` from HashMap to BTreeMap for sorted iteration
- [x] Added skip-blocked-runs algorithm: when a slot is blocked, skip to the end of the blocked period
- [x] Added `SlotCheckResult` enum and `find_blocked_run_end()` helper
- [x] Benchmark: 2-5x improvement achieved for heavily contended resources
- [x] Keep 2000-day limit as safety net

### Phase 1: Hybrid Leveling (v0.12.0) ✅ COMPLETE

**BDD Conflict Cluster Analysis + Heuristic Resolution** — Shipped in v0.12.0

- [x] Implement `ClusterAnalysis` and `ConflictCluster` structs
- [x] Add `analyze_clusters()` to BDD analyzer (identifies independent conflict groups)
- [x] Implement `hybrid_level_resources()` function
- [x] Process unconstrained tasks first (no leveling needed)
- [x] Level conflict clusters independently (reduces complexity)
- [x] Add `--leveling-strategy=hybrid` CLI flag
- [x] Benchmark: **4-5x speedup** achieved (500 tasks: 0.23s→0.05s, 2000 tasks: 5.35s→1.07s)
- [x] Unit tests for hybrid leveling correctness and determinism

### Phase 2: Parallel Cluster Leveling (v0.12.1)

**Target: 10,000 tasks in <30s**

Based on profiling (BDD is <1% of time, heuristic is bottleneck):

- [ ] Add `rayon` for parallel processing
- [ ] Level independent clusters concurrently
- [ ] Expected speedup: ~Nx for N clusters (5x for typical 5-resource projects)
- [ ] Benchmark: 5000 tasks should drop from 1.5min to ~20s

### Phase 3: Interval Tree Slot Finding (v0.12.2)

**Target: 10,000 tasks in <10s**

Replace O(n) day-by-day slot search with O(log n) interval tree:

- [ ] Add `intervaltree` or `nodit` crate
- [ ] Implement interval-based `find_available_slot()`
- [ ] Combined with parallel clusters: O(n log n / P) where P = parallelism

### Phase 4: Optional Optimal Leveling (v0.13.0+)

For cases where makespan optimization matters:

- [ ] Add `pumpkin-solver` as optional dependency
- [ ] Implement `level_optimal()` behind feature flag
- [ ] Document when to use optimal vs heuristic

---

## 6. Benchmark Results (v0.12.0)

### 6.1 Performance Comparison

| Tasks | Standard | Hybrid | Speedup |
|-------|----------|--------|---------|
| 500 | 0.23s | 0.05s | 4.6x |
| 1000 | 0.82s | 0.18s | 4.5x |
| 2000 | 5.4s | 1.07s | 5x |
| 3000 | 4 min 16s | **13.8s** | **19x** |
| 5000 | est. >30 min | **1.5 min** | ~20x |
| 7500 | est. hours | **7.2 min** | - |
| 10000 | ∞ (hangs) | >15 min | - |

### 6.2 Practical Limits

| Strategy | Comfortable | Acceptable | Maximum |
|----------|-------------|------------|---------|
| **Standard** | ≤1000 tasks | ≤2000 tasks | ~3000 tasks |
| **Hybrid** | ≤3000 tasks | ≤5000 tasks | ~7500 tasks |

### 6.3 Profiling Analysis

Profiling with `UTF8PROJ_PROFILE=1` reveals the bottleneck:

| Tasks | BDD Analysis | Heuristic Leveling | Total | BDD % |
|-------|--------------|-------------------|-------|-------|
| 1000 | 7.5ms | 152ms | 161ms | 4.6% |
| 2000 | 27ms | 954ms | 983ms | 2.7% |
| 3000 | 63ms | 12.7s | 12.7s | **0.5%** |

**Key Finding:** The BDD cluster analysis is fast (O(n), <1% of total time). The bottleneck is the **heuristic slot-finding within clusters** which is O(k²) where k is cluster size.

**Implication:** Parallel BDD libraries (OxiDD, etc.) would NOT help — the BDD is already fast. To further improve:
1. **Parallel cluster processing** — level independent clusters concurrently with rayon
2. **Interval tree for slots** — O(log n) gap finding instead of O(n) day-by-day scanning

### 6.4 Cluster Effectiveness

The hybrid approach correctly identifies independent clusters:
- 1000 tasks → 5 clusters of ~200 tasks each (one per resource)
- 2000 tasks → 5 clusters of ~400 tasks each
- 3000 tasks → 5 clusters of ~600 tasks each

This gives ~5x speedup by processing clusters independently, matching benchmarks.

---

## 7. Success Criteria

| Metric | v0.11.0 | v0.12.0 (Hybrid) | Target |
|--------|---------|------------------|--------|
| 1,000 tasks leveling | 0.82s | **0.18s** ✅ | <0.3s |
| 3,000 tasks leveling | 4+ min | **13.8s** ✅ | <30s |
| 5,000 tasks leveling | hours | **1.5 min** ⚠️ | <1 min |
| 10,000 tasks leveling | ∞ | >15 min ❌ | <10s |
| Leveling quality (makespan) | Heuristic | Same ✅ | Same or better |
| What-if analysis | ✅ Supported | ✅ Preserved | ✅ Preserved |

---

## 8. Non-Goals

This RFC explicitly does **not** propose:

1. **Replacing BDD for what-if analysis** — BDD remains the correct tool for constraint reasoning
2. **Migrating to pure SAT/CP** — Hybrid approach leverages both paradigms
3. **Changing the CPM scheduler** — Already excellent at 73k tasks/sec
4. **Breaking the text-based format** — Git-friendliness is preserved
5. **Parallel BDD libraries (OxiDD)** — Profiling shows BDD is <1% of time; not the bottleneck

---

## 9. Relationship to utf8proj Positioning

This optimization maintains utf8proj's core differentiators:

| Differentiator | Status |
|----------------|--------|
| Deterministic behavior | ✅ Hybrid leveling is deterministic |
| BDD what-if analysis | ✅ Preserved and enhanced |
| Text-based format | ✅ No changes |
| Single binary | ✅ No new required dependencies |
| Explainability | ✅ Conflict clusters provide natural explanations |

**Key insight from BDD comparison doc:** utf8proj's value is formal constraint reasoning, not optimization. This RFC improves *performance* without compromising *positioning*.

---

## 10. Open Questions

1. **Cluster ordering:** How should conflict clusters be processed? By size? By resource criticality? By project priority?
   - *Current:* Largest clusters first (deterministic)

2. **BDD caching:** Should conflict analysis be cached across incremental changes?
   - *Answer:* Low priority — BDD is only 0.5-5% of total time

3. **Threshold tuning:** At what project size should hybrid leveling activate? Current data suggests ~500 tasks.
   - *Answer:* Hybrid is always faster; could become default in v0.13.0

4. **Parallel strategy:** Is per-resource parallelism sufficient, or do we need finer-grained concurrency?
   - *Answer from profiling:* Per-cluster parallelism is sufficient — clusters are independent and contain the O(k²) work

5. **v0.11.1 search limit:** Should the 2000-day limit remain as a safety net even after hybrid leveling?
   - *Answer:* Keep for defense-in-depth; doesn't affect performance

6. **OxiDD/parallel BDD:** Would parallel BDD with variable reordering help?
   - *Answer from profiling:* **No** — BDD analysis is <1% of time. The bottleneck is heuristic slot-finding within clusters, not BDD operations.

---

## References

- utf8proj Performance Summary (v0.11.1)
- RFC-0001: utf8proj Architecture
- bdd_vs_ml_comparison.md: Positioning analysis
- OxiDD documentation: BDD operations complexity

---

**Document Version:** 0.1
**Status:** Draft — Pending performance validation of hybrid approach
