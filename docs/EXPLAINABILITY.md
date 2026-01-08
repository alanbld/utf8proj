# Explainability in utf8proj

This document defines the explainability model in utf8proj: what it means, how it works, and why it matters.

## What "Explainability" Means in utf8proj

Explainability is **causal attribution** — answering "why does this task start on this date?" with traceable, verifiable reasoning.

### Explain ≠ Fix

utf8proj does not automatically correct scheduling problems. When a task starts on a weekend, the system explains why — it does not silently move the task to Monday.

### Explain ≠ Optimize

utf8proj does not suggest better schedules. It does not propose resource reallocation or constraint relaxation. The scheduler produces one deterministic output; explain() describes how that output was derived.

### Explain = Causal Attribution

For any scheduled task, utf8proj can answer:

- **Why this start date?** (dependency chains, constraints, calendar availability)
- **Why this duration?** (effort, resource allocation, calendar working hours)
- **Why critical?** (zero slack due to dependency path or constraint)
- **What calendar effects?** (weekends skipped, holidays encountered)

The explanation is always backward-looking: "given the inputs, here is why the output is what it is."

---

## The Explainability Pipeline

```
┌─────────┐    ┌────────┐    ┌─────────────┐    ┌───────────┐    ┌─────────┐
│  Model  │───►│ Solver │───►│ Diagnostics │───►│ explain() │───►│ LSP/UI  │
│ (proj)  │    │ (CPM)  │    │  (analyze)  │    │           │    │         │
└─────────┘    └────────┘    └─────────────┘    └───────────┘    └─────────┘
```

### Stage 1: Model

The `.proj` file defines the project structure: tasks, dependencies, resources, calendars, constraints. This is pure declaration — no scheduling has occurred.

### Stage 2: Solver

The CPM solver computes early/late start and finish dates for all tasks. The solver is deterministic: identical inputs always produce identical schedules. The solver does not explain; it computes.

### Stage 3: Diagnostics

`analyze_project()` examines the model and schedule, emitting diagnostics (E*, W*, H*, I*, C* codes). Diagnostics are observations about the project state — problems, warnings, hints. They do not modify the schedule.

### Stage 4: explain()

The `explain()` function synthesizes solver output and diagnostics into a human-readable explanation for a specific task. It answers: "Why is this task scheduled the way it is?"

### Stage 5: LSP/UI

The explanation is surfaced to users through:

- **LSP hover**: Shows CalendarImpact and related diagnostics inline
- **CLI**: `--explain` flag for command-line output
- **Excel export**: Calendar Analysis sheet with per-task breakdown

Each consumer is read-only — they display explanations but never modify the schedule.

---

## Explanation Domains

utf8proj explanations cover three domains:

### Structural (Dependencies & CPM)

- Which predecessors constrain this task's start?
- What is the critical path through this task?
- How much slack exists before successors are delayed?

This is classical CPM analysis, made explicit.

### Temporal (Calendars)

- How many working days does this task span?
- How many weekend days fall within the task's date range?
- Are any holidays encountered?
- What is the effective "calendar efficiency" of this task?

Temporal explanation separates calendar effects from pure duration.

### Semantic (Diagnostics)

- Does this task have warnings (W*)?
- Are there calendar-specific issues (C*)?
- What hints (H*) apply?

Diagnostics provide structured, codified observations that explain() can reference.

---

## CalendarImpact

`CalendarImpact` is the core data structure for temporal explanation.

### What It Measures

```rust
pub struct CalendarImpact {
    pub calendar_id: String,      // Which calendar applies
    pub non_working_days: u32,    // Total non-working days in span
    pub weekend_days: u32,        // Weekend days specifically
    pub holiday_days: u32,        // Holiday days specifically
    pub total_delay_days: i64,    // Net calendar-induced delay
    pub description: String,      // Human-readable summary
}
```

For a task spanning January 12–23 (12 days), CalendarImpact might report:

- 8 working days
- 4 weekend days (2 Saturdays, 2 Sundays)
- 0 holiday days
- Description: "4 weekend days extended duration"

### What It Does NOT Do

- **Does not reschedule**: CalendarImpact is observational, not prescriptive
- **Does not recommend**: It does not suggest using a different calendar
- **Does not accumulate**: Each task's impact is independent; no project-wide rollup
- **Does not optimize**: It does not identify "better" calendar configurations

### Example Output (LSP Hover)

```markdown
**📆 Calendar Impact:**
• 8 working days, 4 weekend days
```

### Example Output (Excel)

| Task ID | Calendar | Working Days | Weekends | Holidays | Non-Working % |
|---------|----------|--------------|----------|----------|---------------|
| design  | standard | 8            | 4        | 0        | 33.3%         |

---

## Diagnostics Integration

### related_diagnostics

The `Explanation` struct includes task-relevant diagnostic codes:

```rust
pub struct Explanation {
    pub task_id: TaskId,
    pub reason: String,
    pub calendar_impact: Option<CalendarImpact>,
    pub related_diagnostics: Vec<DiagnosticCode>,  // e.g., [C010, H004]
    // ...
}
```

`filter_task_diagnostics()` identifies which diagnostics apply to a specific task by matching task IDs in diagnostic messages.

### Why Diagnostics Remain Primary

Diagnostics are the **source of truth** for project issues. explain() does not duplicate diagnostic logic — it references diagnostic codes.

This separation ensures:

- **Single source**: Diagnostic definitions live in one place
- **Consistency**: LSP, CLI, and Excel all reference the same codes
- **Extensibility**: New diagnostics automatically appear in explanations

### How explain() References Diagnostics

```markdown
**⚠️ Diagnostics:**
• 🔴 `C010` (task starts on non-working day)
• 💡 `H004` (task is unconstrained)
```

The explanation shows codes; users consult the diagnostics panel or documentation for full details. This keeps explanations compact while maintaining traceability.

---

## Design Principles

### 1. Non-Prescriptive

utf8proj describes what is, not what should be. It does not:

- Suggest moving tasks
- Recommend resource changes
- Propose constraint modifications

The user decides what to change; utf8proj explains the current state.

### 2. No Silent Correction

If the model says a task starts on Saturday, the schedule shows Saturday. The system emits C010 (non-working day diagnostic) but does not auto-correct.

This matters because:

- Some domains intentionally schedule weekend work
- Silent correction hides model errors
- Explicit diagnostics create audit trails

### 3. Describe Reality, Don't Enforce Methodology

utf8proj does not impose project management dogma:

- No mandatory WBS structure
- No required baseline comparisons
- No enforced review gates

It provides information; methodology is the user's choice.

### 4. Deterministic Explanation

Given identical inputs, explain() produces identical output. There is no randomness, no heuristics, no "AI suggestions." Explanations are reproducible and auditable.

---

## Comparison: How utf8proj Differs

### vs. Microsoft Project

MS Project auto-levels resources, respects calendars implicitly, and provides limited "why" visibility. Changes happen; reasons are opaque.

utf8proj: No auto-leveling. Calendar effects are explicit. Every scheduling decision is traceable.

### vs. TaskJuggler

TaskJuggler compiles to a schedule with detailed reports but minimal explanation of *why* dates were chosen. The gap file shows what TJ decided, not why.

utf8proj: explain() provides causal attribution. Diagnostics codify observations. CalendarImpact quantifies temporal effects.

### vs. Primavera P6

P6 offers extensive analysis but buries explanation in complex dialogs. Understanding "why" requires expertise in navigating the tool.

utf8proj: Explanation is first-class. Hover over a task, see the reasoning. No dialog diving required.

### The utf8proj Advantage

> utf8proj doesn't just schedule tasks — it explains why time behaves the way it does.

This is the core value proposition. Scheduling engines are commoditized; explanation is rare.

---

## Explicit Non-Goals

To maintain architectural clarity, utf8proj's explainability model explicitly excludes:

### No History Tracking

explain() describes the current schedule. It does not:

- Compare to previous schedules
- Track changes over time
- Provide "what changed" analysis

History is a separate concern, not part of explainability.

### No What-If Optimization

explain() does not answer hypotheticals:

- "What if I used a different calendar?"
- "What if I added a resource?"
- "What if I removed this constraint?"

Scenario analysis requires separate tooling.

### No Automatic Rescheduling

explain() never modifies the schedule. It is read-only by design. Any rescheduling requires explicit user action and re-running the solver.

### No UI Logic in Solver

The solver computes schedules. The solver does not know about:

- LSP hover formatting
- Excel column widths
- Dashboard chart types

Presentation is strictly separated from computation.

---

## Summary

utf8proj's explainability model is built on three pillars:

1. **Causal Attribution**: Every scheduling decision has a traceable reason
2. **Explicit Observation**: Calendar effects, diagnostics, and constraints are visible, not hidden
3. **Non-Prescriptive Design**: The system describes; the user decides

This model enables users to understand their schedules deeply — not just what was scheduled, but why.

---

## See Also

- [DIAGNOSTICS.md](./DIAGNOSTICS.md) — Full diagnostic code reference
- [CLAUDE.md](../CLAUDE.md) — Project context and architecture overview
