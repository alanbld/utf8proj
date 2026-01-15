# RFC-0002: CPM Correctness & Strategic Evolution

**RFC Number:** 0002
**Title:** CPM Solver Correctness & Strategic Repositioning
**Author:** Alan (utf8dok team)
**Status:** Implemented
**Created:** 2026-01-03
**Supersedes:** Portions of RFC-0001 (solver architecture)
**Related:** RFC-0001 (Architecture)  

---

## Summary

This RFC addresses critical architectural issues in utf8proj's CPM solver identified during external review, and proposes a strategic repositioning that plays to utf8proj's genuine strengths rather than competing head-to-head with TaskJuggler.

**Key Changes:**
1. Fundamental CPM solver rewrite based on textbook algorithms
2. Clean separation between WBS (presentation) and DAG (scheduling)
3. Strategic pivot: "Modern CPM engine for developers" not "TaskJuggler replacement"
4. CPM correctness test suite as a first-class deliverable

---

## Problem Statement

### Current Architecture Issues

The existing solver implementation violates fundamental CPM invariants:

| Issue | Symptom | Impact |
|-------|---------|--------|
| Container-aware scheduling | Dependencies across containers ignored | Incorrect task ordering |
| Parent date derivation | Parent tasks scheduled independently | WBS dates don't reflect children |
| Negative slack | Slack < 0 in some configurations | Mathematically impossible in valid CPM |
| Backward pass overflow | Very negative slack causes crashes | Solver instability |

### Root Cause

```
CURRENT (BROKEN):
┌─────────────────────────────────────────────────┐
│  Container A          Container B               │
│  ┌─────────┐          ┌─────────┐              │
│  │ Task 1  │ ──────── │ Task 3  │  ← IGNORED!  │
│  │ Task 2  │          │ Task 4  │              │
│  └─────────┘          └─────────┘              │
│      │                     │                    │
│      ▼                     ▼                    │
│  Schedule A           Schedule B    ← SEPARATE! │
└─────────────────────────────────────────────────┘

REQUIRED (CORRECT):
┌─────────────────────────────────────────────────┐
│  1. Flatten all leaf tasks into single DAG      │
│  2. Resolve ALL dependencies (cross-container)  │
│  3. Single topological sort                     │
│  4. Forward pass (ES/EF) over entire graph      │
│  5. Backward pass (LS/LF) over entire graph     │
│  6. Derive container dates FROM children        │
└─────────────────────────────────────────────────┘
```

**The WBS (Work Breakdown Structure) is for PRESENTATION.**  
**The DAG (Directed Acyclic Graph) is for SCHEDULING.**  
**These must be completely separated.**

---

## Proposed Architecture

### Core Principle: Two-Phase Processing

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        utf8proj REVISED ARCHITECTURE                     │
│                                                                          │
│  PHASE 1: PARSING                    PHASE 2: SCHEDULING                │
│  ────────────────────               ─────────────────────               │
│                                                                          │
│  ┌──────────────┐                   ┌──────────────────────┐            │
│  │   .proj      │                   │   SchedulingGraph    │            │
│  │   file       │                   │                      │            │
│  └──────┬───────┘                   │  • Flat list of      │            │
│         │                           │    leaf tasks only   │            │
│         ▼                           │  • All dependencies  │            │
│  ┌──────────────┐                   │    resolved to       │            │
│  │   WBS Tree   │ ──── flatten ──── │    leaf-to-leaf     │            │
│  │  (hierarchy) │                   │  • No containers     │            │
│  └──────────────┘                   └──────────┬───────────┘            │
│         │                                      │                        │
│         │                                      ▼                        │
│         │                           ┌──────────────────────┐            │
│         │                           │   CPM Algorithm      │            │
│         │                           │                      │            │
│         │                           │  1. Topological sort │            │
│         │                           │  2. Forward pass     │            │
│         │                           │  3. Backward pass    │            │
│         │                           │  4. Critical path    │            │
│         │                           └──────────┬───────────┘            │
│         │                                      │                        │
│         │                                      ▼                        │
│         │                           ┌──────────────────────┐            │
│         │                           │  Leaf Schedule       │            │
│         │                           │  (ES, EF, LS, LF,    │            │
│         │                           │   slack, critical)   │            │
│         │                           └──────────┬───────────┘            │
│         │                                      │                        │
│         ▼                                      ▼                        │
│  ┌─────────────────────────────────────────────────────────┐           │
│  │                    MERGE / DERIVE                        │           │
│  │                                                          │           │
│  │  Container.start = min(children.start)                   │           │
│  │  Container.end   = max(children.end)                     │           │
│  │  Container.effort = sum(children.effort)                 │           │
│  └─────────────────────────────────────────────────────────┘           │
│                              │                                          │
│                              ▼                                          │
│  ┌─────────────────────────────────────────────────────────┐           │
│  │                    COMPLETE SCHEDULE                     │           │
│  │         (WBS structure + computed dates)                 │           │
│  └─────────────────────────────────────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### New Module: `utf8proj-solver/src/dag.rs`

```rust
//! Dependency graph construction and validation
//! 
//! This module handles the critical transformation from hierarchical WBS
//! to flat DAG suitable for CPM scheduling.

use crate::TaskId;
use std::collections::{HashMap, HashSet};

/// A flattened, schedulable graph of leaf tasks only
#[derive(Debug)]
pub struct SchedulingGraph {
    /// All leaf tasks (no containers)
    pub tasks: Vec<LeafTask>,
    
    /// Adjacency list: task_id -> list of successor task_ids
    pub successors: HashMap<TaskId, Vec<TaskId>>,
    
    /// Reverse adjacency: task_id -> list of predecessor task_ids  
    pub predecessors: HashMap<TaskId, Vec<TaskId>>,
    
    /// Topological order (computed once, reused)
    pub topo_order: Vec<TaskId>,
}

/// A leaf task extracted from the WBS
#[derive(Debug, Clone)]
pub struct LeafTask {
    pub id: TaskId,
    pub name: String,
    pub duration: Duration,           // Calendar duration (computed from effort if needed)
    pub effort: Option<Duration>,     // Person-time
    pub assigned: Vec<ResourceId>,
    pub wbs_path: Vec<TaskId>,        // Path from root for reconstruction
}

impl SchedulingGraph {
    /// Flatten a WBS tree into a scheduling graph
    pub fn from_wbs(project: &Project) -> Result<Self, GraphError> {
        let mut tasks = Vec::new();
        let mut successors: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
        let mut predecessors: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
        
        // 1. Collect all leaf tasks
        collect_leaves(&project.tasks, &mut tasks, vec![]);
        
        // 2. Resolve all dependencies to leaf-to-leaf edges
        for task in &tasks {
            let resolved = resolve_dependencies(task, &project.tasks)?;
            successors.entry(task.id.clone()).or_default();
            for pred_id in resolved {
                successors.entry(pred_id.clone()).or_default().push(task.id.clone());
                predecessors.entry(task.id.clone()).or_default().push(pred_id);
            }
        }
        
        // 3. Compute topological order (also validates acyclicity)
        let topo_order = topological_sort(&tasks, &successors)?;
        
        Ok(Self { tasks, successors, predecessors, topo_order })
    }
}

/// Resolve a dependency reference to actual leaf task IDs
/// 
/// Examples:
///   "task_a"                -> [task_a] (if leaf)
///   "container.subtask"     -> [subtask] (if leaf)
///   "container"             -> [all leaves under container]
fn resolve_dependencies(
    task: &LeafTask,
    wbs: &[Task],
) -> Result<Vec<TaskId>, GraphError> {
    // ... implementation
}

/// Kahn's algorithm for topological sort
/// Returns error if cycle detected
fn topological_sort(
    tasks: &[LeafTask],
    successors: &HashMap<TaskId, Vec<TaskId>>,
) -> Result<Vec<TaskId>, GraphError> {
    let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
    let mut queue: VecDeque<TaskId> = VecDeque::new();
    let mut result: Vec<TaskId> = Vec::new();
    
    // Initialize in-degrees
    for task in tasks {
        in_degree.insert(task.id.clone(), 0);
    }
    for (_, succs) in successors {
        for succ in succs {
            *in_degree.get_mut(succ).unwrap() += 1;
        }
    }
    
    // Start with zero in-degree nodes
    for (id, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(id.clone());
        }
    }
    
    // Process
    while let Some(task_id) = queue.pop_front() {
        result.push(task_id.clone());
        if let Some(succs) = successors.get(&task_id) {
            for succ in succs {
                let deg = in_degree.get_mut(succ).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(succ.clone());
                }
            }
        }
    }
    
    // Check for cycle
    if result.len() != tasks.len() {
        let remaining: Vec<_> = tasks.iter()
            .filter(|t| !result.contains(&t.id))
            .map(|t| t.id.clone())
            .collect();
        return Err(GraphError::CycleDetected { tasks: remaining });
    }
    
    Ok(result)
}
```

### New Module: `utf8proj-solver/src/cpm.rs`

```rust
//! Critical Path Method implementation
//! 
//! Textbook CPM algorithm operating on a SchedulingGraph.
//! References: 
//!   - Kelley & Walker (1959) "Critical-Path Planning and Scheduling"
//!   - PMI PMBOK Guide, Chapter 6

use crate::dag::SchedulingGraph;
use chrono::NaiveDate;

/// Result of CPM scheduling for a single task
#[derive(Debug, Clone)]
pub struct CpmResult {
    pub task_id: TaskId,
    pub es: NaiveDate,      // Early Start
    pub ef: NaiveDate,      // Early Finish
    pub ls: NaiveDate,      // Late Start
    pub lf: NaiveDate,      // Late Finish
    pub total_slack: i64,   // Total Float (days) - ALWAYS >= 0
    pub free_slack: i64,    // Free Float (days)
    pub is_critical: bool,  // On critical path (total_slack == 0)
}

/// CPM scheduler operating on flattened graph
pub struct CpmScheduler {
    calendar: Calendar,
}

impl CpmScheduler {
    pub fn schedule(
        &self,
        graph: &SchedulingGraph,
        project_start: NaiveDate,
    ) -> Result<CpmSchedule, CpmError> {
        let mut results: HashMap<TaskId, CpmResult> = HashMap::new();
        
        // ═══════════════════════════════════════════════════════════════
        // FORWARD PASS: Compute Early Start (ES) and Early Finish (EF)
        // ═══════════════════════════════════════════════════════════════
        //
        // For each task in topological order:
        //   ES = max(EF of all predecessors), or project_start if no predecessors
        //   EF = ES + duration
        
        for task_id in &graph.topo_order {
            let task = graph.get_task(task_id);
            
            let es = if let Some(preds) = graph.predecessors.get(task_id) {
                if preds.is_empty() {
                    project_start
                } else {
                    preds.iter()
                        .map(|p| results[p].ef)
                        .max()
                        .unwrap()
                }
            } else {
                project_start
            };
            
            let ef = self.calendar.add_working_days(es, task.duration);
            
            results.insert(task_id.clone(), CpmResult {
                task_id: task_id.clone(),
                es,
                ef,
                ls: NaiveDate::MAX,  // Placeholder
                lf: NaiveDate::MAX,  // Placeholder
                total_slack: 0,
                free_slack: 0,
                is_critical: false,
            });
        }
        
        // ═══════════════════════════════════════════════════════════════
        // BACKWARD PASS: Compute Late Start (LS) and Late Finish (LF)
        // ═══════════════════════════════════════════════════════════════
        //
        // For each task in REVERSE topological order:
        //   LF = min(LS of all successors), or project_end if no successors
        //   LS = LF - duration
        
        let project_end = results.values()
            .map(|r| r.ef)
            .max()
            .unwrap_or(project_start);
        
        for task_id in graph.topo_order.iter().rev() {
            let task = graph.get_task(task_id);
            let result = results.get_mut(task_id).unwrap();
            
            let lf = if let Some(succs) = graph.successors.get(task_id) {
                if succs.is_empty() {
                    project_end
                } else {
                    succs.iter()
                        .map(|s| results[s].ls)
                        .min()
                        .unwrap()
                }
            } else {
                project_end
            };
            
            let ls = self.calendar.subtract_working_days(lf, task.duration);
            
            result.lf = lf;
            result.ls = ls;
        }
        
        // ═══════════════════════════════════════════════════════════════
        // SLACK CALCULATION
        // ═══════════════════════════════════════════════════════════════
        //
        // Total Slack = LS - ES = LF - EF (must be >= 0)
        // Free Slack  = min(ES of successors) - EF
        // Critical    = Total Slack == 0
        
        for task_id in &graph.topo_order {
            let result = results.get_mut(task_id).unwrap();
            
            // Total slack
            let total_slack = self.calendar.working_days_between(result.es, result.ls);
            
            // INVARIANT: Slack must be non-negative
            assert!(
                total_slack >= 0,
                "CPM invariant violated: negative slack for task {}. \
                 ES={}, LS={}, slack={}. This indicates a bug in the algorithm.",
                task_id, result.es, result.ls, total_slack
            );
            
            result.total_slack = total_slack;
            result.is_critical = total_slack == 0;
            
            // Free slack
            let min_succ_es = graph.successors.get(task_id)
                .map(|succs| succs.iter().map(|s| results[s].es).min())
                .flatten();
            
            result.free_slack = match min_succ_es {
                Some(es) => self.calendar.working_days_between(result.ef, es),
                None => result.total_slack,  // Terminal task
            };
        }
        
        // ═══════════════════════════════════════════════════════════════
        // EXTRACT CRITICAL PATH
        // ═══════════════════════════════════════════════════════════════
        
        let critical_path: Vec<TaskId> = graph.topo_order.iter()
            .filter(|id| results[*id].is_critical)
            .cloned()
            .collect();
        
        Ok(CpmSchedule {
            results,
            critical_path,
            project_start,
            project_end,
        })
    }
}

#[derive(Debug)]
pub struct CpmSchedule {
    pub results: HashMap<TaskId, CpmResult>,
    pub critical_path: Vec<TaskId>,
    pub project_start: NaiveDate,
    pub project_end: NaiveDate,
}
```

### Container Date Derivation

```rust
//! utf8proj-solver/src/derive.rs
//!
//! Derive container (summary) task dates from scheduled leaf tasks

impl CpmSchedule {
    /// Derive container dates from leaf task schedule
    /// 
    /// Container.start = min(children.start)
    /// Container.end   = max(children.end)
    pub fn derive_container_dates(&self, wbs: &[Task]) -> HashMap<TaskId, DerivedDates> {
        let mut container_dates: HashMap<TaskId, DerivedDates> = HashMap::new();
        
        fn derive_recursive(
            task: &Task,
            leaf_results: &HashMap<TaskId, CpmResult>,
            container_dates: &mut HashMap<TaskId, DerivedDates>,
        ) -> DerivedDates {
            if task.children.is_empty() {
                // Leaf task: use CPM results directly
                let result = &leaf_results[&task.id];
                DerivedDates {
                    start: result.es,
                    end: result.ef,
                    effort: task.effort,
                    is_critical: result.is_critical,
                }
            } else {
                // Container: derive from children
                let child_dates: Vec<_> = task.children.iter()
                    .map(|c| derive_recursive(c, leaf_results, container_dates))
                    .collect();
                
                let derived = DerivedDates {
                    start: child_dates.iter().map(|d| d.start).min().unwrap(),
                    end: child_dates.iter().map(|d| d.end).max().unwrap(),
                    effort: child_dates.iter().filter_map(|d| d.effort).sum(),
                    is_critical: child_dates.iter().any(|d| d.is_critical),
                };
                
                container_dates.insert(task.id.clone(), derived.clone());
                derived
            }
        }
        
        for task in wbs {
            derive_recursive(task, &self.results, &mut container_dates);
        }
        
        container_dates
    }
}
```

---

## CPM Correctness Test Suite

A first-class deliverable: tests that validate CPM invariants.

```rust
//! tests/cpm_correctness.rs
//!
//! CPM correctness test suite
//! These tests validate fundamental CPM invariants that must hold
//! for ANY valid implementation.

use utf8proj_solver::{CpmScheduler, SchedulingGraph};

/// INVARIANT 1: Slack is always non-negative
#[test]
fn slack_is_never_negative() {
    let projects = load_test_fixtures();
    for project in projects {
        let graph = SchedulingGraph::from_wbs(&project).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph, project.start).unwrap();
        
        for (task_id, result) in &schedule.results {
            assert!(
                result.total_slack >= 0,
                "Task {} has negative slack: {}",
                task_id, result.total_slack
            );
        }
    }
}

/// INVARIANT 2: ES = max(EF of predecessors)
#[test]
fn early_start_respects_predecessors() {
    let projects = load_test_fixtures();
    for project in projects {
        let graph = SchedulingGraph::from_wbs(&project).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph, project.start).unwrap();
        
        for (task_id, result) in &schedule.results {
            if let Some(preds) = graph.predecessors.get(task_id) {
                let max_pred_ef = preds.iter()
                    .map(|p| schedule.results[p].ef)
                    .max();
                
                if let Some(ef) = max_pred_ef {
                    assert!(
                        result.es >= ef,
                        "Task {} starts ({}) before predecessor finishes ({})",
                        task_id, result.es, ef
                    );
                }
            }
        }
    }
}

/// INVARIANT 3: LF = min(LS of successors)
#[test]
fn late_finish_respects_successors() {
    let projects = load_test_fixtures();
    for project in projects {
        let graph = SchedulingGraph::from_wbs(&project).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph, project.start).unwrap();
        
        for (task_id, result) in &schedule.results {
            if let Some(succs) = graph.successors.get(task_id) {
                let min_succ_ls = succs.iter()
                    .map(|s| schedule.results[s].ls)
                    .min();
                
                if let Some(ls) = min_succ_ls {
                    assert!(
                        result.lf <= ls,
                        "Task {} late finish ({}) exceeds successor late start ({})",
                        task_id, result.lf, ls
                    );
                }
            }
        }
    }
}

/// INVARIANT 4: Critical path has zero slack
#[test]
fn critical_path_has_zero_slack() {
    let projects = load_test_fixtures();
    for project in projects {
        let graph = SchedulingGraph::from_wbs(&project).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph, project.start).unwrap();
        
        for task_id in &schedule.critical_path {
            let result = &schedule.results[task_id];
            assert_eq!(
                result.total_slack, 0,
                "Task {} is on critical path but has slack: {}",
                task_id, result.total_slack
            );
        }
    }
}

/// INVARIANT 5: Cross-container dependencies are honored
#[test]
fn cross_container_dependencies_work() {
    // This is the specific bug that was identified
    let project = parse(r#"
        project "Test" { start: 2026-01-01 }
        
        task phase1 "Phase 1" {
            task a "Task A" { duration: 5d }
        }
        
        task phase2 "Phase 2" {
            task b "Task B" { 
                duration: 3d
                depends: phase1.a  # Cross-container!
            }
        }
    "#).unwrap();
    
    let graph = SchedulingGraph::from_wbs(&project).unwrap();
    let schedule = CpmScheduler::new().schedule(&graph, project.start).unwrap();
    
    let a = &schedule.results["a"];
    let b = &schedule.results["b"];
    
    assert!(
        b.es >= a.ef,
        "Cross-container dependency violated: B starts ({}) before A finishes ({})",
        b.es, a.ef
    );
}

/// INVARIANT 6: Container dates derive from children
#[test]
fn container_dates_derive_from_children() {
    let project = parse(r#"
        project "Test" { start: 2026-01-01 }
        
        task parent "Parent" {
            task child1 "Child 1" { duration: 5d }
            task child2 "Child 2" { 
                duration: 3d
                depends: child1
            }
        }
    "#).unwrap();
    
    let graph = SchedulingGraph::from_wbs(&project).unwrap();
    let schedule = CpmScheduler::new().schedule(&graph, project.start).unwrap();
    let container_dates = schedule.derive_container_dates(&project.tasks);
    
    let parent = &container_dates["parent"];
    let child1 = &schedule.results["child1"];
    let child2 = &schedule.results["child2"];
    
    assert_eq!(parent.start, child1.es, "Parent start should equal first child start");
    assert_eq!(parent.end, child2.ef, "Parent end should equal last child end");
}

/// REGRESSION: TaskJuggler-style example from tutorial
#[test]
fn crm_migration_schedules_correctly() {
    let project = include_str!("fixtures/crm_migration.proj");
    let parsed = parse(project).unwrap();
    let graph = SchedulingGraph::from_wbs(&parsed).unwrap();
    let schedule = CpmScheduler::new().schedule(&graph, parsed.start).unwrap();
    
    // Verify cross-phase dependencies
    let planning_complete = &schedule.results["plan_approved"];
    let data_mapping = &schedule.results["data_mapping"];
    let api_design = &schedule.results["api_design"];
    
    // Both data_mapping and api_design depend on plan_approved
    assert!(data_mapping.es >= planning_complete.ef);
    assert!(api_design.es >= planning_complete.ef);
    
    // go_live depends on BOTH data_complete AND integration_complete
    let go_live = &schedule.results["go_live"];
    let data_complete = &schedule.results["data_complete"];
    let integration_complete = &schedule.results["integration_complete"];
    
    assert!(go_live.es >= data_complete.ef);
    assert!(go_live.es >= integration_complete.ef);
}
```

---

## Strategic Repositioning

### OLD Positioning (RFC 001)

> "utf8proj is **not** a TaskJuggler clone. It's a **project scheduling engine**..."

This was too vague and invited unfair comparisons.

### NEW Positioning

> **utf8proj: A modern CPM/PERT engine for developers and automation pipelines, with first-class Excel, JSON, and diagram output.**

### Marketing Message

| Strength | Message |
|----------|---------|
| **Developer-first** | Single binary, zero runtime dependencies, embeddable library |
| **Modern outputs** | JSON API, Excel with formulas, Mermaid/PlantUML diagrams |
| **Automation-ready** | CI/CD integration, WASM support, structured data |
| **Migration path** | TJP import for existing TaskJuggler users |
| **Textbook CPM** | Correct algorithm, invariant-tested, predictable behavior |

### What We're NOT Competing On

| Feature | TaskJuggler | utf8proj | Strategy |
|---------|-------------|----------|----------|
| HTML reports | ✅ Excellent | ⚠️ Basic | Don't compete—use JSON/Excel |
| Macro system | ✅ Powerful | ❌ None | Not needed for target audience |
| 20 years maturity | ✅ | ❌ New | Accept it, move fast |
| Resource leveling | ✅ Full | ⚠️ Detection only | Phase 2 feature |

### What We're Winning On

| Feature | TaskJuggler | utf8proj | Advantage |
|---------|-------------|----------|-----------|
| Installation | Ruby ecosystem | Single binary | **10x easier** |
| JSON output | ❌ None | ✅ First-class | **Required for modern tools** |
| Excel output | ❌ None | ✅ With formulas | **What PMs actually use** |
| Diagram export | ❌ None | ✅ Mermaid/PlantUML | **Documentation integration** |
| WASM | ❌ Impossible | ✅ Planned | **Browser/serverless** |
| Embedding | ❌ Ruby-only | ✅ Rust library | **Composable** |

---

## Revised Roadmap

### Phase 1: CPM Correctness (Weeks 1-2) 🔴 CRITICAL

| Task | Description | Tests |
|------|-------------|-------|
| `dag.rs` | Flatten WBS → DAG | Cycle detection, dependency resolution |
| `cpm.rs` | Textbook forward/backward pass | All 6 invariant tests |
| `derive.rs` | Container date derivation | Parent/child consistency |
| CI integration | Invariant tests in CI | Block merges on failure |

### Phase 2: Resource Awareness (Weeks 3-4) 🟠 IMPORTANT

| Task | Description |
|------|-------------|
| Over-allocation detection | Warn when resource capacity exceeded |
| Utilization reporting | Show resource load % over time |
| Simple leveling (optional) | Serial heuristic for flattening peaks |

### Phase 3: Output Excellence (Weeks 5-6) 🟢 DIFFERENTIATOR

| Task | Description |
|------|-------------|
| Excel export | Multi-sheet workbook with formulas |
| JSON API | Full schedule as structured JSON |
| Mermaid export | Gantt diagrams for docs |
| PlantUML export | Alternative diagram format |

### Phase 4: Integration (Weeks 7-8)

| Task | Description |
|------|-------------|
| TJP import | Migration path from TaskJuggler |
| MSPDI export | MS Project interoperability |
| utf8dok blocks | Document integration |
| WASM build | Browser execution |

---

## Appendix: Reference CPM Algorithm

For implementers, here is the canonical CPM procedure:

```
CRITICAL PATH METHOD (CPM)
══════════════════════════

INPUT:
  - Set of activities A = {a₁, a₂, ..., aₙ}
  - Duration d(aᵢ) for each activity
  - Precedence relations P ⊆ A × A
  - Project start date S

OUTPUT:
  - ES(aᵢ), EF(aᵢ): Early Start, Early Finish for each activity
  - LS(aᵢ), LF(aᵢ): Late Start, Late Finish for each activity
  - Slack(aᵢ): Total float for each activity
  - Critical path: sequence of activities with Slack = 0

ALGORITHM:

1. CONSTRUCT GRAPH
   - Create node for each activity
   - Create edge (aᵢ → aⱼ) for each (aᵢ, aⱼ) ∈ P
   - Verify graph is acyclic (DAG)

2. TOPOLOGICAL SORT
   - Order activities such that all predecessors come before successors
   - Let ordering be [a₁, a₂, ..., aₙ]

3. FORWARD PASS (in topological order)
   FOR each aᵢ in [a₁, a₂, ..., aₙ]:
     IF aᵢ has no predecessors:
       ES(aᵢ) = S
     ELSE:
       ES(aᵢ) = max { EF(aⱼ) : aⱼ is predecessor of aᵢ }
     END
     EF(aᵢ) = ES(aᵢ) + d(aᵢ)
   END

4. DETERMINE PROJECT END
   T = max { EF(aᵢ) : aᵢ ∈ A }

5. BACKWARD PASS (in reverse topological order)
   FOR each aᵢ in [aₙ, aₙ₋₁, ..., a₁]:
     IF aᵢ has no successors:
       LF(aᵢ) = T
     ELSE:
       LF(aᵢ) = min { LS(aⱼ) : aⱼ is successor of aᵢ }
     END
     LS(aᵢ) = LF(aᵢ) - d(aᵢ)
   END

6. COMPUTE SLACK
   FOR each aᵢ in A:
     Slack(aᵢ) = LS(aᵢ) - ES(aᵢ)   // Equivalently: LF(aᵢ) - EF(aᵢ)
     ASSERT Slack(aᵢ) ≥ 0          // INVARIANT!
   END

7. IDENTIFY CRITICAL PATH
   Critical = { aᵢ : Slack(aᵢ) = 0 }
   // The critical path is the longest path through the network

PROPERTIES (invariants):
  - Slack ≥ 0 for all activities (by construction)
  - ES ≤ LS and EF ≤ LF for all activities
  - Activities on critical path determine project duration
  - Any delay on critical path delays project end
```

---

## References

1. Kelley, J.E. & Walker, M.R. (1959). "Critical-Path Planning and Scheduling"
2. PMI. *PMBOK Guide*, 7th Edition, Chapter 6: Schedule Management
3. Moder, J.J. & Phillips, C.R. (1970). *Project Management with CPM and PERT*
4. Wikipedia: [Critical Path Method](https://en.wikipedia.org/wiki/Critical_path_method)

---

## Summary

This RFC proposes:

1. **Architectural fix**: Separate WBS (presentation) from DAG (scheduling)
2. **Algorithm fix**: Implement textbook CPM with proper forward/backward passes
3. **Quality gate**: CPM correctness test suite as CI requirement
4. **Strategic pivot**: Position as "modern CPM engine" not "TaskJuggler alternative"
5. **Differentiation focus**: Excel, JSON, diagrams, WASM—areas TJ can't compete

The result: a **trustworthy**, **embeddable**, **automation-friendly** scheduling engine.
