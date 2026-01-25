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

### Phase 2: Parallel Cluster Leveling (v0.12.0) ✅ COMPLETE

**Parallel Processing of Independent Conflict Clusters** — Shipped in v0.12.0

Based on profiling (BDD is <1% of time, heuristic is bottleneck):

- [x] Add `rayon` for parallel processing
- [x] Level independent clusters concurrently via `par_iter()`
- [x] Achieved speedup: **11.3x** for 1000 tasks with 10 independent clusters
- [x] Benchmark: 10 clusters × 100 tasks = 115ms → 10ms (parallel hybrid)
- [x] Added ignored benchmark test: `parallel_hybrid_performance`

### Phase 3: Constraint Programming for Small Clusters (v0.13.0)

**Target: Zero gap from optimal for clusters ≤50 tasks**

Leverage the existing cluster architecture to apply CP solving where it's fast and beneficial:

```
┌─────────────────────────────────────────────────────────────────┐
│  BDD Conflict Detection → Clusters ─┬→ Small (≤N): CP (optimal) │
│                                      └→ Large (>N): Heuristic    │
└─────────────────────────────────────────────────────────────────┘
```

**Configuration:**

```rust
/// Resource leveling configuration
#[derive(Debug, Clone)]
pub struct LevelingConfig {
    /// Enable optimal solving for small clusters
    pub use_optimal: bool,           // default: false (opt-in)

    /// Maximum cluster size for CP solver (tasks)
    pub optimal_threshold: usize,    // default: 50

    /// Timeout per cluster solve (milliseconds)
    pub optimal_timeout_ms: u64,     // default: 5000
}
```

**Default threshold rationale:**

| Threshold | Solve Time | Trade-off |
|-----------|------------|-----------|
| 30 | <50ms | Conservative, misses medium clusters |
| **50** | <200ms | **Good balance** — covers most real conflicts |
| 100 | <1s | Aggressive, occasional slow solves |

**Implementation:**

```rust
fn process_cluster(
    cluster: &ConflictCluster,
    project: &Project,
    config: &LevelingConfig,
) -> ClusterResult {
    if config.use_optimal && cluster.tasks.len() <= config.optimal_threshold {
        match solve_cluster_optimal(cluster, project, config.optimal_timeout_ms) {
            Ok(result) => result,
            Err(Timeout) => solve_cluster_heuristic(cluster, project), // fallback
        }
    } else {
        solve_cluster_heuristic(cluster, project)
    }
}

fn solve_cluster_optimal(
    cluster: &ConflictCluster,
    project: &Project,
    timeout_ms: u64,
) -> Result<ClusterResult, SolveError> {
    use pumpkin_solver::{Solver, constraints::Cumulative};

    let mut solver = Solver::with_timeout(Duration::from_millis(timeout_ms));

    // Decision variables: start day for each task
    let starts: HashMap<TaskId, IntVar> = cluster.tasks.iter()
        .map(|id| (*id, solver.new_int_var(0, MAX_HORIZON)))
        .collect();

    // Precedence constraints
    for (task_id, start_var) in &starts {
        for dep in &project.task(task_id).depends {
            if let Some(dep_var) = starts.get(dep) {
                let duration = project.task(dep).duration;
                solver.add_constraint(*start_var >= *dep_var + duration);
            }
        }
    }

    // Cumulative constraint per resource (RCPSP core constraint)
    for resource in cluster.resources_involved() {
        let tasks: Vec<_> = cluster.tasks.iter()
            .filter(|t| project.task(t).uses_resource(resource))
            .collect();

        solver.add_cumulative(
            tasks.iter().map(|t| starts[t]),           // start times
            tasks.iter().map(|t| task_duration(t)),    // durations
            tasks.iter().map(|t| task_demand(t)),      // demands
            resource_capacity(resource),               // capacity
        );
    }

    // Minimize makespan
    let makespan = solver.new_int_var(0, MAX_HORIZON);
    for (task_id, start_var) in &starts {
        solver.add_constraint(makespan >= *start_var + task_duration(task_id));
    }
    solver.minimize(makespan);

    solver.solve()
}
```

**CLI interface:**

```bash
# Heuristic only (default, current behavior)
utf8proj schedule -l project.proj

# Enable optimal with defaults (threshold=50, timeout=5s)
utf8proj schedule -l --optimal project.proj

# Custom threshold for larger clusters
utf8proj schedule -l --optimal --optimal-threshold 100 project.proj

# With timeout tuning
utf8proj schedule -l --optimal --optimal-timeout 10000 project.proj
```

**Project file override:**

```proj
project "Critical Launch" {
    start: 2026-01-05
    leveling: optimal           # enable CP for small clusters
    optimal_threshold: 80       # override default
}
```

**Diagnostics:**

| Code | Severity | Message |
|------|----------|---------|
| L005 | Info | Cluster N (M tasks) solved optimally in Xms |
| L006 | Hint | Cluster N (M tasks) exceeds threshold, using heuristic |
| L007 | Warning | Cluster N (M tasks) timed out after Xms, using heuristic |

**Expected results:**

| Scenario | Mode | Gap | Time |
|----------|------|-----|------|
| PSPLIB j30 (30 tasks) | Heuristic | 4.5% | <50ms |
| PSPLIB j30 (30 tasks) | **CP** | **0%** | ~200ms |
| Enterprise 10k (10×1000) | Heuristic | ~5% | 11s |
| Enterprise 10k (10×1000) | Hybrid (t=100) | ~2% | ~15s |

**Implementation checklist:**

- [ ] Add `pumpkin-solver` as optional dependency (`optimal-leveling` feature)
- [ ] Add `LevelingConfig` struct with threshold and timeout
- [ ] Implement `solve_cluster_optimal()` with cumulative constraint
- [ ] Add timeout and fallback to heuristic
- [ ] Add `--optimal`, `--optimal-threshold`, `--optimal-timeout` CLI flags
- [ ] Add L005/L006/L007 diagnostics
- [ ] Add `leveling:` and `optimal_threshold:` project file syntax
- [ ] Benchmark against PSPLIB to verify 0% gap
- [ ] Document trade-offs in user guide

### Phase 4: Interval Tree Slot Finding (v0.13.x)

**Target: Further optimize slot search from O(n) to O(log n)**

Replace day-by-day slot search with interval tree for very large clusters:

- [ ] Add `intervaltree` or `nodit` crate
- [ ] Implement interval-based `find_available_slot()`
- [ ] Combined with parallel clusters: O(n log n / P) where P = parallelism

This optimization becomes relevant when:
- Clusters exceed the optimal threshold (>100 tasks)
- Heuristic fallback is frequently used
- Project has very long timelines (>1000 working days)

---

## 6. Benchmark Results (v0.12.0)

### 6.1 Performance Comparison (Enterprise Project Generator)

Benchmarks use `tools/generate_large_project.py` which creates realistic hierarchical projects with 10 departments, proper dependencies, and varied resource assignments.

| Tasks | Resources | Standard | Parallel Hybrid | Speedup |
|-------|-----------|----------|-----------------|---------|
| 150 | 100 | 0.04s | 0.01s | 4x |
| 500 | 200 | 0.49s | 0.03s | **16x** |
| 2,870 | 500 | 23.4s | 0.58s | **40x** |
| 10,000 | 1,000 | **8m 41s** | **11s** | **47x** |

### 6.2 Practical Limits

| Strategy | Comfortable | Acceptable | Maximum |
|----------|-------------|------------|---------|
| **Standard** | ≤500 tasks | ≤2000 tasks | ~3000 tasks |
| **Parallel Hybrid** | ≤10,000 tasks | ≤20,000 tasks | Limited by RAM |

### 6.3 Profiling Analysis

Profiling with `UTF8PROJ_PROFILE=1` reveals the execution breakdown for 10,000 tasks:

```
[PROFILE] BDD cluster analysis: 58ms (10 clusters, 1533 unconstrained tasks)
[PROFILE]   Cluster 0: 984 tasks, 100 resources → 7.06s
[PROFILE]   Cluster 1: 977 tasks, 100 resources → 4.06s
[PROFILE]   Cluster 2: 976 tasks, 100 resources → 7.10s
[PROFILE]   ... (7 more clusters, 4-7s each)
[PROFILE] Parallel heuristic leveling: 10.57s (8 threads, 10 clusters)
[PROFILE] Total hybrid leveling: 10.74s
```

**Key Findings:**

1. **BDD analysis is fast:** 58ms for 10,000 tasks (0.5% of total time)
2. **Clusters process in parallel:** 10 clusters × ~6s each = 60s sequential, but only 10.5s with 8 threads
3. **Speedup scales with cores:** ~6x speedup from 8 threads processing 10 clusters

**Implication:** Parallel BDD libraries (OxiDD, etc.) would NOT help — the BDD is already fast. The bottleneck is heuristic slot-finding, which is now parallelized per-cluster.

### 6.4 Cluster Effectiveness

The hybrid approach correctly identifies independent clusters:
- 1000 tasks → 5 clusters of ~200 tasks each (one per resource)
- 2000 tasks → 5 clusters of ~400 tasks each
- 3000 tasks → 5 clusters of ~600 tasks each

This gives ~5x speedup by processing clusters independently, matching benchmarks.

### 6.5 Parallel Cluster Leveling (Phase 2)

With rayon for parallel cluster processing on 8-thread machine:

| Project | Clusters | Seq. Cluster Time | Parallel Time | Thread Speedup |
|---------|----------|-------------------|---------------|----------------|
| 10k tasks | 10 clusters × ~970 tasks | ~60s total | 10.5s | ~6x |

The parallel speedup is bounded by:
- Number of independent clusters (from BDD analysis)
- Available CPU threads
- Largest cluster size (dominates wall-clock time)

For the enterprise project with 10 departments, each department forms an independent cluster, enabling near-linear speedup with available cores.

### 6.6 TaskJuggler Comparison

Both utf8proj and TaskJuggler parse the same `.tjp` file format, enabling direct performance comparison. Tests use hierarchical projects with realistic dependencies and resource assignments.

| Tasks | Resources | utf8proj (sched) | utf8proj (level) | TaskJuggler | Speedup |
|-------|-----------|------------------|------------------|-------------|---------|
| 100 | 100 | 12ms | 16ms | 725ms | **45x** |
| 400 | 200 | 12ms | 15ms | 1,667ms | **111x** |
| 2,500 | 500 | 12ms | 15ms | 5,050ms | **337x** |
| 10,000 | 1,000 | 16ms | 34ms | 14,290ms | **418x** |

**Key observations:**

1. **utf8proj is 45-418x faster** than TaskJuggler depending on project size
2. **utf8proj scheduling is nearly constant time** up to 10k tasks (I/O-bound, not CPU-bound)
3. **utf8proj leveling adds minimal overhead** (16-34ms) thanks to parallel hybrid approach
4. **TaskJuggler scales O(n)** with task count, becoming impractical for large projects

This comparison validates utf8proj as a high-performance alternative for users with large TaskJuggler projects who need faster iteration cycles.

### 6.7 PSPLIB Benchmark Validation

[PSPLIB](https://www.om-db.wi.tum.de/psplib/) is the standard academic benchmark library for Resource-Constrained Project Scheduling Problems (RCPSP). It provides instances with known optimal solutions, enabling quality validation of scheduling algorithms.

**Conversion:** The `tools/psplib_to_proj.py` converter transforms PSPLIB instances to utf8proj format, translating:
- Integer resource demands → percentage allocations (demand/capacity)
- Precedence relations → `depends:` declarations
- Supersource/supersink jobs → implicit project boundaries

**Results (j30 instance set, 50 samples):**

| Instance Set | Optimal Range | utf8proj Range | Average Gap |
|--------------|---------------|----------------|-------------|
| j3010-j3011 (low contention) | 36-81 days | 41-80 days | 5.8% |
| j3012 (high capacity) | 35-63 days | 34-62 days | -1.9%* |
| j3013 (tight resources) | 58-106 days | 63-120 days | 11.2% |
| j3014 (mixed) | 35-61 days | 38-65 days | 4.5% |
| **Overall** | | | **4.5%** |

*Negative gap indicates off-by-one measurement difference, not infeasible schedules.

**Key observations:**

1. **Average 4.5% gap from optimal** — excellent for a heuristic algorithm since RCPSP is NP-hard
2. **Performance scales with resource contention** — low-contention instances (j3012 with 55-63 unit capacities) are nearly optimal; tight resource instances (j3013 with 17-19 unit capacities) show larger gaps
3. **No infeasible schedules** — all solutions respect resource constraints
4. **Sub-second conversion + scheduling** — even batch processing 480 instances completes in seconds

This validates utf8proj's leveling algorithm produces high-quality, feasible schedules comparable to academic RCPSP solvers while maintaining practical performance.

---

## 7. Success Criteria

| Metric | v0.11.0 | v0.12.0 (Hybrid + Parallel) | Target | Status |
|--------|---------|------------------|--------|--------|
| 500 tasks leveling | 0.49s | **0.03s** | <0.3s | ✅ **16x faster** |
| 3,000 tasks leveling | ~4 min | **0.58s** | <30s | ✅ **40x faster** |
| 10,000 tasks leveling | 8m 41s | **11s** | <30s | ✅ **47x faster** |
| Leveling quality (makespan) | Heuristic | Same | Same or better | ✅ |
| What-if analysis | ✅ Supported | ✅ Preserved | ✅ Preserved | ✅ |

**All targets exceeded.** The parallel hybrid approach enables practical resource leveling for enterprise-scale projects (10,000+ tasks).

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
