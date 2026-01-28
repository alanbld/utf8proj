# RFC-0016: Playground Audit Console

**Status**: Draft
**Created**: 2026-01-28
**Author**: Claude (with human oversight)

## Summary

Add an Audit Console to the WASM playground that exposes the full diagnostic output, including leveling decisions (L001-L007), calendar analysis (C0xx), progress tracking (P00x), and all other diagnostic codes. This aligns with utf8proj's "explain why, not just what" principle.

## Motivation

The CLI provides rich diagnostic output via `-V` (verbose) that explains scheduling decisions:

```
[L001] hint: task_b shifted +5 days (conflict with task_a on resource dev)
[L003] hint: Project duration increased from 15 to 22 days (+46%)
[C012] info: task_c spans 2 non-working days (weekends)
```

The playground currently:
1. **Discards** `LevelingResult.diagnostics` entirely
2. **Filters out** `Severity::Info` from `analyze_project()` output
3. Provides no way to see *why* tasks were shifted

Users enabling "Resource Leveling" see the schedule change but have no visibility into the decisions.

## Design

### Diagnostic Code Taxonomy (Existing)

The system already has a rigorous diagnostic code taxonomy:

| Prefix | Domain | Severity Range | Example |
|--------|--------|----------------|---------|
| E | Errors | Error | E001 Circular specialization |
| W | Warnings | Warning | W004 Approximate leveling |
| H | Hints | Hint | H002 Unused profile |
| I | Info | Info | I001 Project cost summary |
| L | Leveling | Hint/Warning | L001 Overallocation resolved |
| C | Calendar | Error/Warning/Hint | C010 Non-working day |
| P | Progress | Warning | P005 Remaining/complete conflict |
| R | Regimes | Info/Warning | R001 Event non-zero duration |
| B | Baseline | Info/Warning/Error | B001 Baseline saved |

### Severity Levels

```
Error   → Cannot proceed (exit code 1)
Warning → Likely problem (exit code 0, but attention needed)
Hint    → Suggestion for improvement
Info    → Informational (verbose output)
```

### WASM API Changes

#### 1. Capture Leveling Diagnostics

```rust
// Current (broken):
let result = level_resources_with_options(&project, &base_schedule, &calendar, &options);
result.leveled_schedule  // diagnostics discarded!

// Fixed:
let leveling_result = level_resources_with_options(&project, &base_schedule, &calendar, &options);
let schedule = leveling_result.leveled_schedule;
let leveling_diagnostics = leveling_result.diagnostics;  // capture!
```

#### 2. New Playground Methods

```rust
impl Playground {
    /// Get all diagnostics from the last schedule operation
    /// Returns JSON array of diagnostic objects
    pub fn get_diagnostics(&self) -> String;

    /// Get diagnostics filtered by minimum severity
    /// severity: "error" | "warning" | "hint" | "info"
    pub fn get_diagnostics_filtered(&self, min_severity: &str) -> String;

    /// Get leveling-specific diagnostics (L001-L007)
    pub fn get_leveling_audit(&self) -> String;
}
```

#### 3. Diagnostic JSON Schema

```json
{
  "diagnostics": [
    {
      "code": "L001",
      "severity": "hint",
      "message": "task_b shifted +5 days (conflict with task_a on resource dev)",
      "task_id": "task_b",
      "details": {
        "shift_days": 5,
        "reason": "resource_conflict",
        "conflicting_task": "task_a",
        "resource": "dev"
      }
    },
    {
      "code": "L003",
      "severity": "hint",
      "message": "Project duration increased from 15 to 22 days (+46%)",
      "details": {
        "original_days": 15,
        "leveled_days": 22,
        "increase_percent": 46
      }
    }
  ]
}
```

### Structured Details for L001

The L001 diagnostic is the core audit trail. Its `details` object should include:

| Field | Type | Description |
|-------|------|-------------|
| `task_id` | string | Task that was shifted |
| `shift_days` | i64 | Number of days shifted |
| `original_start` | date | Pre-leveling start date |
| `new_start` | date | Post-leveling start date |
| `reason` | enum | `resource_conflict` \| `dependency_chain` \| `calendar_constraint` |
| `conflicting_task` | string? | Task causing the conflict (if resource_conflict) |
| `resource` | string? | Resource that was overallocated |
| `predecessor` | string? | Predecessor task (if dependency_chain) |

### Frontend Integration (Future)

The Audit Console will be a collapsible bottom panel in the playground UI:

```
┌─────────────────────────────────────────────────────────────────┐
│ Editor                        │ Output / Gantt / JSON           │
│                               │                                 │
│                               │                                 │
├───────────────────────────────┴─────────────────────────────────┤
│ Audit Console                                    [▼] [Filter ▾] │
│ ─────────────────────────────────────────────────────────────── │
│ [L001] hint: task_b shifted +5d (conflict with task_a on dev)   │
│ [L001] hint: task_c shifted +8d (dependency chain from task_b)  │
│ [L003] hint: Project duration increased 15→22 days (+46%)       │
│ [C012] info: task_d spans 2 non-working days                    │
└─────────────────────────────────────────────────────────────────┘
```

Filter options:
- [x] Error  [x] Warning  [x] Hint  [ ] Info
- Category: [All ▾] [Leveling] [Calendar] [Progress]

## Implementation Plan

### Phase 1: WASM Backend (This RFC)

1. **Store diagnostics in Playground struct**
   - Add `diagnostics: Vec<Diagnostic>` field
   - Populate from both `LevelingResult` and `analyze_project()`

2. **Add `get_diagnostics()` method**
   - Returns full diagnostic list as JSON
   - No severity filtering (let frontend decide)

3. **Add `get_leveling_audit()` method**
   - Convenience method for L001-L007 only
   - Useful for "what changed?" queries

4. **Enhance L001 message format**
   - Include structured details for programmatic access
   - Backward-compatible message string

### Phase 2: Frontend Console (Separate RFC)

- Collapsible bottom panel
- Severity filter toggles
- Category filter dropdown
- Click-to-highlight task in Gantt
- Export audit log

### Phase 3: Ghost Bars (Separate RFC)

- Visual overlay on Gantt showing pre-leveling positions
- Requires storing `original_schedule` before leveling

## Test Plan

### Unit Tests (TDD)

```rust
#[test]
fn playground_captures_leveling_diagnostics() {
    let mut pg = Playground::new();
    pg.set_resource_leveling(true);
    pg.schedule(CONFLICTING_TASKS_PROJECT, "native");

    let diagnostics = pg.get_diagnostics();
    let parsed: Vec<DiagnosticInfo> = serde_json::from_str(&diagnostics).unwrap();

    assert!(parsed.iter().any(|d| d.code == "L001"));
}

#[test]
fn get_leveling_audit_returns_only_l_codes() {
    let mut pg = Playground::new();
    pg.set_resource_leveling(true);
    pg.schedule(CONFLICTING_TASKS_PROJECT, "native");

    let audit = pg.get_leveling_audit();
    let parsed: Vec<DiagnosticInfo> = serde_json::from_str(&audit).unwrap();

    assert!(parsed.iter().all(|d| d.code.starts_with("L")));
}

#[test]
fn diagnostics_include_structured_details() {
    let mut pg = Playground::new();
    pg.set_resource_leveling(true);
    pg.schedule(CONFLICTING_TASKS_PROJECT, "native");

    let diagnostics = pg.get_diagnostics();
    // Parse and verify L001 has task_id, shift_days, etc.
}
```

### Integration Tests

- Verify diagnostics survive WASM serialization round-trip
- Verify filtering works correctly
- Verify no diagnostics lost compared to CLI output

## Alternatives Considered

### A. Include diagnostics in `PlaygroundResult`

Rejected: Would change existing API contract. Separate method is additive.

### B. Always include Info-level in `schedule()` output

Rejected: Noisy for basic use case. Opt-in via `get_diagnostics()` is cleaner.

### C. Stream diagnostics via callback

Rejected: Complexity not justified. Batch retrieval is sufficient for playground.

## Open Questions

1. **Should `get_diagnostics()` include parse-time errors?**
   - Current: Parse errors return early, never reach diagnostics
   - Proposal: Keep separate (parse errors in `error` field, runtime diagnostics in `get_diagnostics()`)

2. **Diagnostic deduplication?**
   - If same L001 emitted twice (from leveling + analyze), dedupe?
   - Proposal: No deduplication, maintain audit trail fidelity

## References

- CLAUDE.md: Diagnostic System documentation
- RFC-0003: Resource Leveling
- RFC-0014: Scaling Resource Leveling
- utf8proj-core/src/lib.rs: DiagnosticCode enum (lines 1860-1990)
