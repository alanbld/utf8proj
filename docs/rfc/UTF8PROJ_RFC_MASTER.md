# utf8proj: Master RFC & Reference Architecture
**RFC Version:** 2.0 (Consolidated Master)  
**Date:** 2026-01-04  
**Status:** Living Document - Authoritative Reference  
**Confidence:** 85% (see Design Refinement Survey below)

---

## DOCUMENT PURPOSE & USAGE

This document serves as the **single source of truth** for utf8proj architecture, design decisions, and implementation guidance. It is intended for:

1. **Claude Web KB** - Long-term project memory and context
2. **Git Repository** - Version-controlled design documentation
3. **Claude Code CLI** - Local development reference
4. **Human Developers** - Onboarding and implementation guide
5. **LLM Assistants** - Design consultation and code generation

**Update Policy:** This document evolves with the project. Changes require:
- Clear rationale for modifications
- Update to confidence levels
- Migration notes for breaking changes

---

## PART I: STRATEGIC VISION & POSITIONING

### 1.1 Core Mission

> **utf8proj is a Git-native, mathematically verified CPM engine that brings version control discipline to project management through human-readable text files and progress-aware scheduling.**

### 1.2 What Makes utf8proj Unique

| Dimension | utf8proj Approach | Traditional Tools |
|-----------|-------------------|-------------------|
| **Data Format** | Plain text (.proj files) | Binary files, databases |
| **Version Control** | First-class Git integration | Manual "Save As", proprietary versioning |
| **Scheduling** | Mathematically verified CPM (8 invariants) | Proprietary algorithms, undocumented |
| **Progress Tracking** | Text-based, Git-committable | GUI-only, click-to-update |
| **Deployment** | Single binary, zero dependencies | Ruby runtime, JVM, or cloud subscription |
| **History** | Git diff + playback animation | Manual comparison, screenshot archives |

### 1.3 Target Users & Use Cases

**Primary Personas:**

1. **DevOps Engineer** (Emily, 32)
   - Needs: Infrastructure migration planning, sprint scheduling
   - Workflow: Git-based, CI/CD integration, automation
   - Pain: MS Project files don't merge, TaskJuggler requires Ruby

2. **Technical PM** (Marcus, 38)
   - Needs: Software release planning, dependency tracking
   - Workflow: Markdown docs, code reviews, team collaboration
   - Pain: Gantt tools are GUI-heavy, hard to version control

3. **Open Source Maintainer** (Sarah, 29)
   - Needs: Roadmap planning, contributor coordination
   - Workflow: GitHub-native, transparent planning
   - Pain: No good text-based project scheduling tools

**Anti-Personas** (out of scope):
- Traditional PM using MS Project exclusively
- Large enterprise needing SAP/Oracle integration
- Teams requiring extensive resource management UI

### 1.4 Strategic Positioning

```
COMPETITIVE LANDSCAPE:

Text-Based          │ GUI-Heavy
Simple             │ Complex
────────────────────┼────────────────────
                   │
  TaskJuggler      │ MS Project
  (Ruby, GPL)      │ ($$$$, proprietary)
                   │
  utf8proj ◄───────┼─── Target Position
  (Rust, MIT)      │ (modern, open, correct)
                   │
  cpm-rs           │ ProjectLibre
  (too minimal)    │ (Java, MS Project clone)
                   │
```

**Unique Value Proposition:**
- **For developers**: As easy as editing a Markdown file, as powerful as MS Project
- **For PMs**: Professional CPM scheduling without GUI lock-in
- **For teams**: Merge schedules like code, review changes in Pull Requests

---

## PART II: ARCHITECTURE & DOMAIN MODEL

### 2.1 System Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                     utf8proj SYSTEM ARCHITECTURE                    │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      utf8proj-core                           │   │
│  │  • Domain Model (Project, Task, Resource, Calendar)         │   │
│  │  • Traits (Scheduler, Renderer, Exporter)                   │   │
│  │  • Invariants & Validation                                  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│        ┌─────────────────────┼─────────────────────┐               │
│        │                     │                     │               │
│        ▼                     ▼                     ▼               │
│  ┌──────────┐         ┌──────────┐         ┌──────────┐           │
│  │ Parser   │         │ Solver   │         │ Renderer │           │
│  │          │         │          │         │          │           │
│  │ • .proj  │         │ • CPM    │         │ • SVG    │           │
│  │ • .tjp   │         │ • Leveling│        │ • Excel  │           │
│  │ • pest   │         │ • Progress│        │ • JSON   │           │
│  │ • JSON   │         │ • Forecast│        │ • Mermaid│           │
│  └──────────┘         └──────────┘         └──────────┘           │
│        │                     │                     │               │
│        └─────────────────────┼─────────────────────┘               │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    utf8proj-history                          │   │
│  │  • GitHistoryProvider (git2)                                │   │
│  │  • SidecarHistoryProvider (YAML/JSON)                       │   │
│  │  • EmbeddedHistoryProvider (comment parsing)                │   │
│  │  • PlaybackEngine (animation, diff, impact analysis)        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      utf8proj-cli                            │   │
│  │  • schedule, status, progress, forecast                     │   │
│  │  • history, playback, snapshot, diff                        │   │
│  │  • export, import, validate                                 │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  OPTIONAL EXTENSIONS:                                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ WASM     │  │ VS Code  │  │ GitHub   │  │ EVM      │          │
│  │ Browser  │  │ Extension│  │ Action   │  │ Analytics│          │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘          │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

### 2.2 Crate Structure (Workspace)

```
utf8proj/                           # Cargo workspace root
├── Cargo.toml                      # Workspace manifest
├── README.md                       # User-facing documentation
├── UTF8PROJ_RFC_MASTER.md         # This document
├── LICENSE-MIT
├── LICENSE-APACHE
│
├── crates/
│   ├── utf8proj-core/              # ★ CORE: Domain model & traits
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── project.rs          # Project struct
│   │       ├── task.rs             # Task, Dependency, Progress
│   │       ├── resource.rs         # Resource, Assignment
│   │       ├── calendar.rs         # Calendar, WorkingHours
│   │       ├── constraint.rs       # TaskConstraint enum
│   │       ├── schedule.rs         # Schedule, ScheduledTask
│   │       ├── validation.rs       # Domain validation rules
│   │       └── traits.rs           # Scheduler, Renderer, Exporter
│   │
│   ├── utf8proj-parser/            # ★ PARSER: DSL → Domain model
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── proj/               # Native .proj format
│   │       │   ├── grammar.pest
│   │       │   └── parser.rs
│   │       ├── tjp/                # TaskJuggler compatibility
│   │       │   ├── grammar.pest
│   │       │   └── parser.rs
│   │       └── json/               # JSON import/export
│   │           └── serde.rs
│   │
│   ├── utf8proj-solver/            # ★ SOLVER: Scheduling algorithms
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── cpm.rs              # Critical Path Method (baseline)
│   │       ├── progress_cpm.rs     # Progress-aware CPM
│   │       ├── leveling.rs         # Resource leveling
│   │       ├── dag.rs              # DAG construction & validation
│   │       ├── calendar_math.rs    # Working day calculations
│   │       └── invariants.rs       # 8 CPM correctness tests
│   │
│   ├── utf8proj-render/            # ★ RENDER: Output generation
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── svg_gantt.rs        # SVG Gantt charts
│   │       ├── excel.rs            # Excel with formulas
│   │       ├── mermaid.rs          # Mermaid diagram
│   │       ├── json.rs             # JSON export
│   │       └── table.rs            # Markdown tables
│   │
│   ├── utf8proj-history/           # ★ HISTORY: Version control
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs           # HistoryProvider trait
│   │       ├── git.rs              # GitHistoryProvider
│   │       ├── sidecar.rs          # SidecarHistoryProvider
│   │       ├── embedded.rs         # EmbeddedHistoryProvider
│   │       ├── playback.rs         # PlaybackEngine
│   │       └── diff.rs             # Change detection & impact
│   │
│   └── utf8proj-cli/               # ★ CLI: User interface
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── commands/
│           │   ├── schedule.rs     # Schedule & forecast
│           │   ├── status.rs       # Progress dashboard
│           │   ├── progress.rs     # Update progress
│           │   ├── history.rs      # Version history
│           │   ├── playback.rs     # Animate history
│           │   ├── export.rs       # Export to formats
│           │   └── validate.rs     # Validate project
│           └── ui/
│               ├── dashboard.rs    # Text-based UI
│               └── interactive.rs  # TUI (optional)
│
├── tests/                          # Integration tests
│   ├── fixtures/                   # Test project files
│   │   ├── simple.proj
│   │   ├── software_project.proj
│   │   ├── progress_tracking.proj
│   │   └── taskjuggler_compat.tjp
│   └── integration/
│       ├── cpm_correctness.rs      # 8 invariant tests
│       ├── progress_scheduling.rs
│       ├── history_playback.rs
│       └── export_formats.rs
│
└── examples/                       # Example projects
    ├── 01_minimal.proj
    ├── 02_software_release.proj
    ├── 03_construction.proj
    ├── 04_progress_tracking.proj
    └── 05_git_workflow.sh
```

### 2.3 Core Domain Model (Rust)

```rust
// ═══════════════════════════════════════════════════════════════════
// utf8proj-core/src/project.rs
// ═══════════════════════════════════════════════════════════════════

use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Complete project definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    // Core identification
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    
    // Temporal scope
    pub start: NaiveDate,
    pub end: Option<NaiveDate>,
    
    // Progress tracking
    pub status_date: Option<NaiveDate>,     // "As of" date for progress
    pub baseline: Option<BaselineRef>,      // Git tag or snapshot ID
    
    // Structure
    pub tasks: Vec<Task>,                   // Root tasks (WBS)
    pub resources: Vec<Resource>,
    pub calendars: HashMap<CalendarId, Calendar>,
    pub default_calendar: CalendarId,
    
    // History configuration
    pub history_mode: HistoryMode,
    pub auto_snapshot: bool,
    
    // Metadata
    pub currency: Currency,
    pub created_at: Option<DateTime>,
    pub created_by: Option<String>,
    pub last_modified: Option<DateTime>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HistoryMode {
    Git,           // Use Git for versioning
    Sidecar,       // Use .proj.history file
    Embedded,      // Embed in .proj comments
    Disabled,      // No history tracking
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BaselineRef {
    GitTag(String),       // "v1.0"
    GitCommit(String),    // "abc123"
    Snapshot(String),     // "2026-01-15-baseline"
}

// ═══════════════════════════════════════════════════════════════════
// utf8proj-core/src/task.rs
// ═══════════════════════════════════════════════════════════════════

/// A schedulable unit of work (leaf) or container (parent)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    // ─────────────────────────────────────────────────────────────
    // CORE IDENTITY
    // ─────────────────────────────────────────────────────────────
    pub id: TaskId,                         // Unique ID (hierarchical)
    pub name: String,
    pub description: Option<String>,
    
    // ─────────────────────────────────────────────────────────────
    // DURATION SPECIFICATION (mutually exclusive)
    // ─────────────────────────────────────────────────────────────
    pub effort: Option<Duration>,           // Person-time (10d)
    pub duration: Option<Duration>,         // Calendar time (2w)
    // NOTE: Container tasks derive duration from children
    //       Leaf tasks require effort OR duration
    
    // ─────────────────────────────────────────────────────────────
    // DEPENDENCIES & RELATIONSHIPS
    // ─────────────────────────────────────────────────────────────
    pub depends: Vec<Dependency>,           // Predecessors
    pub children: Vec<Task>,                // Child tasks (WBS)
    
    // ─────────────────────────────────────────────────────────────
    // RESOURCE ASSIGNMENT
    // ─────────────────────────────────────────────────────────────
    pub assigned: Vec<ResourceRef>,         // Resources allocated
    
    // ─────────────────────────────────────────────────────────────
    // SCHEDULING CONSTRAINTS
    // ─────────────────────────────────────────────────────────────
    pub constraints: Vec<TaskConstraint>,
    pub priority: u32,                      // Higher = more important
    pub milestone: bool,                    // Zero-duration marker
    
    // ─────────────────────────────────────────────────────────────
    // PROGRESS TRACKING ★ NEW
    // ─────────────────────────────────────────────────────────────
    pub percent_complete: Option<u8>,       // 0-100
    pub actual_start: Option<NaiveDate>,    // When work actually began
    pub actual_finish: Option<NaiveDate>,   // When work actually ended
    pub status: Option<TaskStatus>,         // Detailed status
    pub remaining_effort: Option<Duration>, // Override auto-calc
    pub notes: Option<String>,              // Progress notes
    
    // ─────────────────────────────────────────────────────────────
    // METADATA
    // ─────────────────────────────────────────────────────────────
    pub created_at: Option<DateTime>,
    pub created_by: Option<String>,
    pub last_modified: Option<DateTime>,
    pub tags: Vec<String>,                  // Categorization
}

/// Task dependency with type and lag
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dependency {
    pub predecessor: TaskId,                // Task that must complete first
    pub dep_type: DependencyType,           // FS, SS, FF, SF
    pub lag: Option<Duration>,              // Delay after predecessor
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DependencyType {
    FinishToStart,   // FS: B starts after A finishes (default)
    StartToStart,    // SS: B starts when A starts
    FinishToFinish,  // FF: B finishes when A finishes
    StartToFinish,   // SF: B finishes when A starts (rare)
}

/// Detailed task status
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TaskStatus {
    NotStarted,
    InProgress,
    Complete,
    OnHold { reason: String },
    Blocked { reason: String, since: NaiveDate },
    AtRisk { reason: String },
    Cancelled { reason: String },
}

/// Scheduling constraint
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TaskConstraint {
    MustStartOn(NaiveDate),
    MustFinishOn(NaiveDate),
    StartNoEarlierThan(NaiveDate),
    StartNoLaterThan(NaiveDate),
    FinishNoEarlierThan(NaiveDate),
    FinishNoLaterThan(NaiveDate),
}

// ═══════════════════════════════════════════════════════════════════
// utf8proj-core/src/schedule.rs
// ═══════════════════════════════════════════════════════════════════

/// Result of scheduling computation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub project_id: String,
    pub scheduled_at: DateTime,
    pub status_date: Option<NaiveDate>,
    
    // Scheduled tasks
    pub tasks: HashMap<TaskId, ScheduledTask>,
    
    // Critical path
    pub critical_path: Vec<TaskId>,
    
    // Metrics
    pub project_duration: Duration,
    pub project_start: NaiveDate,
    pub project_finish: NaiveDate,
    pub total_cost: Option<Decimal>,
    
    // Warnings & issues
    pub warnings: Vec<ScheduleWarning>,
}

/// A task with computed schedule information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub task_id: TaskId,
    
    // ─────────────────────────────────────────────────────────────
    // BASELINE (Planned) - Without progress
    // ─────────────────────────────────────────────────────────────
    pub planned_start: NaiveDate,
    pub planned_finish: NaiveDate,
    pub planned_duration: Duration,
    
    // ─────────────────────────────────────────────────────────────
    // ACTUAL (Historical) - What happened
    // ─────────────────────────────────────────────────────────────
    pub actual_start: Option<NaiveDate>,
    pub actual_finish: Option<NaiveDate>,
    pub percent_complete: u8,
    
    // ─────────────────────────────────────────────────────────────
    // FORECAST (Future) - Considering progress
    // ─────────────────────────────────────────────────────────────
    pub forecast_start: NaiveDate,          // actual OR planned
    pub forecast_finish: NaiveDate,         // based on remaining work
    pub forecast_duration: Duration,        // remaining duration
    
    // ─────────────────────────────────────────────────────────────
    // VARIANCE ANALYSIS
    // ─────────────────────────────────────────────────────────────
    pub start_variance: Duration,           // forecast - planned
    pub finish_variance: Duration,          // forecast - planned
    
    // ─────────────────────────────────────────────────────────────
    // CPM METRICS
    // ─────────────────────────────────────────────────────────────
    pub early_start: NaiveDate,
    pub early_finish: NaiveDate,
    pub late_start: NaiveDate,
    pub late_finish: NaiveDate,
    pub total_float: Duration,              // Slack/float
    pub free_float: Duration,
    pub is_critical: bool,
    
    // ─────────────────────────────────────────────────────────────
    // RESOURCE & COST
    // ─────────────────────────────────────────────────────────────
    pub assignments: Vec<Assignment>,
    pub total_cost: Option<Decimal>,
    
    // ─────────────────────────────────────────────────────────────
    // EVM METRICS (Earned Value Management) - PHASE 2
    // ─────────────────────────────────────────────────────────────
    pub pv: Option<Decimal>,                // Planned Value
    pub ev: Option<Decimal>,                // Earned Value
    pub ac: Option<Decimal>,                // Actual Cost
    pub cpi: Option<f64>,                   // Cost Performance Index
    pub spi: Option<f64>,                   // Schedule Performance Index
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Assignment {
    pub resource_id: ResourceId,
    pub units: f64,                         // 0.5 = half-time
    pub cost: Option<Decimal>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScheduleWarning {
    OverallocatedResource { resource: ResourceId, date: NaiveDate },
    MissedConstraint { task: TaskId, constraint: TaskConstraint },
    NegativeLag { task: TaskId, dependency: TaskId },
    CircularDependency { tasks: Vec<TaskId> },
    InvalidProgress { task: TaskId, reason: String },
}
```

### 2.4 Progress-Aware CPM Algorithm (High-Level)

```rust
// ═══════════════════════════════════════════════════════════════════
// utf8proj-solver/src/progress_cpm.rs
// ═══════════════════════════════════════════════════════════════════

pub struct ProgressAwareCpmScheduler {
    calendar: Calendar,
    status_date: NaiveDate,
}

impl ProgressAwareCpmScheduler {
    pub fn schedule(&self, project: &Project) -> Result<Schedule> {
        // ─────────────────────────────────────────────────────────
        // STEP 1: Build DAG (leaf tasks only, containers excluded)
        // ─────────────────────────────────────────────────────────
        let dag = self.build_scheduling_dag(&project.tasks)?;
        
        // Validation: No cycles, all dependencies valid
        self.validate_dag(&dag)?;
        
        // ─────────────────────────────────────────────────────────
        // STEP 2: Calculate effective tasks (remaining work)
        // ─────────────────────────────────────────────────────────
        let effective_tasks = self.calculate_effective_tasks(&dag, project)?;
        
        // ─────────────────────────────────────────────────────────
        // STEP 3: Forward pass (early start/finish)
        // ─────────────────────────────────────────────────────────
        let early_dates = self.forward_pass(&dag, &effective_tasks, project.start)?;
        
        // ─────────────────────────────────────────────────────────
        // STEP 4: Backward pass (late start/finish, float)
        // ─────────────────────────────────────────────────────────
        let late_dates = self.backward_pass(&dag, &early_dates)?;
        
        // ─────────────────────────────────────────────────────────
        // STEP 5: Resource leveling (if over-allocated)
        // ─────────────────────────────────────────────────────────
        let leveled = self.level_resources(&dag, &early_dates, &late_dates, project)?;
        
        // ─────────────────────────────────────────────────────────
        // STEP 6: Apply actual dates (pin to reality)
        // ─────────────────────────────────────────────────────────
        let schedule = self.apply_actuals(&leveled, project)?;
        
        // ─────────────────────────────────────────────────────────
        // STEP 7: Calculate forecasts & variances
        // ─────────────────────────────────────────────────────────
        self.calculate_forecasts(&schedule, project)?;
        
        // ─────────────────────────────────────────────────────────
        // STEP 8: Identify critical path
        // ─────────────────────────────────────────────────────────
        let critical_path = self.find_critical_path(&schedule)?;
        
        // ─────────────────────────────────────────────────────────
        // STEP 9: Derive container dates from children
        // ─────────────────────────────────────────────────────────
        self.derive_container_dates(&schedule, &project.tasks)?;
        
        Ok(schedule)
    }
    
    /// Calculate effective (remaining) work for each task
    fn calculate_effective_tasks(
        &self, 
        dag: &SchedulingDag, 
        project: &Project
    ) -> HashMap<TaskId, EffectiveTask> {
        let mut effective = HashMap::new();
        
        for task_id in dag.leaf_tasks() {
            let task = project.get_task(task_id)?;
            
            let effective_task = if let Some(actual_finish) = task.actual_finish {
                // Task is complete - no remaining work
                EffectiveTask {
                    original_duration: task.duration()?,
                    effective_duration: Duration::zero(),
                    percent_complete: 100,
                    pinned_start: task.actual_start,
                    pinned_finish: Some(actual_finish),
                }
            } else if let Some(pct) = task.percent_complete {
                // Task is in progress
                let original_duration = task.duration()?;
                let remaining_pct = (100 - pct) as f64 / 100.0;
                
                EffectiveTask {
                    original_duration,
                    effective_duration: original_duration.mul_f64(remaining_pct),
                    percent_complete: pct,
                    pinned_start: task.actual_start,
                    pinned_finish: None,
                }
            } else {
                // Task not started
                EffectiveTask {
                    original_duration: task.duration()?,
                    effective_duration: task.duration()?,
                    percent_complete: 0,
                    pinned_start: None,
                    pinned_finish: None,
                }
            };
            
            effective.insert(task_id.clone(), effective_task);
        }
        
        effective
    }
}
```

---

## PART III: DSL SPECIFICATION

### 3.1 Native .proj Format (Complete Grammar)

```pest
// ═══════════════════════════════════════════════════════════════════
// utf8proj-parser/src/proj/grammar.pest
// ═══════════════════════════════════════════════════════════════════

project_file = { SOI ~ (comment | project_decl | calendar_decl | resource_decl | task_decl)* ~ EOI }

comment = _{ "#" ~ (!NEWLINE ~ ANY)* ~ NEWLINE }
WHITESPACE = _{ " " | "\t" | NEWLINE }

// ───────────────────────────────────────────────────────────────────
// PROJECT DECLARATION
// ───────────────────────────────────────────────────────────────────
project_decl = { "project" ~ string ~ "{" ~ project_body ~ "}" }

project_body = { project_attr* }

project_attr = {
    ("start" ~ ":" ~ date) |
    ("end" ~ ":" ~ date) |
    ("currency" ~ ":" ~ currency_code) |
    ("status_date" ~ ":" ~ date) |
    ("baseline" ~ ":" ~ string) |
    ("history" ~ ":" ~ history_mode) |
    ("auto_snapshot" ~ ":" ~ boolean)
}

history_mode = { "git" | "sidecar" | "embedded" | "none" }
currency_code = @{ ASCII_ALPHA{3} }  // EUR, USD, GBP

// ───────────────────────────────────────────────────────────────────
// CALENDAR DECLARATION
// ───────────────────────────────────────────────────────────────────
calendar_decl = { "calendar" ~ identifier ~ string ~ "{" ~ calendar_body ~ "}" }

calendar_body = { calendar_attr* }

calendar_attr = {
    ("working_hours" ~ ":" ~ time_range ~ ("," ~ time_range)*) |
    ("working_days" ~ ":" ~ day_list) |
    ("holiday" ~ string ~ date_range)
}

time_range = { time ~ "-" ~ time }
time = @{ ASCII_DIGIT{2} ~ ":" ~ ASCII_DIGIT{2} }
day_list = { day ~ ("," ~ day)* }
day = { "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun" }
date_range = { date ~ (".." ~ date)? }

// ───────────────────────────────────────────────────────────────────
// RESOURCE DECLARATION
// ───────────────────────────────────────────────────────────────────
resource_decl = { "resource" ~ identifier ~ string ~ "{" ~ resource_body ~ "}" }

resource_body = { resource_attr* }

resource_attr = {
    ("rate" ~ ":" ~ money) |
    ("capacity" ~ ":" ~ float) |
    ("calendar" ~ ":" ~ identifier) |
    ("email" ~ ":" ~ string)
}

money = @{ float ~ "/" ~ time_unit }

// ───────────────────────────────────────────────────────────────────
// TASK DECLARATION
// ───────────────────────────────────────────────────────────────────
task_decl = { "task" ~ identifier ~ string ~ "{" ~ task_body ~ "}" }

task_body = { (task_attr | task_decl)* }

task_attr = {
    ("effort" ~ ":" ~ duration) |
    ("duration" ~ ":" ~ duration) |
    ("depends" ~ ":" ~ dependency_list) |
    ("assign" ~ ":" ~ identifier_list) |
    ("priority" ~ ":" ~ integer) |
    ("milestone" ~ ":" ~ boolean) |
    
    // ★ PROGRESS TRACKING (NEW)
    ("complete" ~ ":" ~ percent) |
    ("actual_start" ~ ":" ~ date) |
    ("actual_finish" ~ ":" ~ date) |
    ("status" ~ ":" ~ task_status) |
    ("remaining" ~ ":" ~ duration) |
    ("notes" ~ ":" ~ string) |
    
    // CONSTRAINTS
    ("must_start_on" ~ ":" ~ date) |
    ("must_finish_on" ~ ":" ~ date) |
    ("start_no_earlier_than" ~ ":" ~ date) |
    ("start_no_later_than" ~ ":" ~ date) |
    
    // METADATA
    ("created" ~ ":" ~ datetime ~ "by" ~ string) |
    ("modified" ~ ":" ~ datetime ~ "by" ~ string) |
    ("tags" ~ ":" ~ string_list)
}

task_status = {
    "not_started" |
    "in_progress" |
    "complete" |
    ("on_hold" ~ string) |
    ("blocked" ~ string) |
    ("at_risk" ~ string) |
    ("cancelled" ~ string)
}

dependency_list = { dependency ~ ("," ~ dependency)* }
dependency = { task_ref ~ dep_modifier? }
task_ref = { identifier ~ ("." ~ identifier)* }
dep_modifier = { ("+" | "-") ~ duration }

// ───────────────────────────────────────────────────────────────────
// PRIMITIVES
// ───────────────────────────────────────────────────────────────────
duration = @{ float ~ time_unit }
time_unit = { "d" | "w" | "m" | "h" }
percent = @{ integer ~ "%" }

date = @{ ASCII_DIGIT{4} ~ "-" ~ ASCII_DIGIT{2} ~ "-" ~ ASCII_DIGIT{2} }
datetime = @{ date ~ "T" ~ time ~ (":" ~ ASCII_DIGIT{2})? ~ ("Z" | offset)? }
offset = @{ ("+" | "-") ~ ASCII_DIGIT{2} ~ ":" ~ ASCII_DIGIT{2} }

identifier = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
identifier_list = { identifier ~ ("," ~ identifier)* }
string = @{ "\"" ~ (!"\"" ~ ANY)* ~ "\"" }
string_list = { string ~ ("," ~ string)* }

integer = @{ ASCII_DIGIT+ }
float = @{ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)? }
boolean = { "true" | "false" | "yes" | "no" }
```

### 3.2 Complete .proj Example

```proj
# ═════════════════════════════════════════════════════════════════════
# EXAMPLE: Software Release Project with Progress Tracking
# ═════════════════════════════════════════════════════════════════════

project "Backend API v2.0" {
    start: 2026-01-15
    currency: EUR
    
    # Progress tracking configuration
    status_date: 2026-03-01          # Today's date
    baseline: "v2.0-approved"        # Git tag from approved plan
    
    # History tracking
    history: git                     # Use Git for version control
    auto_snapshot: yes               # Auto-commit on save
}

# ═════════════════════════════════════════════════════════════════════
# CALENDARS
# ═════════════════════════════════════════════════════════════════════

calendar "standard" {
    working_hours: 09:00-13:00, 14:00-18:00
    working_days: mon, tue, wed, thu, fri
    
    holiday "New Year" 2026-01-01
    holiday "Easter" 2026-04-10..2026-04-13
}

# ═════════════════════════════════════════════════════════════════════
# RESOURCES
# ═════════════════════════════════════════════════════════════════════

resource pm "Project Manager" {
    rate: 850/day
    email: "alice@company.com"
}

resource backend "Backend Team" {
    rate: 800/day
    capacity: 2                      # 2 full-time developers
}

resource frontend "Frontend Team" {
    rate: 750/day
    capacity: 2
}

resource qa "QA Engineer" {
    rate: 600/day
    capacity: 1
}

# ═════════════════════════════════════════════════════════════════════
# TASKS (Work Breakdown Structure)
# ═════════════════════════════════════════════════════════════════════

task project_root "API v2.0 Release" {
    
    # ─────────────────────────────────────────────────────────────────
    # PHASE 1: PLANNING (COMPLETE)
    # ─────────────────────────────────────────────────────────────────
    task planning "Planning & Design" {
        
        task requirements "Requirements Gathering" {
            effort: 5d
            assign: pm, backend
            
            # Progress: COMPLETE
            complete: 100%
            actual_start: 2026-01-15
            actual_finish: 2026-01-22
            status: complete
            
            created: 2026-01-10T09:00:00 by "alice"
            modified: 2026-01-22T17:00:00 by "alice"
        }
        
        task architecture "Architecture Design" {
            effort: 8d
            assign: backend
            depends: requirements
            
            # Progress: COMPLETE
            complete: 100%
            actual_start: 2026-01-23
            actual_finish: 2026-02-03
            status: complete
            notes: "OpenAPI spec approved by stakeholders"
        }
        
        task design_review "Design Review Milestone" {
            milestone: true
            depends: architecture
            
            complete: 100%
            actual_finish: 2026-02-04
            status: complete
        }
    }
    
    # ─────────────────────────────────────────────────────────────────
    # PHASE 2: DEVELOPMENT (IN PROGRESS)
    # ─────────────────────────────────────────────────────────────────
    task development "Development Phase" {
        depends: planning.design_review
        
        task backend_api "Backend API Implementation" {
            effort: 20d
            assign: backend
            priority: 900
            
            # Progress: IN PROGRESS (65% complete)
            complete: 65%
            actual_start: 2026-02-05
            status: in_progress
            remaining: 7d              # Override: 35% of 20d
            notes: "Database migrations complete, working on auth"
            
            task auth "Authentication Module" {
                effort: 6d
                complete: 90%
                actual_start: 2026-02-05
                status: in_progress
            }
            
            task data "Data Access Layer" {
                effort: 8d
                complete: 100%
                actual_start: 2026-02-05
                actual_finish: 2026-02-15
                status: complete
            }
            
            task business "Business Logic" {
                effort: 6d
                depends: auth, data
                complete: 20%
                actual_start: 2026-02-25
                status: at_risk "Dependency on external service delayed"
            }
        }
        
        task frontend_ui "Frontend UI" {
            effort: 15d
            assign: frontend
            depends: backend_api.auth  # Can start after auth done
            
            # Progress: NOT STARTED (waiting on backend)
            complete: 0%
            status: blocked "Waiting on auth module completion"
        }
        
        task integration "Integration Testing" {
            effort: 5d
            assign: qa, backend
            depends: backend_api, frontend_ui
            
            complete: 0%
            status: not_started
        }
    }
    
    # ─────────────────────────────────────────────────────────────────
    # PHASE 3: TESTING & DEPLOYMENT (NOT STARTED)
    # ─────────────────────────────────────────────────────────────────
    task testing "Testing & QA" {
        depends: development.integration
        
        task qa_testing "QA Test Execution" {
            effort: 8d
            assign: qa
            
            status: not_started
        }
        
        task performance "Performance Testing" {
            effort: 3d
            assign: qa, backend
            depends: qa_testing
            
            status: not_started
        }
        
        task uat "User Acceptance Testing" {
            effort: 5d
            assign: pm
            depends: qa_testing
            
            status: not_started
            must_finish_on: 2026-04-15  # Hard deadline
        }
    }
    
    task deployment "Deployment" {
        depends: testing.uat
        
        task staging "Deploy to Staging" {
            duration: 1d
            assign: backend
            milestone: true
        }
        
        task production "Production Deployment" {
            duration: 1d
            assign: backend, pm
            depends: staging
            milestone: true
            
            must_finish_on: 2026-04-20  # Release deadline
        }
    }
}
```

---

## PART IV: CLI INTERFACE SPECIFICATION

### 4.1 Complete Command Reference

```bash
# ═════════════════════════════════════════════════════════════════════
# CORE SCHEDULING COMMANDS
# ═════════════════════════════════════════════════════════════════════

# Schedule project (baseline, no progress)
utf8proj schedule project.proj

# Schedule with progress awareness
utf8proj schedule project.proj --status-date=2026-03-01

# Show schedule output formats
utf8proj schedule project.proj --format=json > schedule.json
utf8proj schedule project.proj --format=yaml > schedule.yaml
utf8proj schedule project.proj --format=table

# ═════════════════════════════════════════════════════════════════════
# PROGRESS TRACKING
# ═════════════════════════════════════════════════════════════════════

# Show progress dashboard
utf8proj status project.proj

# Update task progress
utf8proj progress --task=backend_api --complete=65
utf8proj progress --task=auth --complete=90 --status=in_progress
utf8proj progress --task=data --actual-finish=2026-02-15

# Batch update from CSV
utf8proj progress --import=progress_update.csv

# ═════════════════════════════════════════════════════════════════════
# FORECASTING & VARIANCE
# ═════════════════════════════════════════════════════════════════════

# Generate forecast report (planned vs actual vs forecast)
utf8proj forecast project.proj
utf8proj forecast project.proj --baseline=v2.0-approved

# Show variance report
utf8proj variance project.proj --format=table
utf8proj variance project.proj --format=excel --output=variance.xlsx

# What-if analysis
utf8proj what-if project.proj --constraint="backend_api.complete=100"

# ═════════════════════════════════════════════════════════════════════
# HISTORY & PLAYBACK
# ═════════════════════════════════════════════════════════════════════

# List versions
utf8proj history project.proj                  # Auto-detect backend
utf8proj history --git project.proj            # Force Git backend
utf8proj history --sidecar project.proj        # Force sidecar
utf8proj history --embedded project.proj       # Force embedded

# Show specific version
utf8proj show project.proj@v1.0
utf8proj show project.proj@2026-02-15

# Compare versions
utf8proj diff project.proj@v1.0 project.proj@HEAD
utf8proj diff --baseline=v1.0 --current=HEAD project.proj

# Create snapshot
utf8proj snapshot project.proj --message="Before sprint 3"

# Revert to version
utf8proj revert project.proj --to=v1.0
utf8proj revert project.proj --to="2 days ago"

# Animate history (playback)
utf8proj playback project.proj --output=animation.gif
utf8proj playback project.proj --output=timeline.html --interactive
utf8proj playback project.proj --output=frames/ --format=svg

# ═════════════════════════════════════════════════════════════════════
# EXPORT FORMATS
# ═════════════════════════════════════════════════════════════════════

# SVG Gantt chart
utf8proj export project.proj --format=svg --output=gantt.svg

# Excel with formulas
utf8proj export project.proj --format=excel --output=schedule.xlsx

# JSON (for APIs)
utf8proj export project.proj --format=json --output=schedule.json

# Mermaid diagram
utf8proj export project.proj --format=mermaid --output=gantt.mmd

# Markdown table
utf8proj export project.proj --format=markdown --output=schedule.md

# iCalendar (for Google Calendar, Outlook)
utf8proj export project.proj --format=ical --output=project.ics

# ═════════════════════════════════════════════════════════════════════
# VALIDATION & DEBUGGING
# ═════════════════════════════════════════════════════════════════════

# Validate project file
utf8proj validate project.proj

# Check for issues (circular deps, over-allocation, etc.)
utf8proj check project.proj --verbose

# Lint project file (style, best practices)
utf8proj lint project.proj

# ═════════════════════════════════════════════════════════════════════
# GIT INTEGRATION
# ═════════════════════════════════════════════════════════════════════

# Create Git baseline
utf8proj git-baseline --tag=v2.0-approved --message="Approved baseline"

# Compare against Git baseline
utf8proj git-compare --baseline=v1.0 --current=HEAD

# Git pre-commit hook (validate before commit)
utf8proj git-hook --install

# ═════════════════════════════════════════════════════════════════════
# INTERACTIVE MODE
# ═════════════════════════════════════════════════════════════════════

# TUI (Terminal UI)
utf8proj ui project.proj

# Web UI (WASM build)
utf8proj serve --port=8080 --project=project.proj
```

### 4.2 Status Dashboard Output Example

```
$ utf8proj status project.proj

╔════════════════════════════════════════════════════════════════════╗
║           Backend API v2.0 - Project Status Dashboard              ║
║                    As of: 2026-03-01                               ║
║                  Baseline: v2.0-approved (2026-01-10)              ║
╠════════════════════════════════════════════════════════════════════╣
║ PROJECT SUMMARY                                                    ║
╠════════════════════════════════════════════════════════════════════╣
║  Planned Start:     2026-01-15    Actual Start:      2026-01-15   ║
║  Planned Finish:    2026-04-20    Forecast Finish:   2026-04-28   ║
║  Planned Duration:  67 days       Forecast Duration: 75 days      ║
║  Variance:          +8 days       (11.9% delay)                   ║
║  Progress:          42% complete                                   ║
╚════════════════════════════════════════════════════════════════════╝

╔════════════════════════════════════════════════════════════════════╗
║ PHASE STATUS                                                       ║
╠════════════════════════════════════════════════════════════════════╣
║ Phase                    │ Status      │ Progress │ Variance       ║
╠══════════════════════════╪═════════════╪══════════╪════════════════╣
║ Planning & Design        │ ✓ Complete  │ 100%     │ On time        ║
║ Development Phase        │ ⚡In Progress│ 65%      │ +2 days        ║
║ Testing & QA             │ ○ Not Start │ 0%       │ TBD            ║
║ Deployment               │ ○ Not Start │ 0%       │ TBD            ║
╚══════════════════════════╧═════════════╧══════════╧════════════════╝

╔════════════════════════════════════════════════════════════════════╗
║ CRITICAL PATH TASKS                                                ║
╠════════════════════════════════════════════════════════════════════╣
║ Task                     │ Status      │ Finish (Planned→Forecast) ║
╠══════════════════════════╪═════════════╪═══════════════════════════╣
║ Backend API              │ ⚡65% done   │ 2026-03-05 → 2026-03-12   ║
║   ↳ Business Logic       │ ⚠️ At Risk   │ 2026-03-03 → 2026-03-10   ║
║ Frontend UI              │ 🚫 Blocked   │ 2026-03-25 → 2026-04-02   ║
║ Integration Testing      │ ○ Pending   │ 2026-04-01 → 2026-04-09   ║
║ Production Deployment    │ ○ Pending   │ 2026-04-20 → 2026-04-28   ║
╚══════════════════════════╧═════════════╧═══════════════════════════╝

╔════════════════════════════════════════════════════════════════════╗
║ ISSUES & RISKS                                                     ║
╠════════════════════════════════════════════════════════════════════╣
║ ⚠️  Business Logic at risk: "Dependency on external service delayed"
║ 🚫  Frontend UI blocked: "Waiting on auth module completion"
║ ⏱️  Production Deployment: Will miss hard deadline (2026-04-20)
╚════════════════════════════════════════════════════════════════════╝

💡 Recommendations:
  • Fast-track Business Logic task to prevent further delays
  • Consider adding resources to Frontend UI once unblocked
  • Negotiate deadline extension or reduce scope for on-time delivery
```

---

## PART V: IMPLEMENTATION ROADMAP

### 5.1 Phased Development Plan

```
PHASE 1: FOUNDATION (Weeks 1-4) ✅ COMPLETE
├─ Domain Model (utf8proj-core)
├─ Parser (.proj DSL)
├─ Basic CPM Solver
├─ SVG Gantt Renderer
└─ CLI Scaffolding

PHASE 2: CPM CORRECTNESS (Weeks 5-6) ✅ COMPLETE
├─ DAG Separation from WBS
├─ 8 Invariant Tests
├─ Container Date Derivation
└─ Slack/Float Verification (247 tests passing, 83.97% coverage)

PHASE 3: PROGRESS TRACKING (Weeks 7-10) ← CURRENT FOCUS
├─ Week 7: Domain Model Extensions
│   ├─ Add progress fields to Task
│   ├─ Update Project for status_date
│   └─ Extend ScheduledTask with variance
│
├─ Week 8: Progress-Aware CPM
│   ├─ Effective duration calculation
│   ├─ Actual date pinning
│   └─ Forecast computation
│
├─ Week 9: CLI & Reporting
│   ├─ `utf8proj status` command
│   ├─ `utf8proj progress` command
│   └─ Progress dashboard renderer
│
└─ Week 10: Testing & Validation
    ├─ Progress integration tests
    ├─ Variance calculation tests
    └─ Documentation & examples

PHASE 4: HISTORY SYSTEM (Weeks 11-14)
├─ Week 11: History Providers
│   ├─ GitHistoryProvider (git2 crate)
│   ├─ SidecarHistoryProvider (YAML)
│   └─ EmbeddedHistoryProvider (comments)
│
├─ Week 12: Playback Engine
│   ├─ Version diff calculation
│   ├─ Impact analysis
│   └─ Animation rendering (GIF, HTML)
│
├─ Week 13: CLI Integration
│   ├─ `utf8proj history` command
│   ├─ `utf8proj playback` command
│   └─ `utf8proj snapshot` command
│
└─ Week 14: User Experience
    ├─ Auto-snapshot on save
    ├─ Conflict resolution UI
    └─ Integration tests

PHASE 5: ADVANCED FEATURES (Weeks 15-18)
├─ Week 15: EVM (Earned Value)
│   ├─ PV/EV/AC calculation
│   ├─ CPI/SPI metrics
│   └─ Forecasting (EAC, ETC)
│
├─ Week 16: Excel Export
│   ├─ rust_xlsxwriter integration
│   ├─ Formulas & charts
│   └─ Progress visualization
│
├─ Week 17: Resource Management
│   ├─ Over-allocation detection
│   ├─ Resource leveling heuristic
│   └─ Cost tracking
│
└─ Week 18: Integration & Polish
    ├─ Mermaid/PlantUML export
    ├─ iCal export
    └─ Documentation

PHASE 6: ECOSYSTEM (Months 5-6)
├─ WASM Build (browser compatibility)
├─ VS Code Extension
├─ GitHub Action
└─ Plugin System
```

### 5.2 Test-Driven Development Guidelines

Every feature follows this workflow:

1. **Write failing tests first** (tests/integration/)
2. **Implement minimal code** to pass tests
3. **Refactor** for clarity and performance
4. **Document** with examples and RFC updates

Example: Progress Tracking TDD

```rust
// Step 1: Write failing test
#[test]
fn task_with_50_percent_complete_has_half_remaining_work() {
    let proj = parse(r#"
        project "Test" { start: 2026-01-01 }
        task impl "Implementation" {
            duration: 10d
            complete: 50%
        }
    "#).unwrap();
    
    let schedule = ProgressAwareCpmScheduler::new().schedule(&proj).unwrap();
    
    assert_eq!(schedule.tasks["impl"].forecast_duration, Duration::days(5));
}

// Step 2: Implement (this test will fail until progress_cpm.rs is written)
// Step 3: Refactor
// Step 4: Document in RFC
```

---

## PART VI: AREAS REQUIRING DESIGN REFINEMENT

### 6.1 Confidence Assessment

| Component | Confidence | Status | Notes |
|-----------|-----------|---------|-------|
| **Domain Model** | 95% | ✅ Solid | Minor tweaks possible for EVM fields |
| **Parser (.proj DSL)** | 90% | ✅ Solid | Grammar complete, may add syntax sugar |
| **Basic CPM** | 98% | ✅ Verified | 247 tests passing, 8 invariants proven |
| **Progress-Aware CPM** | 75% | ⚠️ Needs Design | Algorithm details unclear (see survey Q1-Q5) |
| **Container Derivation** | 80% | ⚠️ Needs Clarity | Edge cases need definition (see Q6-Q8) |
| **History - Git Backend** | 85% | ⚠️ Design WIP | git2 integration straightforward |
| **History - Sidecar** | 70% | ⚠️ Format TBD | YAML vs JSON vs custom (see Q9-Q11) |
| **History - Embedded** | 60% | ⚠️ Complex | Comment parsing, merge conflicts (see Q12-Q14) |
| **Playback Engine** | 65% | ⚠️ Design Needed | Diff algorithm, impact metrics (see Q15-Q17) |
| **Excel Export** | 70% | ⚠️ Library Choice | rust_xlsxwriter vs alternatives (see Q18-Q20) |
| **Resource Leveling** | 65% | ⚠️ Algorithm TBD | Priority-based vs constraint-based (see Q21-Q23) |
| **BDD/SAT Integration** | 50% | ⚠️ Deferred? | Complexity vs value unclear (see Q24-Q26) |
| **TJP Compatibility** | 75% | ⚠️ Scope TBD | Which TJP features to support (see Q27-Q29) |

**Overall Confidence: 85%** (solid foundation, key design decisions needed)

---

## PART VII: DESIGN REFINEMENT SURVEY

**Instructions for Companion LLM Designer:**

This survey identifies areas where the design needs refinement to reach >95% confidence. Please provide detailed answers with:
1. Technical rationale
2. Trade-off analysis
3. Recommended approach
4. Implementation notes
5. Test strategy

---

### SECTION A: Progress-Aware CPM Algorithm

**Q1. Effective Duration Calculation for Partially Complete Tasks**

*Current Design:*
```rust
let remaining_pct = (100 - pct_complete) as f64 / 100.0;
effective_duration = original_duration.mul_f64(remaining_pct);
```

*Questions:*
- Is linear interpolation always correct? (e.g., 50% complete ≠ always 50% time elapsed)
- Should we support non-linear progress curves?
- What about tasks that are "80% done but 50% remaining" (schedule risk)?
- How to handle `remaining_effort` override?

**Recommended design:** [To be filled by companion LLM]

---

**Q2. Handling Dependencies with Partial Completion**

*Scenario:*
```
Task A: 20d duration, 50% complete (10d done, 10d remaining)
Task B: depends on A (finish-to-start)
```

*Questions:*
- Does B wait for A's full remaining 10d, or can it start earlier?
- Should we support "start when predecessor is X% complete" dependencies?
- How does this affect critical path calculation?

**Recommended design:** [To be filled by companion LLM]

---

**Q3. Actual Dates vs Dependencies Conflict Resolution**

*Scenario:*
```
Task A: actual_finish = 2026-02-15
Task B: depends on A, actual_start = 2026-02-10  # Started BEFORE A finished!
```

*Questions:*
- Is this an error, or valid (e.g., parallel work)?
- Should solver warn, error, or adjust?
- How to handle in backward pass?

**Recommended design:** [To be filled by companion LLM]

---

**Q4. Resource Leveling with Partial Completion**

*Scenario:*
```
Resource "dev" has capacity=1
Task A: 50% complete, assigned to dev, 5d remaining
Task B: 0% complete, assigned to dev, 10d duration
```

*Questions:*
- Does A get priority (already started)?
- Can leveling shift partially-complete tasks?
- How to handle over-allocation in the past (before status_date)?

**Recommended design:** [To be filled by companion LLM]

---

**Q5. Container Tasks with Mixed Child Progress**

*Scenario:*
```
Container "Development" {
    Task "Frontend": 100% complete
    Task "Backend": 50% complete
    Task "Testing": 0% complete
}
```

*Questions:*
- What is the container's percent_complete? (weighted average?)
- How to derive container's actual_start (earliest child start?)
- How to derive actual_finish (all children complete?)

**Recommended design:** [To be filled by companion LLM]

---

### SECTION B: Container Task Derivation

**Q6. Container with Both Duration Attribute AND Children**

*Scenario:*
```proj
task project "Project" {
    duration: 30d    # Explicit duration
    
    task phase1 { duration: 10d }
    task phase2 { duration: 15d, depends: phase1 }
}
```

*Questions:*
- Is this valid or an error?
- If valid, which takes precedence?
- Should we warn or ignore?

**Recommended design:** [To be filled by companion LLM]

---

**Q7. Container Progress vs Leaf Progress**

*Scenario:*
```proj
task development "Development" {
    complete: 60%    # User sets container progress directly
    
    task frontend { complete: 100% }
    task backend { complete: 20% }
}
```

*Questions:*
- Does user-set container progress override calculation?
- Should we validate consistency?
- How to handle conflicts?

**Recommended design:** [To be filled by companion LLM]

---

**Q8. Empty Containers**

*Scenario:*
```proj
task future_phase "Future Work" {
    # No children, no duration
}
```

*Questions:*
- Is this valid (placeholder)?
- How to schedule it?
- Should it appear in outputs?

**Recommended design:** [To be filled by companion LLM]

---

### SECTION C: History System - Sidecar Format

**Q9. Sidecar File Format Choice**

*Options:*
1. **YAML** - Human-readable, widely supported
2. **JSON** - Machine-readable, strict schema
3. **Custom format** - Optimized for diffs

*Questions:*
- Which format best balances readability vs parsing performance?
- Should we support multiple formats?
- How to version the format itself?

**Recommended design:** [To be filled by companion LLM]

---

**Q10. Sidecar File Structure**

*Proposed:*
```yaml
# project.proj.history
format_version: 1.0
snapshots:
  - id: "snapshot-1"
    timestamp: "2026-01-15T09:00:00Z"
    author: "alice"
    message: "Initial version"
    content: |
      [full .proj file content]
  
  - id: "snapshot-2"
    timestamp: "2026-02-01T14:30:00Z"
    author: "bob"
    message: "Added testing phase"
    parent: "snapshot-1"
    diff: |
      +task testing { duration: 10d }
```

*Questions:*
- Full content vs diff-based storage?
- How to handle large projects (compression)?
- Limit on number of snapshots?

**Recommended design:** [To be filled by companion LLM]

---

**Q11. Sidecar File Synchronization**

*Questions:*
- When to write sidecar file (on every save, on explicit snapshot)?
- How to handle concurrent edits?
- Should sidecar be committed to Git?

**Recommended design:** [To be filled by companion LLM]

---

### SECTION D: History System - Embedded Format

**Q12. Embedded History Comment Format**

*Proposed:*
```proj
# --- UTF8PROJ HISTORY ---
# Format: embedded-1.0
# Snapshot: 2026-01-15T09:00:00Z by alice
# Message: Initial version
# [compressed/base64 encoded content]

# Snapshot: 2026-02-01T14:30:00Z by bob
# Message: Added testing
# Diff: +task testing { duration: 10d }
# --- END HISTORY ---

project "My Project" {
    # ... actual project content ...
}
```

*Questions:*
- How to efficiently parse/skip history section?
- Max size before switching to sidecar?
- How to handle in text editors?

**Recommended design:** [To be filled by companion LLM]

---

**Q13. Embedded History Merge Conflicts**

*Scenario:* Two users edit same project, Git merge conflict in history section

*Questions:*
- Auto-merge histories or require manual resolution?
- Can we use Git's conflict markers?
- How to preserve all snapshots?

**Recommended design:** [To be filled by companion LLM]

---

**Q14. Embedded History Performance**

*Questions:*
- Performance impact of parsing large history sections?
- Lazy loading strategy?
- When to auto-migrate to sidecar?

**Recommended design:** [To be filled by companion LLM]

---

### SECTION E: Playback Engine

**Q15. Diff Algorithm for Schedule Changes**

*Questions:*
- What constitutes a "meaningful change" for playback?
- How to detect renamed tasks vs deleted+added?
- How to track dependency changes?
- Algorithm: Myers diff, patience diff, or custom?

**Recommended design:** [To be filled by companion LLM]

---

**Q16. Impact Metrics for Playback**

*Proposed:*
```rust
struct ScheduleImpact {
    project_duration_delta: Duration,
    critical_path_changed: bool,
    tasks_added: Vec<TaskId>,
    tasks_removed: Vec<TaskId>,
    progress_delta: HashMap<TaskId, i8>, // -5% to +100%
}
```

*Questions:*
- What other metrics are valuable?
- How to visualize impact in animation?
- Threshold for "significant change"?

**Recommended design:** [To be filled by companion LLM]

---

**Q17. Playback Animation Format**

*Options:*
1. **GIF** - Universal, no interactivity
2. **SVG sequence** - High quality, manual playback
3. **HTML+JS** - Interactive controls, browser-based
4. **Video (MP4)** - Professional presentation

*Questions:*
- Which format(s) to prioritize?
- Frame rate / smoothness trade-offs?
- How to annotate changes in animation?

**Recommended design:** [To be filled by companion LLM]

---

### SECTION F: Excel Export

**Q18. Rust Excel Library Choice**

*Options:*
1. **rust_xlsxwriter** - Pure Rust, actively maintained, formulas supported
2. **calamine** - Read-only, not suitable
3. **umya-spreadsheet** - Supports .xlsx, less mature

*Questions:*
- Which library best supports formulas?
- Chart generation support?
- Performance for 1000+ task projects?

**Recommended design:** [To be filled by companion LLM]

---

**Q19. Excel Sheet Structure**

*Proposed:*
```
Sheet 1: Task List
  - Columns: ID, Name, Start, Finish, Duration, Progress, Variance
  - Formulas: =IF(Actual_Finish<>"", Actual_Finish, Forecast_Finish)

Sheet 2: Gantt Chart
  - Conditional formatting for progress bars
  - Critical path highlighted

Sheet 3: Resource Allocation
  - Resource name, assigned tasks, utilization%

Sheet 4: Variance Report
  - Planned vs Actual vs Forecast
```

*Questions:*
- Best sheet organization?
- Which formulas are most valuable?
- How to update from utf8proj without losing manual edits?

**Recommended design:** [To be filled by companion LLM]

---

**Q20. Excel Chart Generation**

*Questions:*
- Generate charts in Rust or rely on Excel?
- Which chart types (Gantt, resource histogram, burndown)?
- Static vs template-based approach?

**Recommended design:** [To be filled by companion LLM]

---

### SECTION G: Resource Leveling

**Q21. Resource Leveling Algorithm**

*Options:*
1. **Priority-based** - Simple, fast, greedy
2. **Critical path** - Prioritize critical tasks
3. **Constraint-based** - Optimize using constraints
4. **Hybrid** - Combine approaches

*Questions:*
- Which algorithm for v1.0?
- Trade-offs (speed vs optimality)?
- Integration with progress tracking?

**Recommended design:** [To be filled by companion LLM]

---

**Q22. Over-Allocation Handling**

*Scenario:*
```
Resource "dev" capacity=1
Task A: assign dev, 2026-02-01..2026-02-10
Task B: assign dev, 2026-02-05..2026-02-15  # Overlap!
```

*Questions:*
- Shift task B later (if float available)?
- Warn but allow?
- Error and refuse to schedule?

**Recommended design:** [To be filled by companion LLM]

---

**Q23. Resource Leveling with Constraints**

*Questions:*
- How to level when tasks have hard deadlines?
- Can leveling add resources (auto-suggest)?
- Integration with what-if analysis?

**Recommended design:** [To be filled by companion LLM]

---

### SECTION H: BDD/SAT Integration

**Q24. BDD/SAT Necessity for MVP**

*Questions:*
- Is BDD/SAT essential for v1.0?
- What use cases require it vs nice-to-have?
- Complexity vs value trade-off?
- Should we defer to Phase 6 (Ecosystem)?

**Recommended design:** [To be filled by companion LLM]

---

**Q25. BDD Encoding of Constraints**

*If we implement BDD:*

*Questions:*
- How to encode task start/finish dates as BDD variables?
- How to encode dependencies efficiently?
- How to encode resource capacity?
- Performance characteristics (time, memory)?

**Recommended design:** [To be filled by companion LLM]

---

**Q26. BDD What-If Analysis**

*Questions:*
- What insights does BDD provide beyond heuristic solver?
- How to present "number of valid schedules" to users?
- Is constraint criticality analysis valuable?

**Recommended design:** [To be filled by companion LLM]

---

### SECTION I: TaskJuggler Compatibility

**Q27. TJP Feature Subset**

*TaskJuggler has 100+ features. Which to support?*

*Tier 1 (Must Have):*
- [ ] Tasks, resources, dependencies
- [ ] Effort-based scheduling
- [ ] Calendars, holidays
- [ ] Reports (task list, resource allocation)

*Tier 2 (Should Have):*
- [ ] Scenarios (what-if)
- [ ] Resource efficiency
- [ ] Limits (max/min dates)
- [ ] Flags, shifts

*Tier 3 (Nice to Have):*
- [ ] Bookings
- [ ] Accounts, cost tracking
- [ ] Journal entries
- [ ] Complex reports (rich_text, HTML)

*Questions:*
- Which tiers for v1.0?
- Migration path for unsupported features?

**Recommended design:** [To be filled by companion LLM]

---

**Q28. TJP Parser Strategy**

*Options:*
1. **Full grammar** - Parse everything, ignore unsupported
2. **Subset grammar** - Parse only supported features
3. **Hybrid** - Parse all, warn on unsupported

*Questions:*
- How to handle unsupported features gracefully?
- Should we maintain bug-for-bug compatibility?
- How to test compatibility without copying TJ tests?

**Recommended design:** [To be filled by companion LLM]

---

**Q29. TJP to .proj Conversion**

*Questions:*
- Should `utf8proj import project.tjp` convert to .proj?
- How to preserve unsupported features (comments?)?
- Round-trip guarantee (.tjp → .proj → .tjp)?

**Recommended design:** [To be filled by companion LLM]

---

## APPENDIX A: GLOSSARY

**CPM (Critical Path Method):** Algorithm to identify longest path through task network  
**DAG (Directed Acyclic Graph):** Task dependency graph with no cycles  
**WBS (Work Breakdown Structure):** Hierarchical decomposition of project into tasks  
**EVM (Earned Value Management):** Project performance measurement technique  
**Float/Slack:** Amount a task can be delayed without affecting project end date  
**Baseline:** Approved project plan used for comparison  
**Status Date:** Current date for progress reporting  
**Forecast:** Predicted future dates based on progress to date  
**Variance:** Difference between planned and forecast/actual  

---

## APPENDIX B: SUCCESS CRITERIA

utf8proj v1.0 is considered successful when:

**Technical:**
- [ ] All 8 CPM invariants pass for 1000+ task projects
- [ ] Progress tracking correctly calculates remaining work
- [ ] History playback generates smooth animations
- [ ] Excel export opens in MS Excel without errors
- [ ] Test coverage >85%, all integration tests pass

**User Experience:**
- [ ] Non-technical PM can update progress via CLI
- [ ] Developer can `git diff` schedule changes
- [ ] Gantt chart renders in <1s for 1000 tasks
- [ ] Documentation covers all features with examples

**Adoption:**
- [ ] 50+ GitHub stars in first 3 months
- [ ] 3+ production users with public projects
- [ ] 5+ community contributions (PRs, issues, discussions)

---

**END OF MASTER RFC**

This document will be updated as design decisions are finalized through the survey process.
