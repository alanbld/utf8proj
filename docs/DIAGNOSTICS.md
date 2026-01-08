# utf8proj Diagnostic Specifications

This document specifies the diagnostic messages emitted by utf8proj during scheduling and analysis. These diagnostics are the user-facing contract — implementations must use these exact messages.

## Design Principles

1. **Actionable**: Every diagnostic suggests what to do
2. **Contextual**: Messages include task/resource identifiers
3. **Graduated**: Severity reflects actual risk, not pedantry
4. **Machine-readable**: Codes are stable for tooling integration

## Severity Levels

| Level | Code Prefix | Meaning | `--strict` Behavior |
|-------|-------------|---------|---------------------|
| Error | `E`, `C001-C009` | Cannot proceed | Always fatal |
| Warning | `W`, `C010-C019` | Likely problem | Becomes error |
| Hint | `H`, `C020-C029` | Suggestion | Becomes warning |
| Info | `I` | Informational | Unchanged |

## Emission Ordering

For determinism and testability, diagnostics are emitted in this order:

1. **Structural errors** (E001, E002, E003) - fatal issues first
2. **Calendar errors** (C001, C002) - configuration issues
3. **Cost-related warnings** (W002, W004) - budget risk
4. **Assignment-related warnings** (W001, W003) - planning gaps
5. **Calendar warnings** (C010, C011) - scheduling conflicts
6. **MS Project compatibility warnings** (W014) - migration issues
7. **Hints** (H001, H002, H003, H004) - suggestions
8. **Calendar hints** (C020, C022, C023) - calendar suggestions
9. **Info** (I001, I002, I003, I004, I005) - summary last

Within each category, diagnostics are ordered by source location (file, line, column).

This ordering ensures:
- Users see blocking issues immediately
- CI logs are diffable
- LSP clients receive stable sequences

## Diagnostic Catalog

---

### W001: Abstract Assignment

**Severity**: Warning

**Trigger**: Task is assigned to a `resource_profile` (abstract) rather than a concrete `resource`.

**Condition**:
```
task.assignments.any(a => project.profiles.contains(a.resource_id))
```

**Message Template**:
```
warning[W001]: task '{task_id}' is assigned to abstract profile '{profile_id}'
  --> {file}:{line}
   |
   | assign: {profile_id}
   |         ^^^^^^^^^^^^ abstract profile, not a concrete resource
   |
   = note: cost range is ${min} - ${max} ({spread}% spread)
   = hint: assign a concrete resource to lock in exact cost
```

**Example**:
```
warning[W001]: task 'api_development' is assigned to abstract profile 'backend_developer'
  --> crm_migration.proj:45
   |
   | assign: backend_developer
   |         ^^^^^^^^^^^^^^^^^ abstract profile, not a concrete resource
   |
   = note: cost range is $4,000 - $8,000 (100% spread)
   = hint: assign a concrete resource to lock in exact cost
```

**Rationale**: Abstract assignments are valid but represent planning uncertainty. Users should be aware when their schedule contains unresolved abstractions.

---

### W002: Wide Cost Range

**Severity**: Warning

**Trigger**: A task's cost range spread exceeds threshold (default: 50%).

**Condition**:
```
let spread = (cost_range.max - cost_range.min) / cost_range.expected * 100;
spread > threshold
```

**Message Template**:
```
warning[W002]: task '{task_id}' has wide cost uncertainty ({spread}% spread)
  --> {file}:{line}
   |
   = cost range: ${min} - ${max} (expected: ${expected})
   = contributors:
       - {profile_id}: ${profile_min} - ${profile_max}
       - {trait_id} multiplier: {multiplier}x
   |
   = hint: narrow the profile rate range or assign concrete resources
```

**Example**:
```
warning[W002]: task 'data_migration' has wide cost uncertainty (120% spread)
  --> crm_migration.proj:67
   |
   = cost range: $12,000 - $26,400 (expected: $19,200)
   = contributors:
       - senior_developer: $150 - $250/day
       - contractor trait: 1.2x multiplier
   |
   = hint: narrow the profile rate range or assign concrete resources
```

**Configuration**:
```
--cost-spread-threshold=50  # percentage, default 50
```

---

### W003: Unknown Trait

**Severity**: Warning

**Trigger**: A profile references a trait that is not defined.

**Condition**:
```
profile.traits.any(t => !project.traits.contains(t))
```

**Message Template**:
```
warning[W003]: profile '{profile_id}' references unknown trait '{trait_id}'
  --> {file}:{line}
   |
   | traits: {trait_id}
   |         ^^^^^^^^^ not defined
   |
   = note: unknown traits are ignored (multiplier = 1.0)
   = hint: define the trait or remove the reference
```

**Example**:
```
warning[W003]: profile 'senior_developer' references unknown trait 'onsite'
  --> crm_migration.proj:12
   |
   | traits: senior, onsite
   |                 ^^^^^^ not defined
   |
   = note: unknown traits are ignored (multiplier = 1.0)
   = hint: define the trait or remove the reference
```

---

### W004: Approximate Leveling

**Severity**: Warning

**Trigger**: Resource leveling was applied but could not fully resolve all conflicts.

**Condition**:
```
leveling_result.unresolved_conflicts.len() > 0
```

**Message Template**:
```
warning[W004]: resource leveling incomplete - {count} conflict(s) unresolved
   |
   = unresolved:
       - {resource_id} over-allocated on {date_range} ({usage}% vs {capacity}% capacity)
   |
   = note: project extended by {days} day(s) to {new_end}
   = hint: add resources, extend deadlines, or reduce scope
```

**Example**:
```
warning[W004]: resource leveling incomplete - 2 conflict(s) unresolved
   |
   = unresolved:
       - alice over-allocated on 2025-03-10..2025-03-14 (150% vs 100% capacity)
       - bob over-allocated on 2025-03-12 (200% vs 100% capacity)
   |
   = note: project extended by 5 day(s) to 2025-04-15
   = hint: add resources, extend deadlines, or reduce scope
```

---

### W014: Container Dependency Without Child Dependencies

**Severity**: Warning

**Trigger**: A container task has dependencies, but one or more of its children do not depend on any of the container's predecessors.

**Condition**:
```
container.depends.is_not_empty()
  && child.depends.intersection(container.depends).is_empty()
```

**Message Template**:
```
warning[W014]: container '{container_name}' depends on [{deps}] but child '{child_name}' has no matching dependencies
  --> {file}:{line}
   |
   = MS Project behavior: '{child_name}' would be blocked until [{deps}] completes
   = utf8proj behavior: '{child_name}' can start immediately (explicit dependencies only)
   = hint: add 'depends: {first_dep}' to match MS Project behavior
```

**Example**:
```
warning[W014]: container 'Development Phase' depends on [design_approval] but child 'Feature X' has no matching dependencies
  --> project.proj:45
   |
   = MS Project behavior: 'Feature X' would be blocked until [design_approval] completes
   = utf8proj behavior: 'Feature X' can start immediately (explicit dependencies only)
   = hint: add 'depends: design_approval' to match MS Project behavior
```

**Rationale**: MS Project implicitly propagates container dependencies to children. utf8proj requires explicit dependencies. This warning helps users migrating from MS Project understand when their schedule will behave differently, and suggests how to match MS Project semantics if desired.

**Related**: See `docs/DESIGN_PHILOSOPHY.md` and `docs/rfcs/RFC003_CONTAINER_DEPENDENCY_SEMANTICS.md` for the design rationale.

**Auto-Fix**: Use `utf8proj fix container-deps` to automatically propagate container dependencies to children:

```bash
# Preview changes (output to stdout)
utf8proj fix container-deps project.proj

# Write to a new file
utf8proj fix container-deps project.proj -o project_fixed.proj

# Modify in place (use with caution)
utf8proj fix container-deps project.proj --in-place
```

---

### H001: Mixed Abstraction Level

**Severity**: Hint

**Trigger**: A task has assignments at different abstraction levels (both concrete and abstract).

**Condition**:
```
task.assignments.any(is_concrete) && task.assignments.any(is_abstract)
```

**Message Template**:
```
hint[H001]: task '{task_id}' mixes concrete and abstract assignments
  --> {file}:{line}
   |
   | assign: {resource_id}, {profile_id}
   |         ^^^^^^^^^^^^   ^^^^^^^^^^
   |         concrete       abstract
   |
   = note: this is valid but may indicate incomplete refinement
   = hint: consider refining '{profile_id}' to a concrete resource
```

**Example**:
```
hint[H001]: task 'integration_testing' mixes concrete and abstract assignments
  --> crm_migration.proj:89
   |
   | assign: alice, qa_engineer
   |         ^^^^^  ^^^^^^^^^^^
   |         concrete  abstract
   |
   = note: this is valid but may indicate incomplete refinement
   = hint: consider refining 'qa_engineer' to a concrete resource
```

---

### H002: Unused Profile

**Severity**: Hint

**Trigger**: A `resource_profile` is defined but never assigned to any task.

**Condition**:
```
!project.tasks.flatten().any(t => t.assignments.contains(profile.id))
```

**Message Template**:
```
hint[H002]: profile '{profile_id}' is defined but never assigned
  --> {file}:{line}
   |
   | resource_profile {profile_id} "{name}" { ... }
   | ^^^^^^^^^^^^^^^^ unused
   |
   = hint: assign to tasks or remove if no longer needed
```

---

### H003: Unused Trait

**Severity**: Hint

**Trigger**: A `trait` is defined but not referenced by any profile.

**Condition**:
```
!project.profiles.any(p => p.traits.contains(trait.id))
```

**Message Template**:
```
hint[H003]: trait '{trait_id}' is defined but never referenced
  --> {file}:{line}
   |
   | trait {trait_id} { ... }
   | ^^^^^ unused
   |
   = hint: add to profile traits or remove if no longer needed
```

---

### H004: Task Without Scheduling Constraint

**Severity**: Hint

**Trigger**: A leaf task has no predecessors and no date constraints (dangling/orphan task in CPM network).

**Condition**:
```
task.children.is_empty()  // leaf task only
  && task.depends.is_empty()  // no predecessors
  && task.constraints.is_empty()  // no date constraints
```

**Message Template**:
```
hint[H004]: task '{task_name}' has no predecessors or date constraints
  --> {file}
   |
   = '{task_name}' will start on project start date (ASAP scheduling)
   = hint: add 'depends:' or 'start_no_earlier_than:' to anchor scheduling logic
```

**Example**:
```
hint[H004]: task 'Data Migration' has no predecessors or date constraints
  --> project.proj
   |
   = 'Data Migration' will start on project start date (ASAP scheduling)
   = hint: add 'depends:' or 'start_no_earlier_than:' to anchor scheduling logic
```

**Rationale**: In PMI/CPM methodology, every task except the project start should have at least one predecessor or date constraint to define its logical position in the schedule. Tasks without these are "dangling" or "orphan" tasks that will default to ASAP scheduling (project start date), which may be unintentional. This diagnostic helps ensure network completeness and identifies tasks that might be missing dependencies.

**Note**: Container tasks are not checked - only leaf tasks that represent actual work units.

---

### E001: Circular Specialization

**Severity**: Error

**Trigger**: Profile specialization chain contains a cycle.

**Condition**:
```
profile.specializes -> ... -> profile (cycle detected)
```

**Message Template**:
```
error[E001]: circular specialization detected
  --> {file}:{line}
   |
   | resource_profile {profile_a} {
   |     specializes: {profile_b}
   |                  ^^^^^^^^^^ creates cycle
   | }
   |
   = cycle: {profile_a} -> {profile_b} -> ... -> {profile_a}
   = help: remove one specialization to break the cycle
```

**Example**:
```
error[E001]: circular specialization detected
  --> crm_migration.proj:15
   |
   | resource_profile senior_dev {
   |     specializes: lead_dev
   |                  ^^^^^^^^ creates cycle
   | }
   |
   = cycle: senior_dev -> lead_dev -> senior_dev
   = help: remove one specialization to break the cycle
```

---

### E002: Profile Without Rate (Cost-Bearing)

**Severity**: Error (in `--strict` mode) / Warning (default)

**Trigger**: A profile has no rate, cannot inherit one, **and** is used in cost-bearing assignments.

**Condition**:
```
profile.rate.is_none()
  && get_inherited_rate(profile).is_none()
  && is_used_in_assignments(profile)
```

**Important**: If a profile has no rate but is *never assigned*, emit H002 (Unused Profile) instead. This avoids false fatal errors for placeholder profiles.

**Message Template**:
```
warning[E002]: profile '{profile_id}' has no rate defined but is assigned to tasks
  --> {file}:{line}
   |
   | resource_profile {profile_id} "{name}" {
   |     // no rate or rate_range
   | }
   |
   = note: cost calculations will be incomplete for: {task_list}
   = hint: add 'rate:' or 'rate_range:' block, or specialize from a profile with rate
```

---

### I001: Project Cost Summary

**Severity**: Info

**Trigger**: Always emitted at end of successful schedule (unless `--quiet`).

**Message Template**:
```
info[I001]: project '{project_name}' scheduled successfully
   |
   = duration: {duration} ({start} to {end})
   = cost: ${min} - ${max} (expected: ${expected})
   = tasks: {total} ({concrete} concrete, {abstract} abstract assignments)
   = critical path: {critical_count} tasks
```

**Example**:
```
info[I001]: project 'CRM Migration' scheduled successfully
   |
   = duration: 89 days (2025-01-06 to 2025-05-09)
   = cost: $245,000 - $312,000 (expected: $278,500)
   = tasks: 28 (22 concrete, 6 abstract assignments)
   = critical path: 12 tasks
```

---

### I002: Refinement Progress

**Severity**: Info

**Trigger**: Emitted with `--verbose` to show abstraction refinement status.

**Message Template**:
```
info[I002]: refinement status
   |
   = profiles defined: {profile_count}
   = profiles assigned: {assigned_count}
   = concrete assignments: {concrete_count} ({concrete_pct}%)
   = abstract assignments: {abstract_count} ({abstract_pct}%)
   = cost certainty: {certainty}%
```

**Cost Certainty Formula**:
```
certainty = 100 - (total_spread / total_expected * 100)
```

---

### I003: Resource Utilization Summary

**Severity**: Info

**Trigger**: Emitted after successful scheduling when project has resources assigned.

**Message Template**:
```
info[I003]: Resource utilization ({start_date} - {end_date})
  {resource_id}: {percent}% ({used_days}/{total_days} days) [{status}]
  ...
  --> {file}
```

**Status Indicators**:
- `[OVER]` - Utilization > 100% (over-allocated)
- `[HIGH]` - Utilization > 80%
- `[LOW]` - Utilization < 20% (with some assignments)
- `[IDLE]` - No assignments

**Example**:
```
info[I003]: Resource utilization (2026-02-01 - 2026-03-06)
  pm: 41% (8.0/26 days)
  dev1: 231% (60.0/26 days) [OVER]
  dev2: 150% (39.0/26 days) [OVER]
  qa: 15% (4.0/26 days) [LOW]
  --> project.proj
```

**Calculation**:
```
utilization_percent = (used_days / (total_working_days * capacity)) * 100
```

Where:
- `used_days` = sum of daily resource units across all assigned tasks
- `total_working_days` = working days in schedule period (respects calendar)
- `capacity` = resource capacity (1.0 = 100%)

---

## CLI Integration

### Default Output

```bash
$ utf8proj schedule project.proj

warning[W001]: task 'api_development' is assigned to abstract profile 'backend_developer'
  --> project.proj:45
   ...

warning[W002]: task 'data_migration' has wide cost uncertainty (120% spread)
  --> project.proj:67
   ...

info[I001]: project 'CRM Migration' scheduled successfully
   = duration: 89 days (2025-01-06 to 2025-05-09)
   = cost: $245,000 - $312,000 (expected: $278,500)
```

### Strict Mode

```bash
$ utf8proj schedule --strict project.proj

error[W001]: task 'api_development' is assigned to abstract profile 'backend_developer'
  --> project.proj:45
   ...

error: aborting due to 1 previous error
```

### Quiet Mode

```bash
$ utf8proj schedule --quiet project.proj
# No output unless errors
```

### JSON Output (for tooling)

```bash
$ utf8proj schedule --format=json project.proj
```

```json
{
  "diagnostics": [
    {
      "code": "W001",
      "severity": "warning",
      "message": "task 'api_development' is assigned to abstract profile 'backend_developer'",
      "file": "project.proj",
      "line": 45,
      "column": 10,
      "spans": [
        {"start": 44, "end": 62, "label": "abstract profile, not a concrete resource"}
      ],
      "notes": ["cost range is $4,000 - $8,000 (100% spread)"],
      "hints": ["assign a concrete resource to lock in exact cost"]
    }
  ],
  "schedule": { ... }
}
```

---

## Implementation Notes

### Diagnostic Struct

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub spans: Vec<Span>,
    pub notes: Vec<String>,
    pub hints: Vec<String>,
}

pub enum Severity {
    Error,
    Warning,
    Hint,
    Info,
}

pub enum DiagnosticCode {
    // Structural Errors
    E001, // Circular specialization
    E002, // Profile without rate
    E003, // Infeasible constraint

    // Warnings
    W001, // Abstract assignment
    W002, // Wide cost range
    W003, // Unknown trait
    W004, // Approximate leveling
    W005, // Constraint zero slack
    W014, // Container dependency without child dependencies

    // Hints
    H001, // Mixed abstraction
    H002, // Unused profile
    H003, // Unused trait
    H004, // Task without scheduling constraint

    // Info
    I001, // Project cost summary
    I002, // Refinement progress
    I003, // Resource utilization
    I004, // Project status
    I005, // Earned value summary

    // Calendar Errors
    C001, // Zero working hours
    C002, // No working days

    // Calendar Warnings
    C010, // Task on non-working day
    C011, // Calendar mismatch (project vs resource)

    // Calendar Hints
    C020, // Low availability (<50%)
    C022, // Suspicious hours (>16h/day or 7-day week)
    C023, // Redundant holiday
}
```

### Emitter Trait

```rust
pub trait DiagnosticEmitter {
    fn emit(&mut self, diagnostic: &Diagnostic);
    fn has_errors(&self) -> bool;
    fn error_count(&self) -> usize;
    fn warning_count(&self) -> usize;
}
```

### Built-in Emitters

- `TerminalEmitter` - Colored output for CLI
- `JsonEmitter` - Machine-readable JSON
- `LspEmitter` - LSP diagnostic format (future)

---

## Calendar Diagnostics (C001-C023)

Calendar diagnostics help identify configuration issues with working calendars that may affect scheduling. Use `--calendars` flag to filter and show only calendar diagnostics.

### C001: Zero Working Hours

**Severity**: Error

**Trigger**: A calendar has no working hours defined.

**Message Template**:
```
error[C001]: calendar '{calendar_id}' has no working hours defined
  --> {file}
   = hint: add 'working_hours:' to define when work can occur
```

---

### C002: No Working Days

**Severity**: Error

**Trigger**: A calendar has no working days defined.

**Message Template**:
```
error[C002]: calendar '{calendar_id}' has no working days defined
  --> {file}
   = hint: add 'working_days:' (e.g., mon-fri)
```

---

### C010: Task Scheduled on Non-Working Day

**Severity**: Warning

**Trigger**: A task is scheduled to start on a day that is not a working day according to the project calendar.

**Message Template**:
```
warning[C010]: task '{task_id}' scheduled to start on {date} ({day_name}), which is a non-working day
  --> {file}
   = hint: adjust task constraints or calendar
```

---

### C011: Calendar Mismatch

**Severity**: Warning

**Trigger**: A resource is assigned to a task but uses a different calendar than the project.

**Message Template**:
```
warning[C011]: task '{task_id}' uses project calendar '{project_cal}' but assigned resource '{resource_id}' uses calendar '{resource_cal}'
  --> {file}
   = note: different calendars may cause scheduling conflicts
   = hint: ensure project and resource calendars are compatible
```

---

### C020: Low Availability Calendar

**Severity**: Hint

**Trigger**: A calendar has less than 50% availability (fewer than 3 working days per week).

**Message Template**:
```
hint[C020]: calendar '{calendar_id}' has only {days} working day(s) per week (<50% availability)
  --> {file}
   = note: low availability may significantly extend schedule duration
```

---

### C022: Suspicious Working Hours

**Severity**: Hint

**Trigger**: A calendar has more than 16 hours per day, OR has 7 working days per week with 8+ hours per day.

**Message Template (excessive hours)**:
```
hint[C022]: calendar '{calendar_id}' has {hours} hours/day which may be unrealistic
  --> {file}
   = hint: typical work day is 8 hours
```

**Message Template (7-day week)**:
```
hint[C022]: calendar '{calendar_id}' has 7-day workweek with {hours} hours/day
  --> {file}
   = note: verify this is intentional (e.g., 24/7 operations)
```

---

### C023: Redundant Holiday

**Severity**: Hint

**Trigger**: A holiday is defined on a day that is already a non-working day (e.g., Sunday holiday when Sunday is not a working day).

**Message Template**:
```
hint[C023]: holiday '{holiday_name}' on {date} falls on {day_name}, which is already a non-working day
  --> {file}
   = note: this holiday has no scheduling impact
```

---

### CLI Calendar Flag

Use `--calendars` with `check` or `schedule` commands to show only calendar-related diagnostics:

```bash
# Show only calendar diagnostics
utf8proj check project.proj --calendars

# Combined with other flags
utf8proj schedule project.proj --calendars --strict
```

---

## Future Diagnostics (Not Yet Specified)

These may be added in future versions:

- `W006`: Deadline at risk (critical path exceeds constraint)
- `W007`: Resource over-committed across projects
- `H005`: Task with no assignments
- `H006`: Redundant dependency (transitive)
- `C021`: Missing common holiday (planned but not implemented)
