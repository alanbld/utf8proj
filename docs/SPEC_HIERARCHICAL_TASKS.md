# utf8proj: Hierarchical Task Support Specification

## Overview

This document specifies the enhancements needed for utf8proj to support
hierarchical (nested) task structures and advanced dependency types,
enabling full compatibility with MS Project and TaskJuggler projects.

## Current State

| Feature | Status | Test File |
|---------|--------|-----------|
| Flat tasks with constraints | ✓ | ttg_01_base.tjp |
| FS dependencies with lag | ✓ | ttg_02_deps.tjp |
| Nested task containers | ✗ | ttg_03_hierarchy.tjp |
| Milestones (nested) | ✗ | ttg_04_milestones.tjp |
| Deep nesting (3+ levels) | ✗ | ttg_05_detailed.tjp |
| SS/SF/FF dependencies | ✗ | - |
| Negative lag (lead) | ✗ | - |

## Domain Model

### 1. Task Hierarchy

```
Project
├── Task (leaf or container)
│   ├── Task (leaf)
│   ├── Task (container)
│   │   └── Task (leaf)
│   └── Milestone
└── Task
```

**Abstraction**: A `Task` is either:
- **Leaf Task**: Has duration/effort, can be scheduled
- **Container Task**: Groups subtasks, derives dates from children
- **Milestone**: Zero duration marker, derives date from dependencies

### 2. Dependency Model

MS Project dependency types (mapped to TJ3 syntax):

| Type | MS Project | TJ3 Syntax | Semantics |
|------|------------|------------|-----------|
| FS | `5` or `5FS` | `depends task_id` | Finish-to-Start (default) |
| SS | `5SS` | `depends !task_id` | Start-to-Start |
| FF | `5FF` | `depends task_id~` | Finish-to-Finish |
| SF | `5SF` | `depends !task_id~` | Start-to-Finish |

With lag/lead:
| MS Project | TJ3 Syntax | Semantics |
|------------|------------|-----------|
| `5+5d` | `depends task_id { gaplength 5d }` | FS + 5 day lag |
| `5SS+3d` | `depends !task_id { gaplength 3d }` | SS + 3 day lag |
| `5-2d` | `depends task_id { gaplength -2d }` | FS - 2 day lead |

### 3. Path Resolution

Task references in dependencies can be:
- **Absolute**: `phase1.act1_1` (from project root)
- **Relative**: `act1_1` (within same container)
- **Parent-relative**: `..act1_1` (sibling of parent)

## Feature Specifications

### Feature 1: Nested Task Parsing

**Grammar Change**:
```pest
task_body = { (task_attr | task_decl)* }
```

**Parser Change**:
- `Task` struct gains `subtasks: Vec<Task>` field
- Path resolution for nested identifiers

**TDD Test Cases**:
```rust
#[test]
fn parse_nested_task() {
    let input = r#"
        task phase1 "Phase 1" {
            task act1 "Activity 1" { length 10d }
        }
    "#;
    let project = parse_tjp(input).unwrap();
    assert_eq!(project.tasks[0].subtasks.len(), 1);
}

#[test]
fn parse_3_level_nesting() {
    let input = r#"
        task phase1 "Phase 1" {
            task act1 "Activity 1" {
                task sub1 "Sub 1" { length 5d }
            }
        }
    "#;
    let project = parse_tjp(input).unwrap();
    assert_eq!(project.tasks[0].subtasks[0].subtasks.len(), 1);
}
```

### Feature 2: Container Task Scheduling

**Invariant**: Container task dates derive from children
- `container.start = min(child.start for child in children)`
- `container.finish = max(child.finish for child in children)`

**TDD Test Cases**:
```rust
#[test]
fn container_derives_dates_from_children() {
    let input = r#"
        project test "Test" 2025-01-01 - 2025-12-31 {}
        task phase1 "Phase 1" {
            task act1 "Act 1" { start 2025-02-01 length 10d }
            task act2 "Act 2" { start 2025-02-15 length 5d }
        }
    "#;
    let scheduled = schedule(parse_tjp(input).unwrap());
    let phase1 = &scheduled.tasks[0];

    assert_eq!(phase1.start, date(2025, 2, 1));  // min of children
    assert_eq!(phase1.finish, date(2025, 2, 21)); // max of children
}
```

### Feature 3: Dependency Path Resolution

**Algorithm**:
1. Split path by `.`
2. If starts with known root task, resolve from root
3. Otherwise, resolve from current container
4. Support `..` for parent traversal

**TDD Test Cases**:
```rust
#[test]
fn resolve_absolute_path() {
    let input = r#"
        task phase1 "Phase 1" {
            task act1 "Act 1" { length 10d }
        }
        task phase2 "Phase 2" {
            depends phase1.act1
            task act2 "Act 2" { length 5d }
        }
    "#;
    let project = parse_tjp(input).unwrap();
    let dep = &project.tasks[1].dependencies[0];
    assert_eq!(dep.target_path, vec!["phase1", "act1"]);
}

#[test]
fn resolve_sibling_path() {
    let input = r#"
        task phase1 "Phase 1" {
            task act1 "Act 1" { length 10d }
            task act2 "Act 2" {
                depends act1  // sibling reference
                length 5d
            }
        }
    "#;
    let scheduled = schedule(parse_tjp(input).unwrap());
    // act2 should start after act1 finishes
}
```

### Feature 4: SS/FF/SF Dependencies

**Grammar Addition**:
```pest
depends_attr = {
    "depends" ~ dependency_list
}

dependency_list = { dependency ~ ("," ~ dependency)* }

dependency = {
    dep_onstart? ~ task_path ~ dep_onfinish? ~ dep_modifier?
}

dep_onstart = { "!" }   // SS or SF
dep_onfinish = { "~" }  // FF or SF
dep_modifier = { "{" ~ gaplength_spec ~ "}" }
```

**Dependency Resolution Matrix**:
| OnStart | OnFinish | Type | Constraint |
|---------|----------|------|------------|
| false | false | FS | successor.start >= predecessor.finish + lag |
| true | false | SS | successor.start >= predecessor.start + lag |
| false | true | FF | successor.finish >= predecessor.finish + lag |
| true | true | SF | successor.finish >= predecessor.start + lag |

**TDD Test Cases**:
```rust
#[test]
fn ss_dependency_scheduling() {
    let input = r#"
        task act1 "Act 1" { start 2025-02-03 length 20d }
        task act2 "Act 2" { depends !act1 { gaplength 5d } length 10d }
    "#;
    let scheduled = schedule(parse_tjp(input).unwrap());

    // act2 starts 5 days after act1 starts (SS+5d)
    assert_eq!(scheduled.tasks[1].start, date(2025, 2, 10));
}

#[test]
fn ff_dependency_scheduling() {
    let input = r#"
        task act1 "Act 1" { start 2025-02-03 length 20d }
        task act2 "Act 2" { depends act1~ length 10d }
    "#;
    let scheduled = schedule(parse_tjp(input).unwrap());

    // act2 finishes when act1 finishes (FF)
    assert_eq!(scheduled.tasks[1].finish, scheduled.tasks[0].finish);
}
```

### Feature 5: Negative Lag (Lead)

**Constraint**: Lead allows successor to start before predecessor completes.

**TDD Test Cases**:
```rust
#[test]
fn negative_lag_allows_overlap() {
    let input = r#"
        task act1 "Act 1" { start 2025-02-03 length 20d }
        task act2 "Act 2" { depends act1 { gaplength -5d } length 10d }
    "#;
    let scheduled = schedule(parse_tjp(input).unwrap());

    // act2 starts 5 days BEFORE act1 finishes
    let act1_finish = scheduled.tasks[0].finish;
    let act2_start = scheduled.tasks[1].start;
    assert!(act2_start < act1_finish);
}
```

## Implementation Roadmap

### Phase 1: Parser Enhancement (Nested Tasks)
1. Update grammar: `task_body = { (task_attr | task_decl)* }`
2. Add `subtasks` field to `Task` struct
3. Implement recursive task parsing
4. **Validation**: ttg_03_hierarchy.tjp parses

### Phase 2: Flattening Strategy
1. Flatten nested tasks for scheduling (preserve hierarchy for output)
2. Compute fully-qualified task IDs
3. Resolve dependency paths to flat task references
4. **Validation**: ttg_03_hierarchy.tjp schedules correctly

### Phase 3: Container Task Dates
1. Post-scheduling: derive container dates from children
2. Handle empty containers (error or warning)
3. **Validation**: ttg_04_milestones.tjp renders correctly

### Phase 4: Advanced Dependencies
1. Add SS/FF/SF parsing (`!` and `~` markers)
2. Extend constraint generation for each type
3. Implement negative lag handling
4. **Validation**: Create ttg_06_advanced_deps.tjp

### Phase 5: Full move2cloud Support
1. Handle 229 tasks with deep nesting
2. Optimize path resolution
3. Handle circular dependency detection
4. **Validation**: move2cloud.tjp schedules ≈ TJ3

## Test File Progression

| Level | File | Tasks | New Feature |
|-------|------|-------|-------------|
| 1 | ttg_01_base.tjp | 6 | Flat tasks, constraints |
| 2 | ttg_02_deps.tjp | 6 | FS dependencies, lag |
| 3 | ttg_03_hierarchy.tjp | 7 | Nested containers |
| 4 | ttg_04_milestones.tjp | 13 | Milestones in hierarchy |
| 5 | ttg_05_detailed.tjp | 25 | 3-level nesting |
| 6 | ttg_06_advanced_deps.tjp | ~30 | SS/FF/SF, negative lag |
| 7 | move2cloud.tjp | 229 | Full complexity |

## Acceptance Criteria

For each test file level:
1. **Parse**: No syntax errors
2. **Schedule**: CPM produces valid dates
3. **Compare**: Dates match TJ3 output within 1 day tolerance
4. **Render**: Gantt chart visually correct
