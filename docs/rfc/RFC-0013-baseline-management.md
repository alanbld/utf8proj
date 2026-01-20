# RFC-0013: Baseline Management

**RFC Number:** 0013
**Status:** Implemented (v0.11.0)
**Created:** 2026-01-20
**Revised:** 2026-01-20
**Author:** utf8proj contributors
**Related:** RFC-0008 (Progress-Aware CPM), RFC-0012 (Temporal Regimes), RFC-0004 (Cost Ranges)
**Target Version:** 0.11.0

---

## Executive Summary

This RFC introduces **Baseline Management** to utf8proj: the ability to capture frozen schedule snapshots and compare the current schedule against them. Baselines answer the fundamental project question: **"Compared to what?"**

**Core design decisions:**
- Two-file architecture: project file + sidecar `.baselines` file
- Same .proj syntax for both files
- CLI-managed (users don't manually edit baselines)
- Immutable snapshots (delete and recreate, never update)
- Leaf tasks only (containers are derived)
- Stable task IDs required

---

## 1. Problem Statement

### 1.1 The Missing Reference Frame

utf8proj can currently answer:
- "When will tasks finish?" → CPM scheduling ✅
- "Where are we now?" → Progress tracking (RFC-0008) ✅
- "What will it cost?" → Cost ranges (RFC-0004) ✅

utf8proj **cannot** answer:
- "Are we ahead or behind schedule?"
- "How much has the project slipped?"
- "What changed since we committed?"

These questions require a **reference frame** — a frozen schedule state to compare against.

### 1.2 Real-World Usage

| Baseline | Purpose |
|----------|---------|
| `original` | Initial approved plan |
| `contract` | Promised to client |
| `change_order_1` | After scope change |
| `current` | Latest re-baseline |

### 1.3 Foundation for Future Work

Baselines are prerequisite infrastructure for:
- Schedule Variance Analysis (this RFC)
- Earned Value Management (future RFC)
- Forecasting and trend analysis (future RFC)

---

## 2. Design Principles

1. **Separation of concerns**: Current plan ≠ historical snapshots
2. **Syntax consistency**: Baselines use .proj syntax
3. **CLI-first**: Users interact via commands, not file editing
4. **Git-friendly**: Separate files enable focused diffs
5. **Stable references**: Tasks identified by ID, not position
6. **Immutability**: Baselines are frozen; replace, never update

---

## 3. Normative Definitions

> These definitions are **authoritative** and govern all baseline behavior.

### 3.1 What a Baseline Captures

**A baseline captures the computed scheduling result (output), not the planning inputs.**

Specifically, a baseline stores:
- Per-task: Early Start, Early Finish (as computed by CPM)
- Project-level: Project finish date, baseline metadata

A baseline does **not** store:
- Task durations, effort, or constraints
- Calendar definitions
- Resource assignments
- The full scheduling input state

**Implication:** Baselines are results, not re-runnable plans. Comparison is always output-to-output.

### 3.2 What Gets Baselined

**Only leaf tasks are baselined. Container tasks are never stored in baselines.**

Rationale:
- Container dates are derived from children (not authoritative)
- Including containers creates double-counting in variance analysis
- EV calculations require unambiguous task identity
- Keeps baseline files minimal and unambiguous

### 3.3 Baseline Immutability

**Baselines are immutable snapshots. They cannot be updated, only deleted and recreated.**

There is no `--force` flag. To replace a baseline:
```bash
utf8proj baseline remove --name original project.proj
utf8proj baseline save --name original project.proj
```

Rationale:
- Overwriting history silently breaks audit guarantees
- Explicit delete+recreate makes the action visible in Git
- Prevents accidental overwrites

### 3.4 Scheduling Semantics Dependency

**Baselines are interpreted under the scheduling semantics of the utf8proj version performing the comparison.**

Differences in scheduling rules between versions (calendar handling, constraint rounding, progress-aware adjustments) may affect variance interpretation.

Future enhancement: `engine_version` metadata field for explicit tracking.

### 3.5 Variance Computation Reference Point

**Variance is computed by comparing the current scheduled Early Finish (after progress and constraints are applied) to the baseline Early Finish.**

```
variance_days = current_early_finish - baseline_early_finish
```

- Positive variance = delayed
- Negative variance = ahead
- Zero variance = on schedule

### 3.6 Scope Freeze Semantics

**Tasks added after a baseline do not contribute to baseline-derived metrics.**

- Added tasks are reported as "Added" in comparison
- They have no baseline to compare against
- For future EV: added tasks have PV=0 in that baseline context

**Tasks removed after a baseline are reported as "Removed".**

- They appear in baseline but not in current schedule
- For future EV: represents scope reduction

### 3.7 Parent Baseline Semantics

**The `parent` attribute expresses conceptual lineage only.**

Each baseline is a **complete snapshot**. No inheritance or delta semantics are applied. The parent field is metadata for human understanding, not computation.

```proj
baseline change_order_1 {
    parent: original    # "This baseline evolved from original"
    # All tasks listed - NOT a delta from original
}
```

### 3.8 Task ID Qualification

**Baseline task identifiers MUST be globally unique within the project.**

- If a task declares an explicit `id:`, that ID is used as-is
- If a task does not declare an explicit `id:`, utf8proj derives a **fully-qualified ID** by concatenating ancestor task IDs with `.` (dot notation)

**Example:**
```proj
task phase1 "Phase 1" {
    task design "Design" { duration: 5d }
}

task phase2 "Phase 2" {
    task design "Design" { duration: 7d }
}
```

Derived baseline IDs:
- `phase1.design`
- `phase2.design`

**Rationale:**
- Prevents ID collisions in hierarchical projects
- Preserves baseline integrity across refactoring and reparenting
- Aligns with auditability and earned value requirements

**Note:** Fully-qualified IDs are internal/baseline identity; display names and CLI output may remain user-friendly. Explicit `id:` attributes (Phase 2) override qualification.

---

## 4. Architecture

### 4.1 File Structure

```
project/
├── plan.proj                 # Current plan + progress (human-edited)
└── plan.proj.baselines       # Historical snapshots (CLI-managed)
```

**Rationale for two files:**
- Main file stays focused on current planning
- Baseline changes don't pollute schedule diffs
- Files grow independently
- Can .gitignore baselines if desired (not recommended)

### 4.2 Task Identity Requirement

Tasks must have stable identifiers for reliable baseline matching.

```proj
# Recommended: Explicit ID
task "Design Phase" {
    id: design              # Stable identifier
    duration: 10d
}

# Acceptable: Inferred ID (with diagnostic)
task design "Design Phase" {  # Inferred ID: "design"
    duration: 10d
}
```

**ID Resolution Order:**
1. Explicit `id:` field if present
2. Task's declared name (identifier after `task` keyword)
3. Warning if neither: task cannot be reliably baselined

**Namespace:** Task IDs and baseline names occupy **separate namespaces**. A baseline named "design" does not conflict with a task ID "design".

### 4.3 Date Representation

**Task dates are calendar dates (NaiveDate), not instants.**

```proj
# Dates are YYYY-MM-DD without time zone
design: 2026-01-01 -> 2026-01-10
```

**Metadata timestamps are UTC instants (DateTime<Utc>).**

```proj
saved: 2026-01-15T10:30:00Z  # UTC timestamp
```

---

## 5. Baseline File Format

### 5.1 Grammar

```pest
baseline_file = { SOI ~ baseline_block* ~ EOI }

baseline_block = {
    "baseline" ~ identifier ~ "{" ~
    baseline_meta ~
    task_snapshot* ~
    "}"
}

baseline_meta = {
    saved_attr ~
    description_attr? ~
    parent_attr?
}

saved_attr = { "saved:" ~ iso8601_datetime }
description_attr = { "description:" ~ quoted_string }
parent_attr = { "parent:" ~ identifier }

task_snapshot = { identifier ~ ":" ~ date ~ "->" ~ date }
```

### 5.2 Example File

```proj
# plan.proj.baselines
# Auto-generated by utf8proj. Manual edits not recommended.

baseline original {
    saved: 2026-01-15T10:30:00Z
    description: "Initial approved plan"

    design: 2026-01-01 -> 2026-01-10
    build: 2026-01-11 -> 2026-02-15
    test: 2026-02-16 -> 2026-02-28
}

baseline change_order_1 {
    saved: 2026-02-01T14:20:00Z
    description: "Added security audit phase"
    parent: original

    design: 2026-01-01 -> 2026-01-12
    build: 2026-01-13 -> 2026-02-20
    security_audit: 2026-02-21 -> 2026-02-28
    test: 2026-03-01 -> 2026-03-10
}
```

### 5.3 Output Ordering

**Task snapshots are ordered by task ID (lexicographic).**

This ensures:
- Deterministic file output
- Clean Git diffs
- Consistent CLI output

---

## 6. CLI Interface

### 6.1 Save Baseline

```bash
utf8proj baseline save --name <name> [OPTIONS] <PROJECT_FILE>

Options:
    --name <name>           Required. Baseline identifier (alphanumeric + underscore)
    --description <text>    Optional. Human-readable description
    --parent <name>         Optional. Link to predecessor baseline

Examples:
    utf8proj baseline save --name original plan.proj
    utf8proj baseline save --name v2 --description "After scope change" --parent original plan.proj
```

**Behavior:**
1. Schedule project (if needed)
2. Extract leaf task IDs and Early Start/Finish dates
3. Emit error if baseline name already exists (no implicit overwrite)
4. Create/append to `.baselines` file
5. Output summary

**Output:**
```
✓ Baseline "original" saved
  Leaf tasks: 45
  Project finish: 2026-03-01
  File: plan.proj.baselines
```

### 6.2 Compare Against Baseline

```bash
utf8proj compare --baseline <name> [OPTIONS] <PROJECT_FILE>

Options:
    --baseline <name>       Required. Baseline to compare against
    --format text|csv|json  Output format (default: text)
    --show-unchanged        Include zero-variance tasks
    --threshold <days>      Minimum variance to show

Examples:
    utf8proj compare --baseline original plan.proj
    utf8proj compare --baseline original --format csv plan.proj > variance.csv
```

**Text Output:**
```
Schedule Variance vs "original" (saved 2026-01-15)

Task                 Baseline Finish   Current Finish   Variance
design               2026-01-10        2026-01-12       +2d ⚠️
build                2026-02-15        2026-02-20       +5d ⚠️
test                 2026-02-28        2026-03-05       +5d ⚠️
[+] security_audit   -                 2026-02-28       (added)

Summary:
  Compared: 3 tasks
  On schedule: 0
  Delayed: 3
  Ahead: 0
  Added: 1
  Removed: 0
  Project slip: +7 days
```

**JSON Output:**
```json
{
  "baseline": "original",
  "baseline_saved": "2026-01-15T10:30:00Z",
  "comparison_date": "2026-01-20T09:00:00Z",
  "tasks": [
    {
      "id": "design",
      "status": "delayed",
      "baseline_finish": "2026-01-10",
      "current_finish": "2026-01-12",
      "variance_days": 2
    },
    {
      "id": "security_audit",
      "status": "added",
      "baseline_finish": null,
      "current_finish": "2026-02-28",
      "variance_days": null
    }
  ],
  "summary": {
    "compared": 3,
    "delayed": 3,
    "on_schedule": 0,
    "ahead": 0,
    "added": 1,
    "removed": 0,
    "project_variance_days": 7
  }
}
```

### 6.3 List Baselines

```bash
utf8proj baseline list <PROJECT_FILE>

Output:
  Name              Saved                Tasks   Project Finish
  original          2026-01-15 10:30     45      2026-03-01
  change_order_1    2026-02-01 14:20     48      2026-03-15
```

### 6.4 Remove Baseline

```bash
utf8proj baseline remove --name <name> <PROJECT_FILE>

# Requires confirmation
utf8proj baseline remove --name original plan.proj
⚠️ Remove baseline "original"? This cannot be undone. [y/N]
```

### 6.5 Show Baseline Details

```bash
utf8proj baseline show --name <name> <PROJECT_FILE>

Output:
  Baseline: original
  Saved: 2026-01-15T10:30:00Z
  Description: Initial approved plan
  Parent: (none)
  Tasks: 45
  Project finish: 2026-03-01

  First 10 tasks:
    design: 2026-01-01 -> 2026-01-10
    build: 2026-01-11 -> 2026-02-15
    ...
```

---

## 7. Data Model

### 7.1 Core Types

```rust
/// A named schedule snapshot
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Baseline {
    pub name: String,
    pub saved: DateTime<Utc>,
    pub description: Option<String>,
    pub parent: Option<String>,
    pub tasks: BTreeMap<String, TaskSnapshot>,  // Sorted by ID
    pub project_finish: NaiveDate,
}

/// Snapshot of a leaf task's scheduled dates
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub task_id: String,
    pub start: NaiveDate,   // Early Start
    pub finish: NaiveDate,  // Early Finish
}

/// Collection of baselines for a project
#[derive(Clone, Debug, Default)]
pub struct BaselineStore {
    pub baselines: BTreeMap<String, Baseline>,  // Sorted by name
}
```

### 7.2 Comparison Types

```rust
pub struct ScheduleComparison {
    pub baseline_name: String,
    pub baseline_saved: DateTime<Utc>,
    pub tasks: Vec<TaskVariance>,
    pub summary: ComparisonSummary,
}

pub struct TaskVariance {
    pub task_id: String,
    pub baseline_start: Option<NaiveDate>,
    pub baseline_finish: Option<NaiveDate>,
    pub current_start: NaiveDate,
    pub current_finish: NaiveDate,
    pub start_variance_days: Option<i32>,
    pub finish_variance_days: Option<i32>,
    pub status: VarianceStatus,
}

pub enum VarianceStatus {
    OnSchedule,     // finish_variance == 0
    Delayed,        // finish_variance > 0
    Ahead,          // finish_variance < 0
    Added,          // Not in baseline
    Removed,        // Not in current schedule
}

pub struct ComparisonSummary {
    pub tasks_compared: usize,
    pub tasks_on_schedule: usize,
    pub tasks_delayed: usize,
    pub tasks_ahead: usize,
    pub tasks_added: usize,
    pub tasks_removed: usize,
    pub baseline_project_finish: NaiveDate,
    pub current_project_finish: NaiveDate,
    pub project_variance_days: i32,
}
```

---

## 8. Diagnostics

| Code | Severity | Message |
|------|----------|---------|
| B001 | Info | Baseline "{name}" saved ({n} tasks) |
| B002 | Warning | Task "{id}" lacks explicit ID; using inferred "{inferred}" |
| B003 | Error | Baseline "{name}" already exists |
| B004 | Error | Baseline "{name}" not found |
| B005 | Info | Task "{id}" in baseline not found in current schedule (removed) |
| B006 | Info | Task "{id}" not in baseline (added since baseline) |
| B007 | Warning | No baselines file found for this project |
| B008 | Warning | Container task "{id}" excluded from baseline (only leaf tasks baselined) |
| B009 | Error | Cannot baseline: {n} tasks have no ID |

---

## 9. Integration Points

### 9.1 With Progress Tracking (RFC-0008)

**Baselines and progress are orthogonal:**
- Baselines = what we planned (historical)
- Progress = where we are (current)
- Comparison uses current scheduled dates (which incorporate progress)

### 9.2 With Temporal Regimes (RFC-0012)

Baseline dates reflect regime-appropriate scheduling:
- Event regime tasks: exact dates preserved
- Work regime tasks: working-day adjusted dates
- Comparison respects these distinctions

### 9.3 With Cost Tracking (RFC-0004)

**Phase 2 extension point:**
```proj
baseline detailed {
    saved: 2026-01-15T10:30:00Z

    design: 2026-01-01 -> 2026-01-10 {
        cost: 5000.00
        effort: 80h
    }
}
```

### 9.4 Future: Earned Value

With baselines + cost extension:
- **Planned Value (PV)** = Baseline cost at status date
- **Earned Value (EV)** = Baseline cost × progress%
- **Schedule Variance (SV)** = EV - PV

Note: Tasks added after baseline have PV=0 in that baseline context.

---

## 10. Comparison with Industry Tools

| Aspect | MS Project | TaskJuggler | **utf8proj** |
|--------|------------|-------------|--------------|
| Storage | Embedded fields | Implicit versions | **Separate file** |
| Task identity | Fragile (positional) | Stable IDs | **Stable IDs** |
| Immutability | Mutable (reset) | Version-based | **Immutable snapshots** |
| Syntax | Binary/XML | Custom DSL | **Same .proj syntax** |
| Container handling | Baselined | Not applicable | **Excluded (leaf only)** |
| Git integration | Poor | Good | **Native (separate file)** |

**Key difference from MS Project:** utf8proj baselines are immutable, standalone artifacts in a separate file. MS Project embeds mutable baseline fields in each task, creating bloat and sync issues.

**Key difference from TaskJuggler:** utf8proj baselines are explicitly named and managed. TaskJuggler relies on implicit version/scenario comparisons.

---

## 11. Implementation Plan

### Phase 1: Core (v0.11.0)

**Week 1: Grammar & Parser**
- [ ] Add `id:` attribute to task grammar
- [ ] Create baseline file grammar
- [ ] Implement parser/serializer
- [ ] Round-trip tests

**Week 2: Core Types & Engine**
- [ ] `Baseline`, `TaskSnapshot`, `BaselineStore` structs
- [ ] `compare()` function with variance calculation
- [ ] Leaf-task filtering
- [ ] Unit tests

**Week 3: CLI Commands**
- [ ] `baseline save`, `list`, `remove`, `show`
- [ ] `compare --baseline`
- [ ] Output formatters (text, CSV, JSON)
- [ ] Integration tests

**Week 4: Polish**
- [ ] B001-B009 diagnostics
- [ ] Documentation
- [ ] Example projects
- [ ] Performance testing

### Phase 2: Enhanced (v0.12.0+)

- [ ] Cost/effort capture
- [ ] `engine_version` metadata
- [ ] Git commit hash linking
- [ ] Baseline verification

### Phase 3: Advanced (Future)

- [ ] Visual Gantt overlay
- [ ] Baseline diff (compare two baselines)
- [ ] Multi-project coordination

---

## 12. Performance Requirements

| Operation | 100 tasks | 1,000 tasks |
|-----------|-----------|-------------|
| Save baseline | <100ms | <500ms |
| Load baselines | <50ms | <200ms |
| Compare | <50ms | <300ms |

---

## 13. Resolved Questions

| Question | Resolution |
|----------|------------|
| What is baselined? | **Outputs (computed dates), not inputs** |
| Container tasks? | **Excluded (leaf tasks only)** |
| Parent semantics? | **Lineage metadata only, not deltas** |
| Can baselines be updated? | **No. Delete and recreate.** |
| Variance reference point? | **Current Early Finish vs Baseline Early Finish** |
| Added tasks in EV? | **PV=0 for that baseline context** |
| Nested task IDs? | **Fully-qualified (e.g., `phase1.design`)** |

---

## 14. Success Criteria

- [ ] Baseline save/load round-trips correctly
- [ ] Comparison identifies added/removed/changed tasks
- [ ] Variance matches manual calculation
- [ ] No `--force` flag (immutability preserved)
- [ ] Only leaf tasks in baselines
- [ ] Clear documentation with examples

---

## 15. References

- RFC-0008: Progress-Aware CPM
- RFC-0012: Temporal Regimes
- RFC-0004: Progressive Resource Refinement & Cost Ranges
- PMI PMBOK Guide: Baseline Management
- MS Project (counter-example for embedded mutable baselines)

---

## Changelog

| Date | Change |
|------|--------|
| 2026-01-20 | Initial draft |
| 2026-01-20 | Added normative definitions per review feedback |
| 2026-01-20 | Resolved container/immutability/variance questions |
| 2026-01-20 | Removed --force flag (explicit delete+recreate) |
| 2026-01-20 | Added Section 3.8: Task ID Qualification (fully-qualified IDs) |

---

**Document Version:** 0.4
**Status:** Draft — Ready for implementation
