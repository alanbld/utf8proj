# RFC 003: Container Dependency Semantics

**Status:** Accepted  
**Created:** 2026-01-06  
**Author:** Alan (utf8dok team)  
**Related:** RFC 001 (Architecture), DESIGN_PHILOSOPHY.md

---

## Summary

This RFC formally defines utf8proj's semantic handling of container (summary task) dependencies and documents the **intentional divergence** from MS Project's implicit inheritance model.

---

## Problem Statement

During MS Project import implementation, we discovered a fundamental semantic mismatch:

**MS Project behavior:**
```xml
<Task>
  <n>Development</n>
  <PredecessorLink><PredecessorUID>10</PredecessorUID></PredecessorLink>
  <Task>
    <n>Feature X</n>
    <!-- No predecessor listed -->
  </Task>
</Task>
```

**Result:** Feature X implicitly inherits the container's dependency. It **cannot start** until Task 10 completes, even though this constraint is not visible in Feature X's definition.

**This violates utf8proj's core principle:** Scheduling logic must be explicit and visible in the task definition.

---

## Design Decision

### Core Principle

**Containers (summary tasks) are for organization, not scheduling.**

**Consequence:** Container dependencies do **NOT** flow down to children. Each task's scheduling constraints must be explicitly declared.

### Rationale

1. **Visibility:** Dependencies appear in task definition
2. **Git-diffability:** Schedule changes show in diffs
3. **Refactorability:** Reorganizing WBS doesn't break schedule
4. **Explicitness:** No hidden constraints
5. **CPM correctness:** Dependency graph is the source of truth

---

## Specification

### Container Dependency Semantics

```rust
/// Container dependencies are METADATA ONLY
/// They do NOT create scheduling constraints on children

struct Task {
    id: TaskId,
    name: String,
    depends: Vec<TaskId>,  // For containers: informational only
    children: Vec<Task>,
}

impl Scheduler {
    fn build_dependency_graph(&self, project: &Project) -> DependencyGraph {
        let mut graph = DependencyGraph::new();
        
        for task in project.all_tasks() {
            // Add task to graph
            graph.add_node(task.id);
            
            // Add explicit dependencies
            for predecessor in &task.depends {
                graph.add_edge(*predecessor, task.id);
            }
            
            // CRITICAL: Do NOT add edges from container dependencies
            // Container deps are metadata, not scheduling constraints
        }
        
        graph
    }
}
```

### Example

```proj
task development {
  # This dependency is METADATA
  # Documents that development phase follows design
  # Does NOT block children
  depends: design_approval
  
  task feature_x {
    # To block this task, add explicit dependency
    depends: design_approval
    duration: 5d
  }
  
  task feature_y {
    # Different dependency - follows feature_x
    depends: feature_x
    duration: 3d
  }
}
```

**Scheduling result:**
- `feature_x` blocked by `design_approval` (explicit dep)
- `feature_y` blocked by `feature_x` (explicit dep)
- Container `development` reports:
  - Start: min(children.start) = feature_x.start
  - Finish: max(children.finish) = feature_y.finish

---

## MS Project Compatibility

### Import Behavior

When importing MS Project files with container dependencies:

**Default (strict mode):**
```bash
utf8proj import project.mpp -o project.proj

⚠ W014: Container dependency without child dependencies (3 instances)
  Use --explain for details
  Use --fix-container-deps to auto-fix
```

**With auto-fix:**
```bash
utf8proj import project.mpp --fix-container-deps -o project.proj

✓ Auto-added 3 dependencies to match MS Project semantics
```

**Resulting .proj file:**
```proj
task development {
  depends: design_approval  # Original container dependency
  
  task feature_x {
    # AUTO-ADDED to match MS Project implicit inheritance
    depends: design_approval
    duration: 5d
  }
}
```

### Diagnostic: W014

**Code:** W014  
**Severity:** Warning  
**Category:** MS Project Compatibility

**Trigger:** Container has dependencies but one or more children do not depend on any of the container's predecessors.

**Message:**
```
W014: Container dependency without child dependencies

  Container 'Development Phase' depends on 'Design Approval'
  but child 'Feature X' has no dependencies.
  
  MS Project: Feature X implicitly blocked until Design Approval
  utf8proj:   Feature X can start immediately (no explicit dependency)
  
  To match MS Project behavior:
    task feature_x {
      depends: design_approval  # Add explicit dependency
      duration: 5d
    }
  
  Or use: utf8proj import --fix-container-deps project.mpp
```

---

## Implementation

### Parser

```rust
// In utf8proj-parser/src/lib.rs

pub struct TaskDefinition {
    pub id: TaskId,
    pub depends: Vec<TaskId>,  // Parsed from `depends:` clause
    pub children: Vec<TaskDefinition>,
}

// Container dependencies are stored but NOT used for scheduling
// They appear in explain() output and diagnostics
```

### Scheduler

```rust
// In utf8proj-solver/src/cpm.rs

impl CpmScheduler {
    fn build_dependency_graph(&self, project: &Project) -> DependencyGraph {
        let mut graph = DependencyGraph::new();
        
        // Flatten task tree - containers don't affect graph structure
        for task in project.all_leaf_tasks() {
            graph.add_node(task.id);
            
            // Only explicit dependencies create edges
            for pred in &task.depends {
                graph.add_edge(*pred, task.id);
            }
        }
        
        // Container tasks are NOT in the dependency graph
        // Their dates are computed from children after scheduling
        
        graph
    }
    
    fn compute_container_dates(&self, container: &Task, schedule: &Schedule) -> (NaiveDate, NaiveDate) {
        let child_tasks: Vec<_> = container.children.iter()
            .flat_map(|child| schedule.tasks.get(&child.id))
            .collect();
        
        let start = child_tasks.iter()
            .map(|t| t.start)
            .min()
            .unwrap_or(container.computed_start);
        
        let finish = child_tasks.iter()
            .map(|t| t.finish)
            .max()
            .unwrap_or(container.computed_finish);
        
        (start, finish)
    }
}
```

### Validator

```rust
// In utf8proj-validator/src/diagnostics.rs

pub fn check_container_dependencies(project: &Project) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    
    for container in project.containers() {
        if container.depends.is_empty() {
            continue;  // No container dependency
        }
        
        for child in &container.children {
            // Check if child depends on ANY of container's predecessors
            let has_transitive_dep = child.depends.iter()
                .any(|dep| container.depends.contains(dep));
            
            if !has_transitive_dep {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::W014,
                    severity: Severity::Warning,
                    message: format!(
                        "Container '{}' depends on {:?} but child '{}' has no dependencies",
                        container.name, container.depends, child.name
                    ),
                    location: child.location.clone(),
                    suggestion: Some(format!(
                        "Add: depends: {}",
                        container.depends.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", ")
                    )),
                });
            }
        }
    }
    
    diagnostics
}
```

---

## Testing

### Test Cases

```rust
// tests/container_dependencies.rs

#[test]
fn container_dependency_does_not_block_children() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task design { duration: 5d }
        
        task development {
            depends: design  # Container dependency
            
            task feature_x {
                # No explicit dependency
                duration: 3d
            }
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    // feature_x should start immediately (2025-01-01)
    // NOT blocked by design (2025-01-06)
    assert_eq!(schedule.tasks["feature_x"].start, date(2025, 1, 1));
}

#[test]
fn explicit_dependency_blocks_child() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task design { duration: 5d }
        
        task development {
            depends: design
            
            task feature_x {
                depends: design  # Explicit dependency
                duration: 3d
            }
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    // feature_x blocked by design
    assert_eq!(schedule.tasks["feature_x"].start, date(2025, 1, 6));
}

#[test]
fn diagnostic_w014_triggers_on_missing_child_dep() {
    let project = parse(r#"
        task development {
            depends: design
            task feature_x { duration: 3d }
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    
    assert!(diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
}

#[test]
fn container_dates_computed_from_children() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task development {
            task feature_x { duration: 5d }
            task feature_y { duration: 3d, depends: feature_x }
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    let container = schedule.tasks["development"];
    assert_eq!(container.start, schedule.tasks["feature_x"].start);
    assert_eq!(container.finish, schedule.tasks["feature_y"].finish);
}
```

---

## Documentation Impact

### User-Facing Changes

1. **Design Philosophy:** Section added explaining WBS vs DAG separation
2. **MS Project Compatibility Guide:** New document explaining differences
3. **CLI Reference:** Updated `import` command with `--fix-container-deps`
4. **Diagnostic Reference:** Added W014 documentation
5. **Examples:** Added `ms-project-migration/` examples

### Internal Documentation

1. **Architecture Diagram:** Updated to show container/leaf separation
2. **Scheduling Algorithm:** Documented container date computation
3. **Test Strategy:** Added container dependency test suite

---

## Alternatives Considered

### Alternative 1: Mimic MS Project (Rejected)

**Approach:** Make container dependencies flow down to children.

**Pros:**
- Zero-friction MS Project import
- Matches user expectations from MS Project

**Cons:**
- ❌ Violates "explicit is better than implicit"
- ❌ Dependencies hidden from task definition
- ❌ Not git-diffable (reorganizing containers changes schedule silently)
- ❌ Breaks WBS flexibility

**Verdict:** Rejected. Core principle violation.

### Alternative 2: Make it Configurable (Rejected)

**Approach:** Add `container_deps_inherit: true/false` flag.

**Pros:**
- Flexibility for different projects

**Cons:**
- ❌ Two scheduling semantics to maintain
- ❌ Cognitive load on users
- ❌ Test matrix explosion
- ❌ Dilutes core principle

**Verdict:** Rejected. Clarity over flexibility.

### Alternative 3: Diagnostic + Auto-Fix (Accepted)

**Approach:** Detect divergence, warn user, offer auto-fix.

**Pros:**
- ✅ Preserves core principle (explicit dependencies)
- ✅ Educates users about difference
- ✅ Pragmatic migration path
- ✅ Creates "teaching moment"

**Cons:**
- Requires manual review (or auto-fix trust)

**Verdict:** Accepted. Best balance.

---

## Migration Path

### For New Projects

Use explicit dependencies from the start:

```proj
task development {
  task feature_x {
    depends: design_approval  # Explicit
  }
}
```

### For MS Project Imports

**Step 1:** Import with auto-fix
```bash
utf8proj import project.mpp --fix-container-deps -o project.proj
```

**Step 2:** Review generated dependencies
```bash
git diff
# See auto-added dependencies
```

**Step 3:** Refine if needed
```bash
vim project.proj
# Remove unnecessary dependencies
# Keep essential ones
```

---

## Backward Compatibility

This is a **new feature** (MS Project import), so no backward compatibility concerns.

Future versions will maintain this semantic:
- Container dependencies remain metadata only
- W014 diagnostic remains stable
- Auto-fix behavior remains consistent

---

## Success Metrics

1. **Clarity:** Users understand why utf8proj differs
2. **Migration:** MS Project imports succeed with clear diagnostics
3. **Adoption:** Users prefer explicit dependencies after understanding philosophy
4. **Correctness:** No scheduling bugs from container/child interaction

---

## References

1. **DESIGN_PHILOSOPHY.md:** Core principles
2. **MS_PROJECT_COMPATIBILITY.md:** User migration guide
3. **TaskJuggler Documentation:** Similar explicit dependency model
4. **CPM Theory:** Dependency graphs vs organizational hierarchy

---

## Appendix: Real-World Example

**MS Project file:**
```
Summary Task: Q1 Release [Depends: Design Complete]
├─ Backend API (10d)
├─ Frontend UI (8d)
└─ Integration Test (5d, depends: Backend, Frontend)
```

**Naive import (incorrect):**
```proj
task q1_release {
  depends: design_complete
  
  task backend_api { duration: 10d }
  task frontend_ui { duration: 8d }
  task integration_test { 
    depends: backend_api, frontend_ui
    duration: 5d 
  }
}

# PROBLEM: backend_api and frontend_ui can start immediately!
# Doesn't match MS Project where they wait for design_complete
```

**Auto-fixed import (correct):**
```proj
task q1_release {
  depends: design_complete  # Original container dependency
  
  task backend_api { 
    depends: design_complete  # Auto-added
    duration: 10d 
  }
  task frontend_ui { 
    depends: design_complete  # Auto-added
    duration: 8d 
  }
  task integration_test { 
    depends: backend_api, frontend_ui
    duration: 5d 
  }
}

# NOW: All tasks properly blocked by design_complete
```

---

**Status:** Accepted  
**Implementation:** Required for v1.0 MS Project import  
**Breaking Change:** No (new feature)
