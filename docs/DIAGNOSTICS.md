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
| Error | `E` | Cannot proceed | Always fatal |
| Warning | `W` | Likely problem | Becomes error |
| Hint | `H` | Suggestion | Becomes warning |
| Info | `I` | Informational | Unchanged |

## Emission Ordering

For determinism and testability, diagnostics are emitted in this order:

1. **Structural errors** (E001, E002) - fatal issues first
2. **Cost-related warnings** (W002, W004) - budget risk
3. **Assignment-related warnings** (W001, W003) - planning gaps
4. **Hints** (H001, H002, H003) - suggestions
5. **Info** (I001, I002) - summary last

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
    E001, // Circular specialization
    E002, // Profile without rate
    W001, // Abstract assignment
    W002, // Wide cost range
    W003, // Unknown trait
    W004, // Approximate leveling
    H001, // Mixed abstraction
    H002, // Unused profile
    H003, // Unused trait
    I001, // Project cost summary
    I002, // Refinement progress
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

## Future Diagnostics (Not Yet Specified)

These may be added in future versions:

- `W005`: Resource over-committed across projects
- `W006`: Deadline at risk (critical path exceeds constraint)
- `H004`: Task with no assignments
- `H005`: Redundant dependency (transitive)
- `I003`: Resource utilization summary
