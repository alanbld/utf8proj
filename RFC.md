# RFC: utf8proj - Project Scheduling Engine for Rust

**RFC Number:** 001  
**Title:** utf8proj - A Modern Project Scheduling Engine  
**Author:** Alan (utf8dok team)  
**Status:** Draft  
**Created:** 2026-01-01  
**Target:** Separate repository (`utf8dok/utf8proj`)  

---

## Summary

This RFC proposes **utf8proj**, a standalone Rust library and CLI for project scheduling, designed to serve as the scheduling engine for utf8dok and as a general-purpose project management tool. utf8proj aims to fill the gap left by TaskJuggler (Ruby) by providing a modern, single-binary, zero-dependency solution with advanced constraint-solving capabilities.

---

## Motivation

### The Problem

1. **TaskJuggler requires Ruby** - Enterprise environments often resist Ruby installations
2. **No Rust alternative exists** - `cpm-rs` (703 LOC) provides only basic CPM, no resource leveling or DSL
3. **Document-embedded scheduling** - No tool integrates scheduling directly into document generation pipelines
4. **Modern constraint solving** - Existing tools use heuristics only; BDD/SAT techniques are unexplored in this domain

### The Opportunity

| User Need | Current Solution | utf8proj Solution |
|-----------|------------------|-------------------|
| Text-based project definition | TaskJuggler (Ruby) | Native Rust DSL |
| Resource leveling | MS Project ($$$) or TJ | Heuristic + BDD/SAT |
| Document integration | Manual screenshots | Native utf8dok blocks |
| MS Project interop | Limited | MSPDI import/export |
| CI/CD integration | Complex | Single binary, zero deps |

### Strategic Positioning

utf8proj is **not** a TaskJuggler clone. It's a **project scheduling engine** that:

- Provides a library (`utf8proj-core`) for embedding in other tools
- Offers TJP file format compatibility for migration
- Introduces formal methods (BDD/SAT) for what-if analysis
- Integrates natively with document generation (utf8dok)

---

## Prior Art & Competitive Analysis

### Existing Solutions

| Tool | Language | LOC | Strengths | Weaknesses |
|------|----------|-----|-----------|------------|
| **TaskJuggler 3.x** | Ruby | ~15-20k | Mature, full-featured | Ruby dependency, no formal methods |
| **cpm-rs** | Rust | 703 | Pure Rust, CPM | No resources, no DSL, not production-ready |
| **MS Project** | Proprietary | N/A | Industry standard | Cost, closed source |
| **ProjectLibre** | Java | ~100k | MS Project compatible | JVM, GUI-focused |
| **GanttProject** | Java | ~50k | Cross-platform | JVM, limited features |

### Gap Analysis

**No existing Rust solution provides:**
- Complete project scheduling (tasks + resources + calendars)
- Text-based DSL for version control
- Resource leveling algorithms
- MS Project interchange
- Formal constraint solving (BDD/SAT)

---

## Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           utf8proj ARCHITECTURE                              │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                         utf8proj-core                                │    │
│  │                                                                      │    │
│  │  Domain Model: Project, Task, Resource, Calendar, Constraint        │    │
│  │  Traits: Scheduler, Renderer, Exporter                              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│          ┌─────────────────────────┼─────────────────────────┐              │
│          │                         │                         │              │
│          ▼                         ▼                         ▼              │
│  ┌───────────────┐       ┌───────────────┐       ┌───────────────┐         │
│  │ utf8proj-     │       │ utf8proj-     │       │ utf8proj-     │         │
│  │ parser        │       │ solver        │       │ render        │         │
│  │               │       │               │       │               │         │
│  │ • Native DSL  │       │ • Heuristic   │       │ • Gantt SVG   │         │
│  │ • TJP compat  │       │ • BDD (OxiDD) │       │ • Tables      │         │
│  │ • pest grammar│       │ • SAT (opt)   │       │ • iCal        │         │
│  └───────────────┘       └───────────────┘       └───────────────┘         │
│          │                         │                         │              │
│          └─────────────────────────┼─────────────────────────┘              │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                         utf8proj-cli                                 │    │
│  │                                                                      │    │
│  │  Commands: schedule, render, export, import, validate, what-if      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  Optional Extensions:                                                        │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐                   │
│  │ utf8proj-lua  │  │ utf8proj-     │  │ utf8proj-     │                   │
│  │               │  │ mspdi         │  │ utf8dok       │                   │
│  │ Lua scripting │  │ MS Project    │  │ Document      │                   │
│  │ (mlua)        │  │ import/export │  │ integration   │                   │
│  └───────────────┘  └───────────────┘  └───────────────┘                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Crate Structure

```
utf8proj/
├── Cargo.toml                    # Workspace
├── LICENSE-MIT
├── LICENSE-APACHE
├── README.md
├── crates/
│   ├── utf8proj-core/            # Domain model, traits
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── project.rs        # Project struct
│   │       ├── task.rs           # Task, Dependency
│   │       ├── resource.rs       # Resource, Assignment
│   │       ├── calendar.rs       # Calendar, WorkingHours
│   │       ├── constraint.rs     # Constraint types
│   │       ├── schedule.rs       # Schedule result
│   │       └── traits.rs         # Scheduler, Renderer traits
│   │
│   ├── utf8proj-parser/          # DSL parsing
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── native/           # Native .proj DSL
│   │       │   ├── grammar.pest
│   │       │   └── parser.rs
│   │       └── tjp/              # TaskJuggler compatibility
│   │           ├── grammar.pest
│   │           └── parser.rs
│   │
│   ├── utf8proj-solver/          # Scheduling algorithms
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── heuristic.rs      # Priority-based scheduling
│   │       ├── cpm.rs            # Critical Path Method
│   │       ├── leveling.rs       # Resource leveling
│   │       ├── bdd.rs            # BDD-based (OxiDD)
│   │       └── sat.rs            # SAT-based (varisat)
│   │
│   ├── utf8proj-render/          # Output generation
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── gantt.rs          # SVG Gantt charts
│   │       ├── table.rs          # Tabular reports
│   │       ├── ical.rs           # iCal export
│   │       └── mermaid.rs        # Mermaid diagram output
│   │
│   ├── utf8proj-mspdi/           # MS Project interchange
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── import.rs         # MSPDI → utf8proj
│   │       └── export.rs         # utf8proj → MSPDI
│   │
│   ├── utf8proj-lua/             # Lua scripting (optional)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── bindings.rs       # mlua integration
│   │
│   └── utf8proj-cli/             # Command-line interface
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── schedule.rs
│           ├── render.rs
│           ├── export.rs
│           └── whatif.rs
│
├── tests/                        # Integration tests
│   ├── fixtures/                 # Test project files
│   └── integration/
│
└── examples/                     # Example projects
    ├── simple/
    ├── software-project/
    └── construction/
```

### Feature Flags

```toml
# utf8proj/Cargo.toml (meta-crate)

[features]
default = ["heuristic", "cli"]

# Core scheduling algorithms
heuristic = []                    # Priority-based, always fast
cpm = []                          # Critical Path Method
bdd = ["oxidd"]                   # BDD-based constraint reasoning
sat = ["varisat"]                 # SAT solving (pure Rust)
sat-fast = ["cadical"]            # C++ SAT solver (faster)

# Scripting
lua = ["mlua/lua54", "mlua/vendored"]

# Interchange formats
mspdi = []                        # MS Project Data Interchange
tjp = []                          # TaskJuggler compatibility

# CLI
cli = ["clap"]

# Full feature set
full = ["heuristic", "cpm", "bdd", "sat", "lua", "mspdi", "tjp", "cli"]
```

---

## Domain Model

### Core Types

```rust
// utf8proj-core/src/project.rs

use chrono::NaiveDate;
use rust_decimal::Decimal;

/// A complete project definition
#[derive(Clone, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub start: NaiveDate,
    pub end: Option<NaiveDate>,
    pub calendar: CalendarId,
    pub tasks: Vec<Task>,
    pub resources: Vec<Resource>,
    pub scenarios: Vec<Scenario>,
}

/// A schedulable unit of work
#[derive(Clone, Debug)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub effort: Option<Duration>,        // Person-time (e.g., 10 person-days)
    pub duration: Option<Duration>,      // Calendar time (e.g., 2 weeks)
    pub depends: Vec<Dependency>,
    pub assigned: Vec<ResourceRef>,
    pub priority: u32,
    pub constraints: Vec<TaskConstraint>,
    pub milestone: bool,
    pub children: Vec<Task>,             // Hierarchical tasks (WBS)
}

/// Task dependency with type and lag
#[derive(Clone, Debug)]
pub struct Dependency {
    pub predecessor: TaskId,
    pub dep_type: DependencyType,
    pub lag: Option<Duration>,
}

#[derive(Clone, Copy, Debug)]
pub enum DependencyType {
    FinishToStart,   // FS: B starts after A finishes (most common)
    StartToStart,    // SS: B starts when A starts
    FinishToFinish,  // FF: B finishes when A finishes
    StartToFinish,   // SF: B finishes when A starts (rare)
}

/// A person or equipment that can be assigned to tasks
#[derive(Clone, Debug)]
pub struct Resource {
    pub id: ResourceId,
    pub name: String,
    pub rate: Option<Money>,             // Cost per time unit
    pub capacity: f32,                   // 1.0 = full time
    pub calendar: Option<CalendarId>,
    pub efficiency: f32,                 // Skill factor (default 1.0)
}

/// Working time definitions
#[derive(Clone, Debug)]
pub struct Calendar {
    pub id: CalendarId,
    pub name: String,
    pub working_hours: WorkingHours,
    pub holidays: Vec<NaiveDate>,
    pub exceptions: Vec<CalendarException>,
}

/// Constraint that affects scheduling
#[derive(Clone, Debug)]
pub enum TaskConstraint {
    MustStartOn(NaiveDate),
    MustFinishOn(NaiveDate),
    StartNoEarlierThan(NaiveDate),
    StartNoLaterThan(NaiveDate),
    FinishNoEarlierThan(NaiveDate),
    FinishNoLaterThan(NaiveDate),
}

/// The result of scheduling
#[derive(Clone, Debug)]
pub struct Schedule {
    pub tasks: HashMap<TaskId, ScheduledTask>,
    pub critical_path: Vec<TaskId>,
    pub project_duration: Duration,
    pub total_cost: Option<Money>,
}

#[derive(Clone, Debug)]
pub struct ScheduledTask {
    pub task_id: TaskId,
    pub start: NaiveDate,
    pub finish: NaiveDate,
    pub duration: Duration,
    pub assignments: Vec<Assignment>,
    pub slack: Duration,                 // Float/slack time
    pub is_critical: bool,
}
```

### Scheduler Trait

```rust
// utf8proj-core/src/traits.rs

/// Core scheduling abstraction
pub trait Scheduler {
    /// Compute a schedule for the given project
    fn schedule(&self, project: &Project) -> Result<Schedule, ScheduleError>;
    
    /// Explain why a particular scheduling decision was made
    fn explain(&self, project: &Project, task: TaskId) -> Explanation;
    
    /// Check if a schedule is feasible without computing it
    fn is_feasible(&self, project: &Project) -> FeasibilityResult;
}

/// Result of feasibility check
pub struct FeasibilityResult {
    pub feasible: bool,
    pub conflicts: Vec<Conflict>,
    pub suggestions: Vec<Suggestion>,
}

/// What-if analysis (BDD-powered)
pub trait WhatIfAnalysis {
    /// Analyze impact of a constraint change
    fn what_if(&self, project: &Project, change: Constraint) -> WhatIfReport;
    
    /// Count valid schedules under current constraints
    fn count_solutions(&self, project: &Project) -> BigUint;
    
    /// Find all critical constraints
    fn critical_constraints(&self, project: &Project) -> Vec<Constraint>;
}
```

---

## Native DSL Design

### File Extension: `.proj`

```
# project.proj - utf8proj native format

project "API Gateway Migration" {
    start: 2025-02-01
    end: 2025-06-30
    currency: EUR
}

calendar "standard" {
    working_hours: 09:00-12:00, 13:00-18:00
    working_days: mon-fri
    
    holiday "Easter" 2025-04-18..2025-04-21
}

resource dev_team "Development Team" {
    rate: 850/day
    capacity: 3
}

resource arch "Solution Architect" {
    rate: 1200/day
    calendar: standard
}

task design "Design Phase" {
    task requirements "Requirements Analysis" {
        effort: 3d
        assign: arch
    }
    
    task architecture "Architecture Design" {
        effort: 5d
        assign: arch
        depends: requirements
    }
    
    task review "Design Review" {
        effort: 2d
        depends: architecture
        milestone: true
    }
}

task implementation "Implementation" {
    depends: design.review
    
    task sprint1 "Sprint 1 - Core" {
        effort: 10d
        assign: dev_team
    }
    
    task sprint2 "Sprint 2 - Integration" {
        effort: 10d
        assign: dev_team
        depends: sprint1
    }
}

task testing "Testing & Deployment" {
    depends: implementation
    
    task qa "QA Testing" {
        effort: 5d
        assign: dev_team
    }
    
    task deployment "Production Deployment" {
        duration: 1d
        depends: qa
        milestone: true
    }
}

# Reports
report gantt "timeline.svg" {
    tasks: all
    resources: show
    critical_path: highlight
}

report table "resources.md" {
    type: resource_allocation
    columns: name, effort, cost
}
```

### Grammar (pest)

```pest
// utf8proj-parser/src/native/grammar.pest

project_file = { SOI ~ (project_decl | calendar_decl | resource_decl | task_decl | report_decl)* ~ EOI }

project_decl = { "project" ~ string ~ "{" ~ project_body ~ "}" }
project_body = { (project_attr)* }
project_attr = { 
    "start" ~ ":" ~ date |
    "end" ~ ":" ~ date |
    "currency" ~ ":" ~ identifier
}

calendar_decl = { "calendar" ~ string ~ "{" ~ calendar_body ~ "}" }
resource_decl = { "resource" ~ identifier ~ string ~ "{" ~ resource_body ~ "}" }

task_decl = { "task" ~ identifier ~ string ~ "{" ~ task_body ~ "}" }
task_body = { (task_attr | task_decl)* }
task_attr = {
    "effort" ~ ":" ~ duration |
    "duration" ~ ":" ~ duration |
    "assign" ~ ":" ~ identifier_list |
    "depends" ~ ":" ~ dependency_list |
    "priority" ~ ":" ~ number |
    "milestone" ~ ":" ~ boolean |
    constraint_attr
}

duration = @{ number ~ duration_unit }
duration_unit = { "d" | "w" | "m" | "h" }

dependency_list = { dependency ~ ("," ~ dependency)* }
dependency = { task_ref ~ dep_modifier? }
task_ref = { identifier ~ ("." ~ identifier)* }
dep_modifier = { "+" ~ duration | "-" ~ duration }

// ... continued
```

---

## Scheduling Algorithms

### Tier 1: Heuristic (Default)

```rust
// utf8proj-solver/src/heuristic.rs

pub struct HeuristicScheduler {
    priority_rule: PriorityRule,
}

impl Scheduler for HeuristicScheduler {
    fn schedule(&self, project: &Project) -> Result<Schedule, ScheduleError> {
        // 1. Topological sort of tasks by dependencies
        let sorted = topological_sort(&project.tasks)?;
        
        // 2. Forward pass: compute earliest start/finish
        let early = forward_pass(&sorted, project);
        
        // 3. Backward pass: compute latest start/finish
        let late = backward_pass(&sorted, project, &early);
        
        // 4. Resource leveling via priority-based assignment
        let leveled = level_resources(&sorted, &early, &project.resources, self.priority_rule);
        
        // 5. Identify critical path
        let critical_path = find_critical_path(&early, &late);
        
        Ok(Schedule { tasks: leveled, critical_path, .. })
    }
}

#[derive(Clone, Copy)]
pub enum PriorityRule {
    LongestPath,           // Prioritize tasks on longest path
    MostSuccessors,        // Prioritize tasks with most dependents
    ShortestDuration,      // Prioritize shorter tasks
    HighestPriority,       // User-defined priority
    EarliestDeadline,      // Prioritize constrained tasks
}
```

### Tier 2: BDD-Based Constraint Reasoning

```rust
// utf8proj-solver/src/bdd.rs

use oxidd::bdd::{BDDFunction, BDDManager};

pub struct BddScheduler {
    manager: BDDManager,
}

impl BddScheduler {
    /// Build BDD representing all valid schedules
    pub fn build_constraint_space(&self, project: &Project) -> BDDFunction {
        let mut space = self.manager.one(); // Start with "everything valid"
        
        // Add calendar constraints
        for task in &project.tasks {
            let calendar_constraint = self.encode_calendar(task);
            space = space.and(&calendar_constraint);
        }
        
        // Add resource capacity constraints  
        for resource in &project.resources {
            let capacity_constraint = self.encode_capacity(resource, &project.tasks);
            space = space.and(&capacity_constraint);
        }
        
        // Add dependency constraints
        for task in &project.tasks {
            for dep in &task.depends {
                let dep_constraint = self.encode_dependency(dep);
                space = space.and(&dep_constraint);
            }
        }
        
        space
    }
    
    /// Instant what-if analysis
    pub fn what_if(&self, current: &BDDFunction, change: Constraint) -> WhatIfReport {
        let change_bdd = self.encode_constraint(&change);
        let new_space = current.and(&change_bdd);
        
        WhatIfReport {
            still_feasible: !new_space.is_false(),
            solutions_before: current.sat_count(),
            solutions_after: new_space.sat_count(),
            newly_critical: self.find_newly_critical(current, &new_space),
        }
    }
}

impl WhatIfAnalysis for BddScheduler {
    fn what_if(&self, project: &Project, change: Constraint) -> WhatIfReport {
        let space = self.build_constraint_space(project);
        self.what_if(&space, change)
    }
    
    fn count_solutions(&self, project: &Project) -> BigUint {
        let space = self.build_constraint_space(project);
        space.sat_count()
    }
}
```

### Tier 3: SAT Solver (Optional)

```rust
// utf8proj-solver/src/sat.rs

#[cfg(feature = "sat")]
use varisat::{Solver, Lit, CnfFormula};

#[cfg(feature = "sat")]
pub struct SatScheduler {
    solver: Solver,
}

#[cfg(feature = "sat")]
impl SatScheduler {
    /// Encode scheduling problem as SAT and solve
    pub fn schedule(&self, project: &Project) -> Result<Schedule, ScheduleError> {
        let mut formula = CnfFormula::new();
        
        // Variables: task_i_starts_at_t for each task i and time slot t
        let vars = self.create_variables(project);
        
        // Each task starts exactly once
        for task in &project.tasks {
            self.add_exactly_one(&mut formula, &vars.starts[&task.id]);
        }
        
        // Dependencies: predecessor finishes before successor starts
        for task in &project.tasks {
            for dep in &task.depends {
                self.add_dependency_clauses(&mut formula, &vars, dep, task);
            }
        }
        
        // Resource capacity: at any time, sum of assignments ≤ capacity
        for resource in &project.resources {
            self.add_capacity_clauses(&mut formula, &vars, resource, project);
        }
        
        // Solve
        self.solver.add_formula(&formula);
        match self.solver.solve() {
            Ok(true) => self.extract_schedule(&vars),
            Ok(false) => Err(ScheduleError::Infeasible),
            Err(e) => Err(ScheduleError::SolverError(e)),
        }
    }
}
```

---

## MS Project Interoperability

### MSPDI Import/Export

```rust
// utf8proj-mspdi/src/lib.rs

use quick_xml::{Reader, Writer};

/// Import from MS Project XML format
pub fn import_mspdi(xml: &str) -> Result<Project, MspdiError> {
    let reader = Reader::from_str(xml);
    let mspdi: MspdiProject = quick_xml::de::from_reader(reader)?;
    
    Ok(Project {
        id: mspdi.name.clone(),
        name: mspdi.name,
        start: parse_mspdi_date(&mspdi.start_date)?,
        tasks: mspdi.tasks.iter().map(convert_task).collect(),
        resources: mspdi.resources.iter().map(convert_resource).collect(),
        // ...
    })
}

/// Export to MS Project XML format
pub fn export_mspdi(project: &Project, schedule: &Schedule) -> Result<String, MspdiError> {
    let mspdi = MspdiProject {
        name: project.name.clone(),
        start_date: format_mspdi_date(project.start),
        tasks: project.tasks.iter()
            .map(|t| convert_to_mspdi_task(t, schedule.tasks.get(&t.id)))
            .collect(),
        resources: project.resources.iter().map(convert_to_mspdi_resource).collect(),
        assignments: extract_assignments(schedule),
        calendars: vec![convert_calendar(&project.calendar)],
    };
    
    let mut writer = Writer::new(Vec::new());
    quick_xml::se::to_writer(&mut writer, &mspdi)?;
    Ok(String::from_utf8(writer.into_inner())?)
}
```

---

## utf8dok Integration

### AsciiDoc Syntax

```asciidoc
= Project Proposal

== Timeline

[[fig-timeline]]
.Project Schedule
[schedule, report=gantt]
----
include::project.proj[]
----

== Resource Allocation

[[tbl-resources]]
.Team Assignment
[schedule, report=table, type=resources]
----
include::project.proj[]
----

== Cost Summary

[schedule, report=costs]
----
include::project.proj[]
----
```

### Integration Module

```rust
// utf8proj-utf8dok/src/lib.rs

use utf8dok_ast::{Block, Node};
use utf8proj_core::{Project, Schedule};
use utf8proj_render::{GanttRenderer, TableRenderer};

/// Process schedule blocks in utf8dok AST
pub fn process_schedule_block(block: &Block) -> Result<Node, Error> {
    let project = parse_schedule_source(&block.content)?;
    let schedule = schedule(&project)?;
    
    match block.get_attr("report")? {
        "gantt" => {
            let svg = GanttRenderer::new().render(&project, &schedule)?;
            Ok(Node::Image { 
                data: svg.into_bytes(),
                format: ImageFormat::Svg,
                anchor: block.anchor.clone(),
            })
        }
        "table" => {
            let table_type = block.get_attr("type").unwrap_or("tasks");
            let table = TableRenderer::new().render(&project, &schedule, table_type)?;
            Ok(Node::Table(table))
        }
        "costs" => {
            let summary = CostRenderer::new().render(&project, &schedule)?;
            Ok(Node::Table(summary))
        }
        _ => Err(Error::UnknownReportType),
    }
}
```

---

## CLI Design

```bash
# Schedule a project
utf8proj schedule project.proj -o schedule.json

# Generate Gantt chart
utf8proj render project.proj --format svg -o timeline.svg
utf8proj render project.proj --format mermaid -o timeline.mmd

# Export to MS Project
utf8proj export project.proj --format mspdi -o project.xml

# Import from MS Project
utf8proj import project.xml -o project.proj

# What-if analysis
utf8proj what-if project.proj --constraint "resource.dev_team.capacity = 2"

# Validate project file
utf8proj validate project.proj

# TJP compatibility mode
utf8proj schedule project.tjp --tjp-compat
```

---

## Lua Scripting (Optional)

```lua
-- custom_rules.lua

-- Custom priority function
function task_priority(task)
    local base = task.priority or 500
    
    if task:has_tag("security") then
        base = base + 200
    end
    
    if task:has_tag("blocked") then
        base = base - 300
    end
    
    return base
end

-- Custom resource selection
function select_resource(task, available_resources)
    -- Prefer specialists for complex tasks
    if task.effort > duration("5d") then
        for _, res in ipairs(available_resources) do
            if res:has_skill(task.required_skill) then
                return res
            end
        end
    end
    
    -- Default: first available
    return available_resources[1]
end

-- Post-scheduling hook
function on_schedule_complete(schedule)
    -- Log warnings for overallocated resources
    for _, res in ipairs(schedule.resources) do
        if res.utilization > 0.9 then
            warn("Resource " .. res.name .. " is at " .. 
                 (res.utilization * 100) .. "% capacity")
        end
    end
end
```

---

## Licensing Strategy

### Clean-Room Implementation

To avoid GPL contamination from TaskJuggler:

1. **Do NOT** copy TaskJuggler source code
2. **Do NOT** directly port Ruby algorithms to Rust
3. **Do NOT** copy TaskJuggler test files
4. **DO** implement from specifications and manual
5. **DO** use TaskJuggler as black-box oracle for testing
6. **DO** support TJP file format (formats aren't copyrightable)

### License Choice

**Dual License: MIT + Apache-2.0**

This allows:
- Maximum adoption (MIT-compatible with everything)
- Patent protection (Apache-2.0)
- No GPL obligations
- Commercial use without restrictions

---

## Testing Strategy

### Test Generation Approach

```rust
// Tests derived from specification, NOT from TJ test suite

#[test]
fn effort_based_scheduling_uses_resource_availability() {
    // Spec: TJ Manual §4.3 "Effort-based scheduling"
    // Task with 10d effort and 1 resource takes 10 calendar days
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        resource dev "Developer" {}
        task impl "Implementation" {
            effort: 10d
            assign: dev
        }
    "#).unwrap();
    
    let schedule = HeuristicScheduler::new().schedule(&project).unwrap();
    
    assert_eq!(schedule.tasks["impl"].duration, Duration::days(10));
}

#[test]
fn half_time_resource_doubles_duration() {
    // Spec: TJ Manual §4.3 "Part-time resources"
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        resource dev "Developer" { capacity: 0.5 }
        task impl "Implementation" {
            effort: 10d
            assign: dev
        }
    "#).unwrap();
    
    let schedule = HeuristicScheduler::new().schedule(&project).unwrap();
    
    // 10 person-days with 0.5 capacity = 20 calendar days
    assert_eq!(schedule.tasks["impl"].duration, Duration::days(20));
}
```

### Black-Box Oracle Testing

```rust
#[test]
#[ignore] // Requires TaskJuggler installed
fn matches_taskjuggler_scheduling() {
    let tjp = include_str!("fixtures/complex.tjp");
    
    // Run through TaskJuggler
    let tj_output = Command::new("tj3")
        .args(["--output-format", "csv"])
        .stdin(tjp)
        .output()
        .expect("TJ installed");
    let tj_schedule = parse_tj_csv(&tj_output.stdout);
    
    // Run through utf8proj
    let project = utf8proj_parser::tjp::parse(tjp).unwrap();
    let our_schedule = HeuristicScheduler::new().schedule(&project).unwrap();
    
    // Compare results (behavioral equivalence, not code copying)
    for (task_id, tj_task) in tj_schedule.tasks {
        let our_task = &our_schedule.tasks[&task_id];
        assert_eq!(our_task.start, tj_task.start, "Task {} start mismatch", task_id);
        assert_eq!(our_task.finish, tj_task.finish, "Task {} finish mismatch", task_id);
    }
}
```

---

## Implementation Timeline

### LLM-Assisted Development (Aggressive)

| Week | Focus | Deliverables |
|------|-------|--------------|
| **1** | Foundation | Domain model, native DSL parser |
| **2** | Core Scheduling | CPM, heuristic scheduler, resource leveling |
| **3** | Output & CLI | Gantt SVG, tables, CLI commands |
| **4** | Integration | TJP parser, MSPDI export, utf8dok blocks |

### Extended Features

| Week | Focus | Deliverables |
|------|-------|--------------|
| **5** | BDD Integration | OxiDD integration, what-if analysis |
| **6** | Lua Scripting | mlua bindings, custom rules |
| **7-8** | Polish | Documentation, examples, edge cases |

---

## Success Metrics

### Technical

| Metric | Target |
|--------|--------|
| Test coverage | > 80% |
| TJP compatibility | Basic subset (tasks, resources, dependencies) |
| Scheduling performance | < 1s for 1000 tasks |
| Binary size | < 10 MB |

### Adoption

| Metric | 12-Month Target |
|--------|-----------------|
| GitHub stars | 200+ |
| Crates.io downloads | 5,000+ |
| Production users | 10+ |

---

## Open Questions

1. **DSL naming**: `.proj` vs `.sched` vs `.plan`?
2. **TJP compatibility level**: Full vs subset?
3. **BDD default**: Should BDD be in default features or opt-in?
4. **Lua vs Rhai**: Switch from Rhai to Lua for consistency with utf8dok?

---

## References

1. TaskJuggler Manual: https://taskjuggler.org/tj3/manual/
2. MSPDI Schema: https://docs.microsoft.com/en-us/office-project/xml-data-interchange/
3. Critical Path Method: https://en.wikipedia.org/wiki/Critical_path_method
4. OxiDD Documentation: https://docs.rs/oxidd
5. mlua Documentation: https://docs.rs/mlua

---

## Appendix A: Comparison Matrix

| Feature | TaskJuggler | cpm-rs | utf8proj (planned) |
|---------|-------------|--------|---------------------|
| Language | Ruby | Rust | Rust |
| LOC | ~15-20k | 703 | ~8-12k (est) |
| CPM | ✅ | ✅ | ✅ |
| Resource leveling | ✅ | ❌ | ✅ |
| DSL | ✅ TJP | ❌ | ✅ Native + TJP |
| Calendars | ✅ | ❌ | ✅ |
| Cost tracking | ✅ | ❌ | ✅ |
| MSPDI | ⚠️ | ❌ | ✅ |
| BDD/SAT | ❌ | ❌ | ✅ |
| What-if analysis | ⚠️ Scenarios | ❌ | ✅ BDD-powered |
| Document integration | ❌ | ❌ | ✅ utf8dok |
| Single binary | ❌ | ✅ | ✅ |
| License | GPL-2.0 | MIT | MIT/Apache-2.0 |

---

## Appendix B: Example Project Files

### Minimal Example

```
# minimal.proj
project "Hello World" { start: 2025-01-01 }

task hello "Hello" { duration: 1d }
task world "World" { duration: 1d, depends: hello }
```

### Software Project

```
# software.proj
project "Release 2.0" {
    start: 2025-02-01
    end: 2025-04-30
}

resource backend "Backend Team" { capacity: 2, rate: 800/day }
resource frontend "Frontend Team" { capacity: 2, rate: 750/day }
resource qa "QA Engineer" { rate: 600/day }

task planning "Planning" {
    task specs "Write Specifications" { effort: 5d, assign: backend }
    task design "UI Design" { effort: 3d, assign: frontend }
}

task development "Development" {
    depends: planning
    
    task api "API Implementation" { effort: 15d, assign: backend }
    task ui "UI Implementation" { 
        effort: 12d 
        assign: frontend
        depends: api.50%  # Can start when API is 50% done
    }
}

task testing "Testing" {
    depends: development
    
    task integration "Integration Tests" { effort: 5d, assign: qa }
    task uat "User Acceptance" { effort: 3d, depends: integration }
}

task release "Release" {
    depends: testing
    duration: 1d
    milestone: true
}
```
